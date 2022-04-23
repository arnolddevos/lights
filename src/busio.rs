use bytes::{Bytes, BytesMut};
use nom::character::streaming::{line_ending, not_line_ending};
use nom::sequence::pair;
use nom::IResult;
use std::future::Future;
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use tokio::io::{self, AsyncRead, AsyncReadExt};
use tokio::sync::mpsc::Sender;

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
async fn emit_lines<O, F>(buf: &mut BytesMut, out: &mut O)
where
    O: FnMut(Bytes) -> F,
    F: Future<Output = ()>,
{
    while let Some(line) = split_line(buf) {
        if line.len() <= LINE_LEN {
            out(line).await;
        }
    }
}

/// Continuously read lines from a stream.
///
/// Reads CHUNK_LEN bytes at a time or as available and
/// passes it line by line to a function or closure.
///
/// Ignores any line longer than LINE_LEN.
/// The buffer space requirement is not affected by long lines.
///
/// Never returns normally (result type could be `!`).  
/// If the end of the stream is reached an `UnexpectedEof`
/// error is returned.
///
pub async fn read_lines<I, O, F>(mut inp: I, mut out: O) -> io::Result<()>
where
    I: AsyncRead + Unpin,
    O: FnMut(Bytes) -> F,
    F: Future<Output = ()>,
{
    let mut buf = BytesMut::with_capacity(LINE_LEN + CHUNK_LEN);

    // each iteration reads a chunk containing 0 or more lines and 0 or 1 partial line.
    // (if input arrives in small pieces a partial line maybe repeatedly scanned).
    loop {
        read_more(&mut inp, &mut buf).await?;
        emit_lines(&mut buf, &mut out).await;

        // remainder in buf is a partial line longer than the limit
        while buf.len() > LINE_LEN {
            drop_long_line(&mut inp, &mut buf).await?;
            emit_lines(&mut buf, &mut out).await;
        }
    }
}

/// Returns an async function that forwards values to an mpsc queue.   
///
/// Example: arrange for lines of input to be sent to a queue:
/// ```
/// let (rx, _) = tcp_stream.split();
/// let (tx, _) = mpsc::channel(100);
/// read_lines(rx, forward_to(tx))
/// ```
pub fn forward_to<'a, T>(s: &'a Sender<T>) -> impl Fn(T) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
    |value| {
        Box::pin(async {
            let _ = s.send(value).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::read_lines;
    use std::io::Cursor;

    #[tokio::test]
    async fn example() {
        let inp = Cursor::new(b"hello\nworld\nover");
        let res = read_lines(inp, |bytes| async move {
            println!("> {:?}", bytes);
        })
        .await;
        println!("{:?}", res)
    }
}
