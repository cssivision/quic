use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use crate::io_connection::IoConnection;

use bytes::{Buf, BufMut, BytesMut};
use futures::{future::poll_fn, Sink, Stream};
use parking_lot::Mutex;
use ring::rand::{SecureRandom, SystemRandom};
use tokio::net::UdpSocket;
use tokio::time::Sleep;

use super::other;

const MAX_DATAGRAM_SIZE: usize = 1350;

pub async fn new_connection(
    io: UdpSocket,
    server_name: Option<&str>,
    mut config: quiche::Config,
) -> io::Result<Connection> {
    let scid = generate_scid()?;
    let scid = quiche::ConnectionId::from_ref(&scid);

    let conn =
        quiche::connect(server_name, &scid, &mut config).map_err(|e| other(&e.to_string()))?;

    let action = Action { waker: None };
    Ok(Connection {
        conn,
        io: IoConnection::new(io),
        sleep: None,
        action: Arc::new(Mutex::new(action)),
        send_buf: BytesMut::with_capacity(MAX_DATAGRAM_SIZE),
        recv_buf: BytesMut::with_capacity(65535),
    })
}

fn generate_scid() -> io::Result<[u8; quiche::MAX_CONN_ID_LEN]> {
    let mut scid = [0; quiche::MAX_CONN_ID_LEN];

    SystemRandom::new()
        .fill(&mut scid[..])
        .map_err(|e| other(&e.to_string()))?;

    Ok(scid)
}

impl Future for Connection {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.get_mut().poll(cx)
    }
}

pub struct Connection {
    conn: Pin<Box<quiche::Connection>>,
    io: IoConnection,
    sleep: Option<Pin<Box<Sleep>>>,
    action: Arc<Mutex<Action>>,
    send_buf: BytesMut,
    recv_buf: BytesMut,
}

struct Action {
    waker: Option<Waker>,
}

impl Connection {
    async fn main_loop(&mut self) {
        poll_fn(|cx| {
            match self.poll_recv(cx) {
                Poll::Ready(Err(e)) => {}
                Poll::Ready(Ok(_)) => panic!("unexpeted branch"),
                Poll::Pending => if let Err(e) = ready!(self.poll_send(cx)) {},
            }

            Poll::Pending
        })
        .await
    }

    fn poll(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        match ready!(self.poll_send(cx)) {
            Ok(()) => match ready!(self.poll_recv(cx)) {
                Ok(()) => Poll::Ready(Ok(())),
                Err(e) => {
                    log::error!("poll_recv error: {:?}", e);
                    Poll::Ready(Err(e))
                }
            },
            Err(e) => {
                log::error!("poll_send error: {:?}", e);
                Poll::Ready(Err(e))
            }
        }
    }

    fn poll_timeout(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        if let Some(mut sleep) = self.sleep.as_mut() {
            match Pin::new(&mut sleep).poll(cx) {
                Poll::Pending => {}
                Poll::Ready(_) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "connection timeout",
                    )));
                }
            }
        }

        Poll::Ready(Ok(()))
    }

    fn poll_send(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        let mut sink = Pin::new(&mut self.io);
        let mut action = self.action.lock();
        let mut try_send = false;

        loop {
            let send_buf = if self.send_buf.is_empty() {
                self.conn.send(&mut self.send_buf)
            } else {
                Ok(self.send_buf.len())
            };

            match send_buf {
                Ok(v) => match sink.as_mut().poll_ready(cx)? {
                    Poll::Ready(()) => {
                        try_send = true;
                        sink.as_mut().start_send(self.send_buf.split_to(v))?;
                        self.send_buf.clear();
                    }
                    Poll::Pending => {
                        if try_send {
                            ready!(sink.poll_flush(cx))?;
                        }
                        return Poll::Pending;
                    }
                },
                Err(quiche::Error::Done) => {
                    break;
                }
                Err(e) => {
                    log::error!("quiche conn send error: {:?}", e);
                    return Poll::Ready(Err(other(&e.to_string())));
                }
            }
        }

        if try_send {
            ready!(sink.poll_flush(cx))?;
        }
        action.waker = Some(cx.waker().clone());
        Poll::Ready(Ok(()))
    }

    fn poll_recv(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        loop {
            match ready!(Pin::new(&mut self.io).poll_next(cx)) {
                Some(v) => match v {
                    Ok(v) => {
                        self.recv_buf.put(v);

                        while !self.recv_buf.is_empty() {
                            let length = self.recv_buf.len();

                            let read = match self.conn.recv(&mut self.recv_buf[..length]) {
                                Ok(v) => v,

                                Err(e) => {
                                    // An error occurred, handle it.
                                    log::error!("quiche recv err: {:?}", e);
                                    break;
                                }
                            };

                            self.recv_buf.advance(read);
                        }
                    }
                    Err(e) => {
                        log::error!("poll_recv err: {}", e.to_string());
                        return Poll::Ready(Err(e));
                    }
                },
                None => {
                    log::error!("poll_recv recv eof");
                    let e = io::Error::new(io::ErrorKind::UnexpectedEof, "eof");
                    return Poll::Ready(Err(e));
                }
            }
        }
    }
}
