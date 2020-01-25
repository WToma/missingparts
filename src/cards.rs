use rand::Rng;
use std::convert::TryFrom;
use std::fmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
/// The possible suits of cards.
///
/// Note: for programming purposes, `Suit` should be treated as a scalar, therefore the `Clone` and `Copy`
/// traits are derived.
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    /// Returns an array of all possible card suits.
    pub fn arr() -> [Suit; 4] {
        use Suit::*;
        [Clubs, Diamonds, Hearts, Spades]
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl TryFrom<&str> for Suit {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        use Suit::*;

        // try parsing the canonical name first
        let s = s.trim().to_ascii_lowercase();
        for member in &Suit::arr() {
            if member.to_string().as_str().to_ascii_lowercase() == s {
                return Ok(*member);
            }
        }

        // if that fails, accept the first character
        if s.len() > 1 {
            return Err(());
        }

        let first_char = s.chars().next().ok_or(())?;
        match first_char {
            'c' => Ok(Clubs),
            'd' => Ok(Diamonds),
            'h' => Ok(Hearts),
            's' => Ok(Spades),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
/// The possible ranks of cards.
///
/// Note: for programming purposes, `Rank` should be treated as a scalar, therefore the `Clone` and `Copy`
/// traits are derived.
pub enum Rank {
    Ace,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
}

impl Rank {
    /// Returns an array of all possible card ranks in their natural order.
    ///
    /// Note: depending on the game the cards are used for, the strength order may be different
    /// from the index order in the returned array. E.g. in some games `Ace > King`, or `Ace > Ten > King`.
    pub fn arr() -> [Rank; 13] {
        use Rank::*;
        [
            Ace, Two, Three, Four, Five, Six, Seven, Eight, Nine, Ten, Jack, Queen, King,
        ]
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl TryFrom<&str> for Rank {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        use Rank::*;

        // try the canonical name first
        let s = s.trim().to_ascii_lowercase();
        for member in &Rank::arr() {
            if member.to_string().as_str().to_ascii_lowercase() == s {
                return Ok(*member);
            }
        }

        // if that fails, for the figure cards accept the first letter of the name
        if s.len() == 1 {
            let first_char = s.chars().next().ok_or(())?;

            if !first_char.is_digit(10) {
                return match first_char.to_ascii_lowercase() {
                    'a' => Ok(Ace),
                    'j' => Ok(Jack),
                    'q' => Ok(Queen),
                    'k' => Ok(King),
                    _ => Err(()),
                };
            }
        }

        // otherwise try to parse as number and pick the number
        let n: usize = s.parse().map_err(|_| ())?;
        return if n >= 1 && n <= 13 {
            Ok(Rank::arr()[n - 1])
        } else {
            Err(())
        };
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash)]
/// A single piece of card in a 52-piece French deck.
///
/// Note: for programming purposes, `Card` should be treated as a scalar, therefore the `Clone` and `Copy`
/// traits are derived.
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} of {}", self.rank, self.suit)
    }
}

impl TryFrom<&str> for Card {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        // accepted format: "{rank} [of ]{suit}", e.g. "4 of Clubs", "a of h", "K D"
        let mut parts = s.split_whitespace();

        let rank = parts.next().ok_or(format!(
            "'{}' is not a valid card, because the rank is missing",
            s,
        ))?;
        let rank = Rank::try_from(rank).map_err(|_| {
            format!(
                "'{}' is not a valid card, because '{}' is not a valid rank",
                s, rank,
            )
        })?;

        let second_part = parts.next().ok_or(format!(
            "'{}' is not a valid card, because it is missing the suit",
            s
        ))?;
        let suit = if second_part.to_ascii_lowercase() == "of" {
            parts.next().ok_or(format!(
                "'{}' is not a valid card, because it is missing the suit",
                s
            ))?
        } else {
            second_part
        };
        let suit = Suit::try_from(suit).map_err(|_| {
            format!(
                "'{}' is not a valid card, because '{}' is not a valid suit",
                s, suit,
            )
        })?;

        Ok(Card { suit, rank })
    }
}

#[derive(Debug)]
/// A deck of `Card`s.
///
/// As long as the deck was created using the [`shuffle` method](struct.Deck.html#method.shuffle) and
/// no cards were manually added, the deck will remain unique and in random order.
pub struct Deck {
    shuffled_cards: Vec<Card>,
}

