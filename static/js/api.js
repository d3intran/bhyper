/**
 * BHyper Terminal - API Client & Modal/Toast Manager
 */

export async function apiFetch(url, options = {}) {
  options.headers = options.headers || {};
  
  if (window.Telegram?.WebApp?.initData) {
    options.headers['X-TG-Init-Data'] = window.Telegram.WebApp.initData;
  }
  
  const token = new URLSearchParams(location.search).get('token');
  if (token) {
    options.headers['Authorization'] = `Bearer ${token}`;
  }

  const res = await fetch(url, options);
  if (!res.ok) {
    throw new Error(`HTTP ${res.status}: ${res.statusText}`);
  }
  return await res.json();
}

export function openModal(id) {
  const m = document.getElementById(id);
  if (m) {
    m.classList.remove('hidden');
    m.classList.add('flex');
  }
}

export function closeModal(id) {
  const m = document.getElementById(id);
  if (m) {
    m.classList.remove('flex');
    m.classList.add('hidden');
  }
}

export function showToast(msg, type = 'info') {
  const container = document.getElementById('toast-container');
  if (!container) return;
  
  const el = document.createElement('div');
  const isError = type === 'error';
  const isSuccess = type === 'success';
  
  let bgClass = 'bg-slate-900 border border-slate-700 text-white';
  if (isError) bgClass = 'bg-rose-950/90 border border-rose-800 text-rose-200';
  if (isSuccess) bgClass = 'bg-emerald-950/90 border border-emerald-800 text-emerald-200';

  el.className = `${bgClass} px-3.5 py-2 rounded-lg shadow-lg text-xs font-medium transition-all duration-300 transform translate-y-2 opacity-0 flex items-center space-x-2 backdrop-blur-md`;
  el.innerHTML = `<span>${isError ? '✕' : '✓'}</span><span>${msg}</span>`;
  container.appendChild(el);

  requestAnimationFrame(() => {
    el.classList.remove('translate-y-2', 'opacity-0');
  });

  setTimeout(() => {
    el.classList.add('opacity-0', '-translate-y-2');
    setTimeout(() => el.remove(), 300);
  }, 3000);
}
