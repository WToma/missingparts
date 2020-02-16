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

#[derive(Clone, Copy)]
pub struct PlayerAssignedToGame {
    pub game_id: GameId,
    pub player_id_in_game: usize,
}

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
        self.players_in_lobby
            .push(LobbyPlayer::WaitingForGame(PlayerWaitingForGame {
                player_id_in_lobby: player_id,
                game_size_preference: GameSizePreference {
                    min_game_size,
                    max_game_size,
                },
            }));
        self.game_size_prefs_ranges
            .insert(min_game_size, max_game_size, player_id);
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
        for p in &self.players_in_lobby {
            if let LobbyPlayer::WaitingForGame(player_waiting_for_game) = p {
                self.game_size_prefs_ranges.insert(
                    player_waiting_for_game.game_size_preference.min_game_size,
                    // add +1 because the RangeMap range ends are exclusive
                    player_waiting_for_game.game_size_preference.max_game_size + 1,
                    player_waiting_for_game.player_id_in_lobby,
                );
            }
        }
    }
}

struct GameSizePreference {
    min_game_size: usize,
    max_game_size: usize,
}
