// Bridge between Rust/WASM and ECharts.
// All functions are idempotent — safe to call multiple times on the same id.
const _vrCharts = {};

window.vinrougeInitChart = function(id, optionJson) {
  const el = document.getElementById(id);
  if (!el) return;
  if (_vrCharts[id]) { _vrCharts[id].dispose(); }
  const chart = echarts.init(el, 'dark', { renderer: 'canvas' });
  chart.setOption(JSON.parse(optionJson));
  _vrCharts[id] = chart;
};

window.vinrougeUpdateChart = function(id, optionJson) {
  const chart = _vrCharts[id];
  if (!chart) return;
  chart.setOption(JSON.parse(optionJson), true);
};

window.vinrougeDisposeChart = function(id) {
  const chart = _vrCharts[id];
  if (!chart) return;
  chart.dispose();
  delete _vrCharts[id];
};

window.vinrougeDownload = function(filename, jsonString) {
  const blob = new Blob([jsonString], { type: 'application/json' });
  const url  = URL.createObjectURL(blob);
  const a    = document.createElement('a');
  a.href = url; a.download = filename;
  document.body.appendChild(a);
  a.click();
  setTimeout(() => { document.body.removeChild(a); URL.revokeObjectURL(url); }, 150);
};

window.vinrougeResizeChart = function(id) {
  const chart = _vrCharts[id];
  if (chart) chart.resize();
};
