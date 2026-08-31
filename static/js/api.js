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
    m.setAttribute('aria-hidden', 'false');
    const firstInput = m.querySelector('input:not([readonly]), button');
    if (firstInput) firstInput.focus();
  }
}

export function closeModal(id) {
  const m = document.getElementById(id);
  if (m) {
    m.classList.remove('flex');
    m.classList.add('hidden');
    m.setAttribute('aria-hidden', 'true');
  }
}

export function setupModalListeners() {
  // Global Escape key to dismiss modals
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      document.querySelectorAll('[id^="modal-"]:not(.hidden)').forEach(modal => {
        closeModal(modal.id);
      });
    }
  });

  // Click backdrop to dismiss
  document.querySelectorAll('[id^="modal-"]').forEach(modal => {
    modal.addEventListener('click', (e) => {
      if (e.target === modal) {
        closeModal(modal.id);
      }
    });
  });
}

export function showToast(msg, type = 'info') {
  const container = document.getElementById('toast-container');
  if (!container) return;
  
  const el = document.createElement('div');
  const isError = type === 'error';
  const isSuccess = type === 'success';
  
  // Toasts ride an inverse surface so they stay legible over both the Obsidian
  // and Studio canvases. The accent lives in the icon and the hairline, never
  // in a tinted fill that would fight the semantic palette.
  let toneClass = 'border-edge-inverse';
  let icon = '<i data-lucide="info" class="w-4 h-4 opacity-70"></i>';

  if (isError) {
    toneClass = 'border-rose-500/40';
    icon = '<i data-lucide="alert-triangle" class="w-4 h-4 text-rose-400"></i>';
  } else if (isSuccess) {
    toneClass = 'border-emerald-500/40';
    icon = '<i data-lucide="check-circle" class="w-4 h-4 text-emerald-400"></i>';
  }

  el.className = `bg-inverse ${toneClass} text-inverse border px-3.5 py-2.5 rounded-lg shadow-lg text-xs font-medium transition-all duration-200 transform translate-y-2 opacity-0 flex items-center space-x-2.5 max-w-sm pointer-events-auto`;
  el.innerHTML = `<span>${icon}</span><span class="flex-1">${msg}</span>`;
  container.appendChild(el);

  if (window.lucide) {
    window.lucide.createIcons();
  }

  requestAnimationFrame(() => {
    el.classList.remove('translate-y-2', 'opacity-0');
  });

  setTimeout(() => {
    el.classList.add('opacity-0', '-translate-y-2');
    setTimeout(() => el.remove(), 250);
  }, 3200);
}
