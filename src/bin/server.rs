use std::convert::{Infallible, TryFrom};
use std::fmt;
use std::str;
use std::str::FromStr;
use std::sync::Arc;

use http::request::Parts;

use hyper::header::{HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, LOCATION};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error as HyperError, Method, Request, Response, Server, StatusCode};

use json5;
use serde::{de, Deserialize, Serialize};
use serde_json;

use missingparts::cards::Card;
use missingparts::game_manager::GameManager;
use missingparts::lobby::{Lobby, PlayerIdInLobby};
use missingparts::server_core_types::{GameId, Token, TokenVerifier};

async fn missingparts_service(
    lobby: Arc<Lobby>,
    game_manager: Arc<GameManager>,
    req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    let (parts, body) = req.into_parts();
    let rich_parts = RichParts::from(&parts);
    let response_mime_type = if let Some(mime_type) = rich_parts.guess_response_type() {
        mime_type
    } else {
        return Ok(Response::builder()
                                .status(StatusCode::NOT_ACCEPTABLE)
                                .body(Body::from("no compatible Accept value found. supported application/json and application/json5"))
                                .unwrap());
    };

    if rich_parts.does_match(&Method::POST, "/lobby") {
        let body: Result<JoinLobbyRequest, BodyParseError> =
            rich_parts.deserialize_by_content_type(body, 1024).await;
        match body {
            Ok(body) => Ok(process_join_lobby(
                body,
                &response_mime_type,
                lobby,
                game_manager,
            )),
            Err(e) => Ok(e.into()),
        }
    } else if let Ok(TupleWrapper1(player_id_in_lobby)) =
        rich_parts.try_match(&Method::GET, "/lobby/players/{}/game")
    {
        let player_id_in_lobby = PlayerIdInLobby(player_id_in_lobby);
        let maybe_token = rich_parts.token().and_then(|t| Token::from_str(t).ok());
        let verified = match maybe_token {
            Some(token) => lobby.verify(&player_id_in_lobby, &token),
            None => false,
        };
        if verified {
            Ok(process_get_lobby_player(
                player_id_in_lobby,
                &response_mime_type,
                lobby,
            ))
        } else {
            Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap())
        }
    } else if let Ok(TupleWrapper2(game_id, player_id_in_game)) =
        rich_parts.try_match(&Method::GET, "/games/{}/players/{}/private")
    {
        let game_id = GameId(game_id);
        game_manager.with_game(game_id, |g| {
            let maybe_token = rich_parts.token().and_then(|t| Token::from_str(t).ok());
            let verified = match maybe_token {
                Some(token) => g.verify(&player_id_in_game, &token),
                None => false,
            };
            if verified {
                let resp = PrivateCardResponse {
                    missing_part: g.get_private_card(player_id_in_game),
                };
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(response_mime_type.serialize(&resp))
                    .unwrap())
            } else {
                Ok(Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(Body::empty())
                    .unwrap())
            }
        })
    } else {
        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap())
    }
}

fn process_join_lobby(
    body: JoinLobbyRequest,
    response_mime_type: &SupportedMimeType,
    lobby: Arc<Lobby>,
    game_manager: Arc<GameManager>,
) -> Response<Body> {
    let add_player_result = lobby.add_player(body.min_game_size, body.max_game_size);
    match add_player_result {
        Ok((player_id_in_lobby, token)) => {
            lobby.start_game(&*game_manager);
            match lobby.get_player_game(player_id_in_lobby) {
                None => {
                    let resp = JoinedLobbyResponse {
                        player_id_in_lobby: player_id_in_lobby.0,
                        token: token.0,
                    };
                    Response::builder()
                        .status(StatusCode::CREATED)
                        .header(
                            LOCATION,
                            format!("/lobby/players/{:?}/game", player_id_in_lobby.0),
                        )
                        .body(response_mime_type.serialize(&resp))
                        .unwrap()
                }
                Some(player_assigned_to_game) => {
                    let resp = JoinedGameResponse {
                        game_id: player_assigned_to_game.game_id.0,
                        player_id_in_game: player_assigned_to_game.player_id_in_game,
                        token: Some(token.0),
                    };
                    Response::builder()
                        .status(StatusCode::CREATED)
                        .header(
                            LOCATION,
                            format!(
                                "/games/{:?}/players/{:?}/private",
                                player_assigned_to_game.game_id.0,
                                player_assigned_to_game.player_id_in_game
                            ),
                        )
                        .body(response_mime_type.serialize(&resp))
                        .unwrap()
                }
            }
        }
        Err(()) => {
            let resp = InvalidGameSizePreference {
                min_game_size: body.min_game_size,
                max_game_size: body.max_game_size,
            };
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(response_mime_type.serialize(&resp))
                .unwrap()
        }
    }
}

