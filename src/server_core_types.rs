//! Declares some common types for the server

/// The ID of a game managed by the server
#[derive(Clone, Copy, Hash, PartialEq)]
pub struct GameId(pub usize);
