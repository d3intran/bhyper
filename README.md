<div align="center">

# ⚡ BHyper 2.0

**Institutional High-Performance Binance × Hyperliquid Cross-Exchange Funding Rate & Basis Arbitrage Engine**

[![CI](https://github.com/d3intran/bhyper/actions/workflows/ci.yml/badge.svg)](https://github.com/d3intran/bhyper/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg)]()

*A zero-allocation, delta-neutral, sub-millisecond arbitrage framework built in pure Rust for quantitative traders, automated hedge funds, and small-capital agility (0 - 00).*

[English](#-highlights) | [中文说明](#-核心特性-chinese) | [Architecture](#%EF%B8%8F-architecture) | [Quickstart](#-quickstart) | [CLI Reference](#-cli-command-reference)

</div>

---

## 🌟 Highlights

- **⚡ Sub-Millisecond Pure Rust WebSocket Core**: Real-time dual WebSocket streams (`wss://fstream.binance.com` + `wss://api.hyperliquid.xyz`), in-memory lock-free `MarketDataCache`, and hardware-accelerated EIP-712 / HMAC-SHA256 crypto providers (`alloy-primitives` / `k256` / `ring` / `sha3`).
- **📊 Asymmetric Funding Normalization (8h vs 1h)**: Accurately reconciles Binance (8h epoch: 00:00, 08:00, 16:00 UTC) and Hyperliquid (1h hourly) settlement cycles, projecting 1h, 4h, and 8h net cashflows.
- **🛡️ 5 Deterministic Profitability Locks & Timing Sniper**: Pre-settlement execution windows (T - 60s ~ T - 10s), VWAP slippage bounds, and rate manipulation guards ensure positive expected net return before order dispatch.
- **🔬 Institutional Liquidity & Volatility Guards**: Automated filtering for minimum 24h volume (00k), minimum open interest (00k), max bid-ask spread (15 bps), and Oracle/Mark divergence locks (<1.5%).
- **📐 Small-Capital Precision Matching (0 ~ 00 Ready)**: Built-in GCD lot precision alignment matching Binance `stepSize` with Hyperliquid `szDecimals`, eliminating lot truncation risks and naked Delta exposure.
- **🧪 Production-Grade Paper Trading & Virtual Engine**: Full simulation environment with dual virtual wallets, realistic Maker/Taker fee deduction, margin allocation, and hourly funding accrual.
- **📖 Append-Only Trade Journal Ledger (`trade_journal.jsonl`)**: Comprehensive audit trail recording all trading events (`INTENT`, `OPEN_FILL`, `FUNDING`, `CLOSE_FILL`, `ORPHAN_UNWIND`) with microsecond timestamps.
- **📈 Quantitative Performance & PnL Attribution Analytics**: Institutional metrics reporting Win Rate, Profit Factor, Net Realized PnL, Gross Funding vs Basis PnL, Total Roundtrip Fees, and Maximum Drawdown.
- **⚖️ Cross-Exchange Margin Health & Rebalance Advisory**: Real-time margin balance monitoring and capital transfer recommendations across Binance and Hyperliquid.
- **📲 Remote Telegram Telemetry**: Instant alert dispatching for arbitrage triggers, automated position executions, margin health, and funding disbursements.

---

## 🏗️ Architecture

```mermaid
flowchart TB
    subgraph Ingestion [Market Data Ingestion]
        BN_WS[Binance FAPI WebSocket<br/>!markPrice@arr@1s]
        HL_WS[Hyperliquid L1 WebSocket<br/>allMids / userEvents / userFills]
    end

    subgraph Memory [In-Memory Cache]
        Cache[Lock-Free MarketDataCache<br/>Sub-Millisecond Multi-Asset Matrix]
    end

    subgraph Core [BHyper Arbitrage Engine]
        Ranker[Multi-Asset APR Ranker<br/>200+ Live Pairs Matrix]
        LiquidityFilter[Liquidity & Volatility Sentinel<br/>Vol > 00k, OI > 00k, Spread < 15bps]
        PrecisionMatcher[GCD Lot Precision Matcher<br/>Zero-Delta Small Capital Alignment]
        TriggerEngine[Profit Trigger Engine<br/>5 Deterministic Profit Locks & Horizons]
        RiskSentinel[Dynamic Risk Sentinel<br/>Automated Take-Profit / Stop-Loss / Decay Exit]
    end

    subgraph Execution [Order Routing & Simulation]
        PaperEngine[Paper Trading Engine<br/>Virtual Wallets, Fees & Funding Accrual]
        LiveExecutor[Two-Leg State Machine<br/>Dual-IOC & Maker-Taker EIP-712 / HMAC]
    end

    subgraph Audit [State & Ledger Auditing]
        StateStore[StateStore (state.json)<br/>Crash Recovery & Reconcile]
        Journal[Trade Journal (trade_journal.jsonl)<br/>Append-Only Microsecond Ledger]
        Analytics[Performance Analytics<br/>PnL Attribution & Win Rate Report]
    end

    subgraph Telemetry [Remote Alerts]
        TG[Telegram Telemetry Bot]
    end

    BN_WS --> Cache
    HL_WS --> Cache
    Cache --> Ranker
    Ranker --> LiquidityFilter
    LiquidityFilter --> PrecisionMatcher
    PrecisionMatcher --> TriggerEngine
    RiskSentinel --> TriggerEngine
    TriggerEngine --> PaperEngine
    TriggerEngine --> LiveExecutor
    PaperEngine --> Journal
    LiveExecutor --> StateStore
    LiveExecutor --> Journal
    Journal --> Analytics
    TriggerEngine -.-> TG
    RiskSentinel -.-> TG
```

---

## 🚀 Quickstart

### 1. Installation & Build

Ensure you have Rust 1.75+ installed (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`):

```bash
# Clone the repository
git clone https://github.com/d3intran/bhyper.git
cd bhyper

# Build optimized release binary
cargo build --release
```

### 2. Market Scanner & Live Opportunity Matrix

Scan real-time funding rate differentials and projected net returns across 200+ crypto perpetual pairs:

```bash
./target/release/bhyper scan --limit 20
```

### 3. Real-Time WebSocket Streaming Dashboard

Run the sub-second live market data stream:

```bash
./target/release/bhyper stream --limit 15
```

### 4. Precision & Small-Capital Lot Sizing

Inspect GCD precision compatibility for small capital (0 - 00):

```bash
./target/release/bhyper precision --limit 15 --target-usd 50
```

### 5. Deterministic Profit Trigger Evaluation

Evaluate opportunities against all 5 profit locks:

```bash
# Test with 0 margin allocation
./target/release/bhyper trigger --margin-usd 50

# Bypass pre-settlement timing window for testing
./target/release/bhyper trigger --margin-usd 50 --ignore-window
```

### 6. Full Simulation & Paper Trading Daemon

Run the autonomous 24/7 paper trading daemon with continuous WebSocket streaming:

```bash
# Run continuous paper trading daemon
./target/release/bhyper paper --initial-capital 100 --margin-usd 50 --interval-secs 2
```

### 7. Interactive Manual Paper Trading

Test simulated order open & close lifecycles on demand:

```bash
# Manually open a simulated arbitrage position on SUI
./target/release/bhyper paper-trade --symbol SUI --margin-usd 50 --action open

# Manually close the simulated position on SUI
./target/release/bhyper paper-trade --symbol SUI --action close

# Reset paper trading balance to 00
./target/release/bhyper reset-paper --initial-capital 100
```

### 8. Trade Execution Journal & Performance Audit

Inspect the append-only chronological ledger and quantitative PnL attribution report:

```bash
# View recent execution journal ledger entries
./target/release/bhyper journal --limit 20

# Filter journal by symbol or event type
./target/release/bhyper journal --symbol SAGA --event FUNDING

# Generate comprehensive quantitative performance report
./target/release/bhyper report --initial-capital 100
```

### 9. Cross-Exchange Margin Health & Rebalance Advisory

```bash
# Check wallet balances and capital rebalance suggestions
./target/release/bhyper balance
```

---

## 💻 CLI Command Reference

| Command | Description | Example |
| :--- | :--- | :--- |
| `scan` | Scan live funding rate opportunities across Binance and Hyperliquid | `bhyper scan --limit 20` |
| `stream` | Real-time WebSocket terminal dashboard (sub-second refresh) | `bhyper stream --limit 15` |
| `precision` | Inspect GCD lot precision and minimum notional alignment | `bhyper precision --target-usd 50` |
| `trigger` | Test opportunity feasibility against 5 profit locks | `bhyper trigger --margin-usd 50` |
| `paper` | Run 24/7 continuous autonomous paper trading simulation daemon | `bhyper paper --initial-capital 100` |
| `paper-trade`| Manually execute single simulated open or close action | `bhyper paper-trade --symbol SUI --action open` |
| `reset-paper`| Reset virtual paper trading wallet to fresh initial capital | `bhyper reset-paper --initial-capital 100` |
| `journal` | Inspect chronological append-only trade execution ledger | `bhyper journal --limit 30` |
| `report` | Generate institutional quantitative review and PnL attribution | `bhyper report --initial-capital 100` |
| `balance` | Inspect margin balances and capital rebalance advisory | `bhyper balance` |
| `trade` | Execute live two-leg arbitrage (requires `--live-danger`) | `bhyper trade --margin-usd 50 --live-danger` |
| `positions` | Inspect currently open live positions in local state store | `bhyper positions` |
| `reconcile` | Audit on-exchange live positions against local state store | `bhyper reconcile` |
| `unwind` | Emergency unwind and close open position on both exchanges | `bhyper unwind --symbol SAGA` |

---

## ⚙️ Configuration (`config.toml`)

```toml
[strategy]
min_spread_apr_pct = 20.0             # Minimum annual spread APR threshold
min_net_profit_bps = 5.0              # Minimum net profit after fees & slippage
target_notional_usd = 50.0            # Default margin allocated per trade
maker_taker_mode = "DualTaker"        # "DualTaker" | "MakerTaker"
sniper_window_secs = [300, 10]        # Pre-settlement sniper window (T-300s to T-10s)

[liquidity]
min_24h_volume_usd = 500000.0         # Exclude illiquid assets (< 00k 24h vol)
min_open_interest_usd = 200000.0      # Exclude low OI contracts (< 00k OI)
max_bid_ask_spread_bps = 15.0         # Reject wide bid-ask spreads (> 15 bps)
max_oracle_mark_divergence_pct = 1.5  # Reject sudden rate manipulation spikes

[risk]
max_active_positions = 2              # Maximum concurrent arbitrage positions
stop_loss_pct = 0.5                   # Basis divergence stop-loss threshold
take_profit_apr_decay_pct = 5.0       # Auto-exit when funding APR decays below 5%
max_drawdown_pct = 5.0                # Global portfolio drawdown breaker
```

---

## 🇨🇳 核心特性 (Chinese)

- **⚡ 极速 Rust WebSocket 双所直连**：亚毫秒级处理币安 `!markPrice@arr@1s` 与 Hyperliquid `allMids` 全量行情流，内存无锁高性能运算，微秒级 EIP-712 与 HMAC 硬件签名。
- **📊 8h / 1h 跨交易所结算排期智能归一化**：精准处理币安 8 小时大周期与 Hyperliquid 1 小时整点周期的非对称结算，动态计算 1h、4h、8h 多持仓周期净现金流。
- **🛡️ 机构级流动性与防插针风控哨兵**：内置 24h 成交额（00k）、持仓量 OI（00k）、买卖盘口价差（15 bps）与预言机偏离锁（<1.5%），彻底杜绝费率突变与操纵风险。
- **📐 GCD 步长对齐与小资金保护机制（0 ~ 00 本金适配）**：内置 `LotPrecisionMatcher` 算法，严格对齐币安 `stepSize` 与 Hyperliquid `szDecimals`，实现 100% 零 Delta 漂移。
- **🧪 工业级模拟盘环境与全息交易流水账本**：
  - 双所虚拟钱包管理（支持资金划拨与 Maker/Taker 费率真实扣减）。
  - 支持 7×24 小时后台守护挂载（`bhyper paper`）与交互式单笔即时测试（`bhyper paper-trade`）。
  - 追加写入式交易流水账本 `trade_journal.jsonl`，详尽记录 `INTENT`、`OPEN_FILL`、`FUNDING`、`CLOSE_FILL` 全生命周期事件。
  - 一键生成机构级量化复盘报告（`bhyper report`），清晰拆解资金费收入（Gross Funding）、基差损益（Basis PnL）、交易手续费（Fees）与胜率。
- **⚖️ 跨所保证金健康度诊断与调仓建议**：实时评估两所保证金分布，生成最优调仓建议。
- **📲 Telegram 实时监控与远程预警**：开平仓、利差衰减平仓、整点资金费到账实时推送到群聊。

---

## 📜 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
