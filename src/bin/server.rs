use chashmap::CHashMap;
use missingparts::actionerror::ActionError;
use missingparts::cards::Card;
use missingparts::gameplay::{GameDescription, Gameplay};
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

struct LobbyPlayer {
    min_game_size: usize,
    max_game_size: usize,
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
        players_waiting_for_game.push(LobbyPlayer {
            min_game_size,
            max_game_size,
        });
        player_id
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
    let create_game = warp::post()
        .and(warp::path!("games"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(move |request: CreateGameRequest| {
            let new_index = games_mutex_for_handler.new_game(request.num_players);
            let reply_body = warp::reply::json(&CreateGameResponse { id: new_index });
            warp::reply::with_status(
                warp::reply::with_header(reply_body, "Location", format!("/games/{}", new_index)),
                warp::http::StatusCode::CREATED,
            )
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
        .map(move |game_id, player_id, player_action: PlayerAction| {
            let action_result = games_mutex_for_handler
                .with_mut_game(game_id, |g| g.make_move(player_id, player_action.clone()));
            match action_result {
                Ok(_) => warp::reply::with_status(
                    // TODO: use proper error handling instead. but that's hard to implement
                    // both arms of the match need to result the _exact same type_ otherwise the type
                    // checker complains
                    warp::reply::json(&()),
                    warp::http::StatusCode::OK,
                ),

                Err(action_error) => warp::reply::with_status(
                    warp::reply::json(&action_error),
                    warp::http::StatusCode::BAD_REQUEST,
                ),
            }
        });

    let games = get_game.or(create_game).or(get_private_card).or(make_move);

    let lobby: Arc<Lobby> = Arc::new(Lobby::new());
    let join_lobby = warp::post()
        .and(warp::path!("lobby"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(move |request: JoinLobbyRequest| {
            warp::reply::json(&JoinedLobbyResponse {
                id: lobby.add_player(request.min_game_size, request.max_game_size),
            })
        });
    let all_actions = games.or(join_lobby);

    warp::serve(all_actions).run(([127, 0, 0, 1], 3030)).await;
}

#[derive(Deserialize)]
struct CreateGameRequest {
    num_players: usize,
}

#[derive(Serialize)]
struct CreateGameResponse {
    id: usize,
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
    id: usize,
}