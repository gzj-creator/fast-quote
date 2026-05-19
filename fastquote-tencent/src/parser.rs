use fastquote_core::*;

/// 解析腾讯行情文本格式:
/// v_sh600000="1~浦发银行~600000~12.34~12.21~12.53~...";
pub fn parse_depth_quote(raw: &str) -> Vec<DepthMarket> {
    raw.lines()
        .filter_map(parse_depth_line)
        .collect::<Vec<DepthMarket>>()
}

fn parse_depth_line(line: &str) -> Option<DepthMarket> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let content = line.split_once('=')?.1;
    let content = content.trim().trim_end_matches(';').trim_matches('"');
    if content.is_empty() {
        return None;
    }

    let fields = content.split('~').collect::<Vec<_>>();
    if fields.len() < 48 {
        return None;
    }

    let market = if fields[0] == "1" { "SH" } else { "SZ" };
    let code = fields[2].to_string();
    let parse_price = |idx: usize| Price::from_f64(fields.get(idx).unwrap_or(&"0").parse().unwrap_or(0.0));
    let parse_i64 = |idx: usize| fields.get(idx).unwrap_or(&"0").parse().unwrap_or(0);

    Some(DepthMarket {
        symbol: Symbol::new(market, code),
        time: Time::now(),
        last_price: parse_price(3),
        open: parse_price(4),
        high: parse_price(5),
        low: Price(0),
        close: parse_price(4),
        volume: Volume(parse_i64(6)),
        amount: Amount(parse_i64(37)),
        bid: parse_levels(&fields[9..19]),
        ask: parse_levels(&fields[19..29]),
    })
}

fn parse_levels(fields: &[&str]) -> [Level; 5] {
    let mut levels = [Level {
        price: Price(0),
        volume: Volume(0),
    }; 5];

    for (idx, level) in levels.iter_mut().enumerate() {
        let price_idx = idx * 2;
        let volume_idx = price_idx + 1;
        if volume_idx >= fields.len() {
            break;
        }
        *level = Level {
            price: Price::from_f64(fields[price_idx].parse().unwrap_or(0.0)),
            volume: Volume(fields[volume_idx].parse().unwrap_or(0)),
        };
    }

    levels
}
