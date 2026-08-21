<div align="center">

# ⚡ BHyper

**Ultra Low-Latency Binance × Hyperliquid Cross-Exchange Funding Rate & Basis Arbitrage Engine**

[![CI](https://github.com/d3intran/bhyper/actions/workflows/ci.yml/badge.svg)](https://github.com/d3intran/bhyper/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg)]()

*A deterministic, delta-neutral, high-frequency arbitrage framework built in pure Rust for quantitative traders and small-capital agility ($50 - $500).*

[English](#features) | [中文说明](#核心特性-chinese) | [Architecture](#architecture) | [Quickstart](#quickstart) | [Full Blueprint](PLAN.md)

</div>

---

## 🌟 Highlights

- **⚡ Sub-Millisecond Pure Rust WebSocket Engine**: Real-time dual WebSocket feeds (`wss://fstream.binance.com` + `wss://api.hyperliquid.xyz`), in-memory lock-free `MarketDataCache`, and hardware-accelerated EIP-712 / HMAC-SHA256 signing (`alloy-primitives` / `k256` / `ring` / `sha3` / `rmp-serde`).
- **📊 Real-Time Multi-Asset APR & Settlement Schedule Alignment**: Accurately handles the asymmetric settlement cycles between Binance (8h epoch: 00:00, 08:00, 16:00 UTC) and Hyperliquid (1h hourly), computing projected 1h, 4h, and 8h net cashflows.
- **🎯 5 Profitability Locks & Timing Sniper**: Enforces VWAP calculations, entry basis cushion guards, and pre-settlement execution windows (T - 60s ~ T - 10s) ensuring positive expected returns on every trade.
- **🛡️ Small-Capital Protection & GCD Lot Precision Alignment ($50 ~ $100 Ready)**: Specifically engineered GCD precision matching algorithm eliminating lot step-size truncation risks, exchange minimum notional constraints ($12 buffer), and multi-leg orphan exposure.
- **🧪 Verified Two-Leg Execution Engine**:
  - **Dual-IOC Taker-Taker**: Atomically validates fill on Hyperliquid before dispatching the Binance hedge; aborts with zero exposure if unfilled.
  - **Maker-Taker**: Submits ALO Post-Only maker orders on Hyperliquid with active fill-polling and automated timeout cancellation, completely eliminating naked exposure risks.
- **💾 State Persistence & Cross-Exchange Reconciliation**: Persistent local state storage (`state.json`), automatic restart recovery, and on-exchange position reconciliation to detect and adopt or unwind orphaned positions.
- **📲 Remote Telegram Telemetry**: Automated Telegram alert dispatching for arbitrage triggers, live margin health, and hourly funding disbursements.

---

## 🏗️ Architecture

```mermaid
flowchart TB
    subgraph Ingestion [Market Data Ingestion]
        BN_WS[Binance FAPI WebSocket<br/>!markPrice@arr@1s]
        HL_WS[Hyperliquid L1 WebSocket<br/>allMids / userEvents / userFills]
    end

    subgraph Memory [In-Memory State]
        Cache[Thread-Safe MarketDataCache<br/>Sub-Millisecond Opportunity Matrix]
    end

    subgraph Core [BHyper Arbitrage Core]
        Ranker[Multi-Asset APR Ranker<br/>200+ Live Pairs Matrix]
        PrecisionMatcher[GCD Lot Precision Matcher<br/>0-Delta Small Capital Alignment]
        TriggerEngine[Profit Trigger Engine<br/>5 Deterministic Profit Locks & 8h/1h Horizons]
        RiskSentinel[Dynamic Risk Sentinel<br/>Delta Neutral & Margin Guard]
        StateStore[Atomic StateStore (state.json)<br/>Crash Recovery & History Audit]
    end

    subgraph Execution [Verified Order Router]
        HL_Maker[Hyperliquid L1 Client<br/>Post-Only ALO / IOC EIP-712]
        BN_Taker[Binance FAPI Client<br/>Instant Atomic IOC Hedge / HMAC-SHA256]
        TwoLegExecutor[Two-Leg State Machine<br/>Fill Verification & Orphan Unwind]
    end

    subgraph Alerts [Remote Telemetry]
        TG[Telegram Bot Alerts]
    end

    BN_WS --> Cache
    HL_WS --> Cache
    Cache --> Ranker
    Ranker --> PrecisionMatcher
    PrecisionMatcher --> TriggerEngine
    RiskSentinel --> TriggerEngine
    TriggerEngine --> TwoLegExecutor
    TwoLegExecutor --> StateStore
    TwoLegExecutor --> HL_Maker
    TwoLegExecutor --> BN_Taker
    TwoLegExecutor -.->|Execution Alerts| TG
    RiskSentinel -.->|Risk Alerts| TG
```

---

## 🚀 Quickstart

### 1. Installation & Build

Ensure you have Rust installed (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`):

```bash
# Clone the repository
git clone https://github.com/d3intran/bhyper.git
cd bhyper

# Build optimized release binary
cargo build --release
```

### 2. Live Market Scanner (Multi-Horizon Net PnL)

Scan live funding rate spreads across all 200+ shared pairs with 1h/4h/8h net profit estimates:

```bash
./target/release/bhyper scan --limit 20
```

### 3. Real-Time WebSocket Streaming Dashboard

Run the sub-second live market data stream and opportunity matrix:

```bash
./target/release/bhyper stream --limit 15
```

### 4. GCD Lot Precision Alignment Analysis

Inspect small-capital precision compatibility across all shared pairs:

```bash
./target/release/bhyper precision --limit 15 --target-usd 50
```

### 5. Deterministic Profit Trigger Evaluation

Evaluate live opportunities through the 5 Profitability Locks, Exact Settlement Schedule, & Lot Precision Matcher:

```bash
# Evaluate with $50 margin allocation
./target/release/bhyper trigger --margin-usd 50

# Evaluate bypassing the pre-settlement hourly window (for inspection)
./target/release/bhyper trigger --margin-usd 50 --ignore-window
```

### 6. Automated Arbitrage Execution Engine (Paper Trading & Live)

```bash
# Run Safe Paper Trading Simulation Mode (Default)
./target/release/bhyper trade --margin-usd 50 --dry-run true

# Run Live Trading Mode with verified fill hedging (Requires explicit --live-danger flag)
./target/release/bhyper trade --margin-usd 50 --live-danger

# Run Live Trading in Dual-IOC Taker-Taker mode
./target/release/bhyper trade --margin-usd 50 --live-danger --taker-taker
```

### 7. Position Inspection & Exchange Reconciliation

```bash
# View active managed positions in persistent storage
./target/release/bhyper positions

# Audit and reconcile on-exchange live positions with local state
./target/release/bhyper reconcile
```

### 8. Emergency Manual Unwinding

Instantly close positions on both exchanges simultaneously:

```bash
./target/release/bhyper unwind --symbol SAGA
```

---

## 🇨🇳 核心特性 (Chinese)

- **⚡ 纯 Rust 亚毫秒级 WebSocket 流式架构**：双所 WebSocket 实时行情直连，内存无锁/读写锁缓存，微秒级 EIP-712 / HMAC 硬件加速签名。
- **📊 全币种实时费率与 8h/1h 结算排期归一化**：精确区分币安 8 小时大节点与 Hyperliquid 1 小时整点结算时序，提供 1h、4h、8h 多持仓周期净现金流测算。
- **🎯 5 重确定性盈利锁与整点狙击**：结合盘口全深度 VWAP 计算与整点前窗口狙击，严格覆盖双向手续费与滑点磨损。
- **🛡️ 真实成交校验与两腿对冲状态机**：
  - **Taker-Taker（双向 IOC）**：Hyperliquid 确认成交后再对冲币安；未成交立即终止，杜绝单边裸头寸。
  - **Maker-Taker（挂单成交追踪）**：支持 Post-Only 挂单、超时自动撤单与部分成交按比例对冲，并内置孤儿腿紧急平仓熔断。
- **💾 本地状态持久化与跨所持仓对账**：状态自动落盘至 `state.json`，支持崩溃恢复与 `reconcile` 跨所头寸自动稽核与领养。
- **🛡️ GCD 步长对齐与小资金保护机制（$50 ~ $100 初始本金优化）**：内置 `LotPrecisionMatcher` 算法，严格对齐币安 `stepSize` 与 Hyperliquid `szDecimals`，实现 100% 零 Delta 漂移。
- **📲 Telegram 实时监控与远程预警**：发现高利润机会、完成建仓平仓与风控异常即刻推送。

---

## 📜 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
