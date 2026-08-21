# 🚀 BHyper 2.0: 极致低延迟跨所资金费率与基差套利引擎升级蓝图

> **Status**: Production Implementation  
> **Target Version**: 2.0.0-HighPerformance-Hardened  
> **Author**: Antigravity Quant & Low-Latency Systems Team  
> **Last Updated**: 2026-08-22  

---

## 📑 目录 (Table of Contents)

1. [背景与实战问题诊断 (Problem Formulations)](#1-背景与实战问题诊断)
2. [开源高星项目与量化实操经验调研 (Industry Benchmarking)](#2-开源高星项目与量化实操经验调研)
3. [BHyper 2.0 升级架构与核心方案 (Architectural Upgrades)](#3-bhyper-20-升级架构与核心方案)
   - 3.1 流动性、持仓量 (OI) 与费率防操纵过滤器 (Liquidity & OI Sentinel)
   - 3.2 Binance WebSocket API 极速下单通道 (Zero-HTTP WS Order Dispatch)
   - 3.3 逆向选择防御与智能微价路由 (Smart Micro-Price & Adverse Selection Guard)
   - 3.4 跨所保证金健康度哨兵与再平衡建议 (Cross-Exchange Margin Health & Rebalance Advisory)
   - 3.5 纯 Rust 零分配热路径与性能极限调优 (Zero-Allocation Rust Hot Paths)
4. [系统详细设计与状态转移矩阵 (Detailed Engineering Specs)](#4-系统详细设计与状态转移矩阵)

---

## 1. 背景与实战问题诊断

在对 BHyper 1.0 架构与实盘机制的深度审查中，我们锁定了四大直接影响实盘盈亏与资金安全的致命短板：

| 序号 | 核心问题 | 实盘影响 / 潜在危害 | 改进目标 |
| :--- | :--- | :--- | :--- |
| **P1** | **山寨币流动性陷阱与费率操纵** | SAGA、CHIP、ACE 等小币种 OI 极低，费率在 T-5s 易被大户反向操纵，建仓后被困甚至倒贴利息 | 引入多重流动性、24h 交易量、持仓量 (OI) 及费率波动率硬门槛 |
| **P2** | **Maker-Taker 逆向选择与盘口踩踏** | 整点前抢单严重，Post-Only 挂单仅在行情剧烈逆转时成交（被毒性订单吃单），Taker 对冲滑点剧增 | 引入盘口微价（Micro-Price）偏置、动态滑点保护与部分成交按比例对冲 |
| **P3** | **HTTP REST 下单延迟瓶颈** | 行情端走 WS 但下单端走 HTTP POST，存在 15~40ms 额外握手/传输延迟，丧失先发优势 | 引入 Binance WebSocket API (`ws-fapi.binance.com`) 实现全链路亚毫秒 WS 下单 |
| **P4** | **跨所资金单向流动与保证金失衡** | 随利差结算和单边行情，一侧盈利暴增、另一侧濒临强平，且小资金频繁提币跨链磨损过大 | 引入跨所保证金健康度实时哨兵 (`MarginHealthSentinel`) 与智能再平衡测算 |

---

## 2. 开源高星项目与量化实操经验调研

我们对主流高频套利框架（`Hummingbot`、`NautilusTrader`、`hyperliquid-rust-sdk`、`hypersdk`、`ccxt`）及头部量化团队（Wintermute、Kronos）公开经验进行了交叉比对：

### 2.1 流动性与持仓量（Open Interest）过滤标准
- **业界经验**：顶级跨所做市商通常要求标的在双边的合计 Open Interest $\ge \$1,000,000$，24h 成交额 $\ge \$3,000,000$，且买卖价差（Bid-Ask Spread） $\le 10 \text{ bps}$。
- **BHyper 2.0 落地**：
  - Binance 侧拉取 `/fapi/v1/ticker/24hr` 和 `/fapi/v1/openInterest`；
  - Hyperliquid 侧解析 `AssetCtxItem.open_interest` 并折算为 USD 价值；
  - 设定 `min_open_interest_usd`（默认 $500,000）、`min_24h_volume_usd`（默认 $1,000,000）与 `max_bid_ask_spread_bps`。

### 2.2 订单执行网络优化 (Binance WebSocket API vs REST)
- **业界经验**：Binance 开放了统一 WebSocket API (`wss://ws-fapi.binance.com/ws-fapi/v1`)，直接发送 `order.place` 文本帧，省去 HTTP 连接握手与头解析，RTT 减少 **15 ~ 35ms**。
- **BHyper 2.0 落地**：
  - 构建异步 `BinanceWsApiClient`，利用 `tokio-tungstenite` 维持长连接；
  - 使用预计算 `ring::hmac::Key` 实现签名零分配；
  - 维护基于 `AtomicU64` 的 Request-Response `oneshot` 回调通道，毫秒级获取成交结果，并支持降级 HTTP REST。

### 2.3 逆向选择与微价模型 (Micro-Price & Adverse Selection Guard)
- **业界经验**：挂单价格不应简单等同于 Mark Price，而应根据挂单簿的买卖深度与不平衡度（Imbalance）计算微价（Micro-Price）：
  $$P_{\text{micro}} = \frac{Q_{\text{bid}} \cdot P_{\text{ask}} + Q_{\text{ask}} \cdot P_{\text{bid}}}{Q_{\text{bid}} + Q_{\text{ask}}}$$
- **BHyper 2.0 落地**：
  - 在 Maker-Taker 模式下根据实时买卖深度进行智能定价；
  - 设置严格的 `max_slippage_bps` 保护，若 Taker 腿盘口偏离预设滑点阈值则拒绝成交或快速止损。

### 2.4 跨所保证金健康度与再平衡建议 (Cross-Exchange Margin Sentinel)
- **业界经验**：当跨所资金费率套利运行时，实时计算两所的**保证金利用率**（Margin Utilization %）与**强平距离**（Distance to Liquidation %）：
  $$\text{Margin Utilization} = \frac{\text{Margin Used}}{\text{Account Value}}$$
  $$\text{Liquidation Distance} = \frac{|P_{\text{mark}} - P_{\text{liq}}|}{P_{\text{mark}}}$$
- **BHyper 2.0 落地**：
  - 定时轮询两所账户权益并计算健康指数；
  - 当任一侧保证金利用率超过 75% 或强平距离 $< 20\%$ 时，触发 `ExitSignal::MarginCritical` 自动减仓平仓；
  - 在 `reconcile` 命令中自动给出建议资金划转金额（Rebalance USD Recommendation）。

---

## 3. BHyper 2.0 升级架构与核心方案

```mermaid
flowchart TB
    subgraph DataIngestion [数据采集与指标增强]
        BN_WS_Rate[Binance Mark Price WS]
        BN_WS_Api[Binance WS API: order.place]
        HL_WS[Hyperliquid WS: allMids / userFills]
        BN_REST[Binance REST: 24h Ticker & OI]
        HL_REST[Hyperliquid REST: Meta & AssetCtx]
    end

    subgraph MemoryCache [高性能内存缓存]
        Cache[MarketDataCache<br/>FxHashMap + ArcSwap + Broadcast]
    end

    subgraph DefenseCore [五重防线与风控大脑]
        LiquidityFilter[流动性与 OI 过滤器<br/>OI >= $500k, Vol >= $1M]
        RateStability[费率稳定性与防操纵检查<br/>Oracle/Mark Divergence <= 0.5%]
        PrecisionEngine[GCD 步长零漂移对齐]
        MarginSentinel[跨所保证金健康哨兵<br/>利用率/强平距离/再平衡建议]
    end

    subgraph UltraExecution [超低延迟执行引擎]
        Router[智能订单路由器 (SOR)]
        BN_WS_Router[Binance WS API Client]
        HL_L1_Router[Hyperliquid Fast EIP-712]
        Fallback[HTTP REST 自动降级]
    end

    BN_WS_Rate --> Cache
    HL_WS --> Cache
    BN_REST --> Cache
    HL_REST --> Cache

    Cache --> LiquidityFilter
    LiquidityFilter --> RateStability
    RateStability --> PrecisionEngine
    PrecisionEngine --> MarginSentinel
    MarginSentinel --> Router

    Router --> BN_WS_Router
    Router --> HL_L1_Router
    BN_WS_Router -.->|Fallback| Fallback
```

---

## 4. 实施阶段与交付清单

1. **模块 1 (`types.rs` & `config.rs`)**: 扩展数据结构，增加 24h 成交额、OI、价差、保证金指标以及配置项。
2. **模块 2 (`binance/ws_client.rs` & `binance/client.rs`)**: 升级 Binance WebSocket API 下单客户端与 24h 指标拉取。
3. **模块 3 (`hyperliquid/client.rs`)**: 解析 OI 与流动性元数据。
4. **模块 4 (`strategy/scanner.rs` & `strategy/trigger.rs`)**: 接入流动性/OI/价差/费率稳定性硬锁。
5. **模块 5 (`strategy/executor.rs`)**: 接入 WebSocket API 下单与微价滑点防线。
6. **模块 6 (`risk/sentinel.rs` & `state/store.rs`)**: 接入跨所保证金健康度与再平衡建议。
7. **模块 7 (`telemetry/notifier.rs` & `main.rs`)**: 全面升级 CLI 命令与 Telegram 监控告警。
8. **模块 8 (`tests/arbitrage_tests.rs`)**: 编写全量集成测试，验证零回归与高性能。