impl Deck {
    /// Returns a pre-shuffled deck of all 52 cards.
    ///
    /// # Examples
    /// ```
    /// # use missingparts::cards::*;
    /// let deck = Deck::shuffle();
    /// assert_eq!(deck.len(), 52);
    /// ```
    pub fn shuffle() -> Deck {
        let mut cards: Vec<Card> = Vec::new();
        for suit in &Suit::arr() {
            for rank in &Rank::arr() {
                cards.push(Card {
                    suit: *suit,
                    rank: *rank,
                });
            }
        }
        rand::thread_rng().shuffle(&mut cards[..]);
        Deck {
            shuffled_cards: cards,
        }
    }

    /// Removes the top at most `n` cards from this deck, and returns them.
    ///
    /// # Examples
    /// ```
    /// # fn all_unique(cards: &Vec<Card>) -> bool {
    /// #    use std::collections::HashSet;
    /// #    let mut card_set = HashSet::new();
    /// #    for card in cards {
    /// #        card_set.insert(*card);
    /// #    }
    /// #    card_set.len() == cards.len()
    /// # }
    /// # use missingparts::cards::*;
    /// let mut deck = Deck::shuffle();
    ///
    /// let mut first_removed = deck.remove_top(10);
    /// assert_eq!(first_removed.len(), 10);
    /// assert_eq!(deck.len(), 42);
    ///
    /// let mut second_removed = deck.remove_top(50);
    /// assert_eq!(second_removed.len(), 42);
    /// assert!(!deck.non_empty());
    ///
    /// let mut all_cards = first_removed;
    /// all_cards.append(&mut second_removed);
    /// assert!(all_unique(&all_cards));
    /// ```
    pub fn remove_top(&mut self, n: usize) -> Vec<Card> {
        use std::cmp::min;
        let mut result = Vec::new();
        for _ in 0..min(n, self.shuffled_cards.len()) {
            result.push(self.shuffled_cards.remove(0));
        }
        result
    }

    /// Returns whether the deck has cards in it (`true`) or not (`false`).
    ///
    /// # Examples
    /// ```
    /// # use missingparts::cards::*;
    /// let mut deck = Deck::shuffle();
    /// assert!(deck.non_empty());
    /// deck.remove_top(60);
    /// assert!(!deck.non_empty());
    /// ```
    pub fn non_empty(&self) -> bool {
        !self.shuffled_cards.is_empty()
    }

    /// Returns the number of cards currently in the deck
    ///
    /// # Examples
    /// ```
    /// # use missingparts::cards::*;
    /// let deck = Deck::shuffle();
    /// assert_eq!(deck.len(), 52);
    /// ```
    pub fn len(&self) -> usize {
        self.shuffled_cards.len()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use Rank::*;
    use Suit::*;

    #[test]
    fn card_parse_ok() {
        assert_eq!(
            Card::try_from("Ace of Spades").unwrap(),
            Card {
                suit: Spades,
                rank: Ace,
            },
        );

        assert_eq!(
            Card::try_from("4 of Hearts").unwrap(),
            Card {
                suit: Hearts,
                rank: Four,
            },
        );

        assert_eq!(
            Card::try_from("k c").unwrap(),
            Card {
                suit: Clubs,
                rank: King,
            },
        );

        assert_eq!(
            Card::try_from("5 d").unwrap(),
            Card {
                suit: Diamonds,
                rank: Five,
            },
        );
    }

    #[test]
    fn card_parse_fail() {
        let invalid_card_strings =
            vec!["notacard", "not a card", "14 of Hearts", "King of Donkeys"];

        for invalid_card_string in invalid_card_strings {
            let parse_result = Card::try_from(invalid_card_string);
            assert!(
                parse_result.is_err(),
                "Result for '{}' was not an error, got: {}",
                invalid_card_string,
                parse_result.unwrap(),
            );
        }
    }

    #[test]
    fn card_parse_reflexive() {
        for suit in &Suit::arr() {
            for rank in &Rank::arr() {
                let card = Card {
                    suit: *suit,
                    rank: *rank,
                };
                let card_str = card.to_string();
                assert_eq!(Card::try_from(card_str.as_str()).unwrap(), card);
            }
        }
    }

    // deck tests
    #[test]
    fn remove_top_removes_correct_cards() {
        let mut deck = create_unshuffled_deck();
        assert_eq!(
            *(deck.remove_top(1).first().unwrap()),
            Card::try_from("Ace of Clubs").unwrap()
        );

        assert_eq!(
            deck.remove_top(2),
            vec![
                Card::try_from("2 c").unwrap(),
                Card::try_from("3 c").unwrap()
            ],
        )
    }

    fn create_unshuffled_deck() -> Deck {
        let mut unshuffled_cards = Vec::new();
        for suit in &Suit::arr() {
            for rank in &Rank::arr() {
                unshuffled_cards.push(Card {
                    suit: *suit,
                    rank: *rank,
                });
            }
        }

        Deck {
            shuffled_cards: unshuffled_cards,
        }
    }
}
