use rand::Rng;
use std::convert::TryFrom;
use std::fmt;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub fn arr() -> [Suit; 4] {
        use Suit::*;
        [Clubs, Diamonds, Hearts, Spades]
    }
}

impl TryFrom<&str> for Suit {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        use Suit::*;
        let first_char = s.trim().chars().next().ok_or(())?;
        match first_char.to_ascii_lowercase() {
            'c' => Ok(Clubs),
            'd' => Ok(Diamonds),
            'h' => Ok(Hearts),
            's' => Ok(Spades),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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
    pub fn arr() -> [Rank; 13] {
        use Rank::*;
        [
            Ace, Two, Three, Four, Five, Six, Seven, Eight, Nine, Ten, Jack, Queen, King,
        ]
    }
}

pub fn first_number(s: &str) -> Option<usize> {
    s.split_whitespace()
        .map(|ss| ss.parse())
        .filter(|pr| pr.is_ok())
        .map(|pr| pr.expect("we should have filtered errors already"))
        .next()
}

impl TryFrom<&str> for Rank {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        use Rank::*;

        let first_char = s.chars().next().ok_or(())?;
        match first_char.to_ascii_lowercase() {
            'a' => Ok(Ace),
            'j' => Ok(Jack),
            'q' => Ok(Queen),
            'k' => Ok(King),
            _ => {
                let n: usize = first_number(s).ok_or(())?;
                if n >= 1 && n <= 13 {
                    Ok(Rank::arr()[n - 1])
                } else {
                    Err(())
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} of {:?}", self.rank, self.suit)
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
pub struct Deck {
    shuffled_cards: Vec<Card>,
}

impl Deck {
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

    pub fn remove_top(&mut self, n: usize) -> Vec<Card> {
        use std::cmp::min;
        let mut result = Vec::new();
        for _ in 0..min(n, self.shuffled_cards.len()) {
            result.push(self.shuffled_cards.remove(0));
        }
        result
    }

    pub fn remove_index(&mut self, i: usize) -> Card {
        // panics if i is out of bounds -- use Option?
        self.shuffled_cards.remove(i)
    }

    pub fn non_empty(&self) -> bool {
        !self.shuffled_cards.is_empty()
    }

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
        let invalid_card_strings = vec![
            "notacard",
            "not a card",
            "14 of Hearts",
            "King of Mushrooms",
        ];

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
}
