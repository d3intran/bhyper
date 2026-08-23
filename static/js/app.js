/**
 * BHyper Terminal - Main Application Entrypoint
 */
import { apiFetch, closeModal, openModal, showToast } from './api.js';
import { renderHealthAssessment, renderWalletStats, renderDashboardTopRadar } from './components/overview.js';
import { renderRadarTable } from './components/radar.js';
import { renderPositions } from './components/positions.js';
import { fetchAndPopulateConfig, saveCurrentConfig } from './components/config.js';
import { fetchAndRenderJournal } from './components/journal.js';
import { formatCurrency, formatPnl, triggerHaptic } from './utils/format.js';

export const appState = {
  ws: null,
  currentTab: 'dashboard',
  radarFilter: 'all',
  opportunities: [],
  livePositions: [],
  paperPositions: [],
  paperWallet: null,
  config: null,
  tgWebApp: window.Telegram?.WebApp || null,
};

// 1. 初始化 Telegram Mini App
if (appState.tgWebApp) {
  try {
    appState.tgWebApp.ready();
    appState.tgWebApp.expand();
    if (appState.tgWebApp.initDataUnsafe?.user) {
      const u = appState.tgWebApp.initDataUnsafe.user;
      const elUser = document.getElementById('tg-user-badge');
      if (elUser) elUser.innerText = `@${u.username || u.first_name}`;
    }
  } catch (e) {
    console.warn('Telegram init error:', e);
  }
}

// 2. 主题管理 (Dark / Light)
export function initTheme() {
  const saved = localStorage.getItem('theme');
  if (saved === 'light') {
    document.documentElement.classList.remove('dark');
    updateThemeIcon(false);
  } else {
    document.documentElement.classList.add('dark');
    updateThemeIcon(true);
  }
}

export function toggleTheme() {
  const isDark = document.documentElement.classList.toggle('dark');
  localStorage.setItem('theme', isDark ? 'dark' : 'light');
  updateThemeIcon(isDark);
  refreshIcons();
}

function updateThemeIcon(isDark) {
  const btn = document.getElementById('theme-toggle-btn');
  if (btn) {
    btn.innerHTML = isDark 
      ? `<i data-lucide="sun" class="w-3.5 h-3.5 text-amber-400"></i>` 
      : `<i data-lucide="moon" class="w-3.5 h-3.5 text-slate-400"></i>`;
  }
}

export function refreshIcons() {
  if (window.lucide) {
    window.lucide.createIcons();
  }
}

// 3. 选项卡路由
export function switchTab(tabId) {
  triggerHaptic();
  appState.currentTab = tabId;
  
  document.querySelectorAll('.tab-pane').forEach(el => el.classList.add('hidden'));
  const target = document.getElementById(`tab-${tabId}`);
  if (target) target.classList.remove('hidden');

  // Desktop Nav
  document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.className = 'tab-btn px-3 py-1.5 rounded-lg font-medium text-xs transition-all flex items-center space-x-1.5 text-[var(--text-muted)] hover:text-[var(--text-primary)]';
  });
  const activeNav = document.getElementById(`nav-btn-${tabId}`);
  if (activeNav) {
    activeNav.className = 'tab-btn px-3 py-1.5 rounded-lg font-medium text-xs transition-all flex items-center space-x-1.5 bg-emerald-500 text-white shadow-sm';
  }

  // Mobile Nav
  ['dashboard', 'radar', 'positions', 'config', 'journal'].forEach(t => {
    const mobBtn = document.getElementById(`mob-btn-${t}`);
    if (mobBtn) {
      mobBtn.className = t === tabId 
        ? 'flex flex-col items-center text-emerald-500 text-[10px] font-medium p-1' 
        : 'flex flex-col items-center text-[var(--text-muted)] text-[10px] font-medium p-1';
    }
  });

  if (tabId === 'journal') {
    const type = document.getElementById('journal-filter-type')?.value || '';
    fetchAndRenderJournal(type);
  }
  if (tabId === 'config') {
    fetchAndPopulateConfig().then(cfg => {
      if (cfg) appState.config = cfg;
    });
  }
  refreshIcons();
}

