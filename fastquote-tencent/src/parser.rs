use anyhow::{anyhow, Result};
use fastquote_core::*;
use serde_json::Value;

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
    let parse_price =
        |idx: usize| Price::from_f64(fields.get(idx).unwrap_or(&"0").parse().unwrap_or(0.0));
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

pub fn parse_kline_response(raw: &str, symbol: &Symbol, period: u32) -> Result<Vec<Kline>> {
    let value: Value = serde_json::from_str(raw)?;
    let data = value
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("Tencent kline response missing data"))?;
    let symbol_key = format!("{}{}", symbol.market.to_lowercase(), symbol.code);
    let symbol_data = data
        .get(&symbol_key)
        .ok_or_else(|| anyhow!("Tencent kline response missing symbol {symbol_key}"))?;
    let field = kline_field(period).ok_or_else(|| anyhow!("unsupported kline period {period}"))?;
    let rows = symbol_data
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Tencent kline response missing field {field}"))?;

    let mut klines = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(fields) = row.as_array() else {
            continue;
        };
        if fields.len() < 6 {
            continue;
        }
        let Some(open_time) = fields
            .first()
            .and_then(Value::as_str)
            .and_then(parse_open_time)
        else {
            continue;
        };

        klines.push(Kline {
            symbol: symbol.clone(),
            open_time,
            open: parse_price_field(fields, 1),
            close: parse_price_field(fields, 2),
            high: parse_price_field(fields, 3),
            low: parse_price_field(fields, 4),
            volume: Volume(parse_f64_field(fields, 5).round() as i64),
            amount: Amount(0),
            period,
        });
    }

    Ok(klines)
}

pub fn tencent_period(period: u32) -> Option<&'static str> {
    match period {
        60 => Some("m1"),
        300 => Some("m5"),
        900 => Some("m15"),
        1800 => Some("m30"),
        3600 => Some("m60"),
        86400 => Some("day"),
        _ => None,
    }
}

fn kline_field(period: u32) -> Option<&'static str> {
    match period {
        86400 => Some("qfqday"),
        _ => tencent_period(period),
    }
}

fn parse_price_field(fields: &[Value], idx: usize) -> Price {
    Price::from_f64(parse_f64_field(fields, idx))
}

fn parse_f64_field(fields: &[Value], idx: usize) -> f64 {
    fields
        .get(idx)
        .and_then(Value::as_str)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn parse_open_time(raw: &str) -> Option<Time> {
    let (year, month, day, hour, minute) = if raw.len() >= 12 && raw.as_bytes()[4].is_ascii_digit()
    {
        (
            raw.get(0..4)?.parse::<i32>().ok()?,
            raw.get(4..6)?.parse::<u32>().ok()?,
            raw.get(6..8)?.parse::<u32>().ok()?,
            raw.get(8..10)?.parse::<u32>().ok()?,
            raw.get(10..12)?.parse::<u32>().ok()?,
        )
    } else if raw.len() >= 10 {
        (
            raw.get(0..4)?.parse::<i32>().ok()?,
            raw.get(5..7)?.parse::<u32>().ok()?,
            raw.get(8..10)?.parse::<u32>().ok()?,
            0,
            0,
        )
    } else {
        return None;
    };

    let days = days_from_civil(year, month, day);
    let china_offset_secs = 8 * 60 * 60;
    let secs = days * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60 - china_offset_secs;
    Some(Time(secs * 1_000_000_000))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::from(era * 146_097 + doe - 719_468)
}

#[cfg(test)]
mod tests {
    use super::parse_kline_response;
    use fastquote_core::{Price, Symbol, Volume};

    #[test]
    fn parses_minute_kline_response() {
        let raw = r#"{
            "code": 0,
            "data": {
                "sh600519": {
                    "m1": [
                        ["202605201022", "1323.61", "1324.68", "1326.97", "1323.61", "557.00", {}, "0.44"],
                        ["202605201023", "1326.27", "1322.91", "1326.27", "1322.91", "206.00", {}, "0.16"]
                    ]
                }
            }
        }"#;

        let symbol = Symbol::new("SH", "600519");
        let klines = parse_kline_response(raw, &symbol, 60).unwrap();

        assert_eq!(klines.len(), 2);
        assert_eq!(klines[0].symbol, symbol);
        assert_eq!(klines[0].period, 60);
        assert_eq!(klines[0].open, Price(1_323_610));
        assert_eq!(klines[0].close, Price(1_324_680));
        assert_eq!(klines[0].high, Price(1_326_970));
        assert_eq!(klines[0].low, Price(1_323_610));
        assert_eq!(klines[0].volume, Volume(557));
        assert!(klines[1].open_time.0 > klines[0].open_time.0);
    }

    #[test]
    fn parses_forward_adjusted_day_kline_response() {
        let raw = r#"{
            "code": 0,
            "data": {
                "sh600519": {
                    "qfqday": [
                        ["2026-05-19", "1321.900", "1324.300", "1329.990", "1318.000", "43255.000"]
                    ]
                }
            }
        }"#;

        let symbol = Symbol::new("SH", "600519");
        let klines = parse_kline_response(raw, &symbol, 86400).unwrap();

        assert_eq!(klines.len(), 1);
        assert_eq!(klines[0].period, 86400);
        assert_eq!(klines[0].open, Price(1_321_900));
        assert_eq!(klines[0].close, Price(1_324_300));
        assert_eq!(klines[0].high, Price(1_329_990));
        assert_eq!(klines[0].low, Price(1_318_000));
        assert_eq!(klines[0].volume, Volume(43_255));
    }
}
