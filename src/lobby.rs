//! Provides types and methods to manage players waiting for games.
//!
//! The [`Lobby`](struct.Lobby.html) type is the main way to manage the players.
//!
//! In order to start games, the [`GameCreator`](trait.GameCreator.html) must be implemented.

use crate::range_map::RangeMap;
use crate::server_core_types::{GameId, Token};
use std::cmp::min;
use std::collections::HashMap;
use std::sync::RwLock;

/// An interface of something that can create a game. See the [`new_game`](#method.new_game) method.
pub trait GameCreator {
    /// Creates a new game with the specified number of players, and returns the ID of the game
    /// that was created.
    fn new_game(&self, num_players: usize) -> GameId;
}

/// The ID of a player in the lobby.
///
/// Do not create instances directly, instead use the lobby's [`add_player`](struct.Lobby.html#method.add_player)
/// method to get an instance.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct PlayerIdInLobby(pub usize);

/// Represents a player's assigment to a game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerAssignedToGame {
    /// The ID of the game that the player is assigned to.
    pub game_id: GameId,

    /// The player ID of the player in the game.
    ///
    /// Note that this ID is not necessarily the same (in fact, is usually different) from the ID in the lobby which is
    /// represented by the [`PlayerIdInLobby`](struct.PlayerIdInLobby.html) type.
    pub player_id_in_game: usize,
}

/// Manages the players who are waiting to join a game. Safe to access concurrently.
///
/// - use [`new`](#method.new) to create a new instance.
/// - use [`add_player`](#method.add_player) to have a new player join the lobby.
/// - then use [`get_player_game`](#method.get_player_game) to check whether the player has been assigned to a game.
/// - or [`abandon_lobby`](#method.abandon_lobby) to have the player leave the lobby before joining a game.
/// - to assign players to games, use [`start_game`](#method.start_game).
pub struct Lobby {
    internal: RwLock<NonThreadSafeLobby>,
}

#[derive(Clone, Copy)]
struct PlayerWaitingForGame {
    player_id_in_lobby: PlayerIdInLobby,
    game_size_preference: GameSizePreference,
}

enum LobbyPlayer {
    WaitingForGame(PlayerWaitingForGame),
    InGame(PlayerAssignedToGame),
}

impl Lobby {
    /// Creates a new intance of a lobby. You can have multiple instances of the lobby, but for optimal player
    /// assignment you should only have one and share that. However if player assignments get too slow it may make sense
    /// to create multiple instances.
    pub fn new() -> Lobby {
        Lobby {
            internal: RwLock::new(NonThreadSafeLobby::new()),
        }
    }

    /// Adds a player to the lobby with the given preference for game size (both `min_game_size` and `max_game_size` are
    /// inclusive). The ID of the player is returned, this can be used with
    /// [`get_player_game`](#method.get_player_game) to query if the player has a game yet. Returns an `Err` if the
    /// given game size preferences are invalid.
    pub fn add_player(
        &self,
        min_game_size: usize,
        max_game_size: usize,
    ) -> Result<(PlayerIdInLobby, Token), ()> {
        self.internal
            .write()
            .unwrap()
            .add_player(min_game_size, max_game_size)
    }

    /// Signals to the lobby that the player no longer wants to play a game. The `player_id` should be the one that was
    /// returned by [`add_player`](#method.add_player).
    ///
    /// If the player is already in a game, `Err` is returned, otherwise `Ok`.
    pub fn abandon_lobby(&self, player_id: PlayerIdInLobby) -> Result<(), ()> {
        self.internal.write().unwrap().abandon_lobby(player_id)
    }

    /// Returns the game ID for the given player, if one has been assigned. The `player_id` should be the one that was
    /// returned by [`add_player`](#method.add_player). Returns `None` if the player does not have a game assigned, or
    /// the given `player_id` is not in the lobby (for example they have abandoned the lobby).
    pub fn get_player_game(&self, player_id: PlayerIdInLobby) -> Option<PlayerAssignedToGame> {
        self.internal.read().unwrap().get_player_game(player_id)
    }

