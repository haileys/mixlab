use std::cmp;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Poll, Context};

use futures::StreamExt;
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, Receiver, Sender};

pub async fn start(addr: SocketAddr) -> Result<Receiver<Disambiguation>, io::Error> {
    let mut listener = TcpListener::bind(&addr).await?;

    let (mut result_tx, result_rx) = mpsc::channel::<Disambiguation>(1);
    let (disambiguated_tx, mut disambiguated_rx) = mpsc::channel::<Disambiguation>(1);

    tokio::spawn(async move {
        let mut incoming = listener.incoming();

        loop {
            tokio::select! {
                conn = incoming.next() => {
                    match conn {
                        Some(Ok(conn)) => {
                            handle_connection(conn, disambiguated_tx.clone());
                        }
                        None | Some(Err(_)) => break,
                    }
                }
                conn = disambiguated_rx.recv() => {
                    let conn = match conn {
                        Some(conn) => conn,
                        None => break,
                    };

                    match result_tx.send(conn).await {
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
            }
        }
    });

    Ok(result_rx)
}

fn handle_connection(conn: TcpStream, mut out: Sender<Disambiguation>) {
    if conn.set_nodelay(true).is_err() {
        // nothing to do
        return;
    }

    tokio::spawn(async move {
        match disambiguate(conn).await {
            Ok(conn) => {
                let _ = out.send(conn);
            }
            Err(e) => {
                eprintln!("listen: disambiguation error: {:?}", e);
            }
        }
    });
}

pub enum Disambiguation {
    Http(PeekTcpStream),
    Icecast(PeekTcpStream),
}

pub async fn disambiguate(stream: TcpStream)
    -> Result<Disambiguation, io::Error>
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
