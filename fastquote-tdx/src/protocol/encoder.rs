use super::types::*;
use bytes::{BufMut, BytesMut};
use fastquote_core::Symbol;

/// TDX 协议编码器
pub struct Encoder;

impl Encoder {
    /// 计算 TDX 校验和
    fn checksum(data: &[u8]) -> u16 {
        let sum: u32 = data.iter().map(|&b| b as u32).sum();
        (sum & 0xFFFF) as u16
    }

    /// 构建完整数据包: [len(2B,LE)] [type(2B,LE)] [body] [checksum(2B)]
    fn build_packet(pkt_type: u16, body: &[u8]) -> Vec<u8> {
        let body_len = body.len();
        let total_len = 2 + body_len + 2; // type + body + checksum

        let mut buf = BytesMut::with_capacity(2 + total_len);
        buf.put_u16_le(total_len as u16);
        buf.put_u16_le(pkt_type);
        buf.put_slice(body);

        // checksum 覆盖 [type + body]
        let check_data = &buf[2..];
        let cs = Self::checksum(check_data);
        buf.put_u16_le(cs);

        buf.to_vec()
    }

    /// 写入 symbol (market u8 + code 6 bytes)
    fn put_symbol(buf: &mut BytesMut, symbol: &Symbol) {
        let market_id = match symbol.market.as_str() {
            "SH" => market::SH,
            "SZ" => market::SZ,
            _ => market::SH,
        };
        buf.put_u8(market_id);
        let code_bytes = symbol.code.as_bytes();
        for i in 0..6 {
            buf.put_u8(if i < code_bytes.len() {
                code_bytes[i]
            } else {
                0
            });
        }
    }

    /// 编码登录请求
    pub fn login() -> Vec<u8> {
        let mut body = BytesMut::new();
        body.put_u16_le(0x01); // client id
        Self::build_packet(pkt_type::LOGIN_REQ, &body)
    }

    /// 编码行情订阅请求
    pub fn subscribe(symbols: &[Symbol]) -> Vec<u8> {
        let mut body = BytesMut::new();
        body.put_u16_le(symbols.len() as u16);
        for sym in symbols {
            Self::put_symbol(&mut body, sym);
        }
        Self::build_packet(pkt_type::QUOTE_SUBSCRIBE, &body)
    }

    /// 编码 K 线请求
    pub fn kline_request(symbol: &Symbol, period: u8, count: u16) -> Vec<u8> {
        let mut body = BytesMut::new();
        Self::put_symbol(&mut body, symbol);
        body.put_u8(period);
        body.put_u16_le(count);
        body.put_u16_le(0); // start position
        Self::build_packet(pkt_type::KLINE_REQ, &body)
    }

    /// 编码财务数据请求
    pub fn financial_request(symbol: &Symbol) -> Vec<u8> {
        let mut body = BytesMut::new();
        Self::put_symbol(&mut body, symbol);
        Self::build_packet(pkt_type::FINANCIAL_REQ, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum() {
        let data = [0x01, 0x00, 0x02, 0x00];
        let cs = Encoder::checksum(&data);
        assert_eq!(cs, 0x03);
    }

    #[test]
    fn test_login_packet() {
        let pkt = Encoder::login();
        // len(2) + type(2) + body(2) + checksum(2) = 6 bytes reported
        let len = u16::from_le_bytes([pkt[0], pkt[1]]);
        assert_eq!(len, 6); // type(2) + body(2) + checksum(2)
        let pkt_type = u16::from_le_bytes([pkt[2], pkt[3]]);
        assert_eq!(pkt_type, 0x01);
    }

    #[test]
    fn test_subscribe_packet() {
        let symbols = vec![
            Symbol::new("SH", "600000"),
            Symbol::new("SZ", "000001"),
        ];
        let pkt = Encoder::subscribe(&symbols);
        let len = u16::from_le_bytes([pkt[0], pkt[1]]);
        let pkt_type = u16::from_le_bytes([pkt[2], pkt[3]]);
        assert_eq!(pkt_type, 0x0C);

        // body: count(2) + 2 * (market(1) + code(6)) = 16
        let body_start = 4;
        let body_end = 4 + (len as usize - 2 - 2); // minus type and checksum
        let body = &pkt[body_start..body_end];
        let count = u16::from_le_bytes([body[0], body[1]]);
        assert_eq!(count, 2);
    }
}
