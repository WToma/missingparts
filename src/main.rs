use rand::Rng;
use std::fmt;
use std::io;

#[derive(Debug, Copy, Clone)]
enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    fn arr() -> [Suit; 4] {
        use Suit::*;
        [Clubs, Diamonds, Hearts, Spades]
    }
}

#[derive(Debug, Copy, Clone)]
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

impl Rank {
    fn arr() -> [Rank; 13] {
        use Rank::*;
        [
            Ace, Two, Three, Four, Five, Six, Seven, Eight, Nine, Ten, Jack, Queen, King,
        ]
    }
}

#[derive(Debug)]
struct Card {
    suit: Suit,
    rank: Rank,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} of {:?}", self.rank, self.suit)
    }
}

#[derive(Debug)]
enum PlayerAction {
    Scavenge,
    Share { with_player: usize },
    Trade { with_player: usize },
    Steal { from_player: usize },
    Scrap,
}

impl PlayerAction {
    fn parse(s: &str) -> Option<PlayerAction> {
        use PlayerAction::*;
        let s = s.trim().to_lowercase();
        if s.starts_with("scavenge") {
            return Some(Scavenge);
        } else if s.starts_with("share") {
            return Self::first_number(&s).map(|n| Share { with_player: n });
        } else if s.starts_with("trade") {
            return Self::first_number(&s).map(|n| Trade { with_player: n });
        } else if s.starts_with("steal") {
            return Self::first_number(&s).map(|n| Steal { from_player: n });
        } else if s.starts_with("scrap") {
            return Some(Scrap);
        }

        None
    }

    fn example_actions() -> String {
        let s = "The following are the valid actions:
        - `scavenge` -- inspect 3 parts from the deck, you get to pick 1, the other 2 are discarded
        - `share [player_id]` -- you get 2 new parts from the deck, the other player gets 1
        - `trade [player_id]` -- start a trade with the other player
        - `steal [player_id]` -- steal a part from the other player
        - `scrap` -- discard 4 parts and pick one card from the discard pile";

        String::from(s)
    }

    fn first_number(s: &str) -> Option<usize> {
        s.split_whitespace()
            .map(|ss| ss.parse())
            .filter(|pr| pr.is_ok())
            .map(|pr| pr.expect("we should have filtered errors already"))
            .next()
    }
}

#[derive(Debug)]
struct Deck {
    shuffled_cards: Vec<Card>,
}

impl Deck {
    fn shuffle() -> Deck {
        let mut cards: Vec<Card> = Vec::new();
        for suit in Suit::arr().iter() {
            for rank in Rank::arr().iter() {
                cards.push(Card {
                    suit: *suit,
                    rank: *rank,
                });
            }
        }
        rand::thread_rng().shuffle(&mut cards[..]);
        Deck {
            shuffled_cards: cards,
        }
    }

    fn remove_top(&mut self, n: usize) -> Vec<Card> {
        use std::cmp::min;
        let mut result = Vec::new();
        for _ in 0..min(n, self.shuffled_cards.len()) {
            result.push(self.shuffled_cards.remove(0));
        }
        result
    }

    fn remove_index(&mut self, i: usize) -> Card {
        // panics if i is out of bounds -- use Option?
        self.shuffled_cards.remove(i)
    }
}

#[derive(Debug)]
struct Player {
    missing_part: Card,
    gathered_parts: Vec<Card>,
}

impl Player {
    fn init(missing_parts_deck: &mut Deck) -> Player {
        let missing_part = missing_parts_deck.remove_index(0);
        Player {
            missing_part,
            gathered_parts: Vec::new(),
        }
    }

    fn receive_part(&mut self, c: Card) -> () {
        self.gathered_parts.push(c);
    }

    fn receive_parts(&mut self, mut c: Vec<Card>) -> () {
        while !c.is_empty() {
            self.receive_part(c.remove(0));
        }
    }

    fn remove_part(&mut self, i: usize) -> Card {
        self.gathered_parts.remove(i)
    }

    fn remove_parts(&mut self, n: usize) -> Vec<Card> {
        let result = Vec::new();
        let mut n = n;
        while !self.gathered_parts.is_empty() && n > 0 {
            self.remove_part(0);
            n -= 1;
        }
        result
    }
}

#[derive(Debug)]
struct Gameplay {
    draw: Deck,
    discard: Vec<Card>,
    players: Vec<Player>,
}

impl fmt::Display for Gameplay {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in 0..self.players.len() {
            write!(f, "Player {} has ", i)?;
            let cards = &self.players[i].gathered_parts;
            if !cards.is_empty() {
                card_list(cards, f)?;
            } else {
                write!(f, "nothing")?;
            }
            write!(f, "\n")?;
        }

