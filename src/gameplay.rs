use crate::cards::*;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;

#[derive(Debug, PartialEq)]
pub struct TradeOffer {
    pub offered: Card,
    pub in_exchange: Card,
}

#[derive(Debug)]
pub enum PlayerAction {
    Scavenge,
    FinishScavenge {
        card: Card,
    },
    Share {
        with_player: usize,
    },
    Trade {
        with_player: usize,
        offer: TradeOffer,
    },
    TradeAccept,
    TradeReject,
    Steal {
        card: Card,
        from_player: usize,
    },
    Scrap {
        player_cards: Vec<Card>,
        for_discard_card: Card,
    },
    Escape,
    CheatGetCards {
        cards: Vec<Card>,
    },
    Skip,
}

/// Split `s` by the separators in `separators`, in the order they are defined.
///
/// # Examples
///
/// ```
/// split_in_ord("trade 0 offering 2 of Hearts for Ace of Spades", &["offering", "for"])
/// ```
fn split_in_ord<'a, 'b>(s: &'a str, separators: &'b [&str]) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut s = &s[0..];
    for sep in separators {
        let sep_len: usize = sep.len();
        match s.find(sep) {
            Some(0) => s = &s[sep_len..],
            Some(pos) => {
                parts.push(&s[..pos]);
                s = &s[(pos + sep_len)..];
            }
            None => break,
        }
    }
    if s.len() > 0 {
        parts.push(s);
    }
    parts
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
            let action_params = &s[5..];
            match split_in_ord(action_params, &["offering", "for"]).as_slice() {
                [with_player, offered, in_exchange] => {
                    let with_player = first_number(*with_player).ok_or(format!("{} is not a valid player", with_player))?;
                    let offered = Card::try_from(*offered)?;
                    let in_exchange = Card::try_from(*in_exchange)?;

                    return Ok(Trade { with_player, offer: TradeOffer { offered, in_exchange }});
                },
                _ => return Err(String::from("to `trade`, specify which player to trade with, the card offered, and \
                what you expect in return (e.g. `trade 0 offering [your card] for [player 0's card]`)")),
            }
        } else if s.starts_with("steal") {
            let action_params_s = &s[5..];
            match action_params_s
                .split("from")
                .collect::<Vec<&str>>()
                .as_slice()
            {
                [card, player] => {
                    let card = Card::try_from(*card)?;
                    let player = first_number(*player)
                        .ok_or(format!("'{}' does not specify a player", player))?;
                    return Ok(Steal {
                        from_player: player,
                        card,
                    });
                }
                _ => {
                    return Err(String::from(
                        "to `steal`, specify the card to steal, then `from`, then who to steal \
                         from, e.g. `steal Ace of Spades from 0`",
                    ))
                }
            }
        } else if s.starts_with("scrap") {
            let action_params_s = &s[5..];
            match action_params_s
                .split("for")
                .collect::<Vec<&str>>()
                .as_slice()
            {
                [player_cards, for_discard_card] => {
                    let player_cards = player_cards
                        .split(&[',', ';'][..])
                        .map(Card::try_from)
                        .collect::<Result<Vec<Card>, String>>()?;
                    let for_discard_card = Card::try_from(*for_discard_card)?;
                    return Ok(Scrap {
                        player_cards,
                        for_discard_card,
                    });
                }
                _ => return Err(String::from(
                    "to `scrap`, specify the cards to scrap, then `for`, then a card to get \
                     from discard, e.g. \
                     `scrap 2 of Hearts, 3 of Hearts, 4 of Hearts, 5 of Hearts for Ace of Spades`",
                )),
            }
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
    pub fn example_actions() -> String {
        let s = "The following are the valid actions:
        - `scavenge` -- inspect 3 parts from the deck, you get to pick 1, the other 2 are discarded
        - `share [player_id]` -- you get 2 new parts from the deck, the other player gets 1
        - `trade [player_id] offering [your card] for [their card]` -- start a trade with the other player
        - `steal [card] from [player_id]` -- steal a part from the other player
        - `scrap [4 cards you have] for [card in discard]` -- discard 4 parts and pick one card from the discard pile
        - `escape` -- escape the wasteland
        - `skip` -- skip your turn";

        String::from(s)
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

    fn remove_specific_part(&mut self, c: &Card) -> Option<Card> {
        if self.escaped {
            panic!("gameplay bug: remove_part triggered on escaped player");
        }
        vec_remove_item(&mut self.gathered_parts, c)
    }

    fn remove_parts(&mut self, cards_to_remove: &Vec<Card>) -> Vec<Card> {
        if self.escaped {
            panic!("gameplay bug: remove_parts triggered on escaped player");
        }
        let mut result = Vec::new();
        for card_to_remove in cards_to_remove {
            vec_remove_item(&mut self.gathered_parts, card_to_remove).map(|c| result.push(c));
        }
        return result;
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

#[derive(PartialEq)]
pub enum GameState {
    WaitingForPlayerAction {
        player: usize,
    },
    WaitingForScavengeComplete {
        player: usize,
        scavenged_cards: Vec<Card>,
    },
    WaitingForTradeConfirmation {
        initiating_player: usize,
        trading_with_player: usize,
        offer: TradeOffer,
    },
    Finished,
}

pub struct Gameplay {
    draw: Deck,
    discard: Vec<Card>,
    players: Vec<Player>,
    state: GameState,
}

pub enum ActionError {
    DeckEmpty,
    PlayerEscaped {
        escaped_player: usize,
    },
    CardIsNotInDiscard {
        card: Card,
    },
    WrongNumberOfCardsToScrap {
        num_specified: u32,
        num_needed: u32,
    },
    CardIsNotWithPlayer {
        initiating_player: bool,
        player: usize,
        card: Card,
    },
    EscapeConditionNotSatisfied,
    NotPlayersTurn {
        player: usize,
    },
    CardWasNotScavenged {
        card: Card,
    },
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

        write!(f, "The deck has {} cards left\n", self.draw.len())
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
        match self {
            DeckEmpty => write!(f, "the draw deck is empty"),
            PlayerEscaped { escaped_player } => {
                write!(f, "player {} already escaped", escaped_player)
            }
            CardIsNotInDiscard { card } => {
                write!(f, "the discard pile does not contain the {}", card)
            }
            WrongNumberOfCardsToScrap {
                num_specified,
                num_needed,
            } => write!(
                f,
                "you did not offer enough cards ({} offered, {} needed)",
                num_specified, num_needed
            ),
            EscapeConditionNotSatisfied => write!(f, "you don't have all 4 suits of the same rank"),
            CardIsNotWithPlayer {
                initiating_player,
                player,
                card,
            } => {
                if *initiating_player {
                    write!(f, "you don't actually have the {}", card)
                } else {
                    write!(f, "player {} doesn't actually have {}", player, card)
                }
            }
            NotPlayersTurn { player } => write!(f, "it is not player {}'s turn", player),
            CardWasNotScavenged { card } => write!(
                f,
                "{} was not in the scavenged cards, pick a valid one",
                card
            ),
        }
    }
}

// TODO: since this is inefficient, all places that use this should instead use a different data type
fn vec_remove_item<T: PartialEq>(v: &mut Vec<T>, to_remove: &T) -> Option<T> {
    let mut index = None;
    for (i, elem) in v.iter().enumerate() {
        if elem == to_remove {
            index = Some(i);
            break;
        }
    }
    let index = index?;
    Some(v.remove(index))
}

pub struct GameResults {
    pub winners: Vec<usize>,
    pub escaped_but_not_winner: Vec<usize>,
    pub stuck: Vec<usize>,
}

impl Gameplay {
    pub fn init(num_players: usize) -> (Gameplay, Vec<Card>) {
        let mut missing_parts_deck = Deck::shuffle();
        let mut players = Vec::new();
        let mut secret_cards = Vec::new();
        for _ in 0..num_players {
            let player = Player::init(&mut missing_parts_deck);
            secret_cards.push(player.missing_part);
            players.push(player);
        }
        let gameplay = Gameplay {
            players,
            draw: Deck::shuffle(),
            discard: Vec::new(),
            state: GameState::WaitingForPlayerAction { player: 0 },
        };
        (gameplay, secret_cards)
    }

    pub fn get_results(&self) -> GameResults {
        let mut winners = Vec::new();
        let mut escaped_but_not_winner = Vec::new();
        let mut stuck = Vec::new();

        for (i, player) in self.players.iter().enumerate() {
            let has_4_parts = player.has_4_parts();
            let has_missing_part = player.has_missing_part();
            if has_4_parts && has_missing_part {
                winners.push(i);
            } else if has_4_parts {
                // check with Andy what should happen if the endgame is triggered, and some players
                // satisfy the escape condition, but they themselves didn't make an escape move. do they count
                // as escaped or not?
                escaped_but_not_winner.push(i);
            } else {
                stuck.push(i);
            }
        }

        GameResults {
            winners,
            escaped_but_not_winner,
            stuck,
        }
    }

    pub fn get_state(&self) -> &GameState {
        &self.state
    }

    pub fn process_player_action(
        &mut self,
        player_index: usize,
        player_action: PlayerAction,
    ) -> Result<(), ActionError> {
        use PlayerAction::*;
        match player_action {
            Scavenge => {
                self.precondition_waiting_for_player_action(player_index)?;
                // question for Andy: should we re-shuffle the discard pile into draw here
                self.precondition_draw_nonempty()?;
                let player = &mut self.players[player_index];
                if player.can_make_move() {
                    let deck_cards = self.draw.remove_top(3);

                    self.state = GameState::WaitingForScavengeComplete {
                        player: player_index,
                        scavenged_cards: deck_cards,
                    };

                    // need to return early here, so that we don't process the post-move actions just yet
                    return Ok(());
                }
            }
            FinishScavenge { card } => {
                let mut scavenged_cards = match self.state {
                    GameState::WaitingForScavengeComplete {
                        player,
                        ref mut scavenged_cards,
                    } if player == player_index => scavenged_cards,
                    _ => {
                        return Err(ActionError::NotPlayersTurn {
                            player: player_index,
                        })
                    }
                };

                let valid_picked_card = vec_remove_item(&mut scavenged_cards, &card)
                    .ok_or(ActionError::CardWasNotScavenged { card })?;
                self.players[player_index].receive_part(valid_picked_card);
                self.discard.append(&mut scavenged_cards);
            }
            Share { with_player } => {
                self.precondition_waiting_for_player_action(player_index)?;
                // question for Andy: should we re-shuffle the discard pile into draw here
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
            Trade {
                with_player,
                offer:
                    TradeOffer {
                        offered,
                        in_exchange,
                    },
            } => {
                self.precondition_waiting_for_player_action(player_index)?;
                self.precondition_player_has_card(player_index, &offered, true)?;
                self.precondition_player_has_card(with_player, &in_exchange, false)?;
                self.precondition_player_not_escaped(with_player)?;
                let player = &self.players[player_index];
                if player.can_make_move() {
                    self.state = GameState::WaitingForTradeConfirmation {
                        initiating_player: player_index,
                        trading_with_player: with_player,
                        offer: TradeOffer {
                            offered: offered,
                            in_exchange: in_exchange,
                        },
                    };

                    // need to return early here to prevent the turn from advancing. we can only advance one the
                    // trade is complete (or rejected).
                    return Ok(());
                }
            }
            TradeReject => {
                let initiating_player =
                    self.precondition_waiting_for_trade_confirmation(player_index)?;

                // (check with Andy if they should be able to negotiate, of if the player should lose their turn in
                // this case)
                // give the player whose trade was rejected another action
                self.state = GameState::WaitingForPlayerAction {
                    player: initiating_player,
                };
                return Ok(());
            }
            TradeAccept => {
                match &self.state {
                    GameState::WaitingForTradeConfirmation {
                        initiating_player,
                        trading_with_player,
                        offer,
                    } if player_index == *trading_with_player => {
                        // just double check -- should not fail at this point since we checked when we accepted the
                        // offer into the game state
                        self.precondition_player_has_card(
                            *initiating_player,
                            &offer.offered,
                            true,
                        )?;
                        self.precondition_player_has_card(
                            *trading_with_player,
                            &offer.in_exchange,
                            false,
                        )?;

                        // this weird dance is to avoid having 2 elements borrowed mut at the same time, which the borrow
                        // checker does not like
                        let player_card = {
                            let player = &mut self.players[*initiating_player];
                            player.remove_specific_part(&offer.offered)
                        }
                        .ok_or(ActionError::CardIsNotWithPlayer {
                            initiating_player: false,
                            player: *initiating_player,
                            card: offer.offered,
                        })?;
                        let other_player_card = {
                            let other_player = &mut self.players[*trading_with_player];
                            other_player.remove_specific_part(&offer.in_exchange)
                        }
                        .ok_or(ActionError::CardIsNotWithPlayer {
                            initiating_player: false,
                            player: *trading_with_player,
                            card: offer.in_exchange,
                        })?;

                        {
                            let player = &mut self.players[*initiating_player];
                            player.receive_part(other_player_card);
                        }
                        {
                            let other_player = &mut self.players[*trading_with_player];
                            other_player.receive_part(player_card);
                        }
                    }
                    _ => {
                        return Err(ActionError::NotPlayersTurn {
                            player: player_index,
                        })
                    }
                }
            }
            Steal { from_player, card } => {
                self.precondition_waiting_for_player_action(player_index)?;
                self.precondition_player_has_card(from_player, &card, false)?;
                self.precondition_player_not_escaped(from_player)?;
                let player = &self.players[player_index];
                if player.can_make_move() {
                    let stolen_card = {
                        let other_player = &mut self.players[from_player];
                        other_player.remove_specific_part(&card)
                    };

                    let player = &mut self.players[player_index];
                    stolen_card.map(|c| player.receive_part(c));
                }
            }
            Scrap {
                player_cards,
                for_discard_card,
            } => {
                self.precondition_waiting_for_player_action(player_index)?;
                if !self.discard.contains(&for_discard_card) {
                    return Err(ActionError::CardIsNotInDiscard {
                        card: for_discard_card,
                    });
                }
                if player_cards.len() != 4 {
                    return Err(ActionError::WrongNumberOfCardsToScrap {
                        num_specified: player_cards.len() as u32,
                        num_needed: 4,
                    });
                }
                for supposedly_player_card in &player_cards {
                    self.precondition_player_has_card(player_index, &supposedly_player_card, true)?;
                }
                let player = &mut self.players[player_index];
                if player.can_make_move() {
                    let taken_from_discard = vec_remove_item(&mut self.discard, &for_discard_card);
                    taken_from_discard.map(|c| player.receive_part(c));
                    // remove_parts will remove cards from the player even if they don't have all the parts
                    // so it's important to pre-check this. also at this point we've modified the discard
                    let mut cards_taken_from_player = player.remove_parts(&player_cards);
                    self.discard.append(&mut cards_taken_from_player);
                }
            }
            Escape => {
                self.precondition_waiting_for_player_action(player_index)?;
                let player = &mut self.players[player_index];
                if !player.has_4_parts() {
                    return Err(ActionError::EscapeConditionNotSatisfied);
                }
                if !player.escaped {
                    // not using the can_make_move check here: escape is possible without moves
                    // check with Andy what should happen if the endgame is triggered, and some players
                    // satisfy the escape condition, but they themselves didn't make an escape move. do they count
                    // as escaped or not?
                    player.escape();
                    self.trigger_endgame();
                }
            }
            CheatGetCards { cards } => {
                self.precondition_waiting_for_player_action(player_index)?;
                let player = &mut self.players[player_index];
                player.receive_parts(cards);
            }
            Skip => self.precondition_waiting_for_player_action(player_index)?,
        }
        self.auto_escape();
        self.players[player_index].decrease_remaining_moves();
        self.move_to_next_player();
        Ok(())
    }

    fn auto_escape(&mut self) {
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

    fn trigger_endgame(&mut self) {
        for player in &mut self.players {
            if !player.escaped {
                player.set_remaining_moves(1);
            }
        }
    }

    fn move_to_next_player(&mut self) {
        use GameState::*;
        let last_player = match self.state {
            WaitingForPlayerAction { player } => player,
            WaitingForScavengeComplete { player, .. } => player,
            Finished => return,
            WaitingForTradeConfirmation {
                initiating_player, ..
            } => initiating_player,
        };
        let num_players = self.players.len();

        let mut new_state = Finished;
        for i in 1..num_players {
            let player_index = (last_player + i) % num_players;
            if self.players[player_index].can_make_move() {
                new_state = WaitingForPlayerAction {
                    player: player_index,
                };
            }
        }

        self.state = new_state;
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

    fn precondition_player_has_card(
        &self,
        p: usize,
        c: &Card,
        initiating_player: bool,
    ) -> Result<(), ActionError> {
        if !self.players[p].gathered_parts.contains(c) {
            Err(ActionError::CardIsNotWithPlayer {
                initiating_player,
                player: p,
                card: *c,
            })
        } else {
            Ok(())
        }
    }

    fn precondition_waiting_for_player_action(&self, p: usize) -> Result<(), ActionError> {
        match self.state {
            GameState::WaitingForPlayerAction { player } if player == p => Ok(()),
            _ => Err(ActionError::NotPlayersTurn { player: p }),
        }
    }

    fn precondition_waiting_for_trade_confirmation(&self, p: usize) -> Result<usize, ActionError> {
        match self.state {
            GameState::WaitingForTradeConfirmation {
                initiating_player,
                trading_with_player,
                ..
            } if trading_with_player == p => Ok(initiating_player),
            _ => Err(ActionError::NotPlayersTurn { player: p }),
        }
    }
}
