use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};

pub async fn index() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        HTML_PAGE,
    )
}

const HTML_PAGE: &str = r##"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>FastQuote 实时行情</title>
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif; background: #1a1a2e; color: #e0e0e0; }
.header { background: #16213e; padding: 12px 24px; display: flex; align-items: center; gap: 16px; border-bottom: 1px solid #0f3460; }
.header h1 { font-size: 18px; color: #e94560; white-space: nowrap; }
.header .spacer { flex: 1; }
.header .status { font-size: 12px; padding: 4px 10px; border-radius: 4px; }
.status.connected { background: #1b5e20; color: #4caf50; }
.status.disconnected { background: #b71c1c; color: #f44336; }
.src-tabs { display: flex; gap: 4px; }
.src-tab { padding: 4px 14px; font-size: 12px; border: 1px solid #0f3460; border-radius: 4px; background: transparent; color: #8892b0; cursor: pointer; transition: all 0.15s; }
.src-tab:hover { border-color: #e94560; color: #e0e0e0; }
.src-tab.active { background: #e94560; border-color: #e94560; color: #fff; }
.container { max-width: 1200px; margin: 0 auto; padding: 16px; }
.panel { background: #16213e; border-radius: 8px; margin-bottom: 16px; overflow: hidden; border: 1px solid #0f3460; }
.panel-title { padding: 10px 16px; font-size: 14px; font-weight: 600; background: #0f3460; color: #e94560; display: flex; align-items: center; gap: 8px; }
table { width: 100%; border-collapse: collapse; }
th { padding: 8px 12px; text-align: right; font-size: 12px; color: #8892b0; font-weight: 500; border-bottom: 1px solid #0f3460; white-space: nowrap; }
th:first-child { text-align: left; }
td { padding: 6px 12px; text-align: right; font-size: 13px; font-variant-numeric: tabular-nums; border-bottom: 1px solid #1a1a2e; white-space: nowrap; cursor: pointer; }
td:first-child { text-align: left; font-weight: 600; }
tr:hover td { background: #1a1a3e; }
tr.selected td { background: #0f3460; }
.up { color: #f44336; }
.down { color: #4caf50; }
.flat { color: #8892b0; }
.depth-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0; }
.depth-side { padding: 12px 16px; }
.depth-side h3 { font-size: 12px; color: #8892b0; margin-bottom: 8px; }
.depth-row { display: flex; justify-content: space-between; padding: 3px 0; font-size: 13px; font-variant-numeric: tabular-nums; }
.depth-row .price { min-width: 80px; text-align: left; }
.depth-row .volume { min-width: 80px; text-align: right; color: #8892b0; }
.empty { text-align: center; padding: 40px; color: #555; font-size: 14px; }
.stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); gap: 12px; padding: 12px 16px; }
.stat-item { text-align: center; }
.stat-item .label { font-size: 11px; color: #8892b0; }
.stat-item .value { font-size: 15px; margin-top: 2px; }
.chart-area { padding: 8px 16px 16px; }
canvas { width: 100%; height: 200px; display: block; }
.detail-bottom { display: grid; grid-template-columns: 1fr 1fr; }
@media (max-width: 768px) { .detail-bottom { grid-template-columns: 1fr; } }
.src-badge { font-size: 10px; padding: 1px 6px; border-radius: 3px; font-weight: 400; vertical-align: middle; }
.src-badge.tdx { background: #1a237e; color: #7986cb; }
.src-badge.tencent { background: #1b5e20; color: #81c784; }
</style>
</head>
<body>
<div class="header">
  <h1>FastQuote</h1>
  <div class="src-tabs" id="src-tabs">
    <button class="src-tab" data-src="Tdx">通达信</button>
    <button class="src-tab active" data-src="Tencent">腾讯</button>
  </div>
  <div class="spacer"></div>
  <span id="status" class="status disconnected">未连接</span>
</div>
<div class="container">
  <div class="panel">
    <div class="panel-title">自选行情 <span id="count" style="color:#8892b0;font-weight:400;font-size:12px;">0 只</span></div>
    <table>
      <thead><tr><th>代码</th><th>来源</th><th>最新价</th><th>涨跌</th><th>涨跌幅</th><th>开盘</th><th>最高</th><th>最低</th><th>成交量</th><th>成交额</th><th>时间</th></tr></thead>
      <tbody id="quotes"></tbody>
    </table>
    <div id="empty-msg" class="empty">等待行情数据...</div>
  </div>
  <div class="panel" id="detail-panel" style="display:none;">
    <div class="panel-title" id="detail-title">盘口详情</div>
    <div class="stats" id="detail-stats"></div>
    <div class="chart-area"><canvas id="chart"></canvas></div>
    <div class="detail-bottom">
      <div class="depth-grid">
        <div class="depth-side"><h3>卖盘</h3><div id="ask-depth"></div></div>
        <div class="depth-side"><h3>买盘</h3><div id="bid-depth"></div></div>
      </div>
    </div>
  </div>
</div>
<script>
const quotes = new Map();        // key -> { key, source, ... }
const priceHistory = new Map();  // source:key -> [{ price, time }]
const MAX_HISTORY = 120;
const KLINE_PERIOD = 60;
const KLINE_COUNT = 120;
const requestedKlines = new Set();
let selectedKey = null;
let activeSource = 'Tencent';    // 'Tdx' | 'Tencent'
let ws = null;
const tbody = document.getElementById('quotes');
const emptyMsg = document.getElementById('empty-msg');
const statusEl = document.getElementById('status');
const countEl = document.getElementById('count');
const canvas = document.getElementById('chart');
const ctx = canvas.getContext('2d');

// source tabs
document.querySelectorAll('.src-tab').forEach(btn => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('.src-tab').forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    activeSource = btn.dataset.src;
    selectedKey = null;
    updateTable();
  });
});

function priceStr(p) { return (p / 1000).toFixed(3); }
function volStr(v) {
  if (v >= 100000000) return (v / 100000000).toFixed(2) + '亿';
  if (v >= 10000) return (v / 10000).toFixed(1) + '万';
  return v.toLocaleString();
}
function amtStr(a) {
  const yuan = a / 100;
  if (yuan >= 100000000) return (yuan / 100000000).toFixed(2) + '亿';
  if (yuan >= 10000) return (yuan / 10000).toFixed(1) + '万';
  return yuan.toLocaleString();
}
function timeStr(ns) {
  const d = new Date(ns / 1000000);
  return d.toLocaleTimeString('zh-CN', { hour12: false });
}
function pctClass(pct) { return pct > 0 ? 'up' : pct < 0 ? 'down' : 'flat'; }
function signStr(v) { return v > 0 ? '+' : ''; }
function srcBadge(src) {
  const cls = src === 'Tdx' ? 'tdx' : 'tencent';
  const label = src === 'Tdx' ? 'TDX' : '腾讯';
  return `<span class="src-badge ${cls}">${label}</span>`;
}

function histKey(source, key) { return source + ':' + key; }
function quoteKey(q) { return histKey(q.source, q.key); }

function renderDetailTitle(q) {
  const title = document.getElementById('detail-title');
  const cls = q.source === 'Tdx' ? 'tdx' : 'tencent';
  const label = q.source === 'Tdx' ? 'TDX' : '腾讯';
  title.textContent = '';
  title.append(document.createTextNode(q.key + ' 盘口详情 '));
  const badge = document.createElement('span');
  badge.className = 'src-badge ' + cls;
  badge.textContent = label;
  title.appendChild(badge);
}

function pushHistory(source, key, price, time) {
  const hk = histKey(source, key);
  let arr = priceHistory.get(hk);
  if (!arr) { arr = []; priceHistory.set(hk, arr); }
  arr.push({ price, time });
  if (arr.length > MAX_HISTORY) arr.shift();
}

function replaceKlineHistory(source, key, kline) {
  const hk = histKey(source, key);
  let arr = priceHistory.get(hk) || [];
  const point = { price: kline.close, time: kline.open_time };
  const idx = arr.findIndex(p => p.time === point.time);
  if (idx >= 0) arr[idx] = point; else arr.push(point);
  arr.sort((a, b) => a.time - b.time);
  if (arr.length > MAX_HISTORY) arr = arr.slice(-MAX_HISTORY);
  priceHistory.set(hk, arr);
}

function requestKline(q) {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  const reqKey = quoteKey(q) + ':' + KLINE_PERIOD;
  if (requestedKlines.has(reqKey)) return;
  requestedKlines.add(reqKey);
  ws.send(JSON.stringify({
    cmd: 'req_kline',
    symbol: { market: q.market, code: q.code },
    period: KLINE_PERIOD,
    count: KLINE_COUNT
  }));
}

function drawChart() {
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);
  const W = rect.width;
  const H = rect.height;
  ctx.clearRect(0, 0, W, H);

  // find the history for selected stock + active source
  const q = quotes.get(selectedKey);
  if (!q) return;

  const hk = histKey(q.source, q.key);
  const pts = priceHistory.get(hk);
  if (!pts || pts.length < 2) {
    ctx.fillStyle = '#555';
    ctx.font = '13px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('等待数据积累...', W / 2, H / 2);
    return;
  }

  const pad = { top: 20, right: 60, bottom: 24, left: 12 };
  const cw = W - pad.left - pad.right;
  const ch = H - pad.top - pad.bottom;

  const prices = pts.map(p => p.price);
  let minP = Math.min(...prices);
  let maxP = Math.max(...prices);
  if (maxP === minP) { maxP += 1; minP -= 1; }
  const range = maxP - minP;

  const openPrice = q.open;
  const isUp = openPrice > 0 ? prices[prices.length - 1] >= openPrice : true;
  const lineColor = isUp ? '#f44336' : '#4caf50';
  const fillTop = isUp ? 'rgba(244,67,54,0.15)' : 'rgba(76,175,80,0.15)';

  // grid
  ctx.strokeStyle = '#1a2a4e';
  ctx.lineWidth = 0.5;
  for (let i = 0; i <= 4; i++) {
    const y = pad.top + (ch * i / 4);
    ctx.beginPath(); ctx.moveTo(pad.left, y); ctx.lineTo(W - pad.right, y); ctx.stroke();
  }

  // open price line
  if (openPrice > 0 && openPrice >= minP && openPrice <= maxP) {
    const openY = pad.top + ch - ((openPrice - minP) / range * ch);
    ctx.strokeStyle = '#555';
    ctx.setLineDash([4, 4]);
    ctx.beginPath(); ctx.moveTo(pad.left, openY); ctx.lineTo(W - pad.right, openY); ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = '#8892b0';
    ctx.font = '10px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText('开盘 ' + priceStr(openPrice), W - pad.right + 4, openY + 3);
  }

  // line
  ctx.beginPath();
  for (let i = 0; i < pts.length; i++) {
    const x = pad.left + (i / (pts.length - 1)) * cw;
    const y = pad.top + ch - ((pts[i].price - minP) / range * ch);
    if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
  }
  ctx.strokeStyle = lineColor;
  ctx.lineWidth = 1.5;
  ctx.stroke();

  // fill
  const lastX = pad.left + cw;
  ctx.lineTo(lastX, pad.top + ch);
  ctx.lineTo(pad.left, pad.top + ch);
  ctx.closePath();
  const grad = ctx.createLinearGradient(0, pad.top, 0, pad.top + ch);
  grad.addColorStop(0, fillTop);
  grad.addColorStop(1, 'rgba(0,0,0,0)');
  ctx.fillStyle = grad;
  ctx.fill();

  // dot
  const lastPt = pts[pts.length - 1];
  const lx = lastX;
  const ly = pad.top + ch - ((lastPt.price - minP) / range * ch);
  ctx.beginPath();
  ctx.arc(lx, ly, 3, 0, Math.PI * 2);
  ctx.fillStyle = lineColor;
  ctx.fill();

  ctx.fillStyle = lineColor;
  ctx.font = 'bold 11px sans-serif';
  ctx.textAlign = 'left';
  ctx.fillText(priceStr(lastPt.price), lx + 4, ly + 4);

  // time
  ctx.fillStyle = '#555';
  ctx.font = '10px sans-serif';
  ctx.textAlign = 'left';
  ctx.fillText(new Date(pts[0].time / 1000000).toLocaleTimeString('zh-CN', { hour12: false }), pad.left, H - 4);
  ctx.textAlign = 'right';
  ctx.fillText(new Date(lastPt.time / 1000000).toLocaleTimeString('zh-CN', { hour12: false }), W - pad.right, H - 4);

  // min/max
  ctx.fillStyle = '#8892b0';
  ctx.font = '10px sans-serif';
  ctx.textAlign = 'left';
  ctx.fillText(priceStr(maxP), W - pad.right + 4, pad.top + 10);
  ctx.fillText(priceStr(minP), W - pad.right + 4, pad.top + ch);
}

function updateTable() {
  let sorted = [...quotes.values()];
  // filter by active source
  sorted = sorted.filter(q => q.source === activeSource);
  sorted.sort((a, b) => {
    if (a.key !== b.key) return a.key.localeCompare(b.key);
    return b.time - a.time;
  });
  sorted.forEach(q => { q.rowKey = quoteKey(q); });

  countEl.textContent = sorted.length + ' 只';
  emptyMsg.style.display = sorted.length ? 'none' : 'block';
  if (!sorted.length) {
    document.getElementById('detail-panel').style.display = 'none';
  }

  // reset tbody
  const existingMap = new Map();
  tbody.querySelectorAll('tr').forEach(tr => existingMap.set(tr.dataset.key, tr));

  sorted.forEach((q) => {
    const rowKey = quoteKey(q);
    const prevClose = q.close || q.last_price;
    const change = prevClose ? q.last_price - prevClose : 0;
    const changePct = prevClose ? (change / prevClose * 100) : 0;
    const cls = pctClass(change);

    let tr = existingMap.get(rowKey);
    if (!tr) {
      tr = document.createElement('tr');
      tbody.appendChild(tr);
    }
    tr.dataset.key = rowKey;
    tr.onclick = () => selectQuote(rowKey);
    existingMap.delete(rowKey);

    tr.className = rowKey === selectedKey ? 'selected' : '';
    tr.innerHTML = `<td>${q.key}</td>` +
      `<td>${srcBadge(q.source)}</td>` +
      `<td class="${cls}">${priceStr(q.last_price)}</td>` +
      `<td class="${cls}">${signStr(change)}${priceStr(change)}</td>` +
      `<td class="${cls}">${signStr(changePct)}${changePct.toFixed(2)}%</td>` +
      `<td>${priceStr(q.open)}</td>` +
      `<td class="up">${priceStr(q.high)}</td>` +
      `<td class="down">${priceStr(q.low)}</td>` +
      `<td>${volStr(q.volume)}</td>` +
      `<td>${amtStr(q.amount)}</td>` +
      `<td>${timeStr(q.time)}</td>`;
  });

  existingMap.forEach(tr => tr.remove());

  if (!selectedKey && sorted.length) {
    selectQuote(sorted[0].rowKey);
  }
}

function selectQuote(key) {
  selectedKey = key;
  updateTable();
  renderDetail();
  const q = quotes.get(selectedKey);
  if (q) requestKline(q);
}

function renderDetail() {
  const panel = document.getElementById('detail-panel');
  const q = quotes.get(selectedKey);
  if (!q) { panel.style.display = 'none'; return; }

  panel.style.display = 'block';
  renderDetailTitle(q);

  const prevClose = q.close || q.last_price;
  const change = prevClose ? q.last_price - prevClose : 0;
  const changePct = prevClose ? (change / prevClose * 100) : 0;
  const cls = pctClass(change);

  document.getElementById('detail-stats').innerHTML =
    `<div class="stat-item"><div class="label">最新价</div><div class="value ${cls}">${priceStr(q.last_price)}</div></div>` +
    `<div class="stat-item"><div class="label">涨跌幅</div><div class="value ${cls}">${signStr(changePct)}${changePct.toFixed(2)}%</div></div>` +
    `<div class="stat-item"><div class="label">开盘</div><div class="value">${priceStr(q.open)}</div></div>` +
    `<div class="stat-item"><div class="label">最高</div><div class="value up">${priceStr(q.high)}</div></div>` +
    `<div class="stat-item"><div class="label">最低</div><div class="value down">${priceStr(q.low)}</div></div>` +
    `<div class="stat-item"><div class="label">成交量</div><div class="value">${volStr(q.volume)}</div></div>` +
    `<div class="stat-item"><div class="label">成交额</div><div class="value">${amtStr(q.amount)}</div></div>`;

  const bidDiv = document.getElementById('bid-depth');
  const askDiv = document.getElementById('ask-depth');
  bidDiv.innerHTML = '';
  askDiv.innerHTML = '';

  for (let i = 0; i < 5; i++) {
    const bid = q.bid[i];
    if (bid && bid.volume > 0) {
      bidDiv.innerHTML += `<div class="depth-row"><span class="price up">买${i+1} ${priceStr(bid.price)}</span><span class="volume">${volStr(bid.volume)}</span></div>`;
    }
  }
  for (let i = 4; i >= 0; i--) {
    const ask = q.ask[i];
    if (ask && ask.volume > 0) {
      askDiv.innerHTML += `<div class="depth-row"><span class="price down">卖${i+1} ${priceStr(ask.price)}</span><span class="volume">${volStr(ask.volume)}</span></div>`;
    }
  }

  drawChart();
}

function onMessage(event) {
  try {
    const msg = JSON.parse(event.data);
    const source = msg.source || 'Unknown';
    if (!msg.data) return;

    const kline = msg.data.Kline || (msg.type === 'kline' ? msg.data : null);
    if (kline) {
      const key = kline.symbol.market + kline.symbol.code;
      replaceKlineHistory(source, key, kline);
      if (selectedKey === histKey(source, key)) drawChart();
      return;
    }

    const d = msg.data.Depth || (msg.type === 'depth' ? msg.data : null);
    if (!d) return;
    const key = d.symbol.market + d.symbol.code;
    const lastPrice = d.last_price;

    // store with source, key is source:key for uniqueness
    const qKey = histKey(source, key);
    quotes.set(qKey, {
      key,
      market: d.symbol.market,
      code: d.symbol.code,
      source,
      last_price: lastPrice,
      open: d.open,
      high: d.high,
      low: d.low,
      close: d.close,
      volume: d.volume,
      amount: d.amount,
      time: d.time,
      bid: d.bid,
      ask: d.ask,
    });
    pushHistory(source, key, lastPrice, d.time);

    // update selectedKey reference if needed
    if (selectedKey === key || selectedKey === qKey) {
      selectedKey = qKey;
    }

    updateTable();
    if (qKey === selectedKey) renderDetail();
  } catch(e) {}
}

function connect() {
  ws = new WebSocket((location.protocol === 'https:' ? 'wss:' : 'ws:') + '//' + location.host + '/ws');
  ws.onopen = () => {
    statusEl.textContent = '已连接';
    statusEl.className = 'status connected';
    requestedKlines.clear();
    const q = quotes.get(selectedKey);
    if (q) requestKline(q);
  };
  ws.onclose = () => { statusEl.textContent = '已断开'; statusEl.className = 'status disconnected'; setTimeout(connect, 3000); };
  ws.onerror = () => ws.close();
  ws.onmessage = onMessage;
}

window.addEventListener('resize', () => { if (selectedKey) drawChart(); });
connect();
</script>
</body>
</html>"##;

#[cfg(test)]
mod tests {
    use super::HTML_PAGE;

    #[test]
    fn detail_title_does_not_render_badge_markup_as_text() {
        assert!(!HTML_PAGE.contains("detail-title').textContent"));
        assert!(HTML_PAGE.contains("renderDetailTitle(q)"));
    }

    #[test]
    fn quote_selection_requests_history_kline_data() {
        assert!(HTML_PAGE.contains("cmd: 'req_kline'"));
        assert!(HTML_PAGE.contains("requestKline(q)"));
    }

    #[test]
    fn websocket_kline_messages_feed_chart_history() {
        assert!(HTML_PAGE.contains("msg.data.Kline"));
        assert!(HTML_PAGE.contains("replaceKlineHistory(source, key, kline)"));
    }

    #[test]
    fn chart_uses_symbol_key_when_reading_price_history() {
        assert!(!HTML_PAGE.contains("histKey(q.source, selectedKey)"));
        assert!(HTML_PAGE.contains("histKey(q.source, q.key)"));
    }

    #[test]
    fn default_source_is_tencent_without_all_tab() {
        assert!(!HTML_PAGE.contains(r#"data-src="all""#));
        assert!(HTML_PAGE.contains(r#"class="src-tab active" data-src="Tencent""#));
        assert!(HTML_PAGE.contains("let activeSource = 'Tencent'"));
    }

    #[test]
    fn table_auto_selects_first_visible_quote_for_chart() {
        assert!(HTML_PAGE.contains("selectQuote(sorted[0].rowKey)"));
        assert!(HTML_PAGE.contains("if (!selectedKey && sorted.length)"));
    }

    #[test]
    fn chart_formats_history_prices_without_rescaling() {
        assert!(!HTML_PAGE.contains("priceStr(openPrice * 1000)"));
        assert!(!HTML_PAGE.contains("priceStr(lastPt.price * 1000)"));
        assert!(!HTML_PAGE.contains("priceStr(maxP * 1000)"));
        assert!(HTML_PAGE.contains("priceStr(lastPt.price)"));
    }
}