    /// Attempts to start a game for the players waiting in the lobby, respecting their game size preferences. If a game
    /// can be started the given `game_manager` will be called to actually do that, and the players will be assigned to
    /// the `game_id` returned by the `game_manager`.
    pub fn start_game<T: GameCreator>(&self, game_manager: &T) {
        self.internal.write().unwrap().start_game(game_manager)
    }
}

struct NonThreadSafeLobby {
    players_in_lobby: HashMap<PlayerIdInLobby, LobbyPlayer>,
    tokens: HashMap<PlayerIdInLobby, Token>,
    next_player_id: usize,
    game_size_prefs_ranges: RangeMap<usize, PlayerIdInLobby>,
}

impl NonThreadSafeLobby {
    fn new() -> NonThreadSafeLobby {
        NonThreadSafeLobby {
            players_in_lobby: HashMap::new(),
            tokens: HashMap::new(),
            next_player_id: 0,
            game_size_prefs_ranges: RangeMap::new(),
        }
    }

    fn add_player(
        &mut self,
        min_game_size: usize,
        max_game_size: usize,
    ) -> Result<(PlayerIdInLobby, Token), ()> {
        if min_game_size < 2 || max_game_size < min_game_size || max_game_size > 52 {
            return Err(());
        }

        let player_id = PlayerIdInLobby(self.next_player_id);
        self.next_player_id += 1;
        let player_waiting_for_game = PlayerWaitingForGame {
            player_id_in_lobby: player_id,
            game_size_preference: GameSizePreference {
                min_game_size,
                max_game_size,
            },
        };
        self.insert_player_to_game_size_prefs(&player_waiting_for_game);
        self.players_in_lobby.insert(
            player_id,
            LobbyPlayer::WaitingForGame(player_waiting_for_game),
        );
        let token = Token::random();
        self.tokens.insert(player_id, token.clone());
        Ok((player_id, token))
    }

    /// Signals to the lobby that the player no longer wants to play a game.
    ///
    /// If the player is already in a game, `Err` is returned, otherwise `Ok`.
    fn abandon_lobby(&mut self, player_id: PlayerIdInLobby) -> Result<(), ()> {
        if let Some(LobbyPlayer::InGame(_)) = self.players_in_lobby.get(&player_id) {
            return Err(());
        }

        self.players_in_lobby.remove(&player_id);
        self.rebuild_game_size_prefs();
        Ok(())
    }

    /// Returns the game ID for the given player, if one has been assigned.
    fn get_player_game(&self, player_id: PlayerIdInLobby) -> Option<PlayerAssignedToGame> {
        if let Some(LobbyPlayer::InGame(game_assignment)) = self.players_in_lobby.get(&player_id) {
            Some(*game_assignment)
        } else {
            None
        }
    }

    /// Attempts to start a game for the players waiting in the lobby, respecting their game size
    /// preferences.
    fn start_game<T: GameCreator>(&mut self, game_manager: &T) {
        let player_ids_in_optimal_game = self
            .game_size_prefs_ranges
            .reverse_iterator()
            .filter_map(|(range, player_ids)| {
                let num_players_in_range = player_ids.len();
                if range.start <= num_players_in_range {
                    let num_players_to_play = min(
                        range.end - 1, // -1 because RangeMap range ends are exclusive
                        num_players_in_range,
                    );
                    Some(&player_ids[..num_players_to_play])
                } else {
                    None
                }
            })
            .next();

        if let Some(player_ids_in_game) = player_ids_in_optimal_game {
            let game_id = game_manager.new_game(player_ids_in_game.len());

            for (player_id_in_game, player_id_in_lobby) in player_ids_in_game.iter().enumerate() {
                self.players_in_lobby.insert(
                    *player_id_in_lobby,
                    LobbyPlayer::InGame(PlayerAssignedToGame {
                        game_id,
                        player_id_in_game,
                    }),
                );
            }

            self.rebuild_game_size_prefs();
        }
    }

