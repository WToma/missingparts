use std::collections::BTreeMap;

pub struct GameSizePreference {
    pub min_game_size: usize,
    pub max_game_size: usize,
}

impl GameSizePreference {
    /// Given the game size preferences, returns which players (by index into the preferences)
    /// should be matched together for a game.
    ///
    /// Beware! O(n * (game size range))
    pub fn get_largest_game(prefs: &[&GameSizePreference]) -> Vec<usize> {
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
