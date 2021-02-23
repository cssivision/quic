use std::io;
use std::pin::Pin;
use std::time::Duration;

use ring::rand::*;
use tokio::net::UdpSocket;

use super::other;

const MAX_DATAGRAM_SIZE: usize = 1350;

pub struct Connection {
    io: UdpSocket,
    conn: Pin<Box<quiche::Connection>>,
    buf: [u8; MAX_DATAGRAM_SIZE],
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

    let mut buf = [0u8; MAX_DATAGRAM_SIZE];
    let write = conn.send(&mut buf).map_err(|e| other(&e.to_string()))?;
    log::info!(
        "connecting from {:} with scid {:?}",
        io.local_addr().unwrap(),
        scid,
    );

    let written = io.send(&buf[0..write]).await?;
    log::info!("written len: {}", written);

    conn.timeout();

    /*
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(conn.timeout().unwrap());
        }
    });
    */

    Ok(Connection { io, conn, buf })
}

fn generate_scid() -> io::Result<[u8; quiche::MAX_CONN_ID_LEN]> {
    let mut scid = [0; quiche::MAX_CONN_ID_LEN];

    SystemRandom::new()
        .fill(&mut scid[..])
        .map_err(|e| other(&e.to_string()))?;

    Ok(scid)
}
