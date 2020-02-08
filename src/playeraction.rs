//! Defines the `PlayerAction` type, which represents the actions available during the game.

use crate::cards::*;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

/// The various actions that a player can make during their turn, and also some actions that can be used to complete
/// a multi-part action in a turn.
///
/// Actions can be categorized roughly into 2 types:
/// - a _turn action_ can be taken at the beginnig of a player's turn.
/// - a _completing action_ can be taken to complete a turn action requiring a multi-step intreaction, such as
///   scavenging (pick which card to keep) or trading (the offer must be accepted or rejected).
///
/// Unless otherwise indicated for a variant, it's a turn action. For completing actions the variant-level documentation
/// will indicate for which [`GameState`](../gameplay/enum.GameState.html) they are valid.
///
/// The actions can be used with
/// [`Gameplay::process_player_action`](../gameplay/struct.Gameplay.html#method.process_player_action).
/// In general only the player whose turn it is according to the game state can make actions. There are some exceptions,
/// this is indicated on the action-level documentation.
///
/// As a convenience, turn actions can be parsed from a `&str` using `try_from` (see the `std::convert::TryFrom`). See
/// examples for each action of a valid string that can be parsed into that action. If the parsing fails, a human
/// readable (English) error message is returned that should explain what the problem is, and what would be a valid
/// version of the action. This can be shown on the user interface.
#[derive(Debug, PartialEq, Deserialize, Clone)]
pub enum PlayerAction {
    /// Pick one of the top 3 cards from the draw pile.
    ///
    /// Preconditions:
    /// - the draw pile must not be empty.
    ///
    /// Effect: the game state changes to
    /// [`WaitingForScavengeComplete`](../gameplay/enum.GameState.html#variant.WaitingForScavengeComplete), which
    /// indicates which cards have been drawn from the draw pile. To complete the turn, use
    /// [`FinishScavenge`](#variant.FinishScavenge).
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::playeraction::PlayerAction;
    /// # use std::convert::TryFrom;
    /// # use PlayerAction::*;
    /// assert_eq!(PlayerAction::try_from("scavenge").unwrap(), Scavenge);
    /// ```
    Scavenge,

    /// Pick which scavenged cards to keep.
    ///
    /// Preconditions:
    /// - the game state must be
    /// [`WaitingForScavengeComplete`](../gameplay/enum.GameState.html#variant.WaitingForScavengeComplete)
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
    /// # use missingparts::playeraction::PlayerAction;
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
    /// [`WaitingForTradeConfirmation`](../gameplay/enum.GameState.html#variant.WaitingForTradeConfirmation), which
    /// indicates who needs to approve the transaction, and what was the offer made. The turn can be completed by
    /// [`TradeAccept`](#variant.TradeAccept) or [`TradeReject`](#variant.TradeReject).
    ///
    /// Parsing example:
    /// ```
    /// # use missingparts::playeraction::*;
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
    /// - the game state is
    /// [`WaitingForTradeConfirmation`](../gameplay/enum.GameState.html#variant.WaitingForTradeConfirmation),
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
    /// - the game state is
    /// [`WaitingForTradeConfirmation`](../gameplay/enum.GameState.html#variant.WaitingForTradeConfirmation),
    /// and the action is made by the player indicated in the state.
    ///
    /// Effect: the game state goes back to
    /// [`WaitingForPlayerAction`](../gameplay/enum.GameState.html#variant.WaitingForPlayerAction). So the player who
    /// initated the trade that got rejected gets to pick another action.
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
    /// # use missingparts::playeraction::PlayerAction;
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
    /// # use missingparts::playeraction::PlayerAction;
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
    /// # use missingparts::playeraction::PlayerAction;
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
    /// # use missingparts::playeraction::PlayerAction;
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
    /// # use missingparts::playeraction::PlayerAction;
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

/// The parameters of a trade offer in a `Trade` action during the game.
///
/// For example, if _player A_ has cards `[4 of Clubs, 2 of Hearts]`, and _player B_ has `[6 of Diamonds, Ace of
/// Spades]`, and it's _player A_'s turn, a valid trade offer _player A_ could make to _player B_ would be `offered=4
/// of Clubs, in_exchange=Ace of Spades`.
#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub struct TradeOffer {
    /// This is the card that the initiator of the play is willing to give up.
    pub offered: Card,

    /// This is the card required from the other party in the trade.
    pub in_exchange: Card,
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
/// # use missingparts::playeraction::*;
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