    fn rebuild_game_size_prefs(&mut self) {
        self.game_size_prefs_ranges = RangeMap::new();
        let mut players_waiting_for_game = Vec::new();
        for p in self.players_in_lobby.values() {
            if let LobbyPlayer::WaitingForGame(player_waiting_for_game) = p {
                players_waiting_for_game.push(player_waiting_for_game.clone());
            }
        }
        for p in players_waiting_for_game {
            self.insert_player_to_game_size_prefs(&p);
        }
    }

    fn insert_player_to_game_size_prefs(&mut self, player_waiting_for_game: &PlayerWaitingForGame) {
        self.game_size_prefs_ranges.insert(
            player_waiting_for_game.game_size_preference.min_game_size,
            // add +1 because the RangeMap range ends are exclusive
            player_waiting_for_game.game_size_preference.max_game_size + 1,
            player_waiting_for_game.player_id_in_lobby,
        );
    }
}

#[derive(Clone, Copy)]
struct GameSizePreference {
    min_game_size: usize,
    max_game_size: usize,
}

#[cfg(test)]
mod tests {

    use crate::lobby::*;

    #[test]
    fn test_add_player_no_game() {
        let l = Lobby::new();
        let player_1_id = l.add_player(2, 4).unwrap().0; //     a single player joins the lobby
        assert!(l.get_player_game(player_1_id).is_none()); // and has no game yet
    }

    #[test]
    fn test_start_game() {
        let l = Lobby::new();
        let player_1_id = l.add_player(2, 4).unwrap().0; // player 1 joins the lobby with minimum game size 2
        let player_2_id = l.add_player(2, 4).unwrap().0; // player 2 joins the lobby with minimum game size 2
        let player_3_id = l.add_player(6, 6).unwrap().0; // player 2 joins the lobby with minimum game size 6

        l.start_game(&mock_game_creator(2, GameId(42))); // starting a game assigns
        assert_eq!(
            l.get_player_game(player_1_id), // player 1 as player 0
            Some(PlayerAssignedToGame {
                game_id: GameId(42),
                player_id_in_game: 0
            })
        );
        assert_eq!(
            l.get_player_game(player_2_id), // and player 2 as player 1
            Some(PlayerAssignedToGame {
                game_id: GameId(42),
                player_id_in_game: 1
            })
        );
        assert!(l.get_player_game(player_3_id).is_none()); // player 3 not matched because their size preferences
    }

    #[test]
    fn test_player_assignment_optimal() {
        let l = Lobby::new();
        l.add_player(2, 4).unwrap().0;
        l.add_player(2, 4).unwrap().0;
        l.add_player(2, 4).unwrap().0;
        l.add_player(2, 4).unwrap().0;

        // larger game sizes are preferred, so the largest possible game is started
        l.start_game(&mock_game_creator(4, GameId(42)));
    }

    #[test]
    fn test_player_assignment_prefers_earlier_player() {
        let l = Lobby::new();
        let mut players = Vec::new();
        players.push(l.add_player(3, 6).unwrap().0);
        players.push(l.add_player(3, 4).unwrap().0);
        players.push(l.add_player(2, 6).unwrap().0);
        players.push(l.add_player(2, 4).unwrap().0);
        let player_5_id = l.add_player(2, 4).unwrap().0;
        // everyone is OK with 3 and 4 player games, so a 4 player game will be started
        l.start_game(&mock_game_creator(4, GameId(42)));

        for p in players {
            // the first 4 players get assigned to the game
            assert_eq!(l.get_player_game(p).map(|pg| pg.game_id), Some(GameId(42)));
        }

        // and the 5th player remains unassigned
        assert!(l.get_player_game(player_5_id).is_none());
    }

