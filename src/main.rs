use bytes::{Buf, BufMut, Bytes, BytesMut};
use nom::character::streaming::{line_ending, not_line_ending};
use nom::sequence::pair;
use nom::IResult;
use std::future::Future;
use std::io::{Error, ErrorKind};
use std::ops::BitOr;
use tokio::net::TcpStream;
use tokio::{
    io,
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
};

const HOST: &str = "C228F35.gracelands";
const PORT: u16 = 10001;

struct Setting(u8);

// Options 1
const CONNECT: Setting = Setting(1 << 0);
const SR_CHK: Setting = Setting(1 << 3);
const SMART: Setting = Setting(1 << 4);
const MONITOR: Setting = Setting(1 << 5);
const ID_MON: Setting = Setting(1 << 6);

// Options 3
const PARAM_CHANGE_NOTIFY: Setting = Setting(1 << 0);
const LOCAL_SAL: Setting = Setting(1 << 1);
const POWER_UP_NOTIFY: Setting = Setting(1 << 2);
const EX_STAT: Setting = Setting(1 << 3);

// Options can be combined
impl BitOr for Setting {
    type Output = Setting;

    fn bitor(self, rhs: Self) -> Self::Output {
        Setting(self.0 | rhs.0)
    }
}

struct Param(u8);
const APPLICATION1: Param = Param(0x21);
const APPLICATION2: Param = Param(0x22);
const OPTIONS1: Param = Param(0x30);
const OPTIONS1_NV: Param = Param(0x41);
const OPTIONS3: Param = Param(0x42);

struct Level(u8);
const ON: Level = Level(0xff);
const OFF: Level = Level(0x0);

struct Group(u8);
struct Rate(u8);

struct SetParam(Param, Setting);
struct SetVar(Group, Level, Rate);
struct Reset;
struct StopRamp(Group);

struct Message(String);

impl From<Reset> for Message {
    fn from(_: Reset) -> Self {
        Message("~".into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to a CBUS device
    let mut stream = TcpStream::connect((HOST, PORT)).await?;

    let (inp, out) = stream.split();

    // Write some data.
    stream.write_all(b"hello world!").await?;

    Ok(())
}

const LINE_LEN: usize = 1024;
const CHUNK_LEN: usize = 4096;

/// Read available input up to CHUNK_LEN bytes and append it to a buffer
async fn read_more<I>(inp: &mut I, buf: &mut BytesMut) -> io::Result<()>
where
    I: AsyncRead + Unpin,
{
    let start = buf.len();
    buf.resize(start + CHUNK_LEN, 0); // could use set_len() to save initialisation
    let fresh = inp.read(&mut buf[start..]).await?;
    buf.truncate(start + fresh);
    if fresh > 0 {
        Ok(())
    } else {
        Err(Error::from(ErrorKind::UnexpectedEof))
    }
}

/// Split a line from the buffer, if possible.
fn split_line(buf: &mut BytesMut) -> Option<Bytes> {
    let p: IResult<&[u8], _> = pair(not_line_ending, line_ending)(buf);
    match p {
        Ok((_, (nle, le))) => {
            let n = nle.len();
            let m = le.len();
            let line = buf.split_to(n);
            drop(buf.split_to(m));
            Some(line.freeze())
        }
        Err(_) => None,
    }
}

/// Drop all input up to and including the next line end.
async fn drop_long_line<I>(inp: &mut I, buf: &mut BytesMut) -> io::Result<()>
where
    I: AsyncRead + Unpin,
{
    loop {
        buf.clear();
        read_more(inp, buf).await?;
        if let Some(line) = split_line(buf) {
            drop(line);
            break Ok(());
        }
    }
}

/// Emit the complete lines currently in the buffer.
async fn emit_lines<O, F>(buf: &mut BytesMut, out: O)
where
    O: Fn(Bytes) -> F,
    F: Future,
{
    while let Some(line) = split_line(buf) {
        out(line).await;
    }
}

/// Read input as available or CHUNK_LEN bytes at a time and
/// pass it line by line to a function or closure.
/// Ignore any line longer than LINE_LEN without buffering it.
async fn read_lines<I, O, F>(mut inp: I, out: O) -> io::Result<()>
where
    I: AsyncRead + Unpin,
    O: Fn(Bytes) -> F,
    F: Future,
{
    let mut buf = BytesMut::with_capacity(LINE_LEN + CHUNK_LEN);

    // each iteration reads a chunk containing 0 or more lines and 0 or 1 partial line.
    // (if input arrives in small pieces a partial line maybe repeatedly scanned).
    loop {
        read_more(&mut inp, &mut buf).await?;
        emit_lines(&mut buf, &out).await;

        // remainder in buf is a partial line longer than the limit
        while buf.len() > LINE_LEN {
            drop_long_line(&mut inp, &mut buf).await?;
            emit_lines(&mut buf, &out).await;
        }
    }
}