fn process_get_lobby_player(
    player_id_in_lobby: PlayerIdInLobby,
    response_mime_type: &SupportedMimeType,
    lobby: Arc<Lobby>,
) -> Response<Body> {
    match lobby.get_player_game(player_id_in_lobby) {
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
        Some(player_assigned_to_game) => {
            let resp = JoinedGameResponse {
                game_id: player_assigned_to_game.game_id.0,
                player_id_in_game: player_assigned_to_game.player_id_in_game,
                token: None, // token remains the same
            };

            Response::builder()
                .status(StatusCode::TEMPORARY_REDIRECT)
                .header(
                    LOCATION,
                    format!(
                        "/games/{:?}/players/{:?}/private",
                        player_assigned_to_game.game_id.0,
                        player_assigned_to_game.player_id_in_game
                    ),
                )
                .body(response_mime_type.serialize(&resp))
                .unwrap()
        }
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lobby = Arc::new(Lobby::new());
    let game_manager = Arc::new(GameManager::new());

    // For every connection, we must make a `Service` to handle all
    // incoming HTTP requests on said connection.
    let make_svc = make_service_fn(move |_conn| {
        // This is the `Service` that will handle the connection.
        // `service_fn` is a helper to convert a function that
        // returns a Response into a `Service`.
        let lobby = Arc::clone(&lobby);
        let game_manager = Arc::clone(&game_manager);
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                missingparts_service(Arc::clone(&lobby), Arc::clone(&game_manager), req)
            }))
        }
    });

    let addr = ([127, 0, 0, 1], 3030).into();

    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}

#[derive(Deserialize)]
struct JoinLobbyRequest {
    min_game_size: usize,
    max_game_size: usize,
}

#[derive(Serialize)]
struct JoinedLobbyResponse {
    player_id_in_lobby: usize,
    token: String,
}

#[derive(Serialize)]
struct InvalidGameSizePreference {
    min_game_size: usize,
    max_game_size: usize,
}

#[derive(Serialize)]
struct JoinedGameResponse {
    game_id: usize,
    player_id_in_game: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

#[derive(Serialize)]
struct PrivateCardResponse {
    missing_part: Card,
}

enum BodyParseError {
    UnsupportedContentType(MimeType),
    UnsupportedCharset(String),
    RequestTooLarge(usize, usize),
    BodyReadingError(HyperError),
    ContentLengthMissing,
    EncodingError(str::Utf8Error),
    JsonError(serde_json::Error),
    Json5Error(json5::Error),
}

impl From<HyperError> for BodyParseError {
    fn from(e: HyperError) -> BodyParseError {
        BodyParseError::BodyReadingError(e)
    }
}
impl From<str::Utf8Error> for BodyParseError {
    fn from(e: str::Utf8Error) -> BodyParseError {
        BodyParseError::EncodingError(e)
    }
}
impl From<serde_json::Error> for BodyParseError {
    fn from(e: serde_json::Error) -> BodyParseError {
        BodyParseError::JsonError(e)
    }
}
impl From<json5::Error> for BodyParseError {
    fn from(e: json5::Error) -> BodyParseError {
        BodyParseError::Json5Error(e)
    }
}
impl Into<Response<Body>> for BodyParseError {
    fn into(self) -> Response<Body> {
        use BodyParseError::*;
        let (code, message) = match self {
            UnsupportedContentType(content_type) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "unsupported content type '{}'. Try 'application/json'",
                    content_type
                ),
            ),
            UnsupportedCharset(charset) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "unsupported charset '{}'. the request body must be charset=utf8",
                    charset
                ),
            ),
            RequestTooLarge(size, max_size) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "body must be not larger than {} bytes, was {}",
                    max_size, size
                ),
            ),
            ContentLengthMissing => (
                StatusCode::LENGTH_REQUIRED,
                String::from("Content-Length header must be specified"),
            ),
            BodyReadingError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("failed to read the request body"),
            ),
            EncodingError(utf8_error) => (
                StatusCode::BAD_REQUEST,
                format!(
                    "the request body was not parseable as UTF-8 at position {}",
                    utf8_error.valid_up_to()
                ),
            ),
            JsonError(e) => (
                StatusCode::BAD_REQUEST,
                format!("could not parse the json request: {}", e),
            ),
            Json5Error(e) => (
                StatusCode::BAD_REQUEST,
                format!("could not parse the json5 request: {}", e),
            ),
        };
        Response::builder()
            .status(code)
            .body(Body::from(message))
            .unwrap()
    }
}

