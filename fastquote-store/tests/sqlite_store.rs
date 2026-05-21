use fastquote_core::{Amount, DepthMarket, IndexQuote, Kline, Level, Price, Symbol, Time, Volume};
use fastquote_store::OrmStore;
use std::{env, fs, process, time::SystemTime};

fn depth() -> DepthMarket {
    DepthMarket {
        symbol: Symbol::new("SH", "600000"),
        time: Time(1_000),
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

#[tokio::test]
async fn sqlite_store_creates_schema_and_persists_market_data() {
    let store = OrmStore::new("sqlite::memory:").await.unwrap();
    let depth = depth();
    let kline = Kline {
        symbol: depth.symbol.clone(),
        open_time: Time(60_000),
        open: Price(12_000),
        high: Price(12_500),
        low: Price(11_900),
        close: Price(12_340),
        volume: Volume(1200),
        amount: Amount(1_200_000),
        period: 60,
    };
    let index = IndexQuote {
        symbol: Symbol::new("SH", "000001"),
        time: Time(2_000),
        last_price: Price(3000_000),
        volume: Volume(1),
        amount: Amount(2),
        change_pct: 0.01,
    };

    store.write_depth("Tencent", &depth).await.unwrap();
    store.write_kline("Tencent", &kline).await.unwrap();
    store.write_index("Tencent", &index).await.unwrap();

    assert_eq!(store.count_rows("depth_tick").await.unwrap(), 1);
    assert_eq!(store.count_rows("kline").await.unwrap(), 1);
    assert_eq!(store.count_rows("index_quote").await.unwrap(), 1);
}

#[tokio::test]
async fn sqlite_store_upserts_duplicate_klines() {
    let store = OrmStore::new("sqlite::memory:").await.unwrap();
    let kline = Kline {
        symbol: Symbol::new("SH", "600000"),
        open_time: Time(60_000),
        open: Price(12_000),
        high: Price(12_500),
        low: Price(11_900),
        close: Price(12_340),
        volume: Volume(1200),
        amount: Amount(1_200_000),
        period: 60,
    };

    store.write_kline("Tencent", &kline).await.unwrap();
    store.write_kline("Tencent", &kline).await.unwrap();

    assert_eq!(store.count_rows("kline").await.unwrap(), 1);
}

#[tokio::test]
async fn sqlite_store_creates_missing_database_file() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = env::temp_dir().join(format!(
        "fastquote-store-sqlite-file-{}-{unique}",
        process::id()
    ));
    let db_path = base.join("nested").join("fastquote.db");
    let url = format!("sqlite://{}", db_path.display());

    assert!(!db_path.exists());
    assert!(!db_path.parent().unwrap().exists());

    let store = OrmStore::new(&url).await.unwrap();

    assert_eq!(store.count_rows("depth_tick").await.unwrap(), 0);
    assert!(db_path.exists());

    fs::remove_dir_all(base).unwrap();
}
