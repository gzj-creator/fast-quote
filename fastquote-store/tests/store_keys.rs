use fastquote_core::{Amount, DepthMarket, IndexQuote, Kline, Level, Price, Symbol, Time, Volume};
use fastquote_store::redis_store::{depth_key, index_key, kline_key};

fn depth() -> DepthMarket {
    DepthMarket {
        symbol: Symbol::new("SH", "600000"),
        time: Time(1),
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
fn redis_keys_include_market_code_and_period() {
    let depth = depth();
    let kline = Kline {
        symbol: depth.symbol.clone(),
        open_time: Time(60),
        open: Price(1),
        high: Price(2),
        low: Price(1),
        close: Price(2),
        volume: Volume(10),
        amount: Amount(100),
        period: 60,
    };
    let index = IndexQuote {
        symbol: Symbol::new("SH", "000001"),
        time: Time(1),
        last_price: Price(3000_000),
        volume: Volume(1),
        amount: Amount(2),
        change_pct: 0.01,
    };

    assert_eq!(depth_key(&depth.symbol), "quote:depth:SH600000");
    assert_eq!(kline_key(&kline.symbol, kline.period), "quote:kline:SH600000:60");
    assert_eq!(index_key(&index.symbol), "quote:index:SH000001");
}
