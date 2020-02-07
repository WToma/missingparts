use missingparts::actionerror::ActionError;
use missingparts::cards::Card;
use missingparts::gameplay::{GameDescription, Gameplay};
use missingparts::playeraction::PlayerAction;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use warp::Filter;

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

#[tokio::main]
async fn main() {
    let games_mutex: Arc<Mutex<Vec<ManagedGame>>> = Arc::new(Mutex::new(Vec::new()));

    // TODO: the handler panic leaves the server in a zombie state
    //   need to handle panics within each handler!

    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let get_game = warp::get()
        .and(warp::path!("games" / usize))
        .map(move |game_id| {
            let id: usize = game_id;
            let games: &Vec<ManagedGame> = &games_mutex_for_handler.lock().unwrap();
            let game_and_cards: &ManagedGame = &games[id];
            warp::reply::json(&game_and_cards.describe())
        });

    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let create_game = warp::post()
        .and(warp::path!("games"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(move |request: CreateGameRequest| {
            let games: &mut Vec<ManagedGame> = &mut games_mutex_for_handler.lock().unwrap();
            let new_index = games.len();
            games.push(ManagedGame::new(request.num_players));
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
            let games: &Vec<ManagedGame> = &games_mutex_for_handler.lock().unwrap();
            let game_and_cards: &ManagedGame = &games[game_id];
            warp::reply::json(&PrivateCardResponse {
                missing_part: *&game_and_cards.get_private_card(player_id),
            })
        });

    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let make_move = warp::post()
        .and(warp::path!("games" / usize / "players" / usize / "moves"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(move |game_id, player_id, player_action: PlayerAction| {
            let games: &mut Vec<ManagedGame> = &mut games_mutex_for_handler.lock().unwrap();
            let game_and_cards: &mut ManagedGame = &mut games[game_id];
            let action_result = game_and_cards.make_move(player_id, player_action);
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

    warp::serve(games).run(([127, 0, 0, 1], 3030)).await;
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
