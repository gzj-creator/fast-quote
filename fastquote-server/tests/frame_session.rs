use fastquote_core::{Amount, DepthMarket, Level, Price, Symbol, Time, Volume};
use fastquote_server::tcp::frame::{self, msg_type};
use fastquote_server::tcp::session::TcpSession;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

fn depth() -> DepthMarket {
    DepthMarket {
        symbol: Symbol::new("SH", "600000"),
        time: Time(123),
        last_price: Price(12_340),
        open: Price(12_000),
        high: Price(12_500),
        low: Price(11_900),
        close: Price(12_100),
        volume: Volume(1000),
        amount: Amount(1_000_000),
        bid: [Level {
            price: Price(12_330),
            volume: Volume(500),
        }; 5],
        ask: [Level {
            price: Price(12_350),
            volume: Volume(300),
        }; 5],
    }
}

#[test]
fn frame_header_round_trips_message_type_and_payload_len() {
    let frame = frame::encode_frame(msg_type::PING, &[1, 2, 3]);

    let (msg_type, payload_len) = frame::decode_header(&frame).unwrap();

    assert_eq!(msg_type, frame::msg_type::PING);
    assert_eq!(payload_len, 3);
    assert_eq!(&frame[frame::HEADER_LEN..], &[1, 2, 3]);
}

#[test]
fn depth_payload_encodes_symbol_and_numeric_fields() {
    let payload = frame::encode_depth(&depth());

    assert_eq!(&payload[0..2], b"SH");
    assert_eq!(&payload[2..8], b"600000");
    assert_eq!(i64::from_be_bytes(payload[8..16].try_into().unwrap()), 123);
    assert_eq!(
        i64::from_be_bytes(payload[16..24].try_into().unwrap()),
        12_340
    );
}

#[tokio::test]
async fn tcp_session_replies_to_ping() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (_tx, rx) = broadcast::channel(8);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        TcpSession::new(stream, rx).run().await;
    });

    let mut client = TcpStream::connect(addr).await.unwrap();
    client
        .write_all(&frame::encode_frame(msg_type::PING, &[]))
        .await
        .unwrap();

    let mut header = [0u8; frame::HEADER_LEN];
    client.read_exact(&mut header).await.unwrap();
    let (msg_type, payload_len) = frame::decode_header(&header).unwrap();

    assert_eq!(msg_type, frame::msg_type::PONG);
    assert_eq!(payload_len, 0);
}
