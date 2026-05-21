use crate::entity::{depth_tick, index_quote, kline as kline_entity};
use anyhow::Result;
use fastquote_core::{DepthMarket, IndexQuote, Kline};
use sea_orm::{
    sea_query::{Alias, OnConflict},
    ActiveValue::NotSet,
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait, Iden,
    PaginatorTrait, Schema, Set, StatementBuilder,
};
use std::path::Path;

pub struct OrmStore {
    db: DatabaseConnection,
}

pub type SqliteStore = OrmStore;

impl OrmStore {
    pub async fn new(url: &str) -> Result<Self> {
        ensure_parent_dir(url)?;
        let db = Database::connect(connect_options(url)).await?;
        init_schema(&db).await?;
        Ok(Self { db })
    }

    pub async fn write_depth(&self, source: &str, depth: &DepthMarket) -> Result<()> {
        depth_tick::Entity::insert(depth_tick::ActiveModel {
            source: Set(source.to_owned()),
            market: Set(depth.symbol.market.clone()),
            code: Set(depth.symbol.code.clone()),
            time_ns: Set(depth.time.0),
            last_price: Set(depth.last_price.0),
            open: Set(depth.open.0),
            high: Set(depth.high.0),
            low: Set(depth.low.0),
            close: Set(depth.close.0),
            volume: Set(depth.volume.0),
            amount: Set(depth.amount.0),
            bid_json: Set(serde_json::to_string(&depth.bid)?),
            ask_json: Set(serde_json::to_string(&depth.ask)?),
            created_at: NotSet,
        })
        .on_conflict(
            OnConflict::columns([
                depth_tick::Column::Source,
                depth_tick::Column::Market,
                depth_tick::Column::Code,
                depth_tick::Column::TimeNs,
            ])
            .update_columns([
                depth_tick::Column::LastPrice,
                depth_tick::Column::Open,
                depth_tick::Column::High,
                depth_tick::Column::Low,
                depth_tick::Column::Close,
                depth_tick::Column::Volume,
                depth_tick::Column::Amount,
                depth_tick::Column::BidJson,
                depth_tick::Column::AskJson,
            ])
            .to_owned(),
        )
        .exec(&self.db)
        .await?;
        Ok(())
    }

    pub async fn write_kline(&self, source: &str, kline: &Kline) -> Result<()> {
        kline_entity::Entity::insert(kline_entity::ActiveModel {
            source: Set(source.to_owned()),
            market: Set(kline.symbol.market.clone()),
            code: Set(kline.symbol.code.clone()),
            period: Set(i64::from(kline.period)),
            open_time_ns: Set(kline.open_time.0),
            open: Set(kline.open.0),
            high: Set(kline.high.0),
            low: Set(kline.low.0),
            close: Set(kline.close.0),
            volume: Set(kline.volume.0),
            amount: Set(kline.amount.0),
            created_at: NotSet,
        })
        .on_conflict(
            OnConflict::columns([
                kline_entity::Column::Source,
                kline_entity::Column::Market,
                kline_entity::Column::Code,
                kline_entity::Column::Period,
                kline_entity::Column::OpenTimeNs,
            ])
            .update_columns([
                kline_entity::Column::Open,
                kline_entity::Column::High,
                kline_entity::Column::Low,
                kline_entity::Column::Close,
                kline_entity::Column::Volume,
                kline_entity::Column::Amount,
            ])
            .to_owned(),
        )
        .exec(&self.db)
        .await?;
        Ok(())
    }

    pub async fn write_index(&self, source: &str, index: &IndexQuote) -> Result<()> {
        index_quote::Entity::insert(index_quote::ActiveModel {
            source: Set(source.to_owned()),
            market: Set(index.symbol.market.clone()),
            code: Set(index.symbol.code.clone()),
            time_ns: Set(index.time.0),
            last_price: Set(index.last_price.0),
            volume: Set(index.volume.0),
            amount: Set(index.amount.0),
            change_pct: Set(index.change_pct),
            created_at: NotSet,
        })
        .on_conflict(
            OnConflict::columns([
                index_quote::Column::Source,
                index_quote::Column::Market,
                index_quote::Column::Code,
                index_quote::Column::TimeNs,
            ])
            .update_columns([
                index_quote::Column::LastPrice,
                index_quote::Column::Volume,
                index_quote::Column::Amount,
                index_quote::Column::ChangePct,
            ])
            .to_owned(),
        )
        .exec(&self.db)
        .await?;
        Ok(())
    }

    pub async fn count_rows(&self, table: &str) -> Result<i64> {
        match table {
            "depth_tick" => Ok(depth_tick::Entity::find().count(&self.db).await? as i64),
            "kline" => Ok(kline_entity::Entity::find().count(&self.db).await? as i64),
            "index_quote" => Ok(index_quote::Entity::find().count(&self.db).await? as i64),
            _ => anyhow::bail!("unsupported table {table}"),
        }
    }
}

fn connect_options(url: &str) -> ConnectOptions {
    let mut options = ConnectOptions::new(url.to_owned());
    if url.starts_with("sqlite:") {
        options.map_sqlx_sqlite_opts(|options| options.create_if_missing(true));
    }
    options
}

fn ensure_parent_dir(url: &str) -> Result<()> {
    let Some(path) = url.strip_prefix("sqlite://") else {
        return Ok(());
    };
    if path == ":memory:" {
        return Ok(());
    }
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

async fn init_schema(db: &DatabaseConnection) -> Result<()> {
    let schema = Schema::new(db.get_database_backend());
    create_table(db, schema.create_table_from_entity(depth_tick::Entity)).await?;
    create_table(db, schema.create_table_from_entity(kline_entity::Entity)).await?;
    create_table(db, schema.create_table_from_entity(index_quote::Entity)).await?;
    create_index(
        db,
        "idx_depth_symbol_time",
        "depth_tick",
        [
            depth_tick::Column::Market.to_string(),
            depth_tick::Column::Code.to_string(),
            depth_tick::Column::TimeNs.to_string(),
        ],
    )
    .await?;
    create_index(
        db,
        "idx_kline_symbol_period_time",
        "kline",
        [
            kline_entity::Column::Market.to_string(),
            kline_entity::Column::Code.to_string(),
            kline_entity::Column::Period.to_string(),
            kline_entity::Column::OpenTimeNs.to_string(),
        ],
    )
    .await?;
    create_index(
        db,
        "idx_index_symbol_time",
        "index_quote",
        [
            index_quote::Column::Market.to_string(),
            index_quote::Column::Code.to_string(),
            index_quote::Column::TimeNs.to_string(),
        ],
    )
    .await?;
    Ok(())
}

async fn create_table(
    db: &DatabaseConnection,
    mut stmt: sea_orm::sea_query::TableCreateStatement,
) -> Result<()> {
    stmt.if_not_exists();
    db.execute(db.get_database_backend().build(&stmt)).await?;
    Ok(())
}

async fn create_index<const N: usize>(
    db: &DatabaseConnection,
    name: &str,
    table: &str,
    columns: [String; N],
) -> Result<()> {
    let mut stmt = sea_orm::sea_query::Index::create();
    stmt.name(name).table(Alias::new(table)).if_not_exists();
    for column in columns {
        stmt.col(Alias::new(column));
    }
    db.execute(StatementBuilder::build(&stmt, &db.get_database_backend()))
        .await?;
    Ok(())
}
