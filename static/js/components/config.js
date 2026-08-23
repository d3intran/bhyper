/**
 * BHyper Terminal - Live Strategy Config Component
 */
import { apiFetch, showToast } from '../api.js';

export async function fetchAndPopulateConfig() {
  try {
    const cfg = await apiFetch('/api/config');
    populateConfigFields(cfg);
    return cfg;
  } catch (e) {
    showToast('获取策略配置失败: ' + e.message, 'error');
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

  updated.strategy.min_open_apr_pct = getNum('cfg-min-open-apr', updated.strategy.min_open_apr_pct);
  updated.strategy.min_carry_apr_pct = getNum('cfg-min-carry-apr', updated.strategy.min_carry_apr_pct);
  updated.strategy.min_exit_apr_pct = getNum('cfg-min-exit-apr', updated.strategy.min_exit_apr_pct);
  updated.strategy.max_position_usd_per_pair = getNum('cfg-max-pos-usd', updated.strategy.max_position_usd_per_pair);
  updated.strategy.max_active_positions = getInt('cfg-max-active-pos', updated.strategy.max_active_positions);
  updated.strategy.leverage = getNum('cfg-leverage', updated.strategy.leverage);
  updated.strategy.stop_loss_basis_bps = getNum('cfg-stop-loss-bps', updated.strategy.stop_loss_basis_bps);
  updated.strategy.take_profit_basis_bps = getNum('cfg-take-profit-bps', updated.strategy.take_profit_basis_bps);
  updated.strategy.max_holding_hours = getNum('cfg-max-holding-hours', updated.strategy.max_holding_hours);

  try {
    const res = await apiFetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(updated)
    });
    if (res.status === 'ok') {
      showToast('参数热更新成功，已即刻生效', 'success');
      return updated;
    } else {
      showToast('更新失败: ' + res.message, 'error');
      return currentConfig;
    }
  } catch (e) {
    showToast('请求异常: ' + e.message, 'error');
    return currentConfig;
  }
}
