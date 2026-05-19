use fastquote_core::{MarketData, Source, Symbol};
use fastquote_tdx::protocol::types::pkt_type;
use fastquote_tdx::{TdxAdapter, TdxConfig, TdxConnection};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

fn checksum(data: &[u8]) -> u16 {
    data.iter().map(|&b| b as u32).sum::<u32>() as u16
}

fn packet(pkt_type: u16, body: &[u8]) -> Vec<u8> {
    let total_len = 2 + body.len() + 2;
    let mut out = Vec::with_capacity(2 + total_len);
    out.extend_from_slice(&(total_len as u16).to_le_bytes());
    out.extend_from_slice(&pkt_type.to_le_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(&checksum(&out[2..]).to_le_bytes());
    out
}

fn quote_push_body() -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&1u16.to_le_bytes());
    body.push(1);
    body.push(0);
    body.extend_from_slice(b"600000");
    body.extend_from_slice(&1u16.to_le_bytes());
    for value in [1000i32, 990, 995, 1005, 980, 1001] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body
}

#[tokio::test]
async fn connection_logs_in_and_returns_body_without_checksum() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let mut header = [0u8; 2];
        socket.read_exact(&mut header).await.unwrap();
        let total_len = u16::from_le_bytes(header) as usize;
        let mut rest = vec![0u8; total_len];
        socket.read_exact(&mut rest).await.unwrap();

        socket
            .write_all(&packet(pkt_type::LOGIN_RESP, &[]))
            .await
            .unwrap();
        socket
            .write_all(&packet(pkt_type::QUOTE_PUSH, &[0xAA, 0xBB]))
            .await
            .unwrap();
    });

    let mut conn = TdxConnection::new(TdxConfig {
        host: addr.ip().to_string(),
        port: addr.port(),
        reconnect_ms: 10,
        heartbeat_s: 60,
    });

    conn.connect_and_login().await.unwrap();
    let (pkt_type, body) = conn.recv_packet().await.unwrap().unwrap();

    assert_eq!(pkt_type, pkt_type::QUOTE_PUSH);
    assert_eq!(body, vec![0xAA, 0xBB]);
}

#[tokio::test]
async fn adapter_decodes_quote_push_into_hub_message() {
    let (tx, mut rx) = mpsc::channel(8);
    let adapter = TdxAdapter::new(TdxConfig::default(), tx);

    adapter
        .handle_packet(pkt_type::QUOTE_PUSH, &quote_push_body())
        .await
        .unwrap();

    let msg = rx.recv().await.unwrap();
    assert_eq!(msg.source, Source::Tdx);
    match msg.data {
        MarketData::Depth(depth) => {
            assert_eq!(depth.symbol, Symbol::new("SH", "600000"));
            assert_eq!(depth.last_price.0, 10_000);
        }
        other => panic!("expected depth message, got {other:?}"),
    }
}
