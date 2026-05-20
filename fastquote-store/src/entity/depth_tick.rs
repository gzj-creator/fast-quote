use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "depth_tick")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub source: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub market: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub code: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub time_ns: i64,
    pub last_price: i64,
    pub open: i64,
    pub high: i64,
    pub low: i64,
    pub close: i64,
    pub volume: i64,
    pub amount: i64,
    pub bid_json: String,
    pub ask_json: String,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
