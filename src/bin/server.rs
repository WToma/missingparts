use chashmap::CHashMap;
use missingparts::actionerror::ActionError;
use missingparts::cards::Card;
use missingparts::gameplay::{GameDescription, Gameplay};
use missingparts::lobby::{GameCreator, Lobby, PlayerAssignedToGame, PlayerIdInLobby};
use missingparts::playeraction::PlayerAction;
use missingparts::server_core_types::GameId;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use warp::Filter;

/// A single game that's managed by the `GameManager`.
struct ManagedGame {
    gameplay: Gameplay,
    secret_cards_per_player: Vec<Card>,
}

impl ManagedGame {
    fn new(num_players: usize) -> ManagedGame {
        let (gameplay, secret_cards_per_player) = Gameplay::init(num_players);
        ManagedGame {
            gameplay,
            secret_cards_per_player,
        }
    }

    fn describe(&self) -> GameDescription {
        self.gameplay.describe()
    }

    fn get_private_card(&self, player_id: usize) -> Card {
        self.secret_cards_per_player[player_id]
    }

    fn make_move(
        &mut self,
        player_id: usize,
        player_action: PlayerAction,
    ) -> Result<(), ActionError> {
        self.gameplay
            .process_player_action(player_id, player_action)
    }
}

/// Manages games in the server. This is the primary way games should be interacted with.
/// Safe for concurrent access.
///
/// To start a new game under the manager, use `new_game`. After that use `with_game` for read-only
/// operations on a game, or `with_mut_game` for read-write operations on a game.
struct GameManager {
    games: CHashMap<GameId, ManagedGame>,
    next_game_index: AtomicUsize,
}

impl GameCreator for GameManager {
    /// Starts a new game, and returns the ID of the game that can be used with `with_game` and `with_mut_game`.
    fn new_game(&self, num_players: usize) -> GameId {
        let next_index = GameId(self.next_game_index.fetch_add(1, Ordering::SeqCst));
        self.games.insert(next_index, ManagedGame::new(num_players));
        next_index
    }
}

impl GameManager {
    fn new() -> GameManager {
        GameManager {
            games: CHashMap::new(),
            next_game_index: AtomicUsize::new(0),
        }
    }

    fn with_game<F, T>(&self, game_id: GameId, f: F) -> T
    where
        F: Fn(&ManagedGame) -> T,
    {
        f(&self.games.get(&game_id).unwrap())
    }

    fn with_mut_game<F, T>(&self, game_id: GameId, f: F) -> T
    where
        F: Fn(&mut ManagedGame) -> T,
    {
        f(&mut self.games.get_mut(&game_id).unwrap())
    }
}

#[tokio::main]
async fn main() {
    let game_manager: Arc<GameManager> = Arc::new(GameManager::new());

    // TODO: the handler panic leaves the server in a zombie state
    //   need to handle panics within each handler!

    let game_manager_for_handler = Arc::clone(&game_manager);
    let get_game = warp::get()
        .and(warp::path!("games" / usize))
        .map(move |game_id| {
            let description = game_manager_for_handler.with_game(GameId(game_id), |g| g.describe());
            warp::reply::json(&description)
        });

    let game_manager_for_handler = Arc::clone(&game_manager);
    let get_private_card = warp::get()
        .and(warp::path!("games" / usize / "players" / usize / "private"))
        .map(move |game_id, player_id| {
            let private_card = game_manager_for_handler
                .with_game(GameId(game_id), |g| g.get_private_card(player_id));
            warp::reply::json(&PrivateCardResponse {
                missing_part: private_card,
            })
        });

    let game_manager_for_handler = Arc::clone(&game_manager);
    let make_move = warp::post()
        .and(warp::path!("games" / usize / "players" / usize / "moves"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(
            move |game_id, player_id, player_action: PlayerAction| -> Box<dyn warp::Reply> {
                let action_result = game_manager_for_handler.with_mut_game(GameId(game_id), |g| {
                    g.make_move(player_id, player_action.clone())
                });
                if let Err(action_error) = action_result {
                    Box::new(warp::reply::with_status(
                        warp::reply::json(&action_error),
                        warp::http::StatusCode::BAD_REQUEST,
                    ))
                } else {
                    Box::new(warp::reply())
                }
            },
        );

    let game_actions = get_game.or(get_private_card).or(make_move);

    let lobby: Arc<Lobby> = Arc::new(Lobby::new());
    let lobby_for_handler = Arc::clone(&lobby);
    let game_manager_for_handler = Arc::clone(&game_manager);
    let join_lobby = warp::post()
        .and(warp::path!("lobby"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(move |request: JoinLobbyRequest| {
            let player_id_in_lobby =
                lobby_for_handler.add_player(request.min_game_size, request.max_game_size);
            lobby_for_handler.start_games(&*game_manager_for_handler);

            if let Some(PlayerAssignedToGame {
                game_id,
                player_id_in_game,
            }) = lobby_for_handler.get_player_game(player_id_in_lobby)
            {
                warp::reply::with_status(
                    warp::reply::with_header(
                        warp::reply::json(&JoinedGameResponse {
                            game_id: game_id.0,
                            player_id_in_game,
                        }),
                        "Location",
                        format!("/games/{}/players/{}/private", game_id.0, player_id_in_game),
                    ),
                    warp::http::StatusCode::CREATED,
                )
            } else {
                warp::reply::with_status(
                    warp::reply::with_header(
                        warp::reply::json(&JoinedLobbyResponse {
                            player_id_in_lobby: player_id_in_lobby.0,
                        }),
                        "Location",
                        format!("/lobby/players/{}/game", player_id_in_lobby.0),
                    ),
                    warp::http::StatusCode::CREATED,
                )
            }
        });

    let lobby_for_handler = Arc::clone(&lobby);
    let get_lobby_player_status = warp::get()
        .and(warp::path!("lobby" / "players" / usize / "game"))
        .map(move |player_id| -> Box<dyn warp::Reply> {
            if let Some(PlayerAssignedToGame {
                game_id,
                player_id_in_game,
            }) = lobby_for_handler.get_player_game(PlayerIdInLobby(player_id))
            {
                Box::new(warp::reply::with_status(
                    warp::reply::with_header(
                        warp::reply::json(&JoinedGameResponse {
                            game_id: game_id.0,
                            player_id_in_game,
                        }),
                        "Location",
                        format!("/games/{}/players/{}/private", game_id.0, player_id_in_game),
                    ),
                    warp::http::StatusCode::TEMPORARY_REDIRECT,
                ))
            } else {
                Box::new(warp::reply::with_status(
                    warp::reply(),
                    warp::http::StatusCode::NOT_FOUND,
                ))
            }
        });

    let lobby_actions = join_lobby.or(get_lobby_player_status);
    let all_actions = game_actions.or(lobby_actions);

    warp::serve(all_actions).run(([127, 0, 0, 1], 3030)).await;
}

#[derive(Serialize)]
struct PrivateCardResponse {
    missing_part: Card,
}

#[derive(Deserialize)]
struct JoinLobbyRequest {
    min_game_size: usize,
    max_game_size: usize,
}

#[derive(Serialize)]
struct JoinedLobbyResponse {
    player_id_in_lobby: usize,
}

#[derive(Serialize)]
struct JoinedGameResponse {
    game_id: usize,
    player_id_in_game: usize,
}
