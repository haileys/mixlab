use std::cmp;
use std::str;
use std::pin::Pin;
use std::task::{Poll, Context};

use derive_more::From;
use httparse::Request;
use tokio::net::TcpStream;
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite};

pub enum Disambiguation {
    Http(PeekTcpStream),
    Icecast(PeekTcpStream),
}

pub async fn disambiguate(stream: TcpStream)
    -> io::Result<Disambiguation>
{
    let stream = PeekTcpStream::new(stream).await?;

    if stream.peek() == b"SOURCE " {
        Ok(Disambiguation::Icecast(stream))
    } else {
        Ok(Disambiguation::Http(stream))
    }
}

pub struct PeekTcpStream {
    peek: [u8; 7],
    offset: u8,
    conn: TcpStream,
}

impl PeekTcpStream {
    pub async fn new(conn: TcpStream) -> Result<Self, io::Error> {
        let mut stream = PeekTcpStream {
            peek: [0; 7],
            offset: 0,
            conn: conn,
        };

        stream.conn.read_exact(&mut stream.peek).await?;

        Ok(stream)
    }

    fn peek(&self) -> &[u8] {
        &self.peek[self.offset as usize..]
    }
}

impl AsyncRead for PeekTcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<Result<usize, io::Error>> {
        // Safety: we only access fields of self and do not let refs escape this function
        let mut stream = unsafe { self.get_unchecked_mut() };

        let offset = stream.offset as usize;
        let remaining = stream.peek.len() - offset;

        if remaining > 0 {
            let advanced = cmp::min(buf.len(), remaining);
            buf[0..advanced].copy_from_slice(&stream.peek[offset..(offset + advanced)]);
            stream.offset += advanced as u8;
            Poll::Ready(Ok(advanced))
        } else {
            Pin::new(&mut stream.conn).poll_read(cx, buf)
        }
    }
}

impl AsyncWrite for PeekTcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        unsafe { self.map_unchecked_mut(|stream| &mut stream.conn) }.poll_write(cx, buf)
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        unsafe { self.map_unchecked_mut(|stream| &mut stream.conn) }.poll_flush(cx)
    }

    fn poll_shutdown(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        unsafe { self.map_unchecked_mut(|stream| &mut stream.conn) }.poll_shutdown(cx)
    }
}

#[derive(Debug, From)]
pub enum Error {
    Io(tokio::io::Error),
    Http(httparse::Error),
    HeadersTooLong,
    NoPath,
    NoContentType,
}

#[derive(Debug)]
pub enum ContentType {
    Ogg,
}

#[derive(Debug)]
pub struct RequestInfo {
    pub path: String,
    pub content_type: Option<ContentType>,
    pub stream_data: Vec<u8>,
}

pub async fn parse(stream: &mut PeekTcpStream) -> Result<RequestInfo, Error> {
    let mut buff = [0u8; 4096];
    let mut buff_offset = 0;

    loop {
        if buff_offset == buff.len() {
            return Err(Error::HeadersTooLong);
        }

        buff_offset += stream.read(&mut buff[buff_offset..]).await
            .map_err(Error::Io)?;

        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut request = Request::new(&mut headers);

        match request.parse(&buff[0..buff_offset])? {
            httparse::Status::Partial => {}
            httparse::Status::Complete(len) => {
                return on_complete(&request, &buff[len..buff_offset]);
            }
        }
    }

    fn on_complete(request: &Request, stream_data: &[u8]) -> Result<RequestInfo, Error> {
        let content_type_hdr = request.headers.iter()
            .find(|header| header.name.eq_ignore_ascii_case("content-type"))
            .and_then(|header| str::from_utf8(header.value).ok())
            .ok_or(Error::NoContentType)?;

        let content_type = match content_type_hdr {
            "application/ogg" | "audio/ogg" => Some(ContentType::Ogg),
            _ => None,
        };

        Ok(RequestInfo {
            path: request.path.ok_or(Error::NoPath)?.to_owned(),
            content_type,
            stream_data: stream_data.to_vec(),
        })
    }
}

