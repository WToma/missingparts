use rand::Rng;
use std::io;

fn main() {
    println!("Missing Parts! -- the command line game");
    println!("Gameplay by Andy");

    let player_secret_part_number = rand::thread_rng().gen_range(0, 52);
    println!(
        "Your secret part number (don't tell anyone) is: {}",
        player_secret_part_number
    );

    loop {
        println!("What's your move?");

        let mut player_action = String::new();
        io::stdin()
            .read_line(&mut player_action)
            .expect("failed to read player's action");
        let player_action = player_action.trim();

        println!("Your move was: {}", player_action);
        if player_action.eq("quit") {
            break;
        }
    }
}
