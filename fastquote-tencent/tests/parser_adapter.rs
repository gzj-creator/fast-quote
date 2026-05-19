use fastquote_core::{MarketData, Source, Symbol};
use fastquote_tencent::parser::parse_depth_quote;
use fastquote_tencent::{TencentAdapter, TencentConfig};
use tokio::sync::mpsc;

fn quote_line() -> String {
    let mut fields = vec!["0"; 50];
    fields[0] = "1";
    fields[1] = "浦发银行";
    fields[2] = "600000";
    fields[3] = "12.34";
    fields[4] = "12.21";
    fields[5] = "12.30";
    fields[6] = "1000";
    fields[9] = "12.33";
    fields[10] = "500";
    fields[11] = "12.32";
    fields[12] = "400";
    fields[19] = "12.35";
    fields[20] = "300";
    fields[21] = "12.36";
    fields[22] = "200";
    fields[37] = "12345678";

    format!("v_sh600000=\"{}\";", fields.join("~"))
}

#[test]
fn parse_depth_quote_extracts_symbol_prices_and_levels() {
    let quotes = parse_depth_quote(&quote_line());

    assert_eq!(quotes.len(), 1);
    let quote = &quotes[0];
    assert_eq!(quote.symbol, Symbol::new("SH", "600000"));
    assert_eq!(quote.last_price.0, 12_340);
    assert_eq!(quote.open.0, 12_210);
    assert_eq!(quote.high.0, 12_300);
    assert_eq!(quote.volume.0, 1000);
    assert_eq!(quote.amount.0, 12_345_678);
    assert_eq!(quote.bid[0].price.0, 12_330);
    assert_eq!(quote.bid[0].volume.0, 500);
    assert_eq!(quote.ask[0].price.0, 12_350);
    assert_eq!(quote.ask[0].volume.0, 300);
}

#[tokio::test]
async fn adapter_sends_parsed_quotes_to_hub_channel() {
    let (tx, mut rx) = mpsc::channel(8);
    let adapter = TencentAdapter::new(TencentConfig::default(), tx);

    adapter.handle_quote_text(&quote_line()).await.unwrap();

    let msg = rx.recv().await.unwrap();
    assert_eq!(msg.source, Source::Tencent);
    match msg.data {
        MarketData::Depth(depth) => assert_eq!(depth.symbol, Symbol::new("SH", "600000")),
        other => panic!("expected depth message, got {other:?}"),
    }
}