// 4. WebSocket 实时通信
export function initWebSocket() {
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const token = new URLSearchParams(location.search).get('token');
  const wsUrl = `${protocol}//${location.host}/api/ws${token ? '?token=' + encodeURIComponent(token) : ''}`;

  try {
    appState.ws = new WebSocket(wsUrl);

    appState.ws.onopen = () => {
      const ind = document.getElementById('status-indicator');
      if (ind) ind.className = 'w-1.5 h-1.5 rounded-full bg-emerald-500 pulse-dot inline-block';
      const st = document.getElementById('status-text');
      if (st) {
        st.className = 'font-semibold text-emerald-500 text-[10px]';
        st.innerText = 'WS LIVE';
      }
    };

    appState.ws.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        handleWsTick(data);
      } catch (err) {
        console.error('WS parse error:', err);
      }
    };

    appState.ws.onclose = () => {
      const ind = document.getElementById('status-indicator');
      if (ind) ind.className = 'w-1.5 h-1.5 rounded-full bg-rose-500 inline-block';
      const st = document.getElementById('status-text');
      if (st) {
        st.className = 'font-semibold text-rose-500 text-[10px]';
        st.innerText = 'WS RECONNECT';
      }
      setTimeout(initWebSocket, 2500);
    };
  } catch (e) {
    console.error('WS error:', e);
  }
}

function handleWsTick(msg) {
  if (msg.type === 'TICK' || msg.type === 'INIT') {
    if (msg.opportunities && msg.opportunities.length > 0) {
      appState.opportunities = msg.opportunities;
      const elCount = document.getElementById('header-cache-count');
      if (elCount) elCount.innerText = msg.opportunities.length;
      const elBadge = document.getElementById('radar-count-badge');
      if (elBadge) elBadge.innerText = msg.opportunities.length;

      const kw = document.getElementById('radar-search')?.value || '';
      renderRadarTable(appState.opportunities, appState.radarFilter, kw, openTradeModal);
      renderDashboardTopRadar(appState.opportunities, openTradeModal);
    }

    if (msg.live_positions || msg.paper_positions) {
      appState.livePositions = msg.live_positions || [];
      appState.paperPositions = msg.paper_positions || [];
      renderPositions(appState.livePositions, appState.paperPositions, handleUnwind);
    }

    if (msg.paper_wallet) {
      appState.paperWallet = msg.paper_wallet;
      renderWalletStats(msg.paper_wallet);
    }
  }
}

// 5. 初始化拉取数据
export async function fetchInitialData() {
  try {
    const [statusRes, scanRes, posRes, healthRes] = await Promise.all([
      apiFetch('/api/status'),
      apiFetch('/api/scan'),
      apiFetch('/api/positions'),
      apiFetch('/api/health')
    ]);

    if (statusRes.version) {
      const elVer = document.getElementById('badge-version');
      if (elVer) elVer.innerText = `v${statusRes.version}`;
      
      const pnlEl = document.getElementById('stat-realized-pnl');
      if (pnlEl) {
        pnlEl.innerText = formatPnl(statusRes.total_realized_pnl_usd, 4);
        pnlEl.className = `text-2xl font-num font-bold mt-1.5 tracking-tight ${statusRes.total_realized_pnl_usd >= 0 ? 'text-emerald-500' : 'text-rose-500'}`;
      }
      if (statusRes.total_closed_trades !== undefined) {
        const elTrades = document.getElementById('stat-trade-count');
        if (elTrades) elTrades.innerText = statusRes.total_closed_trades;
        const elWinRate = document.getElementById('stat-win-rate');
        if (elWinRate) elWinRate.innerText = `${statusRes.win_rate_pct.toFixed(1)}%`;
      }
    }

    if (scanRes.opportunities && scanRes.opportunities.length > 0) {
      appState.opportunities = scanRes.opportunities;
      const elCount = document.getElementById('header-cache-count');
      if (elCount) elCount.innerText = scanRes.opportunities.length;
      const elBadge = document.getElementById('radar-count-badge');
      if (elBadge) elBadge.innerText = scanRes.opportunities.length;

      const kw = document.getElementById('radar-search')?.value || '';
      renderRadarTable(appState.opportunities, appState.radarFilter, kw, openTradeModal);
      renderDashboardTopRadar(appState.opportunities, openTradeModal);
    }

    if (posRes.live_positions || posRes.paper_positions) {
      appState.livePositions = posRes.live_positions || [];
      appState.paperPositions = posRes.paper_positions || [];
      renderPositions(appState.livePositions, appState.paperPositions, handleUnwind);
      if (posRes.paper_wallet) {
        appState.paperWallet = posRes.paper_wallet;
        renderWalletStats(posRes.paper_wallet);
      }
    }

    if (healthRes.assessment) {
      renderHealthAssessment(healthRes.assessment);
    }

    refreshIcons();
  } catch (e) {
    console.warn('Initial data warning:', e);
  }
}

