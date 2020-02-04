use std::convert::TryFrom;
use std::io;

use missingparts::cards::Card;
use missingparts::gameplay::*;
use missingparts::playeraction::*;

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

    let (mut gameplay, secret_cards) = Gameplay::init(num_players);
    for (i, secret_card) in secret_cards.iter().enumerate() {
        println!(
            "Player {}, your secret part is {}, don't tell anyone",
            i, secret_card
        )
    }

    let mut quit = false;
    while !quit {
        match gameplay.get_state() {
            GameState::WaitingForPlayerAction { player } => {
                let player = *player;
                println!("{}", gameplay.describe());

                let mut player_made_valid_move = false;
                while !player_made_valid_move {
                    println!("Player {}, what's your move?", player);
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
                        Ok(action) => match gameplay.process_player_action(player, action) {
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
            GameState::WaitingForScavengeComplete {
                player,
                scavenged_cards,
            } => {
                let mut card;
                loop {
                    println!(
                        "You scavenged these parts are: {}. Which one do you want to keep?",
                        scavenged_cards
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<String>>()
                            .join(", ")
                    );
                    let mut card_str = String::new();
                    io::stdin()
                        .read_line(&mut card_str)
                        .expect("failed to read card");
                    let card_str = card_str.trim();
                    card = match Card::try_from(card_str) {
                        Ok(c) => c,
                        Err(e) => {
                            println!("{}", e);
                            continue;
                        }
                    };
                    if !scavenged_cards.contains(&card) {
                        println!("{} was not one of your scavenged parts", card);
                    } else {
                        break;
                    }
                }

                let player = *player;
                // ignore the error except printing it. We checked the necessary precondition (the card is part of the)
                // scavenged parts, this action cannot fail otherwise right now. Really it would be nicer if this was
                // in the loop, but that becomes a huge pain because then `scavenged_cards` needs to be moved to
                // satisfy the borrow checker. If the action _does_ fail, we'll print the response and the game will
                // remain in the same state, so we'll come back to this same branch anyway.
                gameplay
                    .process_player_action(player, PlayerAction::FinishScavenge { card })
                    .err()
                    .map(|err| println!("{}", err));
            }
            GameState::WaitingForTradeConfirmation {
                initiating_player,
                trading_with_player,
                offer:
                    TradeOffer {
                        offered,
                        in_exchange,
                    },
            } => {
                let trading_with_player = *trading_with_player;
                println!(
                    "Player {}! Player {} wants to trade. Here's the deal:
                They give you {}
                You give them {}.
                Deal?",
                    trading_with_player, initiating_player, offered, in_exchange
                );

                let action: PlayerAction;
                loop {
                    let mut action_str = String::new();
                    io::stdin()
                        .read_line(&mut action_str)
                        .expect("failed to read action");
                    match action_str.to_lowercase().trim() {
                        "yes" | "yup" | "ok" => {
                            action = PlayerAction::TradeAccept;
                            break;
                        }
                        "no" | "nope" | "no way" => {
                            action = PlayerAction::TradeReject;
                            break;
                        }
                        _ => println!("please just say 'yes' or 'no'"),
                    }
                }
                // ignore the error except printing it. this action cannot fail right now. Really it would be nicer if
                // this was in the loop, but that becomes a huge pain because of borrowing
                gameplay
                    .process_player_action(trading_with_player, action)
                    .err()
                    .map(|err| println!("{}", err));
            }
            GameState::Finished => break,
        };
    }

    if *(gameplay.get_state()) == GameState::Finished {
        let game_res = gameplay.get_results();
        let winners: Vec<String> = game_res.winners.iter().map(|x| x.to_string()).collect();
        let escaped_but_not_winner: Vec<String> = game_res
            .escaped_but_not_winner
            .iter()
            .map(|x| x.to_string())
            .collect();
        let stuck: Vec<String> = game_res.stuck.iter().map(|x| x.to_string()).collect();

        println!("Winners: {}", winners.join(", "));
        println!(
            "Escaped, but never whole: {}",
            escaped_but_not_winner.join(", ")
        );
        println!("Stuck in the wasteland: {}", stuck.join(", "));
    }
}