/// Simple helpers to parse a content type string that may also contain a charset
struct ContentType {
    content_type: MimeType,
    charset_name: Option<String>,
}
impl From<&str> for ContentType {
    fn from(s: &str) -> ContentType {
        let mut parts = s.split(';');
        let content_type = MimeType::from(parts.next().unwrap()); // the first part is always present

        let charset_name = parts
            .map(|s| s.trim())
            .filter(|s| s.starts_with("charset="))
            .next()
            .map(|s| String::from(&s[8..]));
        ContentType {
            content_type,
            charset_name,
        }
    }
}
impl From<MimeType> for ContentType {
    fn from(m: MimeType) -> ContentType {
        ContentType {
            content_type: m,
            charset_name: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MimeType {
    mime_type: String,
    mime_subtype: String,
}
impl MimeType {
    fn is_compatible_with(&self, other: &MimeType) -> bool {
        Self::is_compatible_part(&self.mime_type, &other.mime_type)
            && Self::is_compatible_part(&self.mime_subtype, &other.mime_subtype)
    }

    fn is_compatible_part(part1: &str, part2: &str) -> bool {
        part1 == "*" || part2 == "*" || part1 == part2
    }

    fn json() -> MimeType {
        MimeType::from("application/json")
    }
    fn json5() -> MimeType {
        MimeType::from("application/json5")
    }
}
impl From<&str> for MimeType {
    fn from(s: &str) -> MimeType {
        let mut parts = s.split('/');
        let mime_type = String::from(parts.next().unwrap()); // OK to unwrap, first element on spliterator always exists
        let mime_subtype = String::from(parts.next().unwrap_or("*"));
        MimeType {
            mime_type,
            mime_subtype,
        }
    }
}
impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.mime_type, self.mime_subtype)
    }
}

enum SupportedMimeType {
    Json,
    Json5,
}
impl SupportedMimeType {
    fn from_mime_type(m: &MimeType) -> Result<SupportedMimeType, &MimeType> {
        use SupportedMimeType::*;
        match m {
            t if t == &MimeType::json() => Ok(Json),
            t if t == &MimeType::json5() => Ok(Json5),
            unsupported => Err(unsupported),
        }
    }

    fn from_mime_type_relaxed(m: &MimeType) -> Result<SupportedMimeType, &MimeType> {
        use SupportedMimeType::*;
        match m {
            t if t.is_compatible_with(&MimeType::json()) => Ok(Json),
            t if t.is_compatible_with(&MimeType::json5()) => Ok(Json5),
            unsupported => Err(unsupported),
        }
    }

    fn serialize<T: Serialize>(&self, x: &T) -> Body {
        use SupportedMimeType::*;
        match self {
            Json => Body::from(serde_json::ser::to_string(x).unwrap()),
            Json5 => Body::from(json5::to_string(x).unwrap()),
        }
    }

    fn deserialize<T: de::DeserializeOwned>(&self, s: &str) -> Result<T, BodyParseError> {
        use SupportedMimeType::*;
        match self {
            Json => Ok(serde_json::de::from_str(s)?),
            Json5 => Ok(json5::from_str(s)?),
        }
    }
}

