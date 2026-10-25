# Finance / Quant — trading indicators, risk cage, backtesting

A pure-pblang quant stack harvested from the ibkr trading project
(`projects/ibkr`). Three layers, cleanly separated:

| Layer | Module | Role |
|-------|--------|------|
| Signals | `stdlib/indicators.sapp` | SMA / EMA (alpha + period forms) / RSI (simple + Wilder) / MACD / Bollinger / ATR / golden & death crosses |
| Judgment | `stdlib/backtest.sapp` | replay signals → equity curve, total return, max drawdown, sharpe, trades, win rate |
| Execution safety | `stdlib/risk_gate.sapp` | the deterministic risk cage: allow/deny per order, pure function, no I/O |

## The cage principle (policy vs mechanism)

**Policy** (what to trade) may come from anywhere — an indicator crossover, an
AI model (pbai), a human. **Mechanism** (whether an order is permitted) is
decided only by `risk_gate.check(order, state, config, live_confirmed)`, a pure
function, so every rule is exhaustively unit-testable and behaves identically
on paper and live. Rules in priority order:

1. `live_guard` — on a live account, refuse everything unless explicitly armed.
2. `whitelist` — only pre-approved symbols.
3. `side` — long-only BUY/SELL.
4. `fractional` / `quantity` — whole positive shares.
5. `no_short` — cannot sell more than held; **closing sells are always allowed**,
   even under the kill switch or a breached loss limit: you can always exit.
6. `daily_loss_limit` — kill switch or drawdown past the limit blocks new BUYs.
7. `max_position` / `buffer` — per-order size cap and preserved cash buffer.

`size_for(budget, price, max_position_pct)` gives the largest whole-share
order that fits the cap.

## Quick start

```pblang
let IND = import("stdlib:indicators.sapp");
let BT  = import("stdlib:backtest.sapp");
let RG  = import("stdlib:risk_gate.sapp");

let fast = IND.ema_period(closes, 12);
let slow = IND.ema_period(closes, 26);
let sig = [];   // 1 = long, 0 = flat, aligned to closes
// ... build sig from IND.cross_over(fast, slow) / cross_under ...
let result = BT.run(closes, sig);         // .total_return .max_drawdown .sharpe

let d = RG.check(order, ledger, cfg, false);
if d.allow { /* place */ } else { print d.rule + ": " + d.reason; }
```

## Honest boundaries

- `backtest` models no commissions, slippage, or sizing — it ranks strategies,
  it does not promise live results.
- `sharpe` is per-step (unannualized); scale by `sqrt(bars_per_year)` yourself.
- Nothing here talks to a broker. Wire-up to a real API is a host-side concern
  (the same boundary as GPU/FFI); the cage is what keeps that wire-up safe.

Tests: `tests/semantics/finance_quant_test.sapp` (hand-computed expected values)
and `indicators_stdlib_test.sapp` (pre-existing simple-form indicators).
