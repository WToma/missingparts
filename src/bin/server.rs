use std::convert::Infallible;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};

use missingparts::lobby::{GameCreator, Lobby, PlayerAssignedToGame, PlayerIdInLobby};
use missingparts::server_core_types::{GameId, Token, TokenVerifier};

use std::sync::Arc;

async fn missingparts_service(
    lobby: Arc<Lobby>,
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
