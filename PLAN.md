# ⚡ BHyper: 极致低延迟 Binance × Hyperliquid 跨所资金费率套利系统设计与实施计划书

> **Project Codename**: `BHyper`  
> **Author**: Antigravity High-Performance Quant Engineering  
> **Target Target Environment**: Azure Japan East (Tokyo) VPS / Local Rust Cross-Compilation  
> **Version**: 1.0.0-Production-Blueprint  
> **Last Updated**: 2026-08-21  

---

## 📑 目录 (Table of Contents)

1. [项目愿景与核心目标 (Executive Summary & Mission)](#1-项目愿景与核心目标)
2. [开源高星项目深度调研与技术借鉴 (Open-Source Ecosystem Benchmarking)](#2-开源高星项目深度调研与技术借鉴)
3. [跨所资金费率套利数学与经济学模型 (Mathematical & Economic Model)](#3-跨所资金费率套利数学与经济学模型)
4. [极致性能与零分配系统架构 (Zero-Allocation Low-Latency Architecture)](#4-极致性能与零分配系统架构)
5. [核心子系统工程设计 (Core Subsystem Engineering Design)](#5-核心子系统工程设计)
   - 5.1 极速双向行情管道 (Market Ingestion Pipeline)
   - 5.2 全资产利差矩阵与状态机 (Funding Arbitrage Matrix & State Engine)
   - 5.3 智能两腿对冲执行引擎 (Smart Order Router & Hedging Engine)
   - 5.4 动态风控与 Delta 中性哨兵 (Risk Sentinel & Delta Tracker)
   - 5.5 Telegram 遥测与交互中枢 (Telegram Telemetry & Control Center)
6. [网络与 VPS 系统级低延迟调优 (System & Network Optimization)](#6-网络与-vps-系统级低延迟调优)
7. [分阶段实施与验证里程碑 (Roadmap & Milestone Schedule)](#7-分阶段实施与验证里程碑)

---

## 1. 项目愿景与核心目标

`BHyper` 是一套专为 **小资金、高胜率、极稳健增长** 打造的跨交易所资金费率套利与基差套利量化系统。它聚焦于全球流动性最强的中心化合约交易所 **Binance (币安)** 与最具流动性的去中心化 L1 订单簿永续合约平台 **Hyperliquid**。

### 核心设计原则：
1. **Delta 中性与本金安全至上**：
   - 无论底层加密资产（BTC, ETH, SOL, SUI, DOGE 等）暴涨或暴跌，系统双向名义价值始终保持 1:1 对冲，杜绝方向性风险。
2. **极致计算与网络性能 (Extreme Low-Latency)**：
   - 采用纯 **Rust** 编写，拒绝任何垃圾回收（Zero-GC）与多余内存拷贝。
   - 充分利用云端 VPS 位于 **日本东区 (Tokyo)** 的物理地理优势（距 Binance AWS 东京机房 ~29ms，距 Hyperliquid CloudFront 节点 ~1.38ms），在费率结算窗口前与价差波动中实现亚毫秒级的两腿极速下单。
3. **真实费用补偿与多资产自动化轮动**：
   - 内置滑点与双边手续费精确折算模型，仅在「净费率差收益率 > 磨损成本」时触发建仓。
   - 支持 50+ 个币安与 Hyperliquid 共同上线币种的 24 小时实时费率扫描，资金永远停留在利差最大、风险最低的标的上。

---

## 2. 开源高星项目深度调研与技术借鉴

在架构设计前，我们对 GitHub 上的顶尖开源项目与高频量化架构进行了深度解构与优缺点剖析：

| 项目名称 / Repo | 核心技术栈 | 核心优势 | 潜在瓶颈 / 缺失 | BHyper 吸收与优化点 |
| :--- | :--- | :--- | :--- | :--- |
| **[lhermoso/hyperliquid-rust-sdk](https://github.com/lhermoso/hyperliquid-rust-sdk)** | Rust, `fastwebsockets`, `simd-json`, `hyper`, `alloy-rs` | 极速无锁解析，直接集成 `simd-json`，零多余封装，EIP-712 签名极快 | 仅为单一交易所 SDK，无跨所撮合对冲逻辑 | **完全采用其高性能通信哲学**：引入 `simd-json` 与 `fastwebsockets` 打造超高速底层连接 |
| **[infinitefield/hypersdk](https://github.com/infinitefield/hypersdk)** | Rust, `alloy`, `rust_decimal` | 工业级类型安全，支持 HyperCore L1 完整接口，EIP-712 签名精准 | 依赖较重，部分异步接口存在不必要的堆分配 | **借鉴其数据结构定义与 EIP-712 签名规范**，但对核心路径做零拷贝精简 |
| **[nautilus_trader](https://github.com/nautechsystems/nautilus_trader)** | Rust + Cython / Python | 架构极为宏大，支持多资产撮合回测与事件驱动 | 系统过于庞大臃肿，对低配 1GB 内存 VPS 资源占用过高 | **学习其 Actor 驱动事件分发模型**，但用纯轻量 Rust 实现，避免 Python 混合开销 |
| **[rustjesty/hyperliquid-drift-arbitrage-bot](https://github.com/rustjesty/hyperliquid-drift-arbitrage-bot)** | Rust, Tokio | 专注于跨所套利（Solana Drift vs HL），具备两腿对冲状态机 | 策略较固定，缺少动态费用测算与断网熔断保护 | **强化其两腿状态机（Leg State Machine）**，完善单边挂单超时自动对冲逻辑 |
| **[second-state/fintool](https://github.com/second-state/fintool)** | Rust CLI, Binance + HL | 轻量 CLI 工具，清晰展示费率与持仓数据 | 偏向手动/半自动分析，缺乏自动化毫秒级执行引擎 | **参考其多所数据模型标准化设计** |

---

## 3. 跨所资金费率套利数学与经济学模型

### 3.1 结算机制差异对比

| 维度 | Binance 永续合约 (FAPI) | Hyperliquid 永续合约 (L1) |
| :--- | :--- | :--- |
| **资金费结算周期** | **8 小时** 一次 (00:00, 08:00, 16:00 UTC) | **1 小时** 一次 (整点结算，00:00, 01:00... UTC) |
| **资金费率计算公式** | 基于 8 小时溢价指数 TWAP，受 $\pm 0.05\%$ 利率项调节与上限钳位 | 基于过去 1 小时 (Mark Price - Index Price) / Index 的 TWAP |
| **费率支付方向** | 费率 $>0$ 时多付空；费率 $<0$ 时空付多 | 费率 $>0$ 时多付空；费率 $<0$ 时空付多 |
| **手续费率基准** | Maker: 0.02% / Taker: 0.05% (BNB 抵扣 10% off) | Maker: 0.01% (甚至负返佣) / Taker: 0.035% |

### 3.2 年化资金费率 (APR) 统一归一化公式

为了在不同周期的平台间进行无量纲比较，系统将所有瞬时资金费率标准化为 **年化百分比 (APR)**：

$$\text{APR}_{\text{Binance}} = R_{\text{BN, 8h}} \times 3 \times 365 = R_{\text{BN, 8h}} \times 1095$$

$$\text{APR}_{\text{Hyperliquid}} = R_{\text{HL, 1h}} \times 24 \times 365 = R_{\text{HL, 1h}} \times 8760$$

### 3.3 跨所利差与开平仓判断方程

定义净资金费利差：
$$\Delta \text{APR} = \text{APR}_{\text{Hyperliquid}} - \text{APR}_{\text{Binance}}$$

#### 交易决策矩阵：
1. **正利差机会 ($\Delta \text{APR} > \text{Threshold}_{\text{open}}$，例如 $+40\%$)**：
   - **Hyperliquid 开空 (Short)**：收取 Hyperliquid 每小时发放的高额资金费。
   - **Binance 开多 (Long)**：支付 Binance 较低的资金费（或同样收取负费率）。
2. **负利差机会 ($\Delta \text{APR} < -\text{Threshold}_{\text{open}}$，例如 $-40\%$)**：
   - **Hyperliquid 开多 (Long)**：支付低费率或收取负费率。
   - **Binance 开空 (Short)**：收取 Binance 8 小时的高额资金费。

### 3.4 摩擦成本与盈亏平衡持仓周期模型

双边开仓与平仓的总交易摩擦成本：
$$\text{Cost}_{\text{roundtrip}} = 2 \times \left( \text{Fee}_{\text{HL}} + \text{Fee}_{\text{BN}} + \text{Slippage}_{\text{HL}} + \text{Slippage}_{\text{BN}} \right)$$

- 若采用 **Maker-Taker 模式**（HL 挂单 Maker 0.00% + BN 市价 Taker 0.04% + 平均滑点 0.02%）：
  $$\text{Cost}_{\text{roundtrip}} \approx 2 \times (0.00\% + 0.04\% + 0.00\% + 0.02\%) = 0.12\% \text{ (12 bps)}$$
- 若两所年化利差 $\Delta \text{APR} = 60\%$（折合每小时收益 $\approx 0.00685\%$）：
  $$\text{Break-Even Time} = \frac{\text{Cost}_{\text{roundtrip}}}{\text{Hourly Rate Spread}} = \frac{0.12\%}{0.00685\%} \approx 17.5 \text{ 小时}$$

**结论**：只要利差维持超过 18 小时，之后每一小时产生的资金费均为纯被动净利润！

---

## 4. 极致性能与零分配系统架构

```mermaid
flowchart TB
    subgraph MarketDataLayer [零拷贝行情采集层]
        BN_WS[Binance FAPI WebSocket<br/>bookTicker / markPrice]
        HL_WS[Hyperliquid L1 WebSocket<br/>allMids / activeAssetCtx]
    end

    subgraph FastChannel [无锁内存环形缓冲区 (Crossbeam SPSC)]
        RingBuf[Lock-Free RingBuffer<br/>Atomic Sequencer]
    end

    subgraph CoreEngine [BHyper 策略与决策核心]
        FundingMatrix[Multi-Asset Spread Ranker<br/>50+ Pairs Dynamic Matrix]
        StateRouter[Execution State Machine<br/>Maker-Taker / Taker-Taker]
        RiskSentinel[Delta & Margin Sentinel<br/>10ms Periodic Heartbeat]
    end

    subgraph ExecutionLayer [预热连接与极速签名执行层]
        BN_Client[Binance FAPI HTTP Pool<br/>Keep-Alive + Ring HMAC-SHA256]
        HL_Client[Hyperliquid L1 Client<br/>Keep-Alive + Alloy EIP-712]
    end

    subgraph TelemetryLayer [遥测与远程指令]
        TG_Bot[Telegram Sentinel Bot<br/>Live Alerts & Status Cards]
    end

    BN_WS -->|fastwebsockets + simd-json| RingBuf
    HL_WS -->|fastwebsockets + simd-json| RingBuf
    RingBuf --> FundingMatrix
    FundingMatrix --> StateRouter
    RiskSentinel --> StateRouter
    StateRouter --> BN_Client
    StateRouter --> HL_Client
    RiskSentinel -.->|Alerts| TG_Bot
```

### 关键工程优化亮点：
1. **零堆分配序列化**：
   - 丢弃通用 `serde_json::Value`，对 Binance 和 Hyperliquid 订单数据采用固定尺寸的 `struct`，并在关键网络入包使用 `simd-json`，单条解析耗时从微秒级降至 **数十纳秒级**。
2. **HTTP 连接池长连接复用 (Keep-Alive Pre-Warmed Pools)**：
   - 维护独立的 `reqwest::Client` / `hyper` 连接池，预热建立与 `fapi.binance.com` 和 `api.hyperliquid.xyz` 的 TLS 会话，杜绝下单时发生 TCP + TLS 握手（避免 50ms+ 延迟浪费）。
3. **EIP-712 硬件加速签名**：
   - 使用 `alloy-primitives` 与 `k256` 进行紧凑内存布局的结构化签名，下单签名时间控制在 **< 8µs**。

---

## 5. 核心子系统工程设计

### 5.1 极速双向行情管道 (`src/binance/` & `src/hyperliquid/`)
- **Binance 行情流**：订阅 `<symbol>@bookTicker`（100ms/实时最优买卖价）与 `<symbol>@markPrice`（每秒标记价格与实时推导资金费率）。
- **Hyperliquid 行情流**：订阅 `activeAssetCtx`（包含所有标的的 Oracle Price, Mark Price, Open Interest, Funding Rate, Premium）与 `allMids`。

### 5.2 全资产利差矩阵与状态机 (`src/strategy/`)
- 维护双向持仓状态机：
  - `Idle`（空闲巡检） $\to$ `EnteringLeg1`（第一腿建仓） $\to$ `EnteringLeg2`（第二腿极速对冲） $\to$ `Holding`（中性收租中） $\to$ `Exiting`（利差收敛平仓） $\to$ `EmergencyUnwind`（单边失败紧急平仓）。
- **Maker-Taker 极速对冲模式**：
  1. 在 Hyperliquid（享受挂单低手续费）盘口前列挂 Maker 限价单；
  2. 监听 Hyperliquid `userFills` WebSocket 推送；
  3. 一旦收到 partial/full fill 事件，在 **5ms 内** 自动向 Binance FAPI 发送市价（IOC）吃单，完成名义价值精准对冲。

### 5.3 动态风控与 Delta 中性哨兵 (`src/risk/`)
- **Delta 漂移实时监控**：
  $$\Delta_{\text{net}} = \text{Position}_{\text{HL}} \times P_{\text{HL}} + \text{Position}_{\text{BN}} \times P_{\text{BN}}$$
  当 $|\Delta_{\text{net}}| > 5\%$ 名义本金时，自动微调小腿仓位，使 Delta 归零。
- **强平距离防护 (Liquidation Safety Buffer)**：
  - 维持低杠杆（2x ~ 3x）。
  - 当任何一侧保证金率跌破安全线（例如距离强平价不足 25%），立即触发平仓对冲或自动补保。
- **断网死人开关 (Dead Man's Switch)**：
  - 若 WebSocket 心跳断连超过 5 秒，系统自动拒绝新开仓，并在恢复网络前保持现有对冲状态。

### 5.4 Telegram 遥测与交互中枢 (`src/telemetry/`)
- 深度联动现有 `agygram` 架构：
  - 自动向绑定的 Telegram Chat 推送：
    - 🔔 **开仓提醒**：`[OPEN] SUI 跨所套利 | 利差: 68.5% APR | 规模: $500`
    - 💰 **资金费到账**：`[SETTLED] HL +$1.42 / BN -$0.18 | 今日累计: +$12.80`
    - 📊 **持仓日报**：双边保证金利用率、当前净利润、当前 Delta 敞口。

---

## 6. 网络与 VPS 系统级低延迟调优

在云端 Linux VPS（Azure Japan East）上部署时，执行如下内核级网络与 I/O 调优：

```bash
# 1. 调整 Linux 内核 TCP 缓冲区与快速回收
sudo sysctl -w net.ipv4.tcp_fastopen=3
sudo sysctl -w net.ipv4.tcp_tw_reuse=1
sudo sysctl -w net.ipv4.tcp_fin_timeout=15
sudo sysctl -w net.core.rmem_max=16777216
sudo sysctl -w net.core.wmem_max=16777216
sudo sysctl -w net.ipv4.tcp_rmem="4096 87380 16777216"
sudo sysctl -w net.ipv4.tcp_wmem="4096 65536 16777216"

# 2. 启用 BBR 拥塞控制算法
sudo sysctl -w net.core.default_qdisc=fq
sudo sysctl -w net.ipv4.tcp_congestion_control=bbr

# 3. 进程优先级与 CPU 绑定 (Nice -20)
sudo renice -n -20 -p $(pgrep bhyper)
```

---

## 7. 分阶段实施与验证里程碑

- [x] **Milestone 1: 架构设计与理论建模** (已完成本项目计划书与数学模型)
- [x] **Milestone 2: 核心代码骨架搭建** (`BHyper` Cargo 项目、多模块解耦、类型定义)
- [x] **Milestone 3: 交易所 SDK 极速接口对接** (Binance FAPI + Hyperliquid L1 签名与 WebSocket)
- [x] **Milestone 4: 跨所费率实时监控与利差扫描器** (50+ 交易对实时 APR 计算看板)
- [x] **Milestone 5: 模拟盘两腿对冲与风控哨兵** (Paper Trading 验证两腿成交一致性)
- [x] **Milestone 6: 实盘小资金上线与 Telegram 实时遥测** (小资金实盘部署云端)

