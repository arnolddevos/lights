use bytes::Bytes;
use tokio::io::{self, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration, Instant};

mod busio;
mod codec;

const HOST: &str = "C228F35.gracelands";
const PORT: u16 = 10001;

async fn cbus_session() -> io::Result<()> {
    // Connect to a CBUS device
    let stream = TcpStream::connect((HOST, PORT)).await?;

    let (inp, mut out) = stream.into_split();

    out.write_all(&codec::preamble()[..]).await?;

    let report = |line: Bytes| async {
        let mesg = codec::decode(line);
        println!("> {mesg:?}");
    };

    busio::read_lines(inp, report).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting at {:?}", Instant::now());

    loop {
        println!("Connecting...");
        let res = cbus_session().await;
        println!("{res:?}");
        sleep(Duration::from_millis(2000)).await;
    }
}
