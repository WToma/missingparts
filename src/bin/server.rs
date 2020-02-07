use missingparts::cards::Card;
use missingparts::gameplay::Gameplay;
use missingparts::playeraction::PlayerAction;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use warp::Filter;

#[tokio::main]
async fn main() {
    let games_mutex: Arc<Mutex<Vec<(Gameplay, Vec<Card>)>>> = Arc::new(Mutex::new(Vec::new()));

    // TODO: the handler panic leaves the server in a zombie state
    //   need to handle panics within each handler!

    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let get_game = warp::get()
        .and(warp::path!("games" / usize))
        .map(move |game_id| {
            let id: usize = game_id;
            let games: &Vec<(Gameplay, Vec<Card>)> = &games_mutex_for_handler.lock().unwrap();
            let game_and_cards: &(Gameplay, Vec<Card>) = &games[id];
            warp::reply::json(&game_and_cards.0.describe())
        });

    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let create_game = warp::post()
        .and(warp::path!("games"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(move |request: CreateGameRequest| {
            let games: &mut Vec<(Gameplay, Vec<Card>)> =
                &mut games_mutex_for_handler.lock().unwrap();
            let new_index = games.len();
            games.push(Gameplay::init(request.num_players));
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
            let games: &Vec<(Gameplay, Vec<Card>)> = &games_mutex_for_handler.lock().unwrap();
            let game_and_cards: &(Gameplay, Vec<Card>) = &games[game_id];
            let cards: &Vec<Card> = &game_and_cards.1;
            let player_card = &cards[player_id];
            warp::reply::json(&PrivateCardResponse {
                missing_part: *player_card,
            })
        });

    let games_mutex_for_handler = Arc::clone(&games_mutex);
    let make_move = warp::post()
        .and(warp::path!("games" / usize / "players" / usize / "moves"))
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .map(move |game_id, player_id, player_action: PlayerAction| {
            let games: &mut Vec<(Gameplay, Vec<Card>)> =
                &mut games_mutex_for_handler.lock().unwrap();
            let game_and_cards: &mut (Gameplay, Vec<Card>) = &mut games[game_id];
            let game: &mut Gameplay = &mut game_and_cards.0;
            let action_result = game.process_player_action(player_id, player_action);
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
