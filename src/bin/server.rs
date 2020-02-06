use missingparts::cards::Card;
use missingparts::gameplay::Gameplay;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use warp::Filter;

#[tokio::main]
async fn main() {
    let games_mutex: Arc<Mutex<Vec<(Gameplay, Vec<Card>)>>> = Arc::new(Mutex::new(Vec::new()));

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

    let games = get_game.or(create_game);

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
