use rand::Rng;
use std::io;

#[derive(Debug)]
enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

#[derive(Debug)]
enum Rank {
    Ace,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
}

#[derive(Debug)]
struct Card {
    suit: Suit,
    rank: Rank,
}

impl Card {
    fn random() -> Card {
        let rnd = rand::thread_rng().gen_range(0, 52);

        use Suit::*;
        let suit = match rnd / 13 {
            0 => Clubs,
            1 => Diamonds,
            2 => Hearts,
            _ => Spades,
        };

        use Rank::*;
        let rank = match rnd % 13 {
            0 => Ace,
            1 => Two,
            2 => Three,
            3 => Four,
            4 => Five,
            5 => Six,
            6 => Seven,
            7 => Eight,
            8 => Nine,
            9 => Ten,
            10 => Jack,
            11 => Queen,
            _ => King,
        };

        Card {
            suit: suit,
            rank: rank,
        }
    }
}

#[derive(Debug)]
enum PlayerAction {
    Scavenge,
    Share { with_player: i32 },
    Trade { with_player: i32 },
    Steal { from_player: i32 },
}

impl PlayerAction {
    fn parse(s: &str) -> Option<PlayerAction> {
        use PlayerAction::*;
        let s = s.trim().to_lowercase();
        if s.starts_with("scavenge") {
            return Some(Scavenge);
        } else if s.starts_with("share") {
            return first_number(&s).map(|n| Share { with_player: n });
        } else if s.starts_with("trade") {
            return first_number(&s).map(|n| Trade { with_player: n });
        } else if s.starts_with("steal") {
            return first_number(&s).map(|n| Steal { from_player: n });
        }

        None
    }

    fn example_actions() -> String {
        let s = "The following are the valid actions:
        - `scavenge` -- inspect 3 parts from the deck, you get to keep 1, the other 2 are discarded
        - `share [player_id]` -- you get 2 new parts from the deck, the other player gets 1
        - `trade [player_id]` -- start a trade with the other player
        - `steal [player_id]` -- steal a part from the other player";

        String::from(s)
    }
}

fn first_number(s: &str) -> Option<i32> {
    s.split_whitespace()
        .map(|ss| ss.parse())
        .filter(|pr| pr.is_ok())
        .map(|pr| pr.expect("we should have filtered errors already"))
        .next()
}

fn main() {
    println!("Missing Parts! -- the command line game");
    println!("Gameplay by Andy");

    let player_secret_part_card = Card::random();
    println!(
        "Your secret part number (don't tell anyone) is: {:?}",
        player_secret_part_card
    );

    loop {
        println!("What's your move?");

        let mut player_action_str = String::new();
        io::stdin()
            .read_line(&mut player_action_str)
            .expect("failed to read player's action");
        let player_action_str = player_action_str.trim();
        if player_action_str.eq("quit") {
            break;
        }

        let player_action = PlayerAction::parse(player_action_str);
        match player_action {
            Some(action) => println!("Your move was: {:?}", action),
            None => println!(
                "`{}` is not a valid action. {}",
                player_action_str,
                PlayerAction::example_actions()
            ),
        }
    }
}
