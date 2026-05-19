use super::types::market;
use bytes::{Buf, Bytes};
use fastquote_core::*;

/// TDX 协议解码器
pub struct Decoder;

impl Decoder {
    /// 解析 TDX 包头，返回 (pkt_type, total_body_len, body_slice)
    /// total_body_len 不包含前 2 bytes 的 len 字段
    pub fn parse_header(data: &[u8]) -> Option<(u16, usize)> {
        if data.len() < 4 {
            return None;
        }
        let total_len = u16::from_le_bytes([data[0], data[1]]) as usize;
        let pkt_type = u16::from_le_bytes([data[2], data[3]]);
        // total_len = type(2) + body + checksum(2)
        Some((pkt_type, total_len))
    }

    /// 提取 body (去掉 type 和 checksum)
    /// data 从包开头开始（含 len 字段）
    /// total_len 是 len 字段的值 (= type(2) + body(N) + checksum(2))
    pub fn extract_body<'a>(data: &'a [u8], total_len: usize) -> Option<&'a [u8]> {
        // data 需要: len(2) + total_len
        let needed = 2 + total_len;
        if data.len() < needed {
            return None;
        }
        // body: data[4..4 + total_len - 4] (去掉 type(2) 和 checksum(2))
        let body_len = total_len.saturating_sub(4);
        if body_len == 0 {
            return Some(&[]);
        }
        Some(&data[4..4 + body_len])
    }

    /// 解析行情推送 -> DepthMarket
    /// TDX 行情推送格式: count(u16) + [market(u8) + code(6B) + fields...] per stock
    pub fn decode_depth(data: &[u8]) -> anyhow::Result<Vec<DepthMarket>> {
        if data.len() < 2 {
            anyhow::bail!("depth data too short: {} bytes", data.len());
        }

        let mut buf = Bytes::copy_from_slice(data);
        let count = buf.get_u16_le() as usize;

        let mut results = Vec::with_capacity(count);
        for _ in 0..count {
            // 最少需要: market(1) + unknown(1) + code(6) + 基础行情字段
            if buf.remaining() < 8 + 24 {
                break;
            }

            let market_id = buf.get_u8();
            let _unknown = buf.get_u8();
            let mut code_buf = [0u8; 6];
            buf.copy_to_slice(&mut code_buf);
            let code = String::from_utf8_lossy(&code_buf)
                .trim_end_matches('\0')
                .to_string();

            let active = buf.get_u16_le(); // 活跃标志
            if active == 0 {
                continue;
            }

            // TDX 快照基础字段 (简化版，按 pytdx 标准 32 字段)
            let last_raw = buf.get_i32_le(); // 最新价 (0.01元单位)
            let close_raw = buf.get_i32_le(); // 昨收
            let open_raw = buf.get_i32_le(); // 开盘
            let high_raw = buf.get_i32_le(); // 最高
            let low_raw = buf.get_i32_le(); // 最低

            // 跳过后续字段直到五档数据
            // 实际 TDX 协议有更多字段，这里跳到五档
            // 简化处理：先填充基础数据
            let to_price = |raw: i32| Price(raw as i64 * 10); // 0.01元 -> ×1000

            let market_str = if market_id == market::SH { "SH" } else { "SZ" };

            // 尝试读取五档 (如果数据够长)
            let mut bid = [Level { price: Price(0), volume: Volume(0) }; 5];
            let ask = [Level { price: Price(0), volume: Volume(0) }; 5];

            // 跳过中间字段到买一价
            // TDX 快照字段顺序: .../买一价/买一量/.../买五价/买五量/卖一价/卖一量/.../卖五量
            // 简化: 直接读取剩余字段
            let mut remaining_fields = Vec::new();
            while buf.remaining() >= 4 {
                remaining_fields.push(buf.get_i32_le());
            }

            // 尝试从剩余字段中提取五档 (偏移量取决于具体协议版本)
            // 买一价~买五价 从 remaining_fields 中提取
            let bid_offset = 0;
            for i in 0..5 {
                if bid_offset + i * 2 + 1 < remaining_fields.len() {
                    bid[i] = Level {
                        price: to_price(remaining_fields[bid_offset + i * 2]),
                        volume: Volume(remaining_fields[bid_offset + i * 2 + 1] as i64),
                    };
                }
            }

            results.push(DepthMarket {
                symbol: Symbol::new(market_str, code),
                time: Time::now(),
                last_price: to_price(last_raw),
                open: to_price(open_raw),
                high: to_price(high_raw),
                low: to_price(low_raw),
                close: to_price(close_raw),
                volume: Volume(0),
                amount: Amount(0),
                bid,
                ask,
            });
        }
        Ok(results)
    }

    /// GB2312/GBK -> UTF-8
    pub fn gb2312_to_utf8(data: &[u8]) -> String {
        let (cow, _, _) = encoding_rs::GBK.decode(data);
        cow.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header() {
        // 构造一个最小包: len=6 (type=2 + body=2 + checksum=2), type=0x02
        let data: Vec<u8> = vec![
            0x06, 0x00, // len = 6 (LE)
            0x02, 0x00, // type = 0x02 (LE)
            0x00, 0x00, // body
            0x08, 0x00, // checksum
        ];
        let (pkt_type, total_len) = Decoder::parse_header(&data).unwrap();
        assert_eq!(pkt_type, 0x02);
        assert_eq!(total_len, 6);
    }

    #[test]
    fn test_extract_body() {
        // total_len = 8 = type(2) + body(2) + checksum(2) ... need to recalculate
        // 实际 total_len = type(2) + body(N) + checksum(2)
        // 如果 body 有 2 bytes, total_len = 2+2+2 = 6
        let data: Vec<u8> = vec![
            0x06, 0x00, // len = 6 (LE)
            0x01, 0x00, // type = 0x01 (LE)
            0xAA, 0xBB, // body (2 bytes)
            0x00, 0x00, // checksum (2 bytes)
        ];
        let body = Decoder::extract_body(&data, 6).unwrap();
        assert_eq!(body, &[0xAA, 0xBB]);
    }

    #[test]
    fn test_gb2312_to_utf8() {
        // "浦发" in GBK
        let gbk_bytes = &[0xC6, 0xD6, 0xB7, 0xA2];
        let utf8 = Decoder::gb2312_to_utf8(gbk_bytes);
        assert_eq!(utf8, "浦发");
    }
}
