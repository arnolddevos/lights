use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

mod busio;
mod codec;

const HOST: &str = "C228F35.gracelands";
const PORT: u16 = 10001;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to a CBUS device
    let mut stream = TcpStream::connect((HOST, PORT)).await?;

    let (mut inp, mut out) = stream.split();

    // Write some data.
    out.write_all(b"hello world!").await?;

    Ok(())
}
