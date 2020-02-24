use std::convert::Infallible;
use std::str;
use std::sync::Arc;

use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE};
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
        let req: Result<Request<JoinLobbyRequest>, BodyParseError> =
            deserialize_by_content_type(req, 1024).await;
        match req {
            Ok(req) => {
                let body = req.into_body();
                let add_player_result = lobby.add_player(body.min_game_size, body.max_game_size);
                match add_player_result {
                    Ok((player_id_in_lobby, token)) => {
                        let resp = JoinedLobbyResponse {
                            player_id_in_lobby: player_id_in_lobby.0,
                            token: token.0,
                        };
                        Ok(Response::builder()
                            .status(StatusCode::CREATED)
                            // TODO: change the response type based on the accept-type or content-type header
                            //   from the request
                            .body(Body::from(serde_json::to_string(&resp).unwrap()))
                            .unwrap())
                    }
                    Err(()) => Ok(Response::builder()
                        // TODO: fill in the proper error about the game size being invalid
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::empty())
                        .unwrap()),
                }
            }
            Err(_) => Ok(Response::builder()
                // TODO: this is inaccurate, since it can also be an error reading the request
                .status(StatusCode::BAD_REQUEST)
                .body(Body::empty())
                .unwrap()),
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

enum BodyParseError {
    UnsupportedContentType(String),
    UnsupportedCharset(String),
    RequestTooLarge(usize),
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

async fn deserialize_by_content_type<T: de::DeserializeOwned>(
    req: Request<Body>,
    max_content_length: usize,
) -> Result<Request<T>, BodyParseError> {
    use BodyParseError::*;
    let content_type = req
        .headers()
        .get(CONTENT_TYPE)
        .iter()
        .next()
        .and_then(|h| h.to_str().ok())
        .map(|h| ContentType::from(h))
        .unwrap_or_else(|| ContentType::from("application/json"));

    match content_type.charset_name.filter(|c| c != "utf8") {
        Some(unsupported_charset) => return Err(UnsupportedCharset(unsupported_charset)),
        None => (),
    };
    let content_type = content_type.content_type;

    let content_length: usize = req
        .headers()
        .get(CONTENT_LENGTH)
        .iter()
        .next()
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.parse().ok())
        .ok_or(ContentLengthMissing)?;

    if content_length > max_content_length {
        return Err(RequestTooLarge(content_length));
    }

    let (parts, body) = req.into_parts();
    let full_body = hyper::body::to_bytes(body).await?;
    let full_body_str = str::from_utf8(&full_body)?;

    match &content_type[..] {
        "application/json" => {
            let body = serde_json::de::from_str(full_body_str)?;
            Ok(Request::from_parts(parts, body))
        }
        "application/json5" => {
            let body = json5::from_str(full_body_str)?;
            Ok(Request::from_parts(parts, body))
        }
        unsupported_content_type => Err(UnsupportedContentType(String::from(
            unsupported_content_type,
        ))),
    }
}

/// Simple helpers to parse a content type string that may also contain a charset
struct ContentType {
    content_type: String,
    charset_name: Option<String>,
}
impl From<&str> for ContentType {
    fn from(s: &str) -> ContentType {
        let mut parts = s.split(';');
        let content_type = String::from(parts.next().unwrap()); // the first part is always present

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
