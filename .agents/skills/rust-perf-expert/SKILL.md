---
name: rust-perf-expert
description: >-
  Advanced Rust performance optimization, zero-cost abstractions, and low-latency/HFT engineering skill.
  Activate whenever the user writes, refactors, benchmarks, or audits Rust code for speed, throughput,
  memory efficiency, and microsecond/nanosecond deterministic latency.
---

# ⚡ Rust High-Performance & Low-Latency Engineering Skill

Based on **The Rust Performance Book** (`nnethercote/perf-book`), **Awesome HFT Rust**, and production quant best practices.

---

## 🛠️ Core Optimization Directives

### 1. Memory & Allocation Elimination (Zero-Alloc Hot Paths)
- **Avoid Heap Allocations**: Never call `format!()`, `to_string()`, `String::from()`, `Box::new()`, or dynamic `Vec::push()` in hot execution loops.
- **Borrowed Deserialization**: Always use `&'a str` / `#[serde(borrow)]` when parsing JSON, MessagePack, or binary packets.
- **Stack Buffers & Compact Types**:
  - Use fixed stack arrays `[u8; N]` or `smallvec` / `compact_str` instead of heap-allocated `String` / `Vec`.
  - Prefer passing `&str` / `&[T]` slices instead of owned `String` / `Vec<T>`.

### 2. Fast Non-Cryptographic Hashing
- Standard `std::collections::HashMap` uses SipHash 1-3 to prevent HashDoS.
- In internal engines (market data caches, symbol indexers, state lookups), **ALWAYS** use:
  - `rustc_hash::FxHashMap` / `FxHashSet` (fastest 64-bit integer & short string hasher).
  - `ahash::AHashMap` (hardware AES-accelerated).

### 3. Concurrency & Lock-Free Architecture
- **Read-Copy-Update / Zero-Lock Reads**: Use `arc_swap::ArcSwap` for publishing immutable state/snapshots to readers without acquiring any `RwLock` or `Mutex`.
- **Atomic Operations**: Use `AtomicU64`, `AtomicI64`, `AtomicBool` with relaxed/acquire-release memory orderings for high-frequency nonces, counters, and flags.
- **Fast Channels**: Use `tokio::sync::broadcast` for 1-to-many fanout and `crossbeam-channel` / `flume` for lock-free cross-thread queues.

### 4. CPU, SIMD & Inlining Optimization
- **Arithmetic over String Manipulation**: Compute decimals and alignments via integer math / `log10` or branchless lookup tables instead of float-to-string parsing.
- **Unstable Sorting**: Use `slice::sort_unstable_by` with `f64::total_cmp` for zero-allocation, branchless hardware floating-point sorting.
- **Precomputed Cryptography**: Pre-initialize HMAC keys (`ring::hmac::Key`) and precompute static EIP-712 domain separators / public keys once during client construction.
- **Inlining**: Mark small, performance-critical helper functions with `#[inline]`.

### 5. Profile & Build Configurations (`Cargo.toml`)
Ensure release builds utilize full link-time optimization and codegen optimization:
```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

---

## 🔍 Performance Verification Commands

When auditing or finishing Rust code changes:
1. **Clippy Strict Performance Check**:
   ```bash
   cargo clippy --all-targets --all-features -- -D clippy::perf -D clippy::all
   ```
2. **Release Benchmark & Test Run**:
   ```bash
   cargo test --release
   ```
