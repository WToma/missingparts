use chashmap::CHashMap;
use missingparts::actionerror::ActionError;
use missingparts::cards::Card;
use missingparts::gameplay::{GameDescription, Gameplay};
use missingparts::gamesizepref::GameSizePreference;
use missingparts::playeraction::PlayerAction;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
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
    games: CHashMap<usize, ManagedGame>,
    next_game_index: Mutex<usize>,
}

impl GameManager {
    fn new() -> GameManager {
        GameManager {
            games: CHashMap::new(),
            next_game_index: Mutex::new(0),
        }
    }

    /// Starts a new game, and returns the ID of the game that can be used with `with_game` and `with_mut_game`.
    fn new_game(&self, num_players: usize) -> usize {
        let next_index = {
            let mut next_game_index_ref = self.next_game_index.lock().unwrap();
            let next_index = *next_game_index_ref;
            *next_game_index_ref += 1;
            next_index
        };
        self.games.insert(next_index, ManagedGame::new(num_players));
        next_index
    }

    fn with_game<F, T>(&self, game_id: usize, f: F) -> T
    where
        F: Fn(&ManagedGame) -> T,
    {
        f(&self.games.get(&game_id).unwrap())
    }

    fn with_mut_game<F, T>(&self, game_id: usize, f: F) -> T
    where
        F: Fn(&mut ManagedGame) -> T,
    {
        f(&mut self.games.get_mut(&game_id).unwrap())
    }
}

#[derive(Clone, Copy)]
struct PlayerAssignedToGame {
    game_id: usize,
    player_id_in_game: usize,
}

enum LobbyPlayer {
    WaitingForGame {
        player_id_in_lobby: usize,
        game_size_preference: GameSizePreference,
    },

    InGame(PlayerAssignedToGame),
}

/// Manages the players who are waiting to join a game. Safe to access concurrently.
struct Lobby {
    players_waiting_for_game: Mutex<Vec<LobbyPlayer>>,
}

impl Lobby {
    fn new() -> Lobby {
        Lobby {
            players_waiting_for_game: Mutex::new(Vec::new()),
        }
    }

    fn add_player(&self, min_game_size: usize, max_game_size: usize) -> usize {
        let players_waiting_for_game = &mut self.players_waiting_for_game.lock().unwrap();
        let player_id = players_waiting_for_game.len();
        players_waiting_for_game.push(LobbyPlayer::WaitingForGame {
            player_id_in_lobby: player_id,
            game_size_preference: GameSizePreference {
                min_game_size,
                max_game_size,
            },
        });
        player_id
    }

    /// Returns the game ID for the given player, if one has been assigned.
    fn get_player_game(&self, player_id: usize) -> Option<PlayerAssignedToGame> {
        let players_waiting_for_game = self.players_waiting_for_game.lock().unwrap();
        if let LobbyPlayer::InGame(game_assignment) = players_waiting_for_game[player_id] {
            Some(game_assignment)
        } else {
            None
        }
    }

    /// Attempts to start a game for the players waiting in the lobby, respecting their game size
    /// preferences.
    fn start_games(&self, game_manager: &GameManager) {
        use LobbyPlayer::*;
        let mut players = self.players_waiting_for_game.lock().unwrap();

        let players_waiting_for_game: Vec<&LobbyPlayer> = players
            .iter()
            .filter(|p| {
                if let WaitingForGame { .. } = p {
                    true
                } else {
                    false
                }
            })
            .collect();
        let preferences: Vec<&GameSizePreference> = players_waiting_for_game
            .iter()
            .filter_map(|p| {
                if let WaitingForGame {
                    game_size_preference,
                    ..
                } = p
                {
                    Some(game_size_preference)
                } else {
                    None
                }
            })
            .collect();
        let indices_in_pwg = GameSizePreference::get_largest_game(&preferences[..]);
        let player_ids_in_game: Vec<usize> = indices_in_pwg
            .iter()
            .filter_map(|i| {
                if let WaitingForGame {
                    player_id_in_lobby, ..
                } = players_waiting_for_game[*i]
                {
                    Some(*player_id_in_lobby)
                } else {
                    None
                }
            })
            .collect();

        if player_ids_in_game.len() > 0 {
            let game_id = game_manager.new_game(player_ids_in_game.len());

            for (player_id_in_game, player_id_in_lobby) in player_ids_in_game.iter().enumerate() {
                players[*player_id_in_lobby] = InGame(PlayerAssignedToGame {
                    game_id,
                    player_id_in_game,
                });
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let games_mutex: Arc<GameManager> = Arc::new(GameManager::new());

    // TODO: the handler panic leaves the server in a zombie state
    //   need to handle panics within each handler!

    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let get_game = warp::get()
        .and(warp::path!("games" / usize))
        .map(move |game_id| {
            let id: usize = game_id;
            let description = games_mutex_for_handler.with_game(id, |g| g.describe());
            warp::reply::json(&description)
        });

    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let get_private_card = warp::get()
        .and(warp::path!("games" / usize / "players" / usize / "private"))
        .map(move |game_id, player_id| {
            let private_card =
                games_mutex_for_handler.with_game(game_id, |g| g.get_private_card(player_id));
            warp::reply::json(&PrivateCardResponse {
                missing_part: private_card,
            })
        });

    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let make_move = warp::post()
        .and(warp::path!("games" / usize / "players" / usize / "moves"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(
            move |game_id, player_id, player_action: PlayerAction| -> Box<dyn warp::Reply> {
                let action_result = games_mutex_for_handler
                    .with_mut_game(game_id, |g| g.make_move(player_id, player_action.clone()));
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
    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let join_lobby = warp::post()
        .and(warp::path!("lobby"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(move |request: JoinLobbyRequest| {
            let player_id_in_lobby =
                lobby_for_handler.add_player(request.min_game_size, request.max_game_size);
            lobby_for_handler.start_games(&games_mutex_for_handler);
            warp::reply::with_header(
                warp::reply::json(&JoinedLobbyResponse { player_id_in_lobby }),
                "Location",
                format!("/lobby/players/{}/game", player_id_in_lobby),
            )
        });

    let lobby_for_handler = Arc::clone(&lobby);
    let get_lobby_player_status = warp::get()
        .and(warp::path!("lobby" / "players" / usize / "game"))
        .map(move |player_id| -> Box<dyn warp::Reply> {
            if let Some(PlayerAssignedToGame {
                game_id,
                player_id_in_game,
            }) = lobby_for_handler.get_player_game(player_id)
            {
                Box::new(warp::reply::with_status(
                    warp::reply::with_header(
                        warp::reply::json(&JoinedGameResponse {
                            game_id,
                            player_id_in_game,
                        }),
                        "Location",
                        format!("/games/{}/players/{}/private", game_id, player_id_in_game),
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
