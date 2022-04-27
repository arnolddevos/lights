use bytes::Bytes;
use codec::Message;
use server::{server_daemon, Post};
use std::fmt::Debug;
use tokio::io::{self, AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio::time::{sleep, Duration, Instant};
use tokio::{select, task};

mod busio;
mod codec;
mod serve;

const HOST: &str = "C228F35.gracelands";
const PORT: u16 = 10001;

/// Something that happened somewhere in the recent past.
#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    Cbus(Message),
    Hmi(Post),
}

async fn input_task<I>(input: I, inbound: Sender<Event>) -> io::Result<()>
where
    I: AsyncRead + Unpin,
{
    async fn accept(line: Bytes, inbound: &Sender<Event>) {
        let mesg = codec::decode(line);
        let _ = inbound.send(Event::Cbus(mesg));
    }

    busio::read_lines(input, |line| accept(line, &inbound)).await
}

async fn output_task<O>(mut outbound: Receiver<Message>, mut output: O) -> io::Result<()>
where
    O: AsyncWrite + Unpin,
{
    loop {
        if let Ok(mesg) = outbound.recv().await {
            println!("< {mesg:?}");
            output.write_all(&codec::encode(mesg)[..]).await?
        }
    }
}

async fn cbus_session(inbound: &Sender<Event>, outbound: &Sender<Message>) -> io::Result<()> {
    // connect to internal pub/sub
    let inbound = inbound.clone();
    let outbound = outbound.subscribe();

    // Connect to a CBUS device
    let stream = TcpStream::connect((HOST, PORT)).await?;
    let (input, mut output) = stream.into_split();

    // configure CBUS device
    output.write_all(&codec::preamble()[..]).await?;

    // run tasks
    let input_task = task::spawn(input_task(input, inbound));
    let output_task = task::spawn(output_task(outbound, output));
    select! {res = input_task => res?, res = output_task => res?}
}

// maintain a connection to the CBUS
async fn cbus_daemon(inbound: Sender<Event>, outbound: Sender<Message>) -> io::Result<()> {
    loop {
        println!("Connecting...");
        let res = cbus_session(&inbound, &outbound).await;
        println!("{res:?}");
        sleep(Duration::from_millis(2000)).await;
    }
}

async fn log_task<T>(mut channel: Receiver<T>)
where
    T: Debug + Clone,
{
    loop {
        let res = channel.recv().await;
        if let Ok(t) = res {
            println!("{t:?}")
        } else {
            println!("{res:?}")
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Starting at {:?}", Instant::now());

    // create the internal pub/sub channels
    let (inbound, _) = broadcast::channel::<Event>(16);
    let (outbound, _) = broadcast::channel::<Message>(16);

    // create the tasks
    let cbus_daemon = task::spawn(cbus_daemon(inbound.clone(), outbound.clone()));
    let server_daemon = task::spawn(server_daemon(inbound.clone()));
    let log_task = task::spawn(log_task(inbound.subscribe()));

    // run all the tasks
    select! {
        res = cbus_daemon => println!("{res:?}"), res = server_daemon => println!("{res:?}"), res = log_task => println!("{res:?}")
    };
}
