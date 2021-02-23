use tokio::net::UdpSocket;

const MAX_DATAGRAM_SIZE: usize = 1350;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
    config.verify_peer(false);

    config
        .set_application_protos(b"\x05hq-29\x05hq-28\x05hq-27\x08http/0.9")
        .unwrap();

    config.set_max_idle_timeout(5000);
    config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_initial_max_data(10_000_000);
    config.set_initial_max_stream_data_bidi_local(1_000_000);
    config.set_initial_max_stream_data_bidi_remote(1_000_000);
    config.set_initial_max_streams_bidi(100);
    config.set_initial_max_streams_uni(100);
    config.set_disable_active_migration(true);

    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect("127.0.0.1:8081").await?;

    let conn = quic::client::handshake(sock, Some("quic.tech"), config);

    Ok(())
}
