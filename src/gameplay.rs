//! Types to play a game of Missing Parts.
//!
//! The [`Gameplay`](struct.Gameplay.html) type is the main way to interact with the game.

use crate::cards::*;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;

/// The parameters of a trade offer in a `Trade` action during the game.
///
/// For example, if _player A_ has cards `[4 of Clubs, 2 of Hearts]`, and _player B_ has `[6 of Diamonds, Ace of
/// Spades]`, and it's _player A_'s turn, a valid trade offer _player A_ could make to _player B_ would be `offered=4
/// of Clubs, in_exchange=Ace of Spades`.
#[derive(Debug, PartialEq)]
pub struct TradeOffer {
    /// This is the card that the initiator of the play is willing to give up.
    pub offered: Card,

    // This is the card required from the other party in the trade.
    pub in_exchange: Card,
}

/// The various actions that a player can make during their turn, and also some actions that can be used to complete
/// a multi-part action in a turn.
///
/// Actions can be categorized roughly into 2 types:
/// - a _turn action_ can be taken at the beginnig of a player's turn.
/// - a _completing action_ can be taken to complete a turn action requiring a multi-step intreaction, such as
///   scavenging (pick which card to keep) or trading (the offer must be accepted or rejected).
///
/// Unless otherwise indicated for a variant, it's a turn action. For completing actions the variant-level documentation
/// will indicate for which [`GameState`](enum.GameState.html) they are valid.
///
/// The actions can be used with [`Gameplay::process_player_action`](struct.Gameplay.html#method.process_player_action).
/// In general only the player whose turn it is according to the game state can make actions. There are some exceptions,
/// this is indicated on the action-level documentation.
///
/// As a convenience, turn actions can be parsed from a `&str` using `try_from` (see the `std::convert::TryFrom`). See
/// examples for each action of a valid string that can be parsed into that action. If the parsing fails, a human
/// readable (English) error message is returned that should explain what the problem is, and what would be a valid
/// version of the action. This can be shown on the user interface.
#[derive(Debug, PartialEq)]
pub enum PlayerAction {
    /// Pick one of the top 3 cards from the draw pile.
    ///
    /// Preconditions:
    /// - the draw pile must not be empty.
    ///
    /// Effect: the game state changes to
    /// [`WaitingForScavengeComplete`](enum.GameState.html#variant.WaitingForScavengeComplete), which indicates which
    /// cards have been drawn from the draw pile. To complete the turn, use [`FinishScavenge`](#variant.FinishScavenge).
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::gameplay::PlayerAction;
    /// # use std::convert::TryFrom;
    /// # use PlayerAction::*;
    /// assert_eq!(PlayerAction::try_from("scavenge").unwrap(), Scavenge);
    /// ```
    Scavenge,

    /// Pick which scavenged cards to keep.
    ///
    /// Preconditions:
    /// - the game state must be [`WaitingForScavengeComplete`](enum.GameState.html#variant.WaitingForScavengeComplete)
    /// and the `card` specified in this action must be one of the cards in the state.
    ///
    /// Effect: the `card` specified in this action will move to the player's hand, the other cards from the game state,
    /// if any, move to the discard pile. The turn of the player completes.
    FinishScavenge {
        /// The card that the player is picking to keep.
        card: Card,
    },

    /// Get the top 2 cards from the draw pile, and another player gets one card.
    ///
    /// Preconditions:
    /// - the draw pile must not be empty
    /// - the specified other player must not have escaped
    ///
    /// Effect: the top 2 cards, (or 1, if the draw pile only has 1) will move to the player making the turn, and 1 card
    /// (if the draw pile is still not empty) will move to `with_player`. The turn of the player making the action
    /// completes.
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::gameplay::PlayerAction;
    /// # use std::convert::TryFrom;
    /// # use PlayerAction::*;
    /// assert_eq!(PlayerAction::try_from("share 1").unwrap(), Share { with_player: 1 });
    /// ```
    Share {
        /// The other player, who should receive one card.
        with_player: usize,
    },

    /// Swap cards with another player, depending on mutual agreement.
    ///
    /// Preconditions:
    /// - both players must have the cards specified in the `offer`.
    /// - the specified other player must not have escaped
    ///
    /// Effect: the game state goes to
    /// [`WaitingForTradeConfirmation`](enum.GameState.html#variant.WaitingForTradeConfirmation), which indicates who
    /// needs to approve the transaction, and what was the offer made. The turn can be completed by
    /// [`TradeAccept`](#variant.TradeAccept) or [`TradeReject`](#variant.TradeReject).
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::gameplay::*;
    /// # use missingparts::cards::*;
    /// # use std::convert::TryFrom;
    /// # use PlayerAction::*;
    /// assert_eq!(
    ///     PlayerAction::try_from("trade 1 offering King of Clubs for Ace of Spades").unwrap(),
    ///     Trade {
    ///         with_player: 1,
    ///         offer: TradeOffer {
    ///             offered: Card::try_from("King of Clubs").unwrap(),
    ///             in_exchange: Card::try_from("Ace of Spades").unwrap(),
    ///         },
    ///     },
    /// );
    /// ```
    Trade {
        /// The other player to trade with.
        with_player: usize,

        /// The offer made in the trade.
        offer: TradeOffer,
    },

