use crate::actionerror::ActionError;
use crate::cards::Card;
use crate::gameplay::{GameDescription, Gameplay};
use crate::lobby::GameCreator;
use crate::playeraction::PlayerAction;
use crate::server_core_types::{GameId, Token, TokenVerifier};

use chashmap::CHashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A single game that's managed by the `GameManager`.
pub struct ManagedGame {
    gameplay: Gameplay,
    secret_cards_per_player: Vec<Card>,
    tokens: Vec<Token>,
}

impl ManagedGame {
    pub fn new(tokens: Vec<Token>) -> ManagedGame {
        let num_players = tokens.len();
        let (gameplay, secret_cards_per_player) = Gameplay::init(num_players);
        ManagedGame {
            gameplay,
            secret_cards_per_player,
            tokens,
        }
    }

    pub fn describe(&self) -> GameDescription {
        self.gameplay.describe()
    }

    pub fn get_private_card(&self, player_id: usize) -> Card {
        self.secret_cards_per_player[player_id]
    }

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
/// To start a new game under the manager, use `new_game`. After that use `with_game` for read-only
/// operations on a game, or `with_mut_game` for read-write operations on a game.
pub struct GameManager {
    games: CHashMap<GameId, ManagedGame>,
    next_game_index: AtomicUsize,
}

impl GameCreator for GameManager {
    /// Starts a new game, and returns the ID of the game that can be used with `with_game` and `with_mut_game`.
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
    pub fn new() -> GameManager {
        GameManager {
            games: CHashMap::new(),
            next_game_index: AtomicUsize::new(0),
        }
    }

    pub fn with_game<F, T>(&self, game_id: GameId, f: F) -> T
    where
        F: Fn(&ManagedGame) -> T,
    {
        f(&self.games.get(&game_id).unwrap())
    }

    pub fn with_mut_game<F, T>(&self, game_id: GameId, f: F) -> T
    where
        F: Fn(&mut ManagedGame) -> T,
    {
        f(&mut self.games.get_mut(&game_id).unwrap())
    }
}
