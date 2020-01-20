use std::convert::TryFrom;
use std::io;

mod cards;
mod gameplay;
use crate::gameplay::*;

fn main() {
    println!("Missing Parts! -- the command line game");
    println!("Gameplay by Andy");

    println!("How many players?");

    let mut num_players_str = String::new();
    io::stdin()
        .read_line(&mut num_players_str)
        .expect("failed to read number of players");
    let num_players_str = num_players_str.trim();
    let num_players = num_players_str
        .parse()
        .expect("number of players must be a positive integer");

    let mut gameplay = Gameplay::init(num_players);
    for (i, player) in gameplay.players.iter().enumerate() {
        println!(
            "Player {}, your secret part is {}, don't tell anyone",
            i, player.missing_part
        )
    }

    let mut game_finished = false;
    let mut quit = false;
    while !quit {
        let mut no_moves_available = true;
        for i in 0..gameplay.players.len() {
            if gameplay.players[i].can_make_move() {
                no_moves_available = false;
                println!("{}", gameplay);

                let mut player_made_valid_move = false;
                while !player_made_valid_move {
                    println!("Player {}, what's your move?", i);
                    let mut player_action_str = String::new();
                    io::stdin()
                        .read_line(&mut player_action_str)
                        .expect("failed to read player's action");
                    let player_action_str = player_action_str.trim();
                    if player_action_str.eq("quit") {
                        quit = true;
                        break;
                    }

                    let player_action = PlayerAction::try_from(player_action_str);
                    match player_action {
                        Ok(action) => match gameplay.process_player_action(i, action) {
                            Ok(_) => player_made_valid_move = true,
                            Err(err) => println!(
                                "`{}` is not possible at this time because {}",
                                player_action_str, err
                            ),
                        },
                        Err(problem) => println!(
                            "`{}` is not a valid action: {}. {}",
                            player_action_str,
                            problem,
                            PlayerAction::example_actions()
                        ),
                    }
                }
            }
            if quit {
                break;
            }
        }

        if no_moves_available {
            game_finished = true;
            quit = true;
        }
    }

    if game_finished {
        let mut winners = Vec::new();
        let mut escaped_but_not_winner = Vec::new();
        let mut stuck = Vec::new();

        for (i, player) in gameplay.players.iter().enumerate() {
            let has_4_parts = player.has_4_parts();
            let has_missing_part = player.has_missing_part();
            if has_4_parts && has_missing_part {
                winners.push(i);
            } else if has_4_parts {
                escaped_but_not_winner.push(i);
            } else {
                stuck.push(i);
            }
        }

        let winners: Vec<String> = winners.iter().map(|x| x.to_string()).collect();
        let escaped_but_not_winner: Vec<String> = escaped_but_not_winner
            .iter()
            .map(|x| x.to_string())
            .collect();
        let stuck: Vec<String> = stuck.iter().map(|x| x.to_string()).collect();

        println!("Winners: {}", winners.join(", "));
        println!(
            "Escaped, but never whole: {}",
            escaped_but_not_winner.join(", ")
        );
        println!("Stuck in the wasteland: {}", stuck.join(", "));
    }
}