// 6. 交互处理
export function openTradeModal(symbol) {
  triggerHaptic();
  const el = document.getElementById('modal-pt-symbol');
  if (el) el.value = symbol;
  openModal('modal-paper-trade');
}

export async function executePaperTrade(action) {
  triggerHaptic();
  const symbol = document.getElementById('modal-pt-symbol')?.value;
  const margin = parseFloat(document.getElementById('modal-pt-margin')?.value || '50');
  closeModal('modal-paper-trade');

  try {
    const res = await apiFetch('/api/action/paper_trade', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ symbol, margin_usd: margin, action })
    });
    if (res.status === 'ok') {
      showToast(`模拟${action === 'open' ? '开仓' : '平仓'}成功: ${symbol}`, 'success');
      fetchInitialData();
    } else {
      showToast('模拟下单失败: ' + res.message, 'error');
    }
  } catch (e) {
    showToast('请求异常: ' + e.message, 'error');
  }
}

export async function handleUnwind(symbol) {
  triggerHaptic();
  closeModal('modal-unwind-all');
  try {
    const res = await apiFetch('/api/action/unwind', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ symbol })
    });
    if (res.status === 'ok') {
      showToast(`平仓指令已发出: ${symbol}`, 'success');
      fetchInitialData();
    } else {
      showToast('平仓失败: ' + res.message, 'error');
    }
  } catch (e) {
    showToast('请求异常: ' + e.message, 'error');
  }
}

// 7. 顶部时钟与结算倒计时 (UTC+8 / Asia/Shanghai)
export function startClock() {
  setInterval(() => {
    const now = new Date();
    const clk = document.getElementById('clock-utc');
    if (clk) {
      clk.innerText = `${now.toLocaleTimeString('zh-CN', { timeZone: 'Asia/Shanghai', hour12: false })} UTC+8`;
    }
    const minsLeft = 59 - now.getUTCMinutes();
    const secsLeft = 59 - now.getUTCSeconds();
    const cd = document.getElementById('funding-countdown');
    if (cd) {
      cd.innerText = `Settlement in ${minsLeft}m ${secsLeft}s`;
    }
  }, 1000);
}

// 8. 挂载全局事件
window.switchTab = switchTab;
window.toggleTheme = toggleTheme;
window.fetchInitialData = fetchInitialData;
window.openModal = openModal;
window.closeModal = closeModal;
window.executePaperTrade = executePaperTrade;
window.handleUnwind = handleUnwind;
window.setRadarFilter = (f) => {
  appState.radarFilter = f;
  document.querySelectorAll('.rf-btn').forEach(btn => {
    btn.className = 'rf-btn px-2.5 py-1 rounded-lg text-xs font-medium bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition';
  });
  const activeBtn = document.getElementById(`rf-btn-${f}`);
  if (activeBtn) {
    activeBtn.className = 'rf-btn px-2.5 py-1 rounded-lg text-xs font-medium bg-emerald-500 text-white transition';
  }
  const kw = document.getElementById('radar-search')?.value || '';
  renderRadarTable(appState.opportunities, appState.radarFilter, kw, openTradeModal);
  refreshIcons();
};

window.filterRadarTable = () => {
  const kw = document.getElementById('radar-search')?.value || '';
  renderRadarTable(appState.opportunities, appState.radarFilter, kw, openTradeModal);
  refreshIcons();
};

window.saveConfigToServer = async () => {
  if (!appState.config) {
    appState.config = await fetchAndPopulateConfig();
  }
  appState.config = await saveCurrentConfig(appState.config);
};

window.fetchJournalEntries = () => {
  const type = document.getElementById('journal-filter-type')?.value || '';
  fetchAndRenderJournal(type);
};

// 启动入口
window.addEventListener('DOMContentLoaded', () => {
  initTheme();
  refreshIcons();
  initWebSocket();
  fetchInitialData();
  startClock();
});
