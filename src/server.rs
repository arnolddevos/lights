use super::codec::{Group, Level, Ramp};
use super::Event;
use tokio::sync::broadcast::Sender;
use warp::http::StatusCode;
use warp::Filter;

#[derive(Clone, PartialEq, Debug)]
pub enum Post {
    Level(Group, Level, Ramp),
    On(Box<str>),
    Off(Box<str>),
}

pub async fn server_daemon(inbound: Sender<Event>) {
    let routes = warp::post()
        .and(warp::path("v1"))
        .and(warp::path("level"))
        .and(warp::header("cbus-group"))
        .and(warp::header("cbus-level"))
        .and(warp::header("cbus-ramp"))
        .map(move |group: u8, level: u8, ramp: u16| {
            let res = inbound.send(Event::Hmi(Post::Level(
                Group(group),
                Level(level),
                Ramp(ramp),
            )));
            if res.is_ok() {
                StatusCode::OK
            } else {
                println!("* server_daemon: {res:?}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        });

    warp::serve(routes).bind(([127, 0, 0, 1], 3030)).await
}
