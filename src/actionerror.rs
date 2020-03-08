//! Defines the `ActionError` type, which is the set of possible error messages that can happen
//! when processing an action from a player.

use crate::cards::*;
#[cfg(test)]
use serde::Deserialize;
use serde::Serialize;
use std::fmt;

/// If the action specified in [`process_player_action`](../gameplay/struct.Gameplay.html#method.process_player_action)
/// cannot be completed, this type describes why. These roughly reflect the possible precondition failures described in
/// the [`PlayerAction`](../playeraction/enum.PlayerAction.html) instance docs.
///
/// In general these can be prevented by providing a user experience that prevents the user from attempting an invalid
/// action. If an action error is received the player must try to give a new action, since the game state had not
/// advanced.
#[derive(Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(Deserialize))]
pub enum ActionError {
    /// There are no cards in the draw pile (for example when trying to use
    /// [`Scavenge`](../playeraction/enum.PlayerAction.html#variant.Scavenge)).
    DeckEmpty,

    /// The specified other player (e.g when trying to use
    /// [`Share`](../playeraction/enum.PlayerAction.html#variant.Share) or
    /// [`Trade`](../playeraction/enum.PlayerAction.html#variant.Trade), etc) had already escaped.
    PlayerEscaped {
        /// Who was the other player who's already escaped.
        escaped_player: usize,
    },

    /// When using [`Scrap`](../playeraction/enum.PlayerAction.html#variant.Scrap), the player tried to pick a card
    /// from the discard pile that is not actually there.
    CardIsNotInDiscard {
        /// The card that the player tried to pick from discard pile, but it isn't actually there.
        card: Card,
    },

    /// When using [`Scrap`](../playeraction/enum.PlayerAction.html#variant.Scrap), the player specified `num_specified`
    /// cards, but exactly `num_needed` is needed to complete the scrap.
    WrongNumberOfCardsToScrap {
        /// How many cards the player tried to scrap.
        num_specified: u32,

        /// Exactly how many cards there must be in a scrap action.
        num_needed: u32,
    },

    /// When trying to use an action that requires a specific card to be in the possession of a player (e.g.
    /// [`Trade`](../playeraction/enum.PlayerAction.html#variant.Trade)) the card was actually not with that player.
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

    /// The player tried to [`Escape`](../playeraction/enum.PlayerAction.html#variant.Escape), but the escape condition
    /// was not satisfied. (The player did not have all 4 suits of the same rank.)
    EscapeConditionNotSatisfied,

    /// A player treid to make an action out of their turn, or tried to use an action not appropriate for the current
    /// game state (e.g. tried to accept a trade when the game was not expecting a trade confirmation, or was trying
    /// to scavenge when the game was in expecting a trade confirmation).
    NotPlayersTurn {
        /// The player who tried to make the invalid move.
        player: usize,
    },

    /// When trying to [`FinishScavenge`](../playeraction/enum.PlayerAction.html#variant.FinishScavenge), the card
    /// specified in the action was not actually one of the cards scavenged.
    CardWasNotScavenged {
        /// The card that the player wanted to keep, but wasn't actually in the scavenged cards.
        card: Card,
    },

    /// While trying to do an action involving another player (e.g. stealing, sharing) the other player specified
    /// was the same player as the one making the move.
    SelfTargeting,

    /// While trying to do an action involving another player (e.g. stealing, sharing) the other player specified
    /// was not an actual player in the game.
    InvalidPlayerReference { non_existent_player: usize },
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
            SelfTargeting => write!(f, "action not possible on self, pick another player"),
            InvalidPlayerReference {
                non_existent_player,
            } => write!(f, "player {} is not a valid player", non_existent_player),
        }
    }
}
