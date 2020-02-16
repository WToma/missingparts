//! Declares some common types for the server

/// The ID of a game managed by the server
#[derive(Clone, Copy, Hash, PartialEq, Debug)]
pub struct GameId(pub usize);
