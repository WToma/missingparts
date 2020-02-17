//! Declares some common types for the server

/// The ID of a game managed by the server
#[derive(Clone, Copy, Hash, PartialEq, Debug)]
pub struct GameId(pub usize);

/// A token that can be used for verifying that the caller is really who they claim to be, for example when calling
/// lobby methods or game methods.
///
/// Use [`random`](#method.random) to create a new random token that can be given out.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(pub String);
impl Token {
    /// Creates a new random token.
    pub fn random() -> Token {
        use rand::distributions::Alphanumeric;
        use rand::{thread_rng, Rng};

        Token(thread_rng().sample_iter(&Alphanumeric).take(128).collect())
    }
}

/// Provides a method to verify that a given token belongs to the given entity.
///
/// The [`verify`](#method.verify) verifies tokens.
pub trait TokenVerifier<T> {
    /// Verifies that the given `token` belongs to the given `id`.
    fn verify(&self, id: &T, token: &Token) -> bool;
}
