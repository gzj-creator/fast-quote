use bytes::{Buf, BufMut, BytesMut};

pub const MAGIC: u16 = 0x4651; // "FQ"
pub const VERSION: u8 = 0x01;
pub const HEADER_LEN: usize = 8;

pub mod msg_type {
    pub const DEPTH: u8 = 0x01;
    pub const KLINE: u8 = 0x02;
    pub const FINANCIAL: u8 = 0x03;
    pub const INDEX: u8 = 0x04;
    pub const BATCH_DEPTH: u8 = 0x05;
    pub const PONG: u8 = 0x21;
    pub const ERROR: u8 = 0x7F;

    pub const SUB: u8 = 0x10;
    pub const UNSUB: u8 = 0x11;
    pub const REQ_KLINE: u8 = 0x12;
    pub const PING: u8 = 0x20;
}

pub fn encode_frame(msg_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(HEADER_LEN + payload.len());
    buf.put_u16(MAGIC);
    buf.put_u8(VERSION);
    buf.put_u8(msg_type);
    buf.put_u32(payload.len() as u32);
    buf.put_slice(payload);
    buf.to_vec()
}

pub fn decode_header(data: &[u8]) -> Option<(u8, u32)> {
    if data.len() < HEADER_LEN {
        return None;
    }

    let mut buf = &data[..HEADER_LEN];
    if buf.get_u16() != MAGIC {
        return None;
    }
    if buf.get_u8() != VERSION {
        return None;
    }

    let msg_type = buf.get_u8();
    let len = buf.get_u32();
    Some((msg_type, len))
}

pub fn encode_depth(depth: &fastquote_core::DepthMarket) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(232);
    put_fixed(&mut buf, depth.symbol.market.as_bytes(), 2);
    put_fixed(&mut buf, depth.symbol.code.as_bytes(), 6);

    buf.put_i64(depth.time.0);
    buf.put_i64(depth.last_price.0);
    buf.put_i64(depth.open.0);
    buf.put_i64(depth.high.0);
    buf.put_i64(depth.low.0);
    buf.put_i64(depth.close.0);
    buf.put_i64(depth.volume.0);
    buf.put_i64(depth.amount.0);

    for level in depth.bid {
        buf.put_i64(level.price.0);
        buf.put_i64(level.volume.0);
    }
    for level in depth.ask {
        buf.put_i64(level.price.0);
        buf.put_i64(level.volume.0);
    }

    buf.to_vec()
}

fn put_fixed(buf: &mut BytesMut, bytes: &[u8], width: usize) {
    for idx in 0..width {
        buf.put_u8(bytes.get(idx).copied().unwrap_or(0));
    }
}
