use std::convert::Infallible;

use std::str;
use std::str::FromStr;
use std::sync::Arc;

use hyper::header::LOCATION;
use hyper::{Body, Method, Request, Response, StatusCode};

use serde::{Deserialize, Serialize};

use crate::cards::Card;
use crate::game_manager::GameManager;
use crate::lobby::{Lobby, PlayerIdInLobby};
use crate::playeraction::PlayerAction;
use crate::server_core_types::{GameId, Token, TokenVerifier};

use crate::server_utils::{
    BodyParseError, RichParts, SupportedMimeType, TupleWrapper1, TupleWrapper2,
};

pub async fn missingparts_service(
    lobby: Arc<Lobby>,
    game_manager: Arc<GameManager>,
    req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    let (parts, body) = req.into_parts();
    let rich_parts = RichParts::from(&parts);
    let response_mime_type = if let Some(mime_type) = rich_parts.guess_response_type() {
        mime_type
    } else {
        return Ok(Response::builder()
                                .status(StatusCode::NOT_ACCEPTABLE)
                                .body(Body::from("no compatible Accept value found. supported application/json and application/json5"))
                                .unwrap());
    };

    if rich_parts.does_match(&Method::POST, "/lobby") {
        let body: Result<JoinLobbyRequest, BodyParseError> =
            rich_parts.deserialize_by_content_type(body, 1024).await;
        match body {
            Ok(body) => Ok(process_join_lobby(
                body,
                &response_mime_type,
                lobby,
                game_manager,
            )),
            Err(e) => Ok(e.into()),
        }
    } else if let Ok(TupleWrapper1(player_id_in_lobby)) =
        rich_parts.try_match(&Method::GET, "/lobby/players/{}/game")
    {
        let player_id_in_lobby = PlayerIdInLobby(player_id_in_lobby);
        let maybe_token = rich_parts.token().and_then(|t| Token::from_str(t).ok());
        let verified = match maybe_token {
            Some(token) => lobby.verify(&player_id_in_lobby, &token),
            None => false,
        };
        if verified {
            Ok(process_get_lobby_player(
                player_id_in_lobby,
                &response_mime_type,
                lobby,
            ))
        } else {
            Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap())
        }
    } else if let Ok(TupleWrapper2(game_id, player_id_in_game)) =
        rich_parts.try_match(&Method::GET, "/games/{}/players/{}/private")
    {
        let game_id = GameId(game_id);
        game_manager.with_game(game_id, |g| {
            let maybe_token = rich_parts.token().and_then(|t| Token::from_str(t).ok());
            let verified = match maybe_token {
                Some(token) => g.verify(&player_id_in_game, &token),
                None => false,
            };
            if verified {
                let resp = PrivateCardResponse {
                    missing_part: g.get_private_card(player_id_in_game),
                };
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(response_mime_type.serialize(&resp))
                    .unwrap())
            } else {
                Ok(Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(Body::empty())
                    .unwrap())
            }
        })
    } else if let Ok(TupleWrapper2(game_id, player_id_in_game)) =
        rich_parts.try_match(&Method::POST, "/games/{}/players/{}/moves")
    {
        let game_id = GameId(game_id);
        let body: Result<PlayerAction, BodyParseError> =
            rich_parts.deserialize_by_content_type(body, 1024).await;
        match body {
            Ok(player_action) => game_manager.with_mut_game(game_id, |g| {
                let maybe_token = rich_parts.token().and_then(|t| Token::from_str(t).ok());
                let verified = match maybe_token {
                    Some(token) => g.verify(&player_id_in_game, &token),
                    None => false,
                };
                if verified {
                    let move_result = g.make_move(player_id_in_game, player_action.clone());
                    match move_result {
                        Ok(_) => Ok(Response::builder()
                            .status(StatusCode::OK)
                            .body(Body::empty())
                            .unwrap()),
                        Err(action_error) => Ok(Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(response_mime_type.serialize(&action_error))
                            .unwrap()),
                    }
                } else {
                    Ok(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(Body::empty())
                        .unwrap())
                }
            }),
            Err(e) => Ok(e.into()),
        }
    } else if let Ok(TupleWrapper1(game_id)) = rich_parts.try_match(&Method::GET, "/games/{}") {
        let game_id = GameId(game_id);
        game_manager.with_game(game_id, |g| {
            let game_description = g.describe();
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(response_mime_type.serialize(&game_description))
                .unwrap())
        })
    } else {
        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap())
    }
}

