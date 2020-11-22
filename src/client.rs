use std::io;
use std::pin::Pin;

use ring::rand::*;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use super::other;

const MAX_DATAGRAM_SIZE: usize = 1350;

pub struct Connection<T> {
    io: T,
    conn: Pin<Box<quiche::Connection>>,
    buf: [u8; MAX_DATAGRAM_SIZE],
}

pub async fn handshake<T>(
    mut io: T,
    server_name: Option<&str>,
    mut config: quiche::Config,
) -> io::Result<Connection<T>>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let scid = generate_scid()?;

    let mut conn =
        quiche::connect(server_name, &scid, &mut config).map_err(|e| other(&e.to_string()))?;

    let mut buf = [0u8; MAX_DATAGRAM_SIZE];
    let write = conn.send(&mut buf).map_err(|e| other(&e.to_string()))?;

    io.write_all(&mut buf[0..write]).await?;

    Ok(Connection { io, conn, buf })
}

fn generate_scid() -> io::Result<[u8; quiche::MAX_CONN_ID_LEN]> {
    let mut scid = [0; quiche::MAX_CONN_ID_LEN];

    SystemRandom::new()
        .fill(&mut scid[..])
        .map_err(|e| other(&e.to_string()))?;

    Ok(scid)
}
