/**
 * BHyper Terminal - Formatting Utilities
 */

export function formatPrice(p) {
  if (p === undefined || p === null || isNaN(p)) return '$0.00';
  if (p >= 1000) return `$${p.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
  if (p >= 1) return `$${p.toFixed(3)}`;
  if (p >= 0.01) return `$${p.toFixed(4)}`;
  return `$${p.toFixed(6)}`;
}

export function formatCurrency(val, dec = 2) {
  if (val === undefined || val === null || isNaN(val)) return '$0.00';
  return `$${Number(val).toFixed(dec)}`;
}

export function formatPnl(val, dec = 4) {
  if (val === undefined || val === null || isNaN(val)) return '+$0.0000';
  const num = Number(val);
  const sign = num >= 0 ? '+' : '-';
  return `${sign}$${Math.abs(num).toFixed(dec)}`;
}

export function formatTimeUtc8(iso) {
  if (!iso) return '-';
  return new Date(iso).toLocaleString('zh-CN', { timeZone: 'Asia/Shanghai', hour12: false });
}

export function formatTimeOnlyUtc8(iso) {
  if (!iso) return '-';
  return new Date(iso).toLocaleTimeString('zh-CN', { timeZone: 'Asia/Shanghai', hour12: false });
}

export function triggerHaptic() {
  if (window.Telegram?.WebApp?.HapticFeedback) {
    window.Telegram.WebApp.HapticFeedback.impactOccurred('light');
  }
}