fn process_join_lobby(
    body: JoinLobbyRequest,
    response_mime_type: &SupportedMimeType,
    lobby: Arc<Lobby>,
    game_manager: Arc<GameManager>,
) -> Response<Body> {
    let add_player_result = lobby.add_player(body.min_game_size, body.max_game_size);
    match add_player_result {
        Ok((player_id_in_lobby, token)) => {
            lobby.start_game(&*game_manager);
            match lobby.get_player_game(player_id_in_lobby) {
                None => {
                    let resp = JoinedLobbyResponse {
                        player_id_in_lobby: player_id_in_lobby.0,
                        token: token.0,
                    };
                    Response::builder()
                        .status(StatusCode::CREATED)
                        .header(
                            LOCATION,
                            format!("/lobby/players/{:?}/game", player_id_in_lobby.0),
                        )
                        .body(response_mime_type.serialize(&resp))
                        .unwrap()
                }
                Some(player_assigned_to_game) => {
                    let resp = JoinedGameResponse {
                        game_id: player_assigned_to_game.game_id.0,
                        player_id_in_game: player_assigned_to_game.player_id_in_game,
                        token: Some(token.0),
                    };
                    Response::builder()
                        .status(StatusCode::CREATED)
                        .header(
                            LOCATION,
                            format!(
                                "/games/{:?}/players/{:?}/private",
                                player_assigned_to_game.game_id.0,
                                player_assigned_to_game.player_id_in_game
                            ),
                        )
                        .body(response_mime_type.serialize(&resp))
                        .unwrap()
                }
            }
        }
        Err(()) => {
            let resp = InvalidGameSizePreference {
                min_game_size: body.min_game_size,
                max_game_size: body.max_game_size,
            };
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(response_mime_type.serialize(&resp))
                .unwrap()
        }
    }
}

fn process_get_lobby_player(
    player_id_in_lobby: PlayerIdInLobby,
    response_mime_type: &SupportedMimeType,
    lobby: Arc<Lobby>,
) -> Response<Body> {
    match lobby.get_player_game(player_id_in_lobby) {
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
        Some(player_assigned_to_game) => {
            let resp = JoinedGameResponse {
                game_id: player_assigned_to_game.game_id.0,
                player_id_in_game: player_assigned_to_game.player_id_in_game,
                token: None, // token remains the same
            };

            Response::builder()
                .status(StatusCode::TEMPORARY_REDIRECT)
                .header(
                    LOCATION,
                    format!(
                        "/games/{:?}/players/{:?}/private",
                        player_assigned_to_game.game_id.0,
                        player_assigned_to_game.player_id_in_game
                    ),
                )
                .body(response_mime_type.serialize(&resp))
                .unwrap()
        }
    }
}

#[derive(Deserialize)]
struct JoinLobbyRequest {
    min_game_size: usize,
    max_game_size: usize,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Deserialize))]
struct JoinedLobbyResponse {
    player_id_in_lobby: usize,
    token: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Deserialize))]
struct InvalidGameSizePreference {
    min_game_size: usize,
    max_game_size: usize,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Deserialize))]
struct JoinedGameResponse {
    game_id: usize,
    player_id_in_game: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Deserialize))]
struct PrivateCardResponse {
    missing_part: Card,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::actionerror::ActionError;
    use crate::gameplay::GameDescription;

    use serde::de::DeserializeOwned;
    use serde_json;
    use tokio::runtime::Runtime;

    #[test]
    fn test_join_lobby() {
        // player 1 joins the lobby
        let resp = TestServer::new().join_lobby(2, 4);

        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_header(&resp, "Location", "/lobby/players/0/game");

        // they get an ID and a token
        let resp: JoinedLobbyResponse = parse_response(resp);
        assert_eq!(resp.player_id_in_lobby, 0);
        assert_ne!(resp.token, "");
    }

    #[test]
    fn test_join_lobby_invalid_game_size() {
        // player 1 joins the lobby, but their game size preference is invalid (max=1 < min=2)
        let resp = TestServer::new().join_lobby(2, 1);

        // they get a 400 Bad Request explaining that the game size preference is invalid
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let resp: InvalidGameSizePreference = parse_response(resp);
        assert_eq!(resp.min_game_size, 2);
        assert_eq!(resp.max_game_size, 1);
    }

