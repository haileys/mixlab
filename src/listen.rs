use std::cmp;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Poll, Context};

use futures::stream::{self, StreamExt};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Receiver, Sender};

pub struct Listener {
    pub local_addr: SocketAddr,
    pub incoming: Receiver<Disambiguation>,
}

pub async fn start(addr: SocketAddr) -> Result<Listener, io::Error> {
    let mut listener = TcpListener::bind(&addr).await?;
    let local_addr = listener.local_addr()?;

    let (mut result_tx, result_rx) = mpsc::channel::<Disambiguation>(1);
    let (disambiguated_tx, disambiguated_rx) = mpsc::channel::<Disambiguation>(1);

    tokio::spawn(async move {
        enum Event {
            Listener(Result<TcpStream, io::Error>),
            Disambiguate(Disambiguation),
        }

        let mut events = stream::select(
            listener.incoming().map(Event::Listener),
            disambiguated_rx.map(Event::Disambiguate),
        );

        while let Some(event) = events.next().await {
            match event {
                Event::Listener(Ok(conn)) => {
                    handle_connection(conn, disambiguated_tx.clone());
                }
                Event::Listener(Err(e)) => {
                    eprintln!("listen: {:?}", e);
                    break;
                }
                Event::Disambiguate(disambiguated) => {
                    match result_tx.send(disambiguated).await {
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
            }
        }
    });

    Ok(Listener {
        local_addr,
        incoming: result_rx,
    })
}

fn handle_connection(conn: TcpStream, mut out: Sender<Disambiguation>) {
    if let Err(e) = conn.set_nodelay(true) {
        eprintln!("listen: set_nodelay: {:?}", e);
        return;
    }

    tokio::spawn(async move {
        match disambiguate(conn).await {
            Ok(conn) => {
                let _: Result<_, _> = out.send(conn).await;
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                // ignore
            }
            Err(e) => {
                eprintln!("listen: error receiving new connection: {:?}", e);
            }
        }
    });
}

#[derive(Debug)]
pub enum Disambiguation {
    Http(PeekTcpStream),
    Icecast(PeekTcpStream),
    Rtmp(PeekTcpStream),
}

pub async fn disambiguate(stream: TcpStream)
    -> Result<Disambiguation, io::Error>
{
    let stream = PeekTcpStream::new(stream).await?;

    match stream.peek() {
        b"SOURCE " => Ok(Disambiguation::Icecast(stream)),
        [0x03, ..] => Ok(Disambiguation::Rtmp(stream)),
        _ => Ok(Disambiguation::Http(stream)),
    }
}

#[derive(Debug)]
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