    /// Accept the trade offer
    ///
    /// Preconditions:
    /// - the game state is [`WaitingForTradeConfirmation`](enum.GameState.html#variant.WaitingForTradeConfirmation),
    /// and the action is made by the player indicated in the state.
    ///
    /// Effect:
    /// The player who initiated the trade gets the exchange card specified in the offer, and the player who is
    /// accepting the trade gets the offered card from the initiating player. (Both players lose the other card, i.e no
    /// new card enters the players' collective hand.) The initiating player's turn completes.
    TradeAccept,

    /// Reject the trade offer
    ///
    ///
    /// Preconditions:
    /// - the game state is [`WaitingForTradeConfirmation`](enum.GameState.html#variant.WaitingForTradeConfirmation),
    /// and the action is made by the player indicated in the state.
    ///
    /// Effect: the game state goes back to
    /// [`WaitingForPlayerAction`](enum.GameState.html#variant.WaitingForPlayerAction). So the player who initated the
    /// trade that got rejected gets to pick another action.
    TradeReject,

    /// Steal a card from another player
    ///
    /// Preconditions:
    /// - the specified other player has the `card`.
    /// - the specified other player must not have escaped
    ///
    /// Effect: the specified other player loses `card`, and the player making this action receives it. The turn of the
    /// player making the action completes.
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::gameplay::PlayerAction;
    /// # use missingparts::cards::*;
    /// # use std::convert::TryFrom;
    /// # use PlayerAction::*;
    /// assert_eq!(
    ///     PlayerAction::try_from("steal Ace of Spades from 1").unwrap(),
    ///     Steal {
    ///         from_player: 1,
    ///         card: Card::try_from("Ace of Spades").unwrap(),
    ///     },
    /// );
    /// ```
    Steal {
        /// The card to be stolen from the other player.
        card: Card,

        /// The player to steal from.
        from_player: usize,
    },

    /// Discard cards in exchange for an item from the discard pile
    ///
    /// Preconditions:
    /// - the player is in possession of the cards specified in `player_cards`.
    /// - the discard pile contains `for_discard_card`.
    ///
    /// Effect: the player gets `for_discard_card` (and it gets removed from the discard pile). The player loses the
    /// cards specified in `player_cards` (and those cards end up in the discard pile). The player's turn completes.
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::gameplay::PlayerAction;
    /// # use missingparts::cards::*;
    /// # use std::convert::TryFrom;
    /// # use PlayerAction::*;
    /// assert_eq!(
    ///     // 'h' is short for 'of Hearts'
    ///     PlayerAction::try_from("scrap 2 h, 3 h, 4 h, 5 h for Ace of Spades").unwrap(),
    ///     Scrap {
    ///         player_cards: vec![
    ///             Card::try_from("2 h").unwrap(),
    ///             Card::try_from("3 h").unwrap(),
    ///             Card::try_from("4 h").unwrap(),
    ///             Card::try_from("5 h").unwrap(),
    ///         ],
    ///         for_discard_card: Card::try_from("Ace of Spades").unwrap(),
    ///     },
    /// );
    /// ```
    Scrap {
        /// The cards that the player is going to discard.
        player_cards: Vec<Card>,

        /// The card that the player will get from discard.
        for_discard_card: Card,
    },

    /// Escape from the game
    ///
    /// Preconditions:
    /// - the player has satisfied the escape condition (has all 4 suits of the same rank, e.g. `[2 H, 2 C, 2 D, 2 S]`)
    ///
    /// Effect: the player is moved to 'escaped' status. The player will not be able to make any moves or be eligible
    /// for `Trade`, `Steal`, `Share` actions from other players. The player's turn completes.
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::gameplay::PlayerAction;
    /// # use std::convert::TryFrom;
    /// # use PlayerAction::*;
    /// assert_eq!(PlayerAction::try_from("escape").unwrap(), Escape);
    /// ```
    Escape,

    /// Skip a turn
    ///
    /// Effect: the player's turn completes.
    ///
    /// Note: this action is provided so that the game cannot get stuck. (Example: only one player has not escaped,
    /// draw pile is empty, and the player does not have enough cards to scrap, and they cannot escape yet.) I don't
    /// think it ever provides strategic advantage to skip a turn (unless assisting another player in a meta-game).
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::gameplay::PlayerAction;
    /// # use std::convert::TryFrom;
    /// # use PlayerAction::*;
    /// assert_eq!(PlayerAction::try_from("skip").unwrap(), Skip);
    /// ```
    Skip,

