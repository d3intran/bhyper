/**
 * BHyper Terminal - Live Strategy Config Component
 * Pure English Institutional Layout
 */
import { apiFetch, showToast } from '../api.js';

export async function fetchAndPopulateConfig() {
  try {
    const cfg = await apiFetch('/api/config');
    populateConfigFields(cfg);
    return cfg;
  } catch (e) {
    showToast('Failed to load strategy configuration: ' + e.message, 'error');
    return null;
  }
}

export function populateConfigFields(cfg) {
  if (!cfg?.strategy) return;
  const s = cfg.strategy;
  
  const setVal = (id, val) => {
    const el = document.getElementById(id);
    if (el && val !== undefined) el.value = val;
  };

  setVal('cfg-min-open-apr', s.min_open_apr_pct);
  setVal('cfg-min-carry-apr', s.min_carry_apr_pct);
  setVal('cfg-min-exit-apr', s.min_exit_apr_pct);
  setVal('cfg-max-pos-usd', s.max_position_usd_per_pair);
  setVal('cfg-max-active-pos', s.max_active_positions);
  setVal('cfg-leverage', s.leverage);
  setVal('cfg-stop-loss-bps', s.stop_loss_basis_bps);
  setVal('cfg-take-profit-bps', s.take_profit_basis_bps);
  setVal('cfg-max-holding-hours', s.max_holding_hours);

  const elRot = document.getElementById('cfg-auto-rotation');
  if (elRot && s.auto_rotation_enabled !== undefined) {
    elRot.checked = s.auto_rotation_enabled;
  }
  const elDyn = document.getElementById('cfg-dynamic-sizing');
  if (elDyn && s.dynamic_sizing_enabled !== undefined) {
    elDyn.checked = s.dynamic_sizing_enabled;
  }
  setVal('cfg-safety-buffer', s.liquidation_safety_buffer_pct);
  setVal('cfg-min-swap-apr', s.min_swap_apr_delta_pct);
  setVal('cfg-min-swap-profit', s.min_swap_profit_usd);
  setVal('cfg-min-holding-mins', s.min_holding_mins_before_swap);
}

export async function saveCurrentConfig(currentConfig) {
  if (!currentConfig) return;
  const updated = JSON.parse(JSON.stringify(currentConfig));

  const getNum = (id, fallback) => {
    const el = document.getElementById(id);
    return el && el.value !== '' ? parseFloat(el.value) : fallback;
  };
  const getInt = (id, fallback) => {
    const el = document.getElementById(id);
    return el && el.value !== '' ? parseInt(el.value, 10) : fallback;
  };
  const getBool = (id, fallback) => {
    const el = document.getElementById(id);
    return el ? el.checked : fallback;
  };

  updated.strategy.min_open_apr_pct = getNum('cfg-min-open-apr', updated.strategy.min_open_apr_pct);
  updated.strategy.min_carry_apr_pct = getNum('cfg-min-carry-apr', updated.strategy.min_carry_apr_pct);
  updated.strategy.min_exit_apr_pct = getNum('cfg-min-exit-apr', updated.strategy.min_exit_apr_pct);
  updated.strategy.max_position_usd_per_pair = getNum('cfg-max-pos-usd', updated.strategy.max_position_usd_per_pair);
  updated.strategy.max_single_position_cap_usd = getNum('cfg-max-pos-usd', updated.strategy.max_single_position_cap_usd ?? 200.0);
  updated.strategy.liquidation_safety_buffer_pct = getNum('cfg-safety-buffer', updated.strategy.liquidation_safety_buffer_pct ?? 15.0);
  updated.strategy.dynamic_sizing_enabled = getBool('cfg-dynamic-sizing', updated.strategy.dynamic_sizing_enabled ?? true);
  updated.strategy.max_active_positions = getInt('cfg-max-active-pos', updated.strategy.max_active_positions);
  updated.strategy.leverage = getNum('cfg-leverage', updated.strategy.leverage);
  updated.strategy.stop_loss_basis_bps = getNum('cfg-stop-loss-bps', updated.strategy.stop_loss_basis_bps);
  updated.strategy.take_profit_basis_bps = getNum('cfg-take-profit-bps', updated.strategy.take_profit_basis_bps);
  updated.strategy.max_holding_hours = getNum('cfg-max-holding-hours', updated.strategy.max_holding_hours);

  updated.strategy.auto_rotation_enabled = getBool('cfg-auto-rotation', updated.strategy.auto_rotation_enabled ?? true);
  updated.strategy.min_swap_apr_delta_pct = getNum('cfg-min-swap-apr', updated.strategy.min_swap_apr_delta_pct ?? 25.0);
  updated.strategy.min_swap_profit_usd = getNum('cfg-min-swap-profit', updated.strategy.min_swap_profit_usd ?? 0.04);
  updated.strategy.min_holding_mins_before_swap = getNum('cfg-min-holding-mins', updated.strategy.min_holding_mins_before_swap ?? 15.0);

  try {
    const res = await apiFetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(updated)
    });
    if (res.status === 'ok') {
      showToast('Configuration hot-reloaded and saved successfully', 'success');
      return updated;
    } else {
      showToast('Failed to update config: ' + res.message, 'error');
      return currentConfig;
    }
  } catch (e) {
    showToast('Network request error: ' + e.message, 'error');
    return currentConfig;
  }
}
