use rand::Rng;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::io;

#[derive(Debug, Copy, Clone, PartialEq)]
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

impl TryFrom<&str> for Suit {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        use Suit::*;
        let first_char = s.trim().chars().next().ok_or(())?;
        match first_char.to_ascii_lowercase() {
            'c' => Ok(Clubs),
            'd' => Ok(Diamonds),
            'h' => Ok(Hearts),
            's' => Ok(Spades),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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

fn first_number(s: &str) -> Option<usize> {
    s.split_whitespace()
        .map(|ss| ss.parse())
        .filter(|pr| pr.is_ok())
        .map(|pr| pr.expect("we should have filtered errors already"))
        .next()
}

impl TryFrom<&str> for Rank {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        use Rank::*;

        let first_char = s.chars().next().ok_or(())?;
        match first_char.to_ascii_lowercase() {
            'a' => Ok(Ace),
            'j' => Ok(Jack),
            'q' => Ok(Queen),
            'k' => Ok(King),
            _ => {
                let n: usize = first_number(s).ok_or(())?;
                if n >= 1 && n <= 13 {
                    Ok(Rank::arr()[n - 1])
                } else {
                    Err(())
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
struct Card {
    suit: Suit,
    rank: Rank,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} of {:?}", self.rank, self.suit)
    }
}

impl TryFrom<&str> for Card {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        // accepted format: "{rank} [of ]{suit}", e.g. "4 of Clubs", "a of h", "K D"
        let mut parts = s.split_whitespace();

        let rank = parts.next().ok_or(format!(
            "'{}' is not a valid card, because the rank is missing",
            s,
        ))?;
        let rank = Rank::try_from(rank).map_err(|_| {
            format!(
                "'{}' is not a valid card, because '{}' is not a valid rank",
                s, rank,
            )
        })?;

        let second_part = parts.next().ok_or(format!(
            "'{}' is not a valid card, because it is missing the suit",
            s
        ))?;
        let suit = if second_part.to_ascii_lowercase() == "of" {
            parts.next().ok_or(format!(
                "'{}' is not a valid card, because it is missing the suit",
                s
            ))?
        } else {
            second_part
        };
        let suit = Suit::try_from(suit).map_err(|_| {
            format!(
                "'{}' is not a valid card, because '{}' is not a valid suit",
                s, suit,
            )
        })?;

        Ok(Card { suit, rank })
    }
}

#[derive(Debug)]
enum PlayerAction {
    Scavenge,
    Share { with_player: usize },
    Trade { with_player: usize },
    Steal { from_player: usize },
    Scrap,
    Escape,
    CheatGetCards { cards: Vec<Card> },
    Skip,
}

impl TryFrom<&str> for PlayerAction {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        use PlayerAction::*;

        let s = s.trim().to_lowercase();
        if s.starts_with("scavenge") {
            return Ok(Scavenge);
        } else if s.starts_with("share") {
            let n = first_number(&s)
                .ok_or("to `share`, specify which player to share with (e.g. `share 0`)")?;
            return Ok(Share { with_player: n });
        } else if s.starts_with("trade") {
            let n = first_number(&s)
                .ok_or("to `trade`, specify which player to trade with (e.g. `trade 0`)")?;
            return Ok(Trade { with_player: n });
        } else if s.starts_with("steal") {
            let n = first_number(&s)
                .ok_or("to `steal`, specify which player to steal from (e.g. `steal 0`)")?;
            return Ok(Steal { from_player: n });
        } else if s.starts_with("scrap") {
            return Ok(Scrap);
        } else if s.starts_with("escape") {
            return Ok(Escape);
        } else if s.starts_with("conjure") {
            let cards = s[7..]
                .split(&[',', ';'][..])
                .map(Card::try_from)
                .collect::<Result<Vec<Card>, String>>()?;
            return Ok(CheatGetCards { cards });
        } else if s.starts_with("skip") {
            return Ok(Skip);
        }

        let first_word = s.split_whitespace().next().unwrap_or(&s);
        Err(format!("'{}' is not a valid action", first_word))
    }
}

impl PlayerAction {
    fn example_actions() -> String {
        let s = "The following are the valid actions:
        - `scavenge` -- inspect 3 parts from the deck, you get to pick 1, the other 2 are discarded
        - `share [player_id]` -- you get 2 new parts from the deck, the other player gets 1
        - `trade [player_id]` -- start a trade with the other player
        - `steal [player_id]` -- steal a part from the other player
        - `scrap` -- discard 4 parts and pick one card from the discard pile
        - `escape` -- escape the wasteland
        - `skip` -- skip your turn";

        String::from(s)
    }
}

#[derive(Debug)]
struct Deck {
    shuffled_cards: Vec<Card>,
}

impl Deck {
    fn shuffle() -> Deck {
        let mut cards: Vec<Card> = Vec::new();
        for suit in &Suit::arr() {
            for rank in &Rank::arr() {
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

    fn non_empty(&self) -> bool {
        !self.shuffled_cards.is_empty()
    }
}

#[derive(Debug)]
struct Player {
    missing_part: Card,
    gathered_parts: Vec<Card>,
    escaped: bool,
    moves_left: Option<u32>,
}

impl Player {
    fn init(missing_parts_deck: &mut Deck) -> Player {
        let missing_part = missing_parts_deck.remove_index(0);
        Player {
            missing_part,
            gathered_parts: Vec::new(),
            escaped: false,
            moves_left: None,
        }
    }

    fn receive_part(&mut self, c: Card) -> () {
        if self.escaped {
            // TODO remove these `panic!` checks. Probably should use a separate type for
            // escaped player states so that these cannot be called accidentally
            panic!("gameplay bug: receive_part triggered on escaped player");
        }
        self.gathered_parts.push(c);
    }

    fn receive_parts(&mut self, mut c: Vec<Card>) -> () {
        if self.escaped {
            panic!("gameplay bug: receive_parts triggered on escaped player");
        }
        while !c.is_empty() {
            self.receive_part(c.remove(0));
        }
    }

    fn remove_part(&mut self, i: usize) -> Card {
        if self.escaped {
            panic!("gameplay bug: remove_part triggered on escaped player");
        }
        self.gathered_parts.remove(i)
    }

    fn remove_parts(&mut self, n: usize) -> Vec<Card> {
        if self.escaped {
            panic!("gameplay bug: remove_parts triggered on escaped player");
        }
        let result = Vec::new();
        let mut n = n;
        while !self.gathered_parts.is_empty() && n > 0 {
            self.remove_part(0);
            n -= 1;
        }
        result
    }

    fn escape(&mut self) -> () {
        if !self.has_4_parts() {
            panic!("gameplay bug: escape triggered without has_4_parts() checked");
        }

        self.escaped = true;
    }

    fn can_make_move(&self) -> bool {
        let has_moves_left = self.moves_left.map_or(true, |x| (x > 0));
        !self.escaped && has_moves_left
    }

    fn set_remaining_moves(&mut self, n: u32) -> () {
        self.moves_left = Some(n);
    }

    fn decrease_remaining_moves(&mut self) -> () {
        self.moves_left = self.moves_left.map(|x| x - 1);
    }

    fn has_4_parts(&self) -> bool {
        let mut num_cards_per_rank = HashMap::new();
        for card in &self.gathered_parts {
            let n = num_cards_per_rank.entry(card.rank).or_insert(0);
            *n += 1;
            if *n >= 4 {
                return true;
            }
        }
        false
    }

    fn has_missing_part(&self) -> bool {
        self.gathered_parts.contains(&self.missing_part)
    }
}

#[derive(Debug)]
struct Gameplay {
    draw: Deck,
    discard: Vec<Card>,
    players: Vec<Player>,
}

#[derive(Debug)]
enum ActionError {
    DeckEmpty,
    PlayerEscaped {
        escaped_player: usize,
    },
    PlayerCardsEmpty {
        initiating_player: bool,
        empty_handed_player: usize,
    },
    DiscardPileEmpty,
    NotEnoughCardsToScrap {
        num_available: u32,
        num_needed: u32,
    },
    EscapeConditionNotSatisfied,
}

impl fmt::Display for Gameplay {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, player) in self.players.iter().enumerate() {
            let in_game_or_escaped = if player.escaped {
                "escaped".to_string()
            } else {
                player
                    .moves_left
                    .map(|x| x.to_string() + " moves left")
                    .unwrap_or("in game".to_string())
            };
            write!(f, "Player {} ({}) has ", i, in_game_or_escaped)?;
            let cards = &player.gathered_parts;
            if !cards.is_empty() {
                card_list(&cards, f)?;
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
    for (i, card) in cards.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }

        write!(f, "{}", card)?;
    }

    fmt::Result::Ok(())
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ActionError::*;
        match *self {
            DeckEmpty => write!(f, "the draw deck is empty"),
            PlayerEscaped { escaped_player } => {
                write!(f, "player {} already escaped", escaped_player)
            }
            PlayerCardsEmpty {
                initiating_player,
                empty_handed_player,
            } => {
                if initiating_player {
                    write!(f, "you don't have any cards")
                } else {
                    write!(f, "player {} doesn't have any cards", empty_handed_player)
                }
            }
            DiscardPileEmpty => write!(f, "the discard pile is empty"),
            NotEnoughCardsToScrap {
                num_available,
                num_needed,
            } => write!(
                f,
                "you don't have enough cards (you have {}, {} needed)",
                num_available, num_needed
            ),
            EscapeConditionNotSatisfied => write!(f, "you don't have all 4 suits of the same rank"),
        }
    }
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

    fn process_player_action(
        &mut self,
        player_index: usize,
        player_action: PlayerAction,
    ) -> Result<(), ActionError> {
        use PlayerAction::*;
        match player_action {
            Scavenge => {
                self.precondition_draw_nonempty()?;
                let player = &mut self.players[player_index];
                if player.can_make_move() {
                    let mut deck_cards = self.draw.remove_top(3);

                    // for now always pick the first card, but at this point we should prompt the player to select
                    let player_card = deck_cards.remove(0);
                    player.receive_part(player_card);
                    self.discard.append(&mut deck_cards);
                }
            }
            Share { with_player } => {
                self.precondition_draw_nonempty()?;
                self.precondition_player_not_escaped(with_player)?;

                let mut deck_cards = self.draw.remove_top(3);
                let player = &self.players[player_index];
                if player.can_make_move() {
                    let player = &mut self.players[player_index];
                    let other_player_card = deck_cards.remove(0);
                    player.receive_parts(deck_cards);

                    let other_player = &mut self.players[with_player];
                    other_player.receive_part(other_player_card);
                }
            }
            Trade { with_player } => {
                self.precondition_player_has_cards(player_index, true)?;
                self.precondition_player_has_cards(with_player, false)?;
                self.precondition_player_not_escaped(with_player)?;
                let player = &self.players[player_index];
                if player.can_make_move() {
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
                }
            }
            Steal { from_player } => {
                self.precondition_player_has_cards(from_player, false)?;
                self.precondition_player_not_escaped(from_player)?;
                let player = &self.players[player_index];
                if player.can_make_move() {
                    // for now we always steal the top card, but in reality at this point the player who is stealing
                    // can choose the card

                    let card = {
                        let other_player = &mut self.players[from_player];
                        other_player.remove_part(0)
                    };

                    let player = &mut self.players[player_index];
                    player.receive_part(card);
                }
            }
            Scrap => {
                if self.discard.is_empty() {
                    return Err(ActionError::DiscardPileEmpty);
                }
                let player = &mut self.players[player_index];
                if player.gathered_parts.len() < 4 {
                    return Err(ActionError::NotEnoughCardsToScrap {
                        num_available: player.gathered_parts.len() as u32,
                        num_needed: 4,
                    });
                }
                if player.can_make_move() {
                    // for now always choose the first card in discard, but in reality at this point the player
                    // can choose
                    let pick_card = self.discard.remove(0);
                    player.receive_part(pick_card);
                    let mut scrapped_cards = player.remove_parts(4);
                    self.discard.append(&mut scrapped_cards);
                }
            }
            Escape => {
                let player = &mut self.players[player_index];
                if !player.has_4_parts() {
                    return Err(ActionError::EscapeConditionNotSatisfied);
                }
                if !player.escaped {
                    // not using the can_make_move check here: escape is possible without moves
                    player.escape();
                    self.trigger_endgame();
                }
            }
            CheatGetCards { cards } => {
                let player = &mut self.players[player_index];
                player.receive_parts(cards);
            }
            Skip => (),
        }
        self.auto_escape();
        self.players[player_index].decrease_remaining_moves();
        Ok(())
    }

    fn auto_escape(&mut self) -> () {
        let mut escaped = false;
        for player in &mut self.players {
            if player.has_4_parts() && player.has_missing_part() {
                player.escape();
                escaped = true;
            }
        }
        if escaped {
            self.trigger_endgame();
        }
    }

    fn trigger_endgame(&mut self) -> () {
        for player in &mut self.players {
            if !player.escaped {
                player.set_remaining_moves(1);
            }
        }
    }

    fn precondition_draw_nonempty(&self) -> Result<(), ActionError> {
        if self.draw.non_empty() {
            Ok(())
        } else {
            Err(ActionError::DeckEmpty)
        }
    }

    fn precondition_player_not_escaped(&self, p: usize) -> Result<(), ActionError> {
        if self.players[p].escaped {
            Err(ActionError::PlayerEscaped { escaped_player: p })
        } else {
            Ok(())
        }
    }

    fn precondition_player_has_cards(
        &self,
        p: usize,
        initiating_player: bool,
    ) -> Result<(), ActionError> {
        if self.players[p].gathered_parts.is_empty() {
            Err(ActionError::PlayerCardsEmpty {
                empty_handed_player: p,
                initiating_player,
            })
        } else {
            Ok(())
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
