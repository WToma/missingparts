# Game Server for the Missing Parts Card Game

## Features

- fully in memory (no persistence)
- supports multiple concurrent games
- HTTP only

## Development Instructions

The program itself is written in Rust, and uses Cargo. So install those. Optionally install Python 3
for the tester tool.

### Running the game server

`cargo run --bin server`
starts the server. At this point you can use `curl` or a similar tool to make requests to it, or use
the tester tool on it.

### Testing
- `cargo test` runs the unit tests, including doc tests.
- With the server running, `./tester.py localhost:3030 missingparts.schema.json` will simulate a 2-player game and verify the server responses against the schema file (Python 3 required).
- There is also a CLI version, which can be used to more easily simulate a game between some players.
You can run it with `cargo run --bin local_console`.

### Documentation 
`cargo doc --open` will generate the HTML documentation for the Rust code and open it in a web browser.

## How to Write a Client?

See `tester.py` for an example.

Or see the API section for some sort of documentation.

## API

To participate in a game, a client first joins the lobby, providing some preferences about the game the
client wants to join. The server creates games based on these preferences. A new client may get assigned
to a game right away, but more often they will have to poll the lobby to see if they are in a game.

When
joining the lobby the client receives a player ID and an authorization token. The player ID will be part
of the URLs for the subsequent requests, and the token needs to be provided in the headers (`Authorization: <token here>`).

After joining a game, the client will get a game ID and a different player ID, which will be part of the URLs for subsequent requests, and the same token needs to be provided in the headers (`Authorization: <token here>`).

Once in a game, the client can query the game state, or issue moves on behalf of the player. To avoid
getting errors that are inappropriate for the game state, the client should poll the game state until it
is its turn to make a move.

The below endpoint descriptions refer to JSON schema types; these are defined in `missingparts.schema.json`. The endpoints accept JSON and JSON5, and they attempt to respond according the the `Accept` header.

### POST `/lobby`

Joins the lobby. No request body, no authentication. Response: `201` with `join_lobby_response`.

### GET `/lobby/players/:playerID/game`

Polls for a game. Auth token required. The response is either `404` (no game yet),
or `307` with `found_game_response`. The `Location` header points at the URL at which the player's
private info can be found, but this can also be constructed from the data in the response body.

### GET `/games/:gameId/players/:playerId/private`

Queries the private info (secret card) of a player. Auth required. Response is `player_private_response`.

### POST `/games/:gameId/players/:playerId/moves`

Make a move on behalf of a player. Auth required. Request body is `player_action`. For a detailed
description of what the available moves are, see the rustdoc for [`PlayerAction`](./target/doc/missingparts/playeraction/enum.PlayerAction.html). No response except in case of errors.
Errors coming from the game are documented in the [`ActionError`](target/doc/missingparts/actionerror/enum.ActionError.html) type, but there is no JSON schema definition
for these yet.

### GET `/games/:gameId`

Poll the game state. No auth required. Response body is `game_description`.
