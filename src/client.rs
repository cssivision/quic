use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use futures::future::poll_fn;
use ring::rand::*;
use tokio::net::UdpSocket;
use tokio::time::Sleep;

use super::other;

const MAX_DATAGRAM_SIZE: usize = 1350;

pub struct Connection {
    io: UdpSocket,
    conn: Pin<Box<quiche::Connection>>,
    recv_buf: [u8; 65535],
    send_buf: [u8; MAX_DATAGRAM_SIZE],
    sleep: Option<Pin<Box<Sleep>>>,
}

pub async fn handshake(
    io: UdpSocket,
    server_name: Option<&str>,
    mut config: quiche::Config,
) -> io::Result<Connection> {
    let scid = generate_scid()?;
    let scid = quiche::ConnectionId::from_ref(&scid);

    let mut conn =
        quiche::connect(server_name, &scid, &mut config).map_err(|e| other(&e.to_string()))?;

    let mut send_buf = [0u8; MAX_DATAGRAM_SIZE];
    let write = conn
        .send(&mut send_buf)
        .map_err(|e| other(&e.to_string()))?;

    log::info!(
        "connecting from {:} with scid {:?}",
        io.local_addr().unwrap(),
        scid,
    );

    let written = io.send(&send_buf[0..write]).await?;
    log::info!("written len: {}", written);
    let recv_buf = [0u8; 65535];

    if let Some(timeout) = conn.timeout() {}

    Ok(Connection {
        io,
        conn,
        send_buf,
        recv_buf,
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

impl Connection {
    fn poll(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

pub struct Stream {}
