//! `gaffer` controls lighting by reacting to events and issuing CBUS messages.
//!
use crate::{codec::Message, server::Post, Event};
use tokio::sync::broadcast::{Receiver, Sender};

/// `gaffer` controls the lighting.  
///
/// It observes inbound events from CBUS and the HMI
/// and generates outbound messages to CBUS
pub async fn gaffer_daemon(mut inbound: Receiver<Event>, outbound: Sender<Message>) {
    loop {
        let res = inbound.recv().await;
        if let Ok(event) = res {
            match event {
                Event::Cbus(message) => react_to_cbus(message, &outbound),
                Event::Hmi(post) => react_to_hmi(post, &outbound),
            }
        } else {
            println!("* gaffer: {res:?}")
        }
    }
}

fn react_to_hmi(post: Post, outbound: &Sender<Message>) {
    let res = match post {
        Post::Level(g, l, r) => outbound.send(Message::SetVar(g, l, r)),
        _ => Ok(0),
    };

    if res.is_err() {
        println!("* gaffer: {res:?}")
    }
}

fn react_to_cbus(message: Message, outbound: &Sender<Message>) {
    match message {
        Message::SetVar(g, l, r) => (),
        _ => (),
    };
}
