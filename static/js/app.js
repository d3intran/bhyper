/**
 * BHyper Terminal - Main Application Entrypoint
 * Mode: Operate (Institutional Clean & Modern - Pure English)
 */
import { apiFetch, closeModal, openModal, setupModalListeners, showToast } from './api.js';
import { renderHealthAssessment, renderWalletStats, renderDashboardTopRadar } from './components/overview.js';
import { renderRadarTable } from './components/radar.js';
import { renderPositions } from './components/positions.js';
import { fetchAndPopulateConfig, saveCurrentConfig } from './components/config.js';
import { fetchAndRenderJournal } from './components/journal.js';
import { renderAboutSection } from './components/about.js';
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

// 1. Initialize Telegram Mini App
if (appState.tgWebApp) {
  try {
    appState.tgWebApp.ready();
    appState.tgWebApp.expand();
    if (appState.tgWebApp.initDataUnsafe?.user) {
      const u = appState.tgWebApp.initDataUnsafe.user;
      const elUser = document.getElementById('tg-user-badge');
      if (elUser) elUser.innerText = `@${u.username || u.first_name || 'Terminal'}`;
    }
  } catch (e) {
    console.warn('Telegram init error:', e);
  }
}

// 2. Theme Management (Dark / Light)
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

// 3. Tab Routing
export function switchTab(tabId) {
  triggerHaptic();
  appState.currentTab = tabId;
  
  document.querySelectorAll('.tab-pane').forEach(el => el.classList.add('hidden'));
  const target = document.getElementById(`tab-${tabId}`);
  if (target) target.classList.remove('hidden');

  // Desktop Navigation
  document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.className = 'tab-btn px-3 py-1.5 rounded-lg font-medium text-xs transition-all flex items-center space-x-1.5 text-[var(--text-muted)] hover:text-[var(--text-primary)]';
    btn.setAttribute('aria-selected', 'false');
  });
  const activeNav = document.getElementById(`nav-btn-${tabId}`);
  if (activeNav) {
    activeNav.className = 'tab-btn px-3 py-1.5 rounded-lg font-medium text-xs transition-all flex items-center space-x-1.5 bg-emerald-500 text-white shadow-sm';
    activeNav.setAttribute('aria-selected', 'true');
  }

  // Mobile Navigation
  ['dashboard', 'radar', 'positions', 'config', 'journal', 'about'].forEach(t => {
    const mobBtn = document.getElementById(`mob-btn-${t}`);
    if (mobBtn) {
      mobBtn.className = t === tabId 
        ? 'touch-target-min flex flex-col items-center justify-center text-emerald-500 text-[10px] font-medium p-1' 
        : 'touch-target-min flex flex-col items-center justify-center text-[var(--text-muted)] text-[10px] font-medium p-1';
      mobBtn.setAttribute('aria-selected', t === tabId ? 'true' : 'false');
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
  if (tabId === 'about') {
    renderAboutSection();
  }
  refreshIcons();
}

// 4. WebSocket Communication
let reconnectTimeout = null;

export function initWebSocket() {
  if (reconnectTimeout) {
    clearTimeout(reconnectTimeout);
    reconnectTimeout = null;
  }

  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const token = new URLSearchParams(location.search).get('token');
  const wsUrl = `${protocol}//${location.host}/api/ws${token ? '?token=' + encodeURIComponent(token) : ''}`;

  try {
    if (appState.ws) {
      appState.ws.close();
    }
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
      reconnectTimeout = setTimeout(initWebSocket, 2500);
    };

    appState.ws.onerror = () => {
      // Handled by onclose
    };
  } catch (e) {
    console.error('WS init error:', e);
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

// 5. Initial Data Loading
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
        pnlEl.className = `text-2xl font-num font-bold mt-1 tracking-tight ${statusRes.total_realized_pnl_usd >= 0 ? 'text-emerald-500' : 'text-rose-500'}`;
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

// 6. Interactive Handlers
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
      showToast(`Simulated ${action === 'open' ? 'Open' : 'Close'} executed: ${symbol}`, 'success');
      fetchInitialData();
    } else {
      showToast('Simulation failed: ' + res.message, 'error');
    }
  } catch (e) {
    showToast('Network error: ' + e.message, 'error');
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
      showToast(`Unwind command dispatched: ${symbol}`, 'success');
      fetchInitialData();
    } else {
      showToast('Unwind failed: ' + res.message, 'error');
    }
  } catch (e) {
    showToast('Network error: ' + e.message, 'error');
  }
}

// 7. Header Clock & Settlement Countdown (UTC+8 / Asia/Shanghai)
export function startClock() {
  const updateClock = () => {
    const now = new Date();
    const clk = document.getElementById('clock-utc');
    if (clk) {
      clk.innerText = `${now.toLocaleTimeString('en-US', { timeZone: 'Asia/Shanghai', hour12: false })} UTC+8`;
    }
    const minsLeft = 59 - now.getUTCMinutes();
    const secsLeft = 59 - now.getUTCSeconds();
    const cd = document.getElementById('funding-countdown');
    if (cd) {
      cd.innerText = `Settlement in ${minsLeft}m ${secsLeft}s`;
    }
  };
  updateClock();
  setInterval(updateClock, 1000);
}

// 8. Keyboard Shortcuts
export function setupKeyboardShortcuts() {
  document.addEventListener('keydown', (e) => {
    if (['INPUT', 'SELECT', 'TEXTAREA'].includes(document.activeElement?.tagName)) {
      return;
    }

    if (e.key === '1') switchTab('dashboard');
    else if (e.key === '2') switchTab('radar');
    else if (e.key === '3') switchTab('positions');
    else if (e.key === '4') switchTab('config');
    else if (e.key === '5') switchTab('journal');
    else if (e.key === '6') switchTab('about');
    else if (e.key === '/') {
      e.preventDefault();
      switchTab('radar');
      const searchInput = document.getElementById('radar-search');
      if (searchInput) searchInput.focus();
    }
  });
}

// 9. Attach Global Handlers
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
    btn.className = 'rf-btn px-2.5 py-1 rounded-lg text-xs font-medium bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition active:scale-95';
    btn.setAttribute('aria-pressed', 'false');
  });
  const activeBtn = document.getElementById(`rf-btn-${f}`);
  if (activeBtn) {
    activeBtn.className = 'rf-btn px-2.5 py-1 rounded-lg text-xs font-medium bg-emerald-500 text-white transition active:scale-95';
    activeBtn.setAttribute('aria-pressed', 'true');
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

// Bootstrap
window.addEventListener('DOMContentLoaded', () => {
  initTheme();
  refreshIcons();
  setupModalListeners();
  setupKeyboardShortcuts();
  initWebSocket();
  fetchInitialData();
  startClock();
});
