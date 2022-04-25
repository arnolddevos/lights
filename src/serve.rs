use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Request, Response, Server};
use tokio::sync::broadcast::Sender;

use super::Event;

#[derive(Clone, PartialEq, Debug)]
pub enum Post {
    On(Box<str>),
    Off(Box<str>),
}

fn ok<T>(t: T) -> Result<T, Error> {
    Ok(t)
}

async fn per_request(
    _request: Request<Body>,
    _inbound: Sender<Event>,
) -> Result<Response<Body>, Error> {
    Ok(Response::new(Body::from("Request noted!")))
}

pub async fn serve_daemon(inbound: Sender<Event>) -> Result<(), Error> {
    pretty_env_logger::init();

    let addr = ([127, 0, 0, 1], 3000).into();

    // The closure inside `make_service_fn` is run for each connection,
    // creating a 'service' to handle requests for that specific connection.
    let make_service = make_service_fn(move |_socket: &AddrStream| {
        // While the state was moved into the make_service closure,
        // we need to clone it here because this closure is called
        // once for every connection.
        let inbound = inbound.clone();

        async move {
            // This is the `Service` that will handle the connection.
            // `service_fn` is a helper to convert a function that
            // returns a Response into a `Service`.
            ok(service_fn(move |request| {
                per_request(request, inbound.clone())
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_service);

    println!("Listening on http://{}", addr);

    server.await
}