/// Helper to work with the Accept http headers
struct Accept {
    mime_types: Vec<MimeType>,
}
impl Accept {
    fn is_empty(&self) -> bool {
        self.mime_types.is_empty()
    }
}
impl From<&str> for Accept {
    fn from(s: &str) -> Accept {
        Accept {
            mime_types: s
                .split(',')
                .map(|part| part.split(";").next().unwrap()) // unwrap is OK because at least 1 part will always exist
                .map(|s| MimeType::from(s))
                .collect(),
        }
    }
}
impl<'a> From<hyper::header::GetAll<'a, HeaderValue>> for Accept {
    fn from(headers: hyper::header::GetAll<'a, HeaderValue>) -> Accept {
        let accepts = headers
            .iter()
            .filter_map(|h| h.to_str().ok())
            .map(Accept::from);
        let mut all_mime_types: Vec<MimeType> = Vec::new();
        for mut accept in accepts {
            all_mime_types.append(&mut accept.mime_types);
        }
        Accept {
            mime_types: all_mime_types,
        }
    }
}
struct RichParts {
    method: http::Method,
    uri_path: String,
    content_type: ContentType,
    content_length: Option<usize>,
    accept: Accept,
    token: Option<String>,
}
impl From<&Parts> for RichParts {
    fn from(parts: &Parts) -> RichParts {
        let method = parts.method.clone();
        let uri_path = parts.uri.path().to_string();
        let content_type = Self::parse_content_type(parts);
        let content_length = Self::parse_content_length(parts);
        let accept = Self::parse_accept(parts);
        let token = Self::parse_token(parts);
        RichParts {
            method,
            uri_path,
            content_type: content_type,
            content_length,
            accept,
            token,
        }
    }
}
impl RichParts {
    fn get_content_type(&self) -> &ContentType {
        &self.content_type
    }

    fn get_content_length(&self) -> &Option<usize> {
        &self.content_length
    }

    fn guess_response_type(&self) -> Option<SupportedMimeType> {
        let explicit_accept_type = self
            .accept
            .mime_types
            .iter()
            .filter_map(|explicit_accept_type| {
                SupportedMimeType::from_mime_type_relaxed(explicit_accept_type).ok()
            })
            .next();
        // if the caller specified a supported accepy type, use that
        explicit_accept_type.or_else(|| {
            if !self.accept.is_empty() {
                // if the caller specified accept types, and none of them were supported, then do not attempt to
                // guess the response type
                None
            } else {
                // if the Content-Type was specified and supported, use that. Otherwise, fall back to the default.
                // `content_type` already does the default handling, so we don't have to repeat that here
                // (if the content type was specified and unsupported, we will likely not use the result of this
                // funciton anyway).
                SupportedMimeType::from_mime_type(&self.content_type.content_type).ok()
            }
        })
    }

    async fn deserialize_by_content_type<T: de::DeserializeOwned>(
        &self,
        body: Body,
        max_content_length: usize,
    ) -> Result<T, BodyParseError> {
        use BodyParseError::*;
        let content_type = self.get_content_type();

        // it is allowed for the Content-Type charset to be left unspecified. in that case we assume utf8. Values
        // other than utf8 are not allowed.
        match &content_type.charset_name.as_ref().filter(|c| c != &"utf8") {
            Some(unsupported_charset) => {
                return Err(UnsupportedCharset(unsupported_charset.to_string()))
            }
            None => (),
        };

        // check that the Content-Type is supported. (assuming application/json if unspecified)
        let content_type = SupportedMimeType::from_mime_type(&content_type.content_type).map_err(
            |unsupported_mime_type| UnsupportedContentType(unsupported_mime_type.clone()),
        )?;

        // check that the Content-Length is defined (mandatory) and that it does not exceed the max size the
        // handler is willing to process (this is to prevent DoS-type attacks. hyper will limit the body input stream
        // to Content-Length bytes).
        let content_length: usize = self.get_content_length().ok_or(ContentLengthMissing)?;
        if content_length > max_content_length {
            return Err(RequestTooLarge(content_length, max_content_length));
        }

        // read & deserialize the body
        let full_body = hyper::body::to_bytes(body).await?;
        let full_body_str = str::from_utf8(&full_body)?;
        content_type.deserialize(full_body_str)
    }

    fn does_match(&self, method: &http::Method, path: &str) -> bool {
        &self.method == method && self.uri_path == path
    }