    #[test]
    fn test_lobby_get_game_not_assigned() {
        let test = TestServer::new();

        // player 1 joins the lobby
        let resp = test.join_lobby(2, 4);

        // they get an ID and a token
        let resp: JoinedLobbyResponse = parse_response(resp);

        // they query their game status
        let resp = test.get_lobby_player_status(&resp);

        // but they don't have a game yet so they get 404 Not Found
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_lobby_get_game() {
        let test = TestServer::new();

        // player 1 joins the lobby
        let resp = test.join_lobby(2, 4);

        // they get an ID and a token
        let resp: JoinedLobbyResponse = parse_response(resp);

        // player 2 joins the lobby
        test.join_lobby(2, 4);

        // player 1 queries their status
        let resp = test.get_lobby_player_status(&resp);

        // and now they have a game
        assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_header(&resp, "Location", "/games/0/players/0/private");

        let resp: JoinedGameResponse = parse_response(resp);
        assert_eq!(resp.game_id, 0);
        assert_eq!(resp.player_id_in_game, 0);
        assert_eq!(resp.token, None); // since the token didn't change, it is empty
    }

    #[test]
    fn test_lobby_get_game_invalid_token() {
        let test = TestServer::new();

        // player 1 joins the lobby, they get an ID and a token
        let lobby_player: JoinedLobbyResponse = parse_response(test.join_lobby(4, 4));
        // player 2 joins the lobby, they get an ID and a token
        let other_player: JoinedLobbyResponse = parse_response(test.join_lobby(4, 4));

        // try to query status with a missing token
        let resp = get(
            &format!("/lobby/players/{:?}/game", lobby_player.player_id_in_lobby),
            None, // missing token
            Arc::clone(&test.lobby),
            Arc::clone(&test.game_manager),
        );
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to query status with the wrong token
        let resp = test.get_lobby_player_status(&JoinedLobbyResponse {
            token: String::from("mellon"),
            ..lobby_player
        });
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to query status with a correct token but belonging to the wrong player
        let resp = test.get_lobby_player_status(&JoinedLobbyResponse {
            token: other_player.token,
            ..lobby_player
        });
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_lobby_get_game_invalid_playerid() {
        let test = TestServer::new();

        // try to query the status for a player that does not exist
        let resp = test.get_lobby_player_status(&JoinedLobbyResponse {
            token: String::from("maybe?"),
            player_id_in_lobby: 0,
        });

        // unauthorized because we do not want to reveal whether the player exists or not
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // the server remains in a good state, so after that we can have a player join
        parse_response::<JoinedLobbyResponse>(test.join_lobby(4, 4));
    }

    #[test]
    fn test_join_lobby_to_game() {
        let test = TestServer::new();

        // player 1 joins the lobby
        test.join_lobby(2, 4);

        // player 2 joins the lobby
        let resp = test.join_lobby(2, 4);

        // they immediately get assigned to a game
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_header(&resp, "Location", "/games/0/players/1/private");

        let resp: JoinedGameResponse = parse_response(resp);
        assert_eq!(resp.game_id, 0);
        assert_eq!(resp.player_id_in_game, 1);

        // since this is the first response they get from the server, they get a token
        assert_ne!(resp.token, None);
        assert_ne!(resp.token.unwrap(), "");
    }

    #[test]
    fn test_game_get_private_card() {
        let test = TestServer::new();

        // 2 players join the lobby, which starts a game
        test.join_lobby(2, 4);
        let player: JoinedGameResponse = parse_response(test.join_lobby(2, 4));

        // player 2 queries their private card using the token they got from the lobby
        let resp = test.get_player_private_card(
            GameId(player.game_id),
            player.player_id_in_game,
            player.token,
        );

        // it works
        assert_eq!(resp.status(), StatusCode::OK);
        parse_response::<PrivateCardResponse>(resp);
    }

    #[test]
    fn test_game_get_private_card_invalid_cases() {
        let test = TestServer::new();

        // 2 players join the lobby, which starts a game
        let other_player: JoinedLobbyResponse = parse_response(test.join_lobby(2, 4));
        let player: JoinedGameResponse = parse_response(test.join_lobby(2, 4));

        // try to query the private card of the player with a missing token
        let resp = test.get_player_private_card(
            GameId(player.game_id),
            player.player_id_in_game,
            None, // missing token
        );
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to query the private card of the player with an invalid token
        let resp = test.get_player_private_card(
            GameId(player.game_id),
            player.player_id_in_game,
            Some(String::from("mellon")), // invalid token
        );
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to query the private card of the player with an invalid token
        let resp = test.get_player_private_card(
            GameId(player.game_id),
            player.player_id_in_game,
            Some(other_player.token), // token belongs to another player
        );
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to query the private card of the player with the correct game ID but invalid player ID
        let resp = test.get_player_private_card(
            GameId(player.game_id),
            3, // this player does not exist in that game
            player.token.clone(),
        );

        // we get unauthorized because we don't want to expose the existence or non-existence of the game/player combo
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to query the private card of the player with the correct player ID but wrong game ID
        let resp =
            test.get_player_private_card(GameId(2), player.player_id_in_game, player.token.clone());

        // we get unauthorized because we don't want to expose the existence or non-existence of the game/player combo
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_game_describe() {
        let test = TestServer::new();

        // 2 players join the lobby, which starts a game
        test.join_lobby(2, 4);
        let player: JoinedGameResponse = parse_response(test.join_lobby(2, 4));

        parse_response::<GameDescription>(test.describe_game(GameId(player.game_id)));
    }

    #[test]
    fn test_game_describe_invalid_game_id() {
        let resp = TestServer::new().describe_game(GameId(0));
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_game_make_move() {
        let test = TestServer::new();

        // 2 players join the lobby, which starts a game
        let player: JoinedLobbyResponse = parse_response(test.join_lobby(2, 4));
        test.join_lobby(2, 4);
        let game: JoinedGameResponse = parse_response(test.get_lobby_player_status(&player));

        // the first player to join the lobby becomes the first player in the game.
        // the first player makes a move (Scavenge)
        let resp = test.make_scavenge_move(
            GameId(game.game_id),
            game.player_id_in_game,
            Some(player.token),
        );
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_game_make_move_invalid_move() {
        let test = TestServer::new();

        // 2 players join the lobby, which starts a game
        test.join_lobby(2, 4);
        let player: JoinedGameResponse = parse_response(test.join_lobby(2, 4));

        // the first player to join the lobby becomes the first player in the game.
        // the _second_  player tries to make a move (Scavenge)
        let resp = test.make_scavenge_move(
            GameId(player.game_id),
            player.player_id_in_game,
            player.token,
        );

        // but since it isn't their turn, we get an error
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // with an appropriate error body
        parse_response::<ActionError>(resp);
    }

    #[test]
    fn test_game_make_move_invalid_cases() {
        let test = TestServer::new();

        // 2 players join the lobby, which starts a game
        let other_player: JoinedLobbyResponse = parse_response(test.join_lobby(2, 4));
        let player: JoinedGameResponse = parse_response(test.join_lobby(2, 4));

        // try to make a move for the player with a missing token
        let resp = test.make_scavenge_move(
            GameId(player.game_id),
            player.player_id_in_game,
            None, // missing token
        );
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to make a move for the player with an invalid token
        let resp = test.make_scavenge_move(
            GameId(player.game_id),
            player.player_id_in_game,
            Some(String::from("mellon")), // invalid token
        );
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to make a move for the player with an invalid token
        let resp = test.make_scavenge_move(
            GameId(player.game_id),
            player.player_id_in_game,
            Some(other_player.token), // token belongs to another player
        );
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to make a move for the player with the correct game ID but invalid player ID
        let resp = test.make_scavenge_move(
            GameId(player.game_id),
            3, // this player does not exist in that game
            player.token.clone(),
        );
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // try to make a move for the player with the correct player ID but wrong game ID
        let resp =
            test.make_scavenge_move(GameId(2), player.player_id_in_game, player.token.clone());
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_invalid_urls() {
        let test = TestServer::new();
        let resp = get(
            "/notfound",
            None,
            Arc::clone(&test.lobby),
            Arc::clone(&test.game_manager),
        );
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let resp = get(
            "/games/this_is_not_a_game_id",
            None,
            Arc::clone(&test.lobby),
            Arc::clone(&test.game_manager),
        );
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let resp = post(
            "/games/0",
            None,
            "".to_string(),
            Arc::clone(&test.lobby),
            Arc::clone(&test.game_manager),
        );
        assert_eq!(resp.status(), StatusCode::NOT_ACCEPTABLE);
    }

    // logic test helpers
    struct TestServer {
        lobby: Arc<Lobby>,
        game_manager: Arc<GameManager>,
    }
    impl TestServer {
        fn new() -> TestServer {
            TestServer {
                lobby: Arc::new(Lobby::new()),
                game_manager: Arc::new(GameManager::new()),
            }
        }

        fn join_lobby(&self, min_game_size: usize, max_game_size: usize) -> Response<Body> {
            post(
                "/lobby",
                None,
                format!(
                    "{{\"min_game_size\": {}, \"max_game_size\": {}}}",
                    min_game_size, max_game_size
                ),
                Arc::clone(&self.lobby),
                Arc::clone(&self.game_manager),
            )
        }

        fn get_lobby_player_status(&self, lobby_player: &JoinedLobbyResponse) -> Response<Body> {
            get(
                &format!("/lobby/players/{:?}/game", lobby_player.player_id_in_lobby),
                Some(&lobby_player.token),
                Arc::clone(&self.lobby),
                Arc::clone(&self.game_manager),
            )
        }

        fn get_player_private_card(
            &self,
            game_id: GameId,
            player_id: usize,
            token: Option<String>,
        ) -> Response<Body> {
            get(
                &format!("/games/{:?}/players/{:?}/private", game_id.0, player_id),
                token.as_ref().map(|s| s.as_str()),
                Arc::clone(&self.lobby),
                Arc::clone(&self.game_manager),
            )
        }

        fn describe_game(&self, game_id: GameId) -> Response<Body> {
            get(
                &format!("/games/{:?}", game_id.0),
                None, // this endpoint does not require authorization
                Arc::clone(&self.lobby),
                Arc::clone(&self.game_manager),
            )
        }

        fn make_scavenge_move(
            &self,
            game_id: GameId,
            player_id: usize,
            token: Option<String>,
        ) -> Response<Body> {
            post(
                &format!("/games/{:?}/players/{:?}/moves", game_id.0, player_id),
                token.as_ref().map(|s| s.as_str()),
                "\"Scavenge\"".to_string(),
                Arc::clone(&self.lobby),
                Arc::clone(&self.game_manager),
            )
        }
    }

    // http test helpers

    fn get(
        uri: &str,
        token: Option<&str>,
        lobby: Arc<Lobby>,
        game_manager: Arc<GameManager>,
    ) -> Response<Body> {
        let req_builder = Request::get(uri);
        let req_builder = match token {
            Some(token) => req_builder.header("Authorization", token),
            None => req_builder,
        };
        let req = req_builder.body(Body::empty()).unwrap();
        Runtime::new()
            .unwrap()
            .block_on(missingparts_service(lobby, game_manager, req))
            .unwrap()
    }

    fn post(
        uri: &str,
        token: Option<&str>,
        body: String,
        lobby: Arc<Lobby>,
        game_manager: Arc<GameManager>,
    ) -> Response<Body> {
        let req_builder = Request::post(uri);
        let req_builder = match token {
            Some(token) => req_builder.header("Authorization", token),
            None => req_builder,
        };
        let req_builder = req_builder.header("Content-Length", &format!("{:?}", body.len()));
        let req = req_builder.body(Body::from(body)).unwrap();
        Runtime::new()
            .unwrap()
            .block_on(missingparts_service(lobby, game_manager, req))
            .unwrap()
    }

    fn parse_response<T: DeserializeOwned>(r: Response<Body>) -> T {
        let body = r.into_body();
        let full_body = Runtime::new()
            .unwrap()
            .block_on(hyper::body::to_bytes(body))
            .unwrap();
        let full_body_str = str::from_utf8(&full_body).unwrap();
        serde_json::from_str(full_body_str)
            .expect(&format!("failed to deserialize `{:?}`", full_body_str))
    }

    fn assert_header<T>(resp: &Response<T>, header_name: &str, header_value: &str) {
        let actual_value = resp
            .headers()
            .get(header_name)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .expect(&format!("missing header `{:?}`", header_name));
        assert_eq!(actual_value, header_value);
    }
}
