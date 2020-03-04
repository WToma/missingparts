use std::convert::Infallible;

use std::str;
use std::str::FromStr;
use std::sync::Arc;

use hyper::header::LOCATION;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};

use serde::{Deserialize, Serialize};

use missingparts::cards::Card;
use missingparts::game_manager::GameManager;
use missingparts::lobby::{Lobby, PlayerIdInLobby};
use missingparts::playeraction::PlayerAction;
use missingparts::server_core_types::{GameId, Token, TokenVerifier};

use missingparts::server_utils::{
    BodyParseError, RichParts, SupportedMimeType, TupleWrapper1, TupleWrapper2,
};

async fn missingparts_service(
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

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lobby = Arc::new(Lobby::new());
    let game_manager = Arc::new(GameManager::new());

    // For every connection, we must make a `Service` to handle all
    // incoming HTTP requests on said connection.
    let make_svc = make_service_fn(move |_conn| {
        // This is the `Service` that will handle the connection.
        // `service_fn` is a helper to convert a function that
        // returns a Response into a `Service`.
        let lobby = Arc::clone(&lobby);
        let game_manager = Arc::clone(&game_manager);
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                missingparts_service(Arc::clone(&lobby), Arc::clone(&game_manager), req)
            }))
        }
    });

    let addr = ([127, 0, 0, 1], 3030).into();

    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}

#[derive(Deserialize)]
struct JoinLobbyRequest {
    min_game_size: usize,
    max_game_size: usize,
}

#[derive(Serialize)]
struct JoinedLobbyResponse {
    player_id_in_lobby: usize,
    token: String,
}

#[derive(Serialize)]
struct InvalidGameSizePreference {
    min_game_size: usize,
    max_game_size: usize,
}

#[derive(Serialize)]
struct JoinedGameResponse {
    game_id: usize,
    player_id_in_game: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

#[derive(Serialize)]
struct PrivateCardResponse {
    missing_part: Card,
}
