/// TDX 包类型
pub mod pkt_type {
    pub const LOGIN_REQ: u16 = 0x01;
    pub const LOGIN_RESP: u16 = 0x02;
    pub const FINANCIAL_RESP: u16 = 0x03;
    pub const FINANCIAL_REQ: u16 = 0x04;
    pub const KLINE_REQ: u16 = 0x05;
    pub const KLINE_RESP: u16 = 0x06;
    pub const QUOTE_SUBSCRIBE: u16 = 0x0C;
    pub const QUOTE_PUSH: u16 = 0x0D;
}

/// TDX 市场代码
pub mod market {
    pub const SZ: u8 = 0;
    pub const SH: u8 = 1;
}

/// TDX K 线周期
pub mod period {
    pub const MIN_5: u8 = 0;
    pub const MIN_15: u8 = 1;
    pub const MIN_30: u8 = 2;
    pub const HOUR_1: u8 = 3;
    pub const DAILY: u8 = 4;
    pub const WEEKLY: u8 = 5;
    pub const MONTHLY: u8 = 6;
    pub const MIN_1: u8 = 8;
}

/// TDX 包头
#[derive(Debug, Clone)]
pub struct TdxHeader {
    pub len: u16,
    pub pkt_type: u16,
}
