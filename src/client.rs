use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use crate::frame::IoConnection;

use futures::future::poll_fn;
use parking_lot::Mutex;
use ring::rand::*;
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

    Ok(Connection {
        io,
        conn,
        sleep: None,
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
    io: UdpSocket,
    conn: Pin<Box<quiche::Connection>>,
    sleep: Option<Pin<Box<Sleep>>>,
    actions: Arc<Mutex<Action>>,
}

struct Action {
    waker: Option<Waker>,
}

impl Connection {
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

    fn poll_timeout(&mut self, cx: &mut Context) {}

    fn poll_send(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        let write = match self.conn.send(&mut self.send_buf) {
            Ok(v) => v,
            Err(quiche::Error::Done) => {
                // Done writing.
            }
            Err(e) => {
                // An error occurred, handle it.
            }
        };

        self.io.poll_send(&self.send_buf[..write]);
        Poll::Ready(Ok(()))
    }

    fn poll_recv(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        let mut sink = Pin::new(&mut self.transport);
        let mut actions = self.actions.lock();

        let mut try_send = false;

        while let Ok(res) = self.buffer.pop() {
            match sink.as_mut().poll_ready(cx)? {
                Poll::Ready(()) => {
                    try_send = true;
                    sink.as_mut().start_send(res)?;
                }
                Poll::Pending => {
                    // If we've got an item already, we need to write it back to the buffer
                    let _ = self.buffer.push(res);
                    if try_send {
                        ready!(sink.poll_flush(cx))?;
                    }
                    return Poll::Pending;
                }
            }
        }

        if try_send {
            ready!(sink.poll_flush(cx))?;
        }

        actions.waker = Some(cx.waker().clone());

        Poll::Ready(Ok(()))
    }
}

pub struct Stream {}
