use fastquote_core::{
    Amount, DepthMarket, HubMessage, Level, MarketData, Price, Source, Symbol, Time, Volume,
};
use fastquote_hub::Hub;
use std::sync::Arc;

#[tokio::test]
async fn hub_broadcasts_depth_messages() {
    let hub = Arc::new(Hub::new(64));
    let mut rx = hub.subscribe();

    let msg = HubMessage {
        source: Source::Tdx,
        data: MarketData::Depth(DepthMarket {
            symbol: Symbol::new("SH", "600000"),
            time: Time::now(),
            last_price: Price(12_340),
            open: Price(12_208),
            high: Price(12_532),
            low: Price(11_832),
            close: Price(11_872),
            volume: Volume(10_012_000),
            amount: Amount(108_200_000),
            bid: [Level {
                price: Price(12_330),
                volume: Volume(500),
            }; 5],
            ask: [Level {
                price: Price(12_350),
                volume: Volume(400),
            }; 5],
        }),
    };

    hub.publish(msg);

    let received = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
        .await
        .expect("timeout")
        .expect("recv error");

    assert_eq!(received.symbol_key(), "SH600000");
}
