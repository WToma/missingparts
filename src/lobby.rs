use crate::range_map::RangeMap;
use crate::server_core_types::GameId;
use std::cmp::min;
use std::sync::RwLock;

pub trait GameCreator {
    /// Creates a new game with the specified number of players, and returns the ID of the game
    /// that was created
    fn new_game(&self, num_players: usize) -> GameId;
}

/// The ID of a player in the lobby
#[derive(Clone, Copy, Hash, PartialEq)]
pub struct PlayerIdInLobby(pub usize);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerAssignedToGame {
    pub game_id: GameId,
    pub player_id_in_game: usize,
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

/// Manages the players who are waiting to join a game. Safe to access concurrently.
pub struct Lobby {
    internal: RwLock<NonThreadSafeLobby>,
}

impl Lobby {
    pub fn new() -> Lobby {
        Lobby {
            internal: RwLock::new(NonThreadSafeLobby::new()),
        }
    }

    pub fn add_player(&self, min_game_size: usize, max_game_size: usize) -> PlayerIdInLobby {
        self.internal
            .write()
            .unwrap()
            .add_player(min_game_size, max_game_size)
    }

    /// Returns the game ID for the given player, if one has been assigned.
    pub fn get_player_game(&self, player_id: PlayerIdInLobby) -> Option<PlayerAssignedToGame> {
        self.internal.read().unwrap().get_player_game(player_id)
    }

    /// Attempts to start a game for the players waiting in the lobby, respecting their game size
    /// preferences.
    pub fn start_games<T: GameCreator>(&self, game_manager: &T) {
        self.internal.write().unwrap().start_games(game_manager)
    }
}

struct NonThreadSafeLobby {
    players_in_lobby: Vec<LobbyPlayer>,
    game_size_prefs_ranges: RangeMap<usize, PlayerIdInLobby>,
}

impl NonThreadSafeLobby {
    fn new() -> NonThreadSafeLobby {
        NonThreadSafeLobby {
            players_in_lobby: Vec::new(),
            game_size_prefs_ranges: RangeMap::new(),
        }
    }

    fn add_player(&mut self, min_game_size: usize, max_game_size: usize) -> PlayerIdInLobby {
        let player_id = PlayerIdInLobby(self.players_in_lobby.len());
        let player_waiting_for_game = PlayerWaitingForGame {
            player_id_in_lobby: player_id,
            game_size_preference: GameSizePreference {
                min_game_size,
                max_game_size,
            },
        };
        self.insert_player_to_game_size_prefs(&player_waiting_for_game);
        self.players_in_lobby
            .push(LobbyPlayer::WaitingForGame(player_waiting_for_game));
        player_id
    }

    /// Returns the game ID for the given player, if one has been assigned.
    fn get_player_game(&self, player_id: PlayerIdInLobby) -> Option<PlayerAssignedToGame> {
        if let LobbyPlayer::InGame(game_assignment) = self.players_in_lobby[player_id.0] {
            Some(game_assignment)
        } else {
            None
        }
    }

    /// Attempts to start a game for the players waiting in the lobby, respecting their game size
    /// preferences.
    fn start_games<T: GameCreator>(&mut self, game_manager: &T) {
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
                let idx: usize = player_id_in_lobby.0;
                self.players_in_lobby[idx] = LobbyPlayer::InGame(PlayerAssignedToGame {
                    game_id,
                    player_id_in_game,
                });
            }

            self.rebuild_game_size_prefs();
        }
    }

    fn rebuild_game_size_prefs(&mut self) {
        self.game_size_prefs_ranges = RangeMap::new();
        let mut players_waiting_for_game = Vec::new();
        for p in &self.players_in_lobby {
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
        let player_1_id = l.add_player(2, 4); //              a single player joins the lobby
        assert!(l.get_player_game(player_1_id).is_none()); // and has no game yet
    }

    #[test]
    fn test_start_game() {
        let l = Lobby::new();
        let player_1_id = l.add_player(2, 4); // player 1 joins the lobby with minimum game size 2
        let player_2_id = l.add_player(2, 4); // player 2 joins the lobby with minimum game size 2
        let player_3_id = l.add_player(6, 6); // player 2 joins the lobby with minimum game size 6

        l.start_games(&mock_game_creator(2, GameId(42))); // starting a game assigns
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
        l.add_player(2, 4);
        l.add_player(2, 4);
        l.add_player(2, 4);
        l.add_player(2, 4);

        // larger game sizes are preferred, so the largest possible game is started
        l.start_games(&mock_game_creator(4, GameId(42)));
    }

    #[test]
    fn test_player_assignment_prefers_earlier_player() {
        let l = Lobby::new();
        let mut players = Vec::new();
        players.push(l.add_player(3, 6));
        players.push(l.add_player(3, 4));
        players.push(l.add_player(2, 6));
        players.push(l.add_player(2, 4));
        let player_5_id = l.add_player(2, 4);
        // everyone is OK with 3 and 4 player games, so a 4 player game will be started
        l.start_games(&mock_game_creator(4, GameId(42)));

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
        let unreasonable_player = l.add_player(12, 42);
        game1_players.push(l.add_player(3, 6));
        game1_players.push(l.add_player(3, 3));
        game1_players.push(l.add_player(2, 6));
        let first_game_2_player = l.add_player(2, 4);

        // we can start a 3 player game with the first 3 players
        l.start_games(&mock_game_creator(3, GameId(1)));
        for p in &game1_players {
            // the first 3 players get assigned to the game
            assert_eq!(l.get_player_game(*p).map(|pg| pg.game_id), Some(GameId(1)));
        }
        assert!(l.get_player_game(unreasonable_player).is_none());
        assert!(l.get_player_game(first_game_2_player).is_none());

        let mut game2_players = Vec::new();
        game2_players.push(first_game_2_player);
        game2_players.push(l.add_player(2, 4));

        // we can now start another 2 player game
        l.start_games(&mock_game_creator(2, GameId(2)));
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

    // #[test]
    // fn test_wrong_player_reference_in_get() {
    //     // should not panic
    //     unimplemented!();
    // }

    // #[test]
    // fn test_wrong_game_size_preferences() {
    //     // should not be accepted
    //     unimplemented!();
    // }

    fn mock_game_creator(expected_game_size: usize, return_game_id: GameId) -> impl GameCreator {
        MockGameCreator {
            expected_game_size,
            return_game_id,
        }
    }

    struct MockGameCreator {
        expected_game_size: usize,
        return_game_id: GameId,
    }
    impl GameCreator for MockGameCreator {
        fn new_game(&self, num_players: usize) -> GameId {
            if num_players == self.expected_game_size {
                self.return_game_id
            } else {
                panic!(format!(
                    "num_players expected={}, actual={}",
                    self.expected_game_size, num_players
                ));
            }
        }
    }
}
