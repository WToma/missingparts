use std::convert::Infallible;
use std::str;
use std::sync::Arc;

use hyper::header::{HeaderName, HeaderValue, CONTENT_LENGTH, CONTENT_TYPE};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error as HyperError, Method, Request, Response, Server, StatusCode};

use json5;
use serde::{de, Deserialize, Serialize};
use serde_json;

use missingparts::lobby::{GameCreator, Lobby, PlayerAssignedToGame, PlayerIdInLobby};
use missingparts::server_core_types::{GameId, Token, TokenVerifier};

async fn missingparts_service(
    _lobby: Arc<Lobby>,
    req: Request<Body>,
) -> Result<Response<Body>, Infallible> {
    if req.method() == &Method::POST && req.uri().path() == "lobby" {
        unimplemented!()
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

enum BodyParseError {
    UnsupportedContentType(String),
    RequestTooLarge(usize),
    BodyReadingError(HyperError),
    ContentLengthMissing,
    EncodingError(str::Utf8Error),
    JsonError {
        error: serde_json::Error,
        assumed: bool,
    },
    Json5Error(json5::Error),
}

async fn deserialize_by_content_type<T: de::DeserializeOwned>(
    req: Request<Body>,
    max_content_length: usize,
) -> Result<Request<T>, BodyParseError> {
    use BodyParseError::*;
    let (content_type, assumed) = req
        .headers()
        .get(CONTENT_TYPE)
        .iter()
        .next()
        .and_then(|h| h.to_str().ok())
        .map(|h| (h, false))
        .unwrap_or((&"application/json", true));

    // TODO: we're silently assuming utf8 content type below, without checking the `charset`
    // parameter from the encoding. We should assume utf-8 if empty, and reject anything
    // that is not utf8. This will also break if the caller wants to use json5 and defines
    // the charset in the encoding header

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

    let content_type = String::from(content_type);

    let (parts, body) = req.into_parts();
    let full_body = hyper::body::to_bytes(body)
        .await
        .map_err(|e| BodyReadingError(e))?;
    let full_body_str = str::from_utf8(&full_body).map_err(|e| EncodingError(e))?;

    match &content_type[..] {
        "application/json" => {
            let body = serde_json::de::from_str(full_body_str)
                .map_err(|e| JsonError { error: e, assumed })?;
            Ok(Request::from_parts(parts, body))
        }
        "application/json5" => {
            let body = json5::from_str(full_body_str).map_err(|e| Json5Error(e))?;
            Ok(Request::from_parts(parts, body))
        }
        unsupported_content_type => Err(UnsupportedContentType(String::from(
            unsupported_content_type,
        ))),
    }
}
