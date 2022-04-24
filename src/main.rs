use bytes::Bytes;
use codec::Message;
use tokio::io::{self, AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio::time::{sleep, Duration, Instant};
use tokio::{select, task};

mod busio;
mod codec;

const HOST: &str = "C228F35.gracelands";
const PORT: u16 = 10001;

async fn accept(line: Bytes, inbound: &Sender<Message>) {
    let mesg = codec::decode(line);
    println!("> {mesg:?}");
    let _ = inbound.send(mesg);
}

async fn input_task<I>(inp: I, inbound: Sender<Message>) -> io::Result<()>
where
    I: AsyncRead + Unpin,
{
    busio::read_lines(inp, |line| accept(line, &inbound)).await
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

async fn cbus_session(inbound: Sender<Message>, outbound: Receiver<Message>) -> io::Result<()> {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting at {:?}", Instant::now());

    let (inbound, _) = broadcast::channel(16);
    let (outbound, _) = broadcast::channel(16);

    loop {
        println!("Connecting...");
        let inbound = inbound.clone();
        let outbound = outbound.subscribe();
        let res = cbus_session(inbound, outbound).await;
        println!("{res:?}");
        sleep(Duration::from_millis(2000)).await;
    }
}
