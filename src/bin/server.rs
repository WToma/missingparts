use std::convert::Infallible;
use std::fmt;
use std::str;
use std::sync::Arc;

use http::request::Parts;

use hyper::header::{HeaderValue, ACCEPT, CONTENT_LENGTH, CONTENT_TYPE};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error as HyperError, Method, Request, Response, Server, StatusCode};

use json5;
use serde::{de, Deserialize, Serialize};
use serde_json;

use missingparts::lobby::Lobby;

async fn missingparts_service(
    lobby: Arc<Lobby>,
    req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    if req.method() == &Method::POST && req.uri().path() == "/lobby" {
        let (parts, body) = req.into_parts();
        let rich_parts = RichParts::from(&parts);
        let body: Result<JoinLobbyRequest, BodyParseError> =
            deserialize_by_content_type(&rich_parts, body, 1024).await;
        let response_mime_type = if let Some(mime_type) = rich_parts.guess_response_type() {
            mime_type
        } else {
            return Ok(Response::builder()
                                .status(StatusCode::NOT_ACCEPTABLE)
                                .body(Body::from("no compatible Accept value found. supported application/json and application/json5"))
                                .unwrap());
        };
        match body {
            Ok(body) => {
                let add_player_result = lobby.add_player(body.min_game_size, body.max_game_size);
                match add_player_result {
                    Ok((player_id_in_lobby, token)) => {
                        let resp = JoinedLobbyResponse {
                            player_id_in_lobby: player_id_in_lobby.0,
                            token: token.0,
                        };
                        Ok(Response::builder()
                            .status(StatusCode::CREATED)
                            .body(response_mime_type.serialize(&resp))
                            .unwrap())
                    }
                    Err(()) => {
                        let resp = InvalidGameSizePreference {
                            min_game_size: body.min_game_size,
                            max_game_size: body.max_game_size,
                        };
                        Ok(Response::builder()
                            .status(StatusCode::CREATED)
                            .body(response_mime_type.serialize(&resp))
                            .unwrap())
                    }
                }
            }
            Err(e) => Ok(e.into()),
        }
    } else {
        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap())
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lobby = Arc::new(Lobby::new());

    // For every connection, we must make a `Service` to handle all
    // incoming HTTP requests on said connection.
    let make_svc = make_service_fn(move |_conn| {
        // This is the `Service` that will handle the connection.
        // `service_fn` is a helper to convert a function that
        // returns a Response into a `Service`.
        let lobby = Arc::clone(&lobby);
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                missingparts_service(Arc::clone(&lobby), req)
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

async fn deserialize_by_content_type<T: de::DeserializeOwned>(
    parts: &RichParts,
    body: Body,
    max_content_length: usize,
) -> Result<T, BodyParseError> {
    use BodyParseError::*;
    let content_type = parts.get_content_type();

    match &content_type.charset_name.as_ref().filter(|c| c != &"utf8") {
        Some(unsupported_charset) => {
            return Err(UnsupportedCharset(unsupported_charset.to_string()))
        }
        None => (),
    };
    let content_type = SupportedMimeType::from_mime_type(&content_type.content_type)
        .map_err(|unsupported_mime_type| UnsupportedContentType(unsupported_mime_type.clone()))?;

    let content_length: usize = parts.get_content_length().ok_or(ContentLengthMissing)?;

    if content_length > max_content_length {
        return Err(RequestTooLarge(content_length, max_content_length));
    }

    let full_body = hyper::body::to_bytes(body).await?;
    let full_body_str = str::from_utf8(&full_body)?;

    content_type.deserialize(full_body_str)
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
    content_type: ContentType,
    content_length: Option<usize>,
    accept: Accept,
}
impl From<&Parts> for RichParts {
    fn from(parts: &Parts) -> RichParts {
        let content_type = Self::parse_content_type(parts);
        let content_length = Self::parse_content_length(parts);
        let accept = Self::parse_accept(parts);
        RichParts {
            content_type: content_type,
            content_length,
            accept,
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
}