        write!(f, "The discard pile has ")?;
        if !self.discard.is_empty() {
            card_list(&self.discard, f)?;
        } else {
            write!(f, "nothing")?;
        }
        write!(f, "\n")?;

        write!(
            f,
            "The deck has {} cards left\n",
            self.draw.shuffled_cards.len()
        )
    }
}
fn card_list(cards: &[Card], f: &mut fmt::Formatter) -> fmt::Result {
    for i in 0..cards.len() {
        if i > 0 {
            write!(f, ", ")?;
        }

        write!(f, "{}", &cards[i])?;
    }

    fmt::Result::Ok(())
}

impl Gameplay {
    fn init(num_players: usize) -> Gameplay {
        let mut missing_parts_deck = Deck::shuffle();
        let mut players = Vec::new();
        for _ in 0..num_players {
            players.push(Player::init(&mut missing_parts_deck));
        }
        Gameplay {
            players,
            draw: Deck::shuffle(),
            discard: Vec::new(),
        }
    }

    fn process_player_action(&mut self, player_index: usize, player_action: PlayerAction) -> () {
        use PlayerAction::*;
        match player_action {
            Scavenge => {
                let player = &mut self.players[player_index];
                let mut deck_cards = self.draw.remove_top(3);
                if deck_cards.len() > 0 {
                    // for now always pick the first card, but at this point we should prompt the player to select
                    let player_card = deck_cards.remove(0);
                    player.receive_part(player_card);
                    self.discard.append(&mut deck_cards);
                }
                // else we should prevent this action from happening
            }
            Share { with_player } => {
                let mut deck_cards = self.draw.remove_top(3);

                if deck_cards.len() > 0 {
                    let player = &mut self.players[player_index];
                    let other_player_card = deck_cards.remove(0);
                    player.receive_parts(deck_cards);

                    let other_player = &mut self.players[with_player];
                    other_player.receive_part(other_player_card);
                } // else we should prevent this action from happening
            }
            Trade { with_player } => {
                let player = &self.players[player_index];
                let other_player = &self.players[with_player];
                if !player.gathered_parts.is_empty() && !other_player.gathered_parts.is_empty() {
                    // for now we always trade the top cards. but in reality at this point both players should
                    // be able to select which card to trade, if any, and if there is no agreement, they can
                    // abort the trade, in which case the action completes without changing the game state

                    // this weird dance is to avoid having 2 elements borrowed mut at the same time, which the borrow
                    // checker does not like
                    let player_card = {
                        let player = &mut self.players[player_index];
                        player.remove_part(0)
                    };
                    let other_player_card = {
                        let other_player = &mut self.players[with_player];
                        other_player.remove_part(0)
                    };

                    {
                        let player = &mut self.players[player_index];
                        player.receive_part(other_player_card);
                    }
                    {
                        let other_player = &mut self.players[with_player];
                        other_player.receive_part(player_card);
                    }
                } // else we should prevent this action from happening
            }
            Steal { from_player } => {
                let other_player = &self.players[from_player];
                if !other_player.gathered_parts.is_empty() {
                    // for now we always steal the top card, but in reality at this point the player who is stealing
                    // can choose the card

                    let card = {
                        let other_player = &mut self.players[from_player];
                        other_player.remove_part(0)
                    };

                    let player = &mut self.players[player_index];
                    player.receive_part(card);
                } // else we should prevent this action from happening
            }
            Scrap => {
                let player = &mut self.players[player_index];
                if player.gathered_parts.len() >= 4 && !self.discard.is_empty() {
                    // for now always choose the first card in discard, but in reality at this point the player
                    // can choose
                    let pick_card = self.discard.remove(0);
                    player.receive_part(pick_card);
                    let mut scrapped_cards = player.remove_parts(4);
                    self.discard.append(&mut scrapped_cards);
                } // else we should prevent this action from happening
            }
        }
    }
}

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
    for i in 0..gameplay.players.len() {
        println!(
            "Player {}, your secret part is {}, don't tell anyone",
            i, &gameplay.players[i].missing_part
        )
    }

    loop {
        let mut quit = false;
        for i in 0..gameplay.players.len() {
            println!("{}", gameplay);
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

            let player_action = PlayerAction::parse(player_action_str);
            match player_action {
                Some(action) => gameplay.process_player_action(i, action),
                None => println!(
                    "`{}` is not a valid action. {}\nYou just wasted a turn",
                    player_action_str,
                    PlayerAction::example_actions()
                ),
            }
        }

        if quit {
            break;
        }
    }
}
