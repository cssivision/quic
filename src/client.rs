use std::io;

use crate::connection::{Connection, Streams};
use crate::other;

use ring::rand::{SecureRandom, SystemRandom};
use tokio::net::UdpSocket;

fn generate_scid() -> io::Result<[u8; quiche::MAX_CONN_ID_LEN]> {
    let mut scid = [0; quiche::MAX_CONN_ID_LEN];

    SystemRandom::new()
        .fill(&mut scid[..])
        .map_err(|e| other(&e.to_string()))?;

    Ok(scid)
}

pub fn connect(
    io: UdpSocket,
    server_name: Option<&str>,
    mut config: quiche::Config,
) -> io::Result<(Connection, Streams)> {
    let scid = generate_scid()?;
    let scid = quiche::ConnectionId::from_ref(&scid);

    let conn =
        quiche::connect(server_name, &scid, &mut config).map_err(|e| other(&e.to_string()))?;

    Ok(Connection::new(io, conn))
}
