use crate::server_core_types::GameId;
use std::collections::BTreeMap;
use std::sync::Mutex;

pub trait GameCreator {
    /// Creates a new game with the specified number of players, and returns the ID of the game
    /// that was created
    fn new_game(&self, num_players: usize) -> GameId;
}

#[derive(Clone, Copy)]
pub struct PlayerAssignedToGame {
    pub game_id: GameId,
    pub player_id_in_game: usize,
}

enum LobbyPlayer {
    WaitingForGame {
        player_id_in_lobby: usize,
        game_size_preference: GameSizePreference,
    },

    InGame(PlayerAssignedToGame),
}

/// Manages the players who are waiting to join a game. Safe to access concurrently.
pub struct Lobby {
    players_waiting_for_game: Mutex<Vec<LobbyPlayer>>,
}

impl Lobby {
    pub fn new() -> Lobby {
        Lobby {
            players_waiting_for_game: Mutex::new(Vec::new()),
        }
    }

    pub fn add_player(&self, min_game_size: usize, max_game_size: usize) -> usize {
        let players_waiting_for_game = &mut self.players_waiting_for_game.lock().unwrap();
        let player_id = players_waiting_for_game.len();
        players_waiting_for_game.push(LobbyPlayer::WaitingForGame {
            player_id_in_lobby: player_id,
            game_size_preference: GameSizePreference {
                min_game_size,
                max_game_size,
            },
        });
        player_id
    }

    /// Returns the game ID for the given player, if one has been assigned.
    pub fn get_player_game(&self, player_id: usize) -> Option<PlayerAssignedToGame> {
        let players_waiting_for_game = self.players_waiting_for_game.lock().unwrap();
        if let LobbyPlayer::InGame(game_assignment) = players_waiting_for_game[player_id] {
            Some(game_assignment)
        } else {
            None
        }
    }

    /// Attempts to start a game for the players waiting in the lobby, respecting their game size
    /// preferences.
    pub fn start_games<T>(&self, game_manager: &T)
    where
        T: GameCreator,
    {
        use LobbyPlayer::*;
        let mut players = self.players_waiting_for_game.lock().unwrap();

        let players_waiting_for_game: Vec<&LobbyPlayer> = players
            .iter()
            .filter(|p| {
                if let WaitingForGame { .. } = p {
                    true
                } else {
                    false
                }
            })
            .collect();
        let preferences: Vec<&GameSizePreference> = players_waiting_for_game
            .iter()
            .filter_map(|p| {
                if let WaitingForGame {
                    game_size_preference,
                    ..
                } = p
                {
                    Some(game_size_preference)
                } else {
                    None
                }
            })
            .collect();
        let indices_in_pwg = GameSizePreference::get_largest_game(&preferences[..]);
        let player_ids_in_game: Vec<usize> = indices_in_pwg
            .iter()
            .filter_map(|i| {
                if let WaitingForGame {
                    player_id_in_lobby, ..
                } = players_waiting_for_game[*i]
                {
                    Some(*player_id_in_lobby)
                } else {
                    None
                }
            })
            .collect();

        if player_ids_in_game.len() > 0 {
            let game_id = game_manager.new_game(player_ids_in_game.len());

            for (player_id_in_game, player_id_in_lobby) in player_ids_in_game.iter().enumerate() {
                players[*player_id_in_lobby] = InGame(PlayerAssignedToGame {
                    game_id,
                    player_id_in_game,
                });
            }
        }
    }
}

struct GameSizePreference {
    min_game_size: usize,
    max_game_size: usize,
}

impl GameSizePreference {
    /// Given the game size preferences, returns which players (by index into the preferences)
    /// should be matched together for a game.
    ///
    /// Beware! O(n * (game size range))
    fn get_largest_game(prefs: &[&GameSizePreference]) -> Vec<usize> {
        // TODO: instead of this, this map could be maintained by the Lobby, and that way
        // we don't need to rebuild the map every time

        // key = game size
        // value = list of player indices who are OK with that size
        let mut pref_map: BTreeMap<usize, Vec<usize>> = BTreeMap::new();

        for (idx, pref) in prefs.iter().enumerate() {
            for size in pref.min_game_size..pref.max_game_size + 1 {
                let players_ok_with_size = pref_map.entry(size).or_insert(Vec::new());
                players_ok_with_size.push(idx);
            }
        }

        let max_available_game: Option<(&usize, &Vec<usize>)> = pref_map
            .iter()
            .filter(|(size, players)| players.len() >= **size)
            .max_by_key(|(size, _)| *size);

        max_available_game.map_or(Vec::new(), |(size, players)| {
            let mut res = Vec::new();
            res.extend_from_slice(&players[..*size]);
            res
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_smallest_game() {
        assert_eq!(
            GameSizePreference::get_largest_game(&vec![&gsp(2, 4), &gsp(2, 4)]),
            vec![0, 1]
        );
    }

    #[test]
    fn test_large_game() {
        assert_eq!(
            GameSizePreference::get_largest_game(&vec![
                &gsp(2, 4),
                &gsp(2, 4),
                &gsp(2, 4),
                &gsp(2, 4)
            ]),
            vec![0, 1, 2, 3]
        );
    }

    #[test]
    fn test_too_few_players() {
        // all players prefer large games, there aren't enough players to satisfy that
        assert_eq!(
            GameSizePreference::get_largest_game(&vec![&gsp(4, 6), &gsp(4, 6), &gsp(4, 6)]),
            Vec::new()
        );
    }

    #[test]
    fn test_many_players() {
        // we have many players, and not all of them will fit in the preferred game size
        assert_eq!(
            GameSizePreference::get_largest_game(&vec![
                &gsp(2, 4),
                &gsp(2, 4),
                &gsp(2, 4),
                &gsp(2, 4),
                &gsp(2, 4),
            ]),
            vec![0, 1, 2, 3] // we have 5 players, player 4 was not matched
        );
    }

    #[test]
    fn test_mismatched_expectations() {
        assert_eq!(
            // 3 players want to play a big game, and one of them wants to play a small one
            GameSizePreference::get_largest_game(&vec![
                &gsp(4, 6),
                &gsp(4, 6),
                &gsp(4, 6),
                &gsp(2, 3),
            ]),
            Vec::new()
        );
    }

    fn gsp(min_game_size: usize, max_game_size: usize) -> GameSizePreference {
        GameSizePreference {
            min_game_size,
            max_game_size,
        }
    }
}
