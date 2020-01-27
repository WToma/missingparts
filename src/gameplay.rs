//! Types to play a game of Missing Parts.
//!
//! The [`Gameplay`](struct.Gameplay.html) type is the main way to interact with the game.

use crate::actionerror::*;
use crate::cards::*;
use crate::playeraction::*;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
struct Player {
    missing_part: Card,
    gathered_parts: Vec<Card>,
    escaped: bool,
    moves_left: Option<u32>,
}

impl Player {
    fn init(missing_parts_deck: &mut Deck) -> Player {
        let missing_part = *(missing_parts_deck
            .remove_top(1)
            .first()
            .expect("the missing parts deck was empty!"));
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

/// The part of the game's observable state that determines which actions or multi-part completing actions can be taken.
#[derive(PartialEq, Debug)]
pub enum GameState {
    /// The game is waiting for a turn action from `player` (see [`PlayerAction`](enum.PlayerAction.html)).
    WaitingForPlayerAction {
        /// The player whose turn it is.
        player: usize,
    },

    /// The game is waiting for a [`FinishScavenge`](enum.PlayerAction.html#variant.FinishScavenge) action from
    /// `player`.
    WaitingForScavengeComplete {
        /// The player who must complete the scavenge.
        player: usize,

        /// The cards that were turned up as part of the scavenge. The
        /// [`FinishScavenge`](enum.PlayerAction.html#variant.FinishScavenge) action must specify one of these cards.
        scavenged_cards: Vec<Card>,
    },

    /// The game is waiting for a trade to be accepted ([`TradeAccept`](enum.PlayerAction.html#variant.TradeAccept)) or
    /// rejected ([`TradeReject`](enum.PlayerAction.html#variant.TradeReject)) by `trading_with_player`.
    WaitingForTradeConfirmation {
        /// The player who initiated the trade.
        initiating_player: usize,

        /// The player who must accept or reject the offer.
        trading_with_player: usize,

        /// The offer that was made by `initiating_player` to `trading_with_player`.
        offer: TradeOffer,
    },

    /// The game is finished. No more actions can be made.
    Finished,
}

/// The main game type.
///
/// - to start a game, create an instance using [`init`](#method.init).
/// - the game can be advanced by using [`process_player_action`](#method.process_player_action).
/// - to determine what are the valid actions, see the [`GameState`](enum.GameState.html) type. To see the current
///   game state, use [`get_state`](#method.get_state).
/// - once the game is finished, [`get_results`](#method.get_results) provides a summary of the game for score keeping.
pub struct Gameplay {
    draw: Deck,
    discard: Vec<Card>,
    players: Vec<Player>,
    state: GameState,
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

// TODO: since this is inefficient, all places that use this should instead use a different data type
/// Removes an element equal to `to_remove` from `v`, returning the removed element. Returns `None` and leaves `v`
/// intact if `to_remove` is not in `v`.
///
/// # Examples
///
/// If the element to remove exists:
/// ```
/// # use missingparts::gameplay::*;
/// let mut v = vec![1, 2, 3];
/// let removed = vec_remove_item(&mut v, &2);
/// assert_eq!(v, vec![1, 3]);
/// assert_eq!(removed, Some(2));
/// ```
///
/// If the element to remove does not exist:
/// ```
/// # use missingparts::gameplay::*;
/// let mut v = vec!['a', 'c'];
/// let removed = vec_remove_item(&mut v, &'b');
/// assert_eq!(removed, None);
/// ```
pub fn vec_remove_item<T: PartialEq>(v: &mut Vec<T>, to_remove: &T) -> Option<T> {
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

fn vec_remove_top_n<T>(v: &mut Vec<T>, n: usize) -> Vec<T> {
    let mut result = Vec::new();
    for _ in 0..n {
        if v.is_empty() {
            break;
        }
        result.push(v.remove(0));
    }
    result
}

/// A short summary of the results fo the game, once the game is finished.
pub struct GameResults {
    /// Players who have escaped, and have their missing part.
    pub winners: Vec<usize>,

    /// Players who have escaped, but do not have their missing part.
    pub escaped_but_not_winner: Vec<usize>,

    /// Players who did not escape.
    pub stuck: Vec<usize>,
}

impl Gameplay {
    /// Create a new game with the specified number of players.
    ///
    /// Returns the new game instance, and the missing parts (secret cards) for each player.
    ///
    /// For now players in the game are identified by `usize` integers. These are also the indices in the secret cards
    /// array. These should be used as the player parameter when advancing the game state, these are the references
    /// in the game state, and in the results as well.
    ///
    /// The game in the state waiting for player 0's (the first player's) action.
    ///
    /// # Examples
    /// ```
    /// # use missingparts::gameplay::*;
    /// let (mut game, secret_cards) = Gameplay::init(2);
    ///
    /// // show these cards to each player, but not to the other players
    /// let secret_part_for_first_player = secret_cards[0];
    /// let secret_part_for_second_player = secret_cards[1];
    /// ```
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

    /// Once the game is finished, get a short summary of the results. (The method can be called even if the game has
    /// not finished yet, and it will reflect the current state, but then the results are subject to change.)
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

    /// Get the current game state to determine which action can be taken. (See the
    /// [`PlayerAction`](enum.PlayerAction.html)) documentation for that.)
    ///
    /// Typically the main game loop will consist of calling `get_state` to determine what action can be taken,
    /// receiving an action from the player, and processing it using
    /// [`process_player_action`](#method.process_player_action).
    ///
    /// # Examples
    /// ```
    /// # use missingparts::gameplay::*;
    /// # use missingparts::playeraction::*;
    /// use GameState::*;
    /// use std::convert::TryFrom;
    /// let (mut game, _) = Gameplay::init(2);
    /// loop {
    ///     match game.get_state() {
    ///         WaitingForPlayerAction { player } => {
    ///             let player = *player;
    ///
    ///             // in both of these cases, in a real situation, prompt the player for another action.
    ///             let player_action = PlayerAction::try_from("share 1").expect("valid action needed");
    ///             game.process_player_action(player, player_action).expect("impossible action");
    /// #           break;
    ///         },
    ///         Finished => break,
    ///         // ... handle other states ...
    ///  #      _ => unimplemented!(),
    ///     }
    /// }
    /// ```
    pub fn get_state(&self) -> &GameState {
        &self.state
    }

    /// Process an action from a player
    ///
    /// This method can be called any time, if an action is not appropriate at the time (for example because it is not
    /// the specified player's turn, or the action is not appropriate for the game state) an error will be returned.
    /// However to avoid wasting cycles it is recommended to call [`get_state`](#method.get_state) first to determine
    /// what actions are possible.
    ///
    /// The player making the action is specified by `player_index`.
    ///
    /// If the action is impossible, the game state will not change, and an error is returned. If an error is not
    /// returned then the action is completed as soon as `process_player_action` returns, and the update should be
    /// reflected in the state returned by `get_state`.
    ///
    /// # Examples
    /// ```
    /// # use missingparts::gameplay::*;
    /// # use missingparts::playeraction::*;
    /// use GameState::*;
    /// use std::convert::TryFrom;
    /// let (mut game, _) = Gameplay::init(2);
    /// loop {
    ///     match game.get_state() {
    ///         WaitingForPlayerAction { player } => {
    ///             let player = *player;
    ///
    ///             // in both of these cases, in a real situation, prompt the player for another action.
    ///             let player_action = PlayerAction::try_from("share 1").expect("valid action needed");
    ///             game.process_player_action(player, player_action).expect("impossible action");
    /// #           break;
    ///         },
    ///         Finished => break,
    ///         // ... handle other states ...
    ///  #      _ => unimplemented!(),
    ///     }
    /// }
    /// ```
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
                self.precondition_player_exists(with_player)?;
                self.precondition_waiting_for_player_action(player_index)?;
                // question for Andy: should we re-shuffle the discard pile into draw here
                self.precondition_draw_nonempty()?;

                Self::precondition_players_different(player_index, with_player)?;
                self.precondition_player_not_escaped(with_player)?;

                let player = &self.players[player_index];
                if player.can_make_move() {
                    let mut deck_cards = self.draw.remove_top(3);

                    let player = &mut self.players[player_index];
                    let player_cards = vec_remove_top_n(&mut deck_cards, 2);
                    player.receive_parts(player_cards);

                    let other_player = &mut self.players[with_player];
                    other_player.receive_parts(deck_cards);
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
                self.precondition_player_exists(with_player)?;
                self.precondition_waiting_for_player_action(player_index)?;
                self.precondition_player_has_card(player_index, &offered, true)?;
                self.precondition_player_has_card(with_player, &in_exchange, false)?;
                self.precondition_player_not_escaped(with_player)?;
                Self::precondition_players_different(player_index, with_player)?;
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
                self.precondition_player_exists(from_player)?;
                self.precondition_waiting_for_player_action(player_index)?;
                self.precondition_player_has_card(from_player, &card, false)?;
                self.precondition_player_not_escaped(from_player)?;
                Self::precondition_players_different(player_index, from_player)?;
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
                {
                    use std::collections::HashSet;
                    let mut unique_cards = HashSet::new();
                    for card in &player_cards {
                        unique_cards.insert(card.clone());
                    }
                    if unique_cards.len() != 4 {
                        return Err(ActionError::WrongNumberOfCardsToScrap {
                            num_specified: unique_cards.len() as u32,
                            num_needed: 4,
                        });
                    }
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

                // returning early so that the cheating player gets another action.
                return Ok(());
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

    fn precondition_players_different(
        action_player: usize,
        target_player: usize,
    ) -> Result<(), ActionError> {
        if action_player == target_player {
            Err(ActionError::SelfTargeting)
        } else {
            Ok(())
        }
    }

    fn precondition_player_exists(&self, p: usize) -> Result<(), ActionError> {
        if self.players.len() > p {
            Ok(())
        } else {
            Err(ActionError::InvalidPlayerReference {
                non_existent_player: p,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn preconditions() {
        // test that for each precondition of each action, if the precondition is not satisfied, the action fails
        // with the appropriate error

        // All turn actions:
        // see: test_turn_actions_preconditions

        // Scavenge
        test_precondition_empty_deck(SCAVENGE);

        // FinishScavenge
        test_precondition_completion_wrong_state(PlayerAction::FinishScavenge { card: c("q c") });
        test_precondition(
            1,                                              // player 1
            action_finish_scavenge("q c"),                  // trying to finish a scavenge
            |mut g| g.state = state_scavenged(0, &["q c"]), // but it's player 0's turn to finish the scavenge
            ActionError::NotPlayersTurn { player: 1 },
        );
        test_precondition(
            0,                                              // player 0
            action_finish_scavenge("q c"), //                  trying to accept Queen of Clubs from scavenge
            |mut g| g.state = state_scavenged(0, &["k c"]), // but the scavenge only contained King of Clubs
            ActionError::CardWasNotScavenged { card: c("q c") },
        );

        // Trade
        test_precondition_as(
            0,     //                                                            player 0
            TRADE, //                                                            trying to trade 2 h
            |g| {
                vec_remove_item(&mut g.players[0].gathered_parts, &c("2 h")); // but does not have it
            },
            ActionError::CardIsNotWithPlayer {
                initiating_player: true,
                player: 0,
                card: c("2 h"),
            },
        );
        test_precondition_as(
            0,     //                                                            player 0
            TRADE, //                                                            trying to trade player 1 for 3h
            |g| {
                vec_remove_item(&mut g.players[1].gathered_parts, &c("3 h")); // but player 1 does not have it
            },
            ActionError::CardIsNotWithPlayer {
                initiating_player: false,
                player: 1,
                card: c("3 h"),
            },
        );
        test_precondition_as(
            0,                               // player 0
            TRADE,                           // trying to trade with player 1
            |g| g.players[1].escaped = true, // but they already escaped
            ActionError::PlayerEscaped { escaped_player: 1 },
        );
        test_precondition_as(
            0,                              // player 0
            "trade 0 offering 2 h for 2 c", // trying to trade with themselves
            |_| (),
            ActionError::SelfTargeting,
        );
        test_precondition_as(
            0,                              // player 0
            "trade 9 offering 2 h for 4 h", // trying to trade with a non-existent player
            |_| (),
            ActionError::InvalidPlayerReference {
                non_existent_player: 9,
            },
        );

        // TradeAccept
        test_precondition_completion_wrong_state(PlayerAction::TradeAccept);
        test_precondition(
            1,                                                   // player 1
            PlayerAction::TradeAccept,                           // trying to finish a trade
            |mut g| g.state = state_trading(1, 0, "3 h", "2 h"), // that they started themselves
            ActionError::NotPlayersTurn { player: 1 },
        );

        // TradeReject
        test_precondition_completion_wrong_state(PlayerAction::TradeReject);
        test_precondition(
            1,                                                   // player 1
            PlayerAction::TradeReject,                           // trying to finish a trade
            |mut g| g.state = state_trading(1, 0, "3 h", "2 h"), // that they started themselves
            ActionError::NotPlayersTurn { player: 1 },
        );

        // Share
        test_precondition_empty_deck(SHARE);
        test_precondition_as(
            0,                               // player 0
            SHARE,                           // trying to share with player 1
            |g| g.players[1].escaped = true, // but they already escaped
            ActionError::PlayerEscaped { escaped_player: 1 },
        );
        test_precondition_as(
            0,              // player 0
            "share with 0", // trying to trade with themselves
            |_| (),
            ActionError::SelfTargeting,
        );
        test_precondition_as(
            0,              // player 0
            "share with 9", // trying to share with a non-existent player
            |_| (),
            ActionError::InvalidPlayerReference {
                non_existent_player: 9,
            },
        );

        // Steal
        test_precondition_as(
            0,     //                                                            player 0
            STEAL, //                                                            trying to steal 3 Hearts from player 1
            |g| {
                vec_remove_item(&mut g.players[1].gathered_parts, &c("3 h")); // but player 1 does not have 3 Hearts
            },
            ActionError::CardIsNotWithPlayer {
                initiating_player: false,
                player: 1,
                card: c("3 h"),
            },
        );
        test_precondition_as(
            0,                               // player 0
            STEAL,                           // trying to steal from player 1
            |g| g.players[1].escaped = true, // but they already escaped
            ActionError::PlayerEscaped { escaped_player: 1 },
        );
        test_precondition_as(
            0,                  // player 0
            "steal 2 h from 0", // trying to steal from themselves
            |_| (),
            ActionError::SelfTargeting,
        );
        test_precondition_as(
            0,                  // player 0
            "share 9 d from 9", // trying to steal from a non-existent player
            |_| (),
            ActionError::InvalidPlayerReference {
                non_existent_player: 9,
            },
        );

        // Scrap
        test_precondition_as(
            0,                             //        player 0
            "scrap 2 h, 2 c, 2 d for q c", //        trying to scrap 3 cards
            |_| (),
            ActionError::WrongNumberOfCardsToScrap {
                //                                   but 4 is needed
                num_specified: 3,
                num_needed: 4,
            },
        );
        test_precondition_as(
            0,                                  //   player 0
            "scrap 2 h, 2 h, 2 c, 2 d for q c", //   trying to scrap 4 cards, out of which only 3 are unique
            |_| (),
            ActionError::WrongNumberOfCardsToScrap {
                //                                   but 4 unique ones are needed
                num_specified: 3,
                num_needed: 4,
            },
        );
        test_precondition_as(
            0,                                  // player 0
            "scrap 2 h, 2 c, 2 d, k d for q c", // trying to scrap some cards, including King of Diamonds
            |g| {
                //                                 but player 0 does not have Kind of Diamonds
                vec_remove_item(&mut g.players[0].gathered_parts, &c("k d"));
            },
            ActionError::CardIsNotWithPlayer {
                initiating_player: true,
                player: 0,
                card: c("k d"),
            },
        );
        test_precondition_as(
            0,     //                                          player 0
            SCRAP, //                                          trying to scrap for Queen of Clubs
            |g| {
                vec_remove_item(&mut g.discard, &c("q c")); // but scrap does not have Queen of Clubs
            },
            ActionError::CardIsNotInDiscard { card: c("q c") },
        );

        // Escape
        test_precondition_as(0, ESCAPE, |_| (), ActionError::EscapeConditionNotSatisfied);
    }

    #[test]
    fn transitions() {
        // Scavenge
        let game_after_scavenge = test_state_transition_as(
            0,                                                 // player 0
            SCAVENGE,                                          // starts a scavenge
            |g| g.draw = Deck::of(cs(&["5 d", "6 d", "7 d"])), // the scavenge unearths these cards
            state_scavenged(0, &["5 d", "6 d", "7 d"]),
        );
        // FinishScavenge
        let game_after_scavenge = test_state_transition_from(
            0,                             //                     then the same player
            action_finish_scavenge("5 d"), //                     finishes the action by choosing 5 d from the loot
            game_after_scavenge,
            GameState::WaitingForPlayerAction { player: 1 }, //   which ends player 0's turn
        );
        assert!(
            game_after_scavenge.players[0] //                     after this player 0
                .gathered_parts
                .contains(&c("5 d")), //                          has the card they chose
            "player 0 did not have 5 d. They had: {:?}",
            game_after_scavenge.players[0].gathered_parts
        );
        assert!(
            game_after_scavenge.discard.contains(&c("6 d"))    // and the discard has the other 2
                && game_after_scavenge.discard.contains(&c("7 d")),
            "the discard did not contain 6 d and 7 d. It was: {:?}",
            game_after_scavenge.discard
        );
        // same with just 1 card in the draw
        let game_after_scavenge = test_state_transition_as(
            0,                                   //              player 0
            SCAVENGE,                            //              starts a scavenge
            |g| g.draw = Deck::of(cs(&["5 d"])), //              the scavenge unearths just one card
            state_scavenged(0, &["5 d"]),
        );
        let game_after_scavenge = test_state_transition_from(
            0,                             //                   then the same player
            action_finish_scavenge("5 d"), //                   finishes the action by choosing that one card
            game_after_scavenge,
            GameState::WaitingForPlayerAction { player: 1 }, // which ends player 0's turn
        );
        assert!(
            game_after_scavenge.players[0] //                   after this player 0
                .gathered_parts
                .contains(&c("5 d")), //                        has the card they chose
            "player 0 did not have 5 d. They had: {:?}",
            game_after_scavenge.players[0].gathered_parts
        );

        // Share
        let game_after_share = test_state_transition_as(
            0,
            SHARE,
            |g| g.draw = Deck::of(cs(&["5 d", "6 d", "7 d"])),
            GameState::WaitingForPlayerAction { player: 1 },
        );
        assert!(
            game_after_share.players[0]
                .gathered_parts
                .contains(&c("5 d"))
                && game_after_share.players[0]
                    .gathered_parts
                    .contains(&c("6 d")),
            "player 0 did not have 5 d and 6 d. They had: {:?}",
            game_after_share.players[0].gathered_parts
        );
        assert!(
            game_after_share.players[1]
                .gathered_parts
                .contains(&c("7 d")),
            "player 1 did not have 7 d. They had: {:?}"
        );
        // same with just 2 cards in the draw
        let game_after_share = test_state_transition_as(
            0,
            SHARE,
            |g| g.draw = Deck::of(cs(&["5 d", "6 d"])),
            GameState::WaitingForPlayerAction { player: 1 },
        );
        assert!(
            game_after_share.players[0]
                .gathered_parts
                .contains(&c("5 d"))
                && game_after_share.players[0]
                    .gathered_parts
                    .contains(&c("6 d")),
            "player 0 did not have 5 d and 6 d. They had: {:?}",
            game_after_share.players[0].gathered_parts
        );
        // trade
        // accept trade
        // reject trade
        // steal
        // scrap
        // escape
        // skip
        // cheat

        unimplemented!();
    }

    #[test]
    fn skip_escaped_out_of_move_players() {
        // test that players who have escaped our out of move are not scheduled for a turn
        unimplemented!();
    }

    #[test]
    fn auto_escape() {
        // test the auto-escape functionality during the game and at the end

        // also test the countdown mechanism
        unimplemented!();
    }

    #[test]
    fn get_results() {
        // test get_results function
        unimplemented!();
    }

    #[test]
    fn test_turn_action_preconditions() {
        for action in turn_actions() {
            test_turn_action_precondition_correct_player(action);
            test_turn_action_precondition_correct_state(action);
        }
        // The following conditions:
        // - player escaped
        // - player is out of moves
        // are covered by `skip_escaped_out_of_move_players`.
    }

    fn test_turn_action_precondition_correct_player(action: &str) {
        test_precondition_as(
            0,
            action,
            |mut g| g.state = GameState::WaitingForPlayerAction { player: 1 },
            ActionError::NotPlayersTurn { player: 0 },
        )
    }

    fn test_turn_action_precondition_correct_state(action: &str) {
        test_precondition_as(
            0,
            action,
            |mut g| {
                g.state = GameState::WaitingForScavengeComplete {
                    player: 0,
                    scavenged_cards: Vec::new(),
                }
            },
            ActionError::NotPlayersTurn { player: 0 },
        )
    }

    fn test_precondition_empty_deck(action: &str) {
        test_precondition_as(
            0,
            action,
            |mut g| g.draw = empty_deck(),
            ActionError::DeckEmpty,
        )
    }

    fn test_precondition_completion_wrong_state(action: PlayerAction) {
        test_precondition(0, action, |_| (), ActionError::NotPlayersTurn { player: 0 });
    }

    fn test_precondition_as<F: Fn(&mut Gameplay)>(
        player: usize,
        action: &str,
        game_setup: F,
        expected_action_error: ActionError,
    ) {
        let action = PlayerAction::try_from(action).unwrap();
        test_precondition(player, action, game_setup, expected_action_error);
    }

    fn test_precondition<F: Fn(&mut Gameplay)>(
        player: usize,
        action: PlayerAction,
        game_setup: F,
        expected_action_error: ActionError,
    ) {
        let mut game = basic_2_player_with_cards();
        game_setup(&mut game);
        assert_eq!(
            game.process_player_action(player, action).unwrap_err(),
            expected_action_error,
        );
    }

    fn test_state_transition_as<F: Fn(&mut Gameplay)>(
        player: usize,
        action: &str,
        game_setup: F,
        expected_state: GameState,
    ) -> Gameplay {
        let action = PlayerAction::try_from(action).unwrap();
        test_state_transition(player, action, game_setup, expected_state)
    }

    fn test_state_transition<F: Fn(&mut Gameplay)>(
        player: usize,
        action: PlayerAction,
        game_setup: F,
        expected_state: GameState,
    ) -> Gameplay {
        let mut game = basic_2_player_with_cards();
        game_setup(&mut game);
        test_state_transition_from(player, action, game, expected_state)
    }

    fn test_state_transition_from(
        player: usize,
        action: PlayerAction,
        mut game: Gameplay,
        expected_state: GameState,
    ) -> Gameplay {
        game.process_player_action(player, action).unwrap();
        let new_state = game.get_state();
        assert_eq!(*new_state, expected_state);
        game
    }

    // turn action constants & helpers for constructions:
    // these all assume the `basic_2_player_with_cards` setup, and that it's player 0's turn.
    static SCAVENGE: &'static str = "scavenge";
    static TRADE: &'static str = "trade 1 offering 2 h for 3 h";
    static STEAL: &'static str = "steal 3 h from 1";
    static SHARE: &'static str = "share with 1";
    static SCRAP: &'static str = "scrap 2 h, 2 c, 2 d, a d for q c";
    static ESCAPE: &'static str = "escape";
    static SKIP: &'static str = "skip";
    static CHEAT_GET_CARDS: &'static str = "conjure 10 d";
    fn turn_actions() -> Vec<&'static str> {
        vec![
            SCAVENGE,
            TRADE,
            STEAL,
            SHARE,
            SCRAP,
            ESCAPE,
            SKIP,
            CHEAT_GET_CARDS,
        ]
    }

    fn action_finish_scavenge(card: &str) -> PlayerAction {
        PlayerAction::FinishScavenge { card: c(card) }
    }

    // primitives for constructing Gameplay objects

    fn basic_2_player_with_cards() -> Gameplay {
        basic_game(&vec![
            vec!["2 h", "2 c", "2 d", "a d"],
            vec!["3 h", "3 c", "3 d", "a c"],
        ])
    }

    fn basic_game(card_strs_per_player: &Vec<Vec<&str>>) -> Gameplay {
        Gameplay {
            players: players_with_cards(card_strs_per_player),
            draw: Deck::shuffle(),
            discard: vec![c("q c")],
            state: GameState::WaitingForPlayerAction { player: 0 },
        }
    }

    fn empty_deck() -> Deck {
        let mut deck = Deck::shuffle();
        deck.remove_top(52);
        deck
    }

    fn players_with_cards(card_strs_per_player: &Vec<Vec<&str>>) -> Vec<Player> {
        let mut players = Vec::new();
        for card_strs in card_strs_per_player {
            let mut cards = Vec::new();
            for card_str in card_strs {
                cards.push(c(*card_str));
            }
            players.push(Player {
                missing_part: c("Ace of Spades"),
                gathered_parts: cards,
                escaped: false,
                moves_left: None,
            });
        }
        players
    }

    fn state_scavenged(player: usize, card_strs: &[&str]) -> GameState {
        GameState::WaitingForScavengeComplete {
            player: player,
            scavenged_cards: cs(card_strs),
        }
    }

    fn state_trading(
        initiating_player: usize,
        trading_with_player: usize,
        offering_str: &str,
        in_exchange_str: &str,
    ) -> GameState {
        GameState::WaitingForTradeConfirmation {
            initiating_player,
            trading_with_player,
            offer: TradeOffer {
                offered: c(offering_str),
                in_exchange: c(in_exchange_str),
            },
        }
    }

    fn c(card_str: &str) -> Card {
        Card::try_from(card_str).unwrap()
    }

    fn cs(card_strs: &[&str]) -> Vec<Card> {
        let mut cards = Vec::new();
        for card_str in card_strs {
            cards.push(c(*card_str));
        }
        cards
    }
}