    #[test]
    fn test_player_assignment_multiple_games() {
        let l = Lobby::new();
        let mut game1_players = Vec::new();
        let unreasonable_player = l.add_player(12, 42).unwrap().0;
        game1_players.push(l.add_player(3, 6).unwrap().0);
        game1_players.push(l.add_player(3, 3).unwrap().0);
        game1_players.push(l.add_player(2, 6).unwrap().0);
        let first_game_2_player = l.add_player(2, 4).unwrap().0;

        // we can start a 3 player game with the first 3 players
        l.start_game(&mock_game_creator(3, GameId(1)));
        for p in &game1_players {
            // the first 3 players get assigned to the game
            assert_eq!(l.get_player_game(*p).map(|pg| pg.game_id), Some(GameId(1)));
        }
        assert!(l.get_player_game(unreasonable_player).is_none());
        assert!(l.get_player_game(first_game_2_player).is_none());

        let mut game2_players = Vec::new();
        game2_players.push(first_game_2_player);
        game2_players.push(l.add_player(2, 4).unwrap().0);

        // we can now start another 2 player game
        l.start_game(&mock_game_creator(2, GameId(2)));
        for p in game1_players {
            // the first 3 players are still assigned to the same game
            assert_eq!(l.get_player_game(p).map(|pg| pg.game_id), Some(GameId(1)));
        }

        for p in game2_players {
            // the next 2 players are assigned to another game
            assert_eq!(l.get_player_game(p).map(|pg| pg.game_id), Some(GameId(2)));
        }

        assert!(l.get_player_game(unreasonable_player).is_none());
    }

    #[test]
    fn test_wrong_player_reference_in_get() {
        let l = Lobby::new();
        l.get_player_game(PlayerIdInLobby(42)); // does not exist, should not panic
    }

    #[test]
    fn test_wrong_game_size_preferences() {
        let l = Lobby::new();
        assert!(l.add_player(0, 2).is_err());
        assert!(l.add_player(1, 2).is_err());
        assert!(l.add_player(3, 2).is_err());
    }

    #[test]
    fn test_player_abandon() {
        let l = Lobby::new();
        let player_to_leave = l.add_player(3, 3).unwrap().0;
        let mut players = Vec::new();
        players.push(l.add_player(3, 4).unwrap().0);
        players.push(l.add_player(3, 5).unwrap().0);
        l.abandon_lobby(player_to_leave).unwrap();
        l.start_game(&GAME_CREATOR_NO_GAME_CREATED);

        // because player 1 left the lobby nor they nor they other players should get a game
        assert!(l.get_player_game(player_to_leave).is_none());
        for p in &players {
            assert!(l.get_player_game(*p).is_none());
        }

        // a new player joins
        players.push(l.add_player(3, 5).unwrap().0);

        // and with that the game can be started
        l.start_game(&mock_game_creator(3, GameId(1)));
        for p in players {
            assert_eq!(l.get_player_game(p).map(|pg| pg.game_id), Some(GameId(1)));
        }

        assert!(l.get_player_game(player_to_leave).is_none());
    }

    fn mock_game_creator(expected_game_size: usize, return_game_id: GameId) -> impl GameCreator {
        MockGameCreator {
            expected_game_size: Some(expected_game_size),
            return_game_id: Some(return_game_id),
        }
    }

    static GAME_CREATOR_NO_GAME_CREATED: MockGameCreator = MockGameCreator {
        expected_game_size: None,
        return_game_id: None,
    };

    struct MockGameCreator {
        expected_game_size: Option<usize>,
        return_game_id: Option<GameId>,
    }
    impl GameCreator for MockGameCreator {
        fn new_game(&self, num_players: usize) -> GameId {
            if num_players
                == self
                    .expected_game_size
                    .expect("new_game called unexpectedly")
            {
                self.return_game_id.expect("new_game called unexpectedly")
            } else {
                panic!(format!(
                    "num_players expected={}, actual={}",
                    self.expected_game_size.unwrap(),
                    num_players
                ));
            }
        }
    }
}