    /// Cheat, and gets some cards from outside the main game resources
    ///
    /// Effect: the player receives the cards specified in `cards`.
    ///
    /// Note: tis action is provided for gameplay testing. Game runners should ensure that this action is not used
    /// by non-test players. After using this action the player gets another action. Using this action introduces
    /// duplicate cards into the game.
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::gameplay::PlayerAction;
    /// # use missingparts::cards::*;
    /// # use std::convert::TryFrom;
    /// # use PlayerAction::*;
    /// assert_eq!(
    ///     // 'h' is short for 'of Hearts', 'a d' is short for 'Ace of Diamonds'
    ///     PlayerAction::try_from("conjure 2 h, a d").unwrap(),
    ///     CheatGetCards {
    ///         cards: vec![
    ///             Card::try_from("2 h").unwrap(),
    ///             Card::try_from("Ace of Diamonds").unwrap(),
    ///         ],
    ///     },
    /// );
    /// ```
    CheatGetCards { cards: Vec<Card> },
}

fn first_number(s: &str) -> Option<usize> {
    s.split_whitespace()
        .map(|ss| ss.parse())
        .filter(|pr| pr.is_ok())
        .map(|pr| pr.expect("we should have filtered errors already"))
        .next()
}

/// Split `s` by the separators in `separators`, in the order they are defined.
///
/// # Examples
///
/// ```
/// # use missingparts::gameplay::*;
/// let parts = split_in_ord("trade 0 offering 2 of Hearts for Ace of Spades", &["offering", "for"]);
/// assert_eq!(parts, ["trade 0 ", " 2 of Hearts ", " Ace of Spades"]);
/// ```
pub fn split_in_ord<'a, 'b>(s: &'a str, separators: &'b [&str]) -> Vec<&'a str> {
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
    /// Returns a short summary of the actions that the player can make. Not context aware, i.e. it will mention actions
    /// that may not be available to the player in the current gameplay situation.
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
#[derive(PartialEq)]
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

/// If the action specified in [`process_player_action`](struct.Gameplay.html#method.process_player_action) cannot be
/// completed, this type describes why. These roughly reflect the possible precondition failures described in the
/// [`PlayerAction`](enum.PlayerAction.html) instance docs.
///
/// In general these can be prevented by providing a user experience that prevents the user from attempting an invalid
/// action. If an action error is received the player must try to give a new action, since the game state had not
/// advanced.
#[derive(Debug)]
pub enum ActionError {
    /// There are no cards in the draw pile (for example when trying to use
    /// [`Scavenge`](enum.PlayerAction.html#variant.Scavenge)).
    DeckEmpty,

    /// The specified other player (e.g when trying to use [`Share`](enum.PlayerAction.html#variant.Share) or
    /// [`Trade`](enum.PlayerAction.html#variant.Trade), etc) had already escaped.
    PlayerEscaped {
        /// Who was the other player who's already escaped.
        escaped_player: usize,
    },

    /// When using [`Scrap`](enum.PlayerAction.html#variant.Scrap), the player tried to pick a card from the discard pile that is not actually there.
    CardIsNotInDiscard {
        /// The card that the player tried to pick from discard pile, but it isn't actually there.
        card: Card,
    },

    /// When using [`Scrap`](enum.PlayerAction.html#variant.Scrap), the player specified `num_specified` cards, but
    /// exactly `num_needed` is needed to complete the scrap.
    WrongNumberOfCardsToScrap {
        /// How many cards the player tried to scrap.
        num_specified: u32,

        /// Exactly how many cards there must be in a scrap action.
        num_needed: u32,
    },

    /// When trying to use an action that requires a specific card to be in the possession of a player (e.g.
    /// [`Trade`](enum.PlayerAction.html#variant.Trade)) the card was actually not with that player.
    CardIsNotWithPlayer {
        /// Whether the player who did not have the required card was the same player as the one initiating the action.
        /// (This is useful to provide a more natural error message, by being able to say "you" instead of "player
        /// XYZ").
        initiating_player: bool,

        /// The player who did not have the required card.
        player: usize,

        /// The card that was supposed to be with `player`, but was not.
        card: Card,
    },

    /// The player tried to [`Escape`](enum.PlayerAction.html#variant.Escape), but the escape condition was not
    /// satisfied. (The player did not have all 4 suits of the same rank.)
    EscapeConditionNotSatisfied,

    /// A player treid to make an action out of their turn, or tried to use an action not appropriate for the current
    /// game state (e.g. tried to accept a trade when the game was not expecting a trade confirmation, or was trying
    /// to scavenge when the game was in expecting a trade confirmation).
    NotPlayersTurn {
        /// The player who tried to make the invalid move.
        player: usize,
    },

    /// When trying to [`FinishScavenge`](enum.PlayerAction.html#variant.FinishScavenge), the card specified in the
    /// action was not actually one of the cards scavenged.
    CardWasNotScavenged {
        /// The card that the player wanted to keep, but wasn't actually in the scavenged cards.
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
                self.precondition_waiting_for_player_action(player_index)?;
                // question for Andy: should we re-shuffle the discard pile into draw here
                self.precondition_draw_nonempty()?;

                // TODO bug: ensure that the player does not trade with themselves.
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
                // TODO: bug: must verify that all cards are actually different.
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
}