    fn try_match<'a, 'b, T>(
        &'a self,
        method: &http::Method,
        path_pattern: &'b str,
    ) -> Result<T, UriMatchError>
    where
        T: TryFrom<Vec<&'a str>, Error = TupleWrapperParseError>,
    {
        use UriMatchError::*;
        if &self.method != method {
            return Err(MethodNotAllowed(method.clone()));
        }

        let mut path_pattern_parts = path_pattern.split('/');
        let mut path_parts = self.uri_path[..].split('/');
        let mut variables: Vec<&'a str> = Vec::new();

        loop {
            let next_pattern_part = path_pattern_parts.next();
            let next_path_part = path_parts.next();
            match (next_pattern_part, next_path_part) {
                (None, None) => break,
                (Some("{}"), Some(variable)) => variables.push(variable),
                (Some(pattern_part), Some(path_part)) if path_part == pattern_part => continue,
                _ => return Err(PathDoesNotMatch(self.uri_path.clone())),
            }
        }

        T::try_from(variables).map_err(|e| PartsParseError(e))
    }

    fn token(&self) -> Option<&str> {
        self.token.as_ref().map(|s| s.as_str())
    }

    // header parsing helpers

    fn parse_content_type(parts: &Parts) -> ContentType {
        parts
            .headers
            .get(CONTENT_TYPE)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .map(|h| ContentType::from(h))
            .unwrap_or_else(|| ContentType::from(MimeType::json()))
    }

    fn parse_content_length(parts: &Parts) -> Option<usize> {
        parts
            .headers
            .get(CONTENT_LENGTH)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.parse().ok())
    }

    fn parse_accept(parts: &Parts) -> Accept {
        Accept::from(parts.headers.get_all(ACCEPT))
    }

    fn parse_token(parts: &Parts) -> Option<String> {
        parts
            .headers
            .get(AUTHORIZATION)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .map(|h| h.to_string())
    }
}

// helpers for parsing URLs

enum UriMatchError {
    MethodNotAllowed(http::Method),
    PathDoesNotMatch(String),
    PartsParseError(TupleWrapperParseError),
}

enum TupleWrapperParseError {
    TooManyParams(usize),
    TooFewParams(usize),
    FailedToParsePart(usize),
}

struct TupleWrapper1<T>(T);
impl<'a, T1> TryFrom<Vec<&'a str>> for TupleWrapper1<T1>
where
    T1: str::FromStr,
{
    type Error = TupleWrapperParseError;

    fn try_from(s: Vec<&'a str>) -> Result<Self, Self::Error> {
        use TupleWrapperParseError::*;
        let l = s.len();
        match l {
            1 => {
                let inner = T1::from_str(s[0]).map_err(|_| FailedToParsePart(0))?;
                Ok(TupleWrapper1(inner))
            }
            too_few if l < 1 => Err(TooFewParams(too_few)),
            too_many => Err(TooManyParams(too_many)),
        }
    }
}

struct TupleWrapper2<T1, T2>(T1, T2);
impl<'a, T1, T2> TryFrom<Vec<&'a str>> for TupleWrapper2<T1, T2>
where
    T1: str::FromStr,
    T2: str::FromStr,
{
    type Error = TupleWrapperParseError;

    fn try_from(s: Vec<&'a str>) -> Result<Self, Self::Error> {
        use TupleWrapperParseError::*;
        let l = s.len();
        match l {
            2 => {
                let inner1 = T1::from_str(s[0]).map_err(|_| FailedToParsePart(0))?;
                let inner2 = T2::from_str(s[1]).map_err(|_| FailedToParsePart(1))?;
                Ok(TupleWrapper2(inner1, inner2))
            }
            too_few if l < 2 => Err(TooFewParams(too_few)),
            too_many => Err(TooManyParams(too_many)),
        }
    }
}

struct TupleWrapper3<T1, T2, T3>(T1, T2, T3);
impl<'a, T1, T2, T3> TryFrom<Vec<&'a str>> for TupleWrapper3<T1, T2, T3>
where
    T1: str::FromStr,
    T2: str::FromStr,
    T3: str::FromStr,
{
    type Error = TupleWrapperParseError;

    fn try_from(s: Vec<&'a str>) -> Result<Self, Self::Error> {
        use TupleWrapperParseError::*;
        let l = s.len();
        match l {
            3 => {
                let inner1 = T1::from_str(s[0]).map_err(|_| FailedToParsePart(0))?;
                let inner2 = T2::from_str(s[1]).map_err(|_| FailedToParsePart(1))?;
                let inner3 = T3::from_str(s[2]).map_err(|_| FailedToParsePart(2))?;
                Ok(TupleWrapper3(inner1, inner2, inner3))
            }
            too_few if l < 3 => Err(TooFewParams(too_few)),
            too_many => Err(TooManyParams(too_many)),
        }
    }
}
