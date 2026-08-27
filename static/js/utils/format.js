/**
 * BHyper Terminal - Formatting & Haptic Utilities
 * High-Precision Institutional Formatting
 */

export function formatPrice(p) {
  if (p === undefined || p === null || isNaN(p)) return '$0.00';
  const num = Number(p);
  if (num >= 1000) return `$${num.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
  if (num >= 1) return `$${num.toFixed(3)}`;
  if (num >= 0.01) return `$${num.toFixed(4)}`;
  return `$${num.toFixed(6)}`;
}

export function formatCurrency(val, dec = 2) {
  if (val === undefined || val === null || isNaN(val)) return '$0.00';
  const num = Number(val);
  return `$${num.toLocaleString('en-US', { minimumFractionDigits: dec, maximumFractionDigits: dec })}`;
}

export function formatPnl(val, dec = 4) {
  if (val === undefined || val === null || isNaN(val)) return '+$0.0000';
  const num = Number(val);
  const sign = num >= 0 ? '+' : '-';
  return `${sign}$${Math.abs(num).toLocaleString('en-US', { minimumFractionDigits: dec, maximumFractionDigits: dec })}`;
}

export function formatTimeUtc8(iso) {
  if (!iso) return '-';
  try {
    return new Date(iso).toLocaleString('zh-CN', { timeZone: 'Asia/Shanghai', hour12: false });
  } catch {
    return String(iso);
  }
}

export function formatTimeOnlyUtc8(iso) {
  if (!iso) return '-';
  try {
    return new Date(iso).toLocaleTimeString('zh-CN', { timeZone: 'Asia/Shanghai', hour12: false });
  } catch {
    return String(iso);
  }
}

export function triggerHaptic() {
  if (window.Telegram?.WebApp?.HapticFeedback) {
    try {
      window.Telegram.WebApp.HapticFeedback.impactOccurred('light');
    } catch {
      // Ignore if not supported
    }
  }
}
