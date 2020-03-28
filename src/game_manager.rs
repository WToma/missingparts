//! Types for managing games in a concurrent server environment.
//!
//! Defines the [`GameManager`](struct.GameManager.html) type which can be used to create, and operate on games.

use crate::actionerror::ActionError;
use crate::cards::Card;
use crate::gameplay::{GameDescription, Gameplay};
use crate::lobby::GameCreator;
use crate::playeraction::PlayerAction;
use crate::server_core_types::{GameId, Token, TokenVerifier};

use chashmap::CHashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A single game that's managed by the [`GameManager`](struct.GameManager.html). To obtain an instance, use
/// [`GameManager.with_game`](struct.GameManager.html#method.with_game) or
/// [`GameManager.with_mut_game`](struct.GameManager.html#method.with_mut_game).
pub struct ManagedGame {
    gameplay: Gameplay,
    secret_cards_per_player: Vec<Card>,
    tokens: Vec<Token>,
}

impl ManagedGame {
    fn new(tokens: Vec<Token>) -> ManagedGame {
        let num_players = tokens.len();
        let (gameplay, secret_cards_per_player) = Gameplay::init(num_players);
        ManagedGame {
            gameplay,
            secret_cards_per_player,
            tokens,
        }
    }

    /// Returns a safely sharable description of the game. This is independent of the actual game state, so feel free
    /// to mutate it. It can also be shown to all players or spectators.
    pub fn describe(&self) -> GameDescription {
        self.gameplay.describe()
    }

    /// Returns the "missing part" of the player identified by `player_id`. This should only be shown to the same
    /// player, since this is a secret.
    pub fn get_private_card(&self, player_id: usize) -> Card {
        self.secret_cards_per_player[player_id]
    }

    /// Process an action from a player
    ///
    /// This method can be called any time, if an action is not appropriate at the time (for example because it is not
    /// the specified player's turn, or the action is not appropriate for the game state) an error will be returned.
    ///
    /// See [`Gameplay.process_player_action`](../gameplay/struct.Gameplay.html#method.process_player_action) for more
    /// info.
    pub fn make_move(
        &mut self,
        player_id: usize,
        player_action: PlayerAction,
    ) -> Result<(), ActionError> {
        self.gameplay
            .process_player_action(player_id, player_action)
    }
}

impl TokenVerifier<usize> for ManagedGame {
    /// Verifies that the given `player_id` in this game has the specified `token`.
    fn verify(&self, player_id: &usize, token: &Token) -> bool {
        let player_id = *player_id;
        player_id < self.tokens.len() && self.tokens[player_id] == *token
    }
}

/// Manages games in the server. This is the primary way games should be interacted with.
/// Safe for concurrent access.
///
/// Use [`new`](#method.new) to create an instance.
///
/// To start a new game under the manager, use [`GameCreator`](#impl-GameCreator).`new_game`. After that use
/// [`with_game`](#method.with_game) for read-only queries on a game, or
/// [`with_mut_game`](#method.with_mut_game) for read-write operations on a game.
pub struct GameManager {
    games: CHashMap<GameId, ManagedGame>,
    next_game_index: AtomicUsize,
}

impl GameCreator for GameManager {
    /// Starts a new game, and returns the ID of the game. In `GameManager` this ID can be used with `with_game` and
    /// `with_mut_game` to perform queries and operations on the game.
    ///
    /// The game will save the player tokens, and the tokens can be used to verify the players making moves on the game.
    fn new_game(&self, player_tokens: Vec<Token>) -> GameId {
        let next_index = GameId(self.next_game_index.fetch_add(1, Ordering::SeqCst));
        self.games
            .insert(next_index, ManagedGame::new(player_tokens));
        next_index
    }
}

impl GameManager {
    /// Create a new GameManager instance.
    pub fn new() -> GameManager {
        GameManager {
            games: CHashMap::new(),
            next_game_index: AtomicUsize::new(0),
        }
    }

    /// Returns `f(game)`, where `game` is the game specified by `game_id` (or `None` if such game does not exist).
    ///
    /// This can be used to run read-only queries on games. For read-write queries use
    /// [`with_mut_game`](#method.with_mut_game). `with_game` allows multiple readers to access the game concurrently,
    /// however while a reader is accessing a game, no writer can access it. Therefore all queries should be quick and
    /// non-blocking. If the game is currently being written, `with_game` will block before `f` can start executing.
    ///
    /// # Example
    ///
    /// To query the number of players in the game, one could do something like this:
    /// ```
    /// # use missingparts::game_manager::*;
    /// # use missingparts::lobby::GameCreator;
    /// # use missingparts::server_core_types::Token;
    ///
    /// // create a game manage and a new game for 2 players
    /// let game_manager = GameManager::new();
    /// let game_id = game_manager.new_game(vec![Token::random(), Token::random()]);
    ///
    /// // query the number of players
    /// let num_players = game_manager.with_game(game_id, |game| game.describe().players.len());
    /// assert_eq!(num_players, Some(2));
    /// ```
    pub fn with_game<F, T>(&self, game_id: GameId, f: F) -> Option<T>
    where
        F: FnOnce(&ManagedGame) -> T,
    {
        let game = self.games.get(&game_id);
        game.map(|g| f(&g))
    }

    /// Returns `f(game)`, where `game` is the game specified by `game_id` (or `None` if such game does not exist).
    ///
    /// This can be used to do read-write operations on games. If the game is currenting being written or read,
    /// `with_mut_game` will block before invoking `f`. While `f` is executing, all other calls to `with_game` or
    /// `with_mut_game` with the same `game_id` will block.
    ///
    /// For an example, see the example of `with_game`.
    pub fn with_mut_game<F, T>(&self, game_id: GameId, f: F) -> Option<T>
    where
        F: Fn(&mut ManagedGame) -> T,
    {
        let game = self.games.get_mut(&game_id);
        game.map(|mut g| f(&mut g))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lobby::GameCreator;

    #[test]
    fn test_concurrent_read_access() {
        let game_manager = GameManager::new();
        let game_id = game_manager.new_game(vec![Token::random(), Token::random()]);

        // this test will deadlock if `with_game` doesn't allow concurrent access to the same game

        game_manager.with_game(game_id, |_| {
            game_manager.with_game(game_id, |_| {});
        });
    }

    #[test]
    fn test_concurrent_read_write_access_for_different_games() {
        let game_manager = GameManager::new();
        let game_id1 = game_manager.new_game(vec![Token::random(), Token::random()]);
        let game_id2 = game_manager.new_game(vec![Token::random(), Token::random()]);

        // this test will deadlock if `GameManager` doesn't allow concurrent `with_game` and `with_mut_game` access
        // to 2 different games

        game_manager.with_mut_game(game_id1, |_| {
            game_manager.with_game(game_id2, |_| {});
        });

        game_manager.with_game(game_id1, |_| {
            game_manager.with_mut_game(game_id2, |_| {});
        });
    }

    // #[test]
    // fn test_blocking_write_access() {
    //     // this test tests that with_game and with_mut_game are not concurrent for the same game
    //     unimplemented!() // not sure how test this reliably
    // }

    // token verification tests are in the handlers.rs tests
}
