# AI turn cost budgets

`stdlib:ai_turn_budget.sapp` estimates affordable output and accounts for actual
usage using prices supplied by the caller. It is pure PBLang: no network, provider
catalog, currency conversion, tokenizer, clock, content, telemetry or stored state.
It complements `resource_budget.sapp`, whose generic token/output resource limits
do not represent separately priced token categories.

Choose one fixed unit fine enough to express **every per-token rate as an integer**.
For example, an application may choose an integer subunit of its accounting
currency. All prices, budgets and prior spend in one calculation must use that same
unit. The library cannot identify mixed units. It does not divide a per-million
price, round a fractional rate or know which pricing plan applies. Provider prices,
taxes, discounts, minimum charges and other fees remain the caller's responsibility.
The example prices below are synthetic, not a provider price list.

## Records and exact arithmetic

Both price and usage maps contain exactly four required keys:

| Key | Usage meaning | Price meaning |
| --- | --- | --- |
| `input` | Uncached input tokens | Units per uncached input token |
| `output` | Output tokens | Units per output token |
| `cache_read` | Tokens read from a prompt cache | Units per cache-read token |
| `cache_write` | Tokens written to a prompt cache | Units per cache-write token |

Usage categories are **disjoint**. If provider-reported input includes cached
tokens, normalize it before calling this module; do not count the same token in
both `input` and a cache category. Different cache durations or billing categories
are unsupported unless the caller can normalize them without losing cost accuracy.
No category or missing rate silently defaults to zero in a raw map.

Counts and per-token rates must be ordinary finite integer `number` values from
0 through 1,000,000,000 inclusive. Budgets, previous spend, each category cost and
total cost must fit 0 through 9,007,199,254,740,991 (`2^53 - 1`). Boxed numbers,
strings, booleans, nil, negative/fractional/nonfinite values, unknown keys, missing
keys and unsupported shapes throw a fixed `ai_turn_budget:` diagnostic. Zero rates
are valid explicit prices. Negative zero behaves as zero.

Multiplication checks the safe quotient before multiplying and validates the
product as well, so a rounded quotient cannot admit a product above the safe
integer range. Addition checks remaining headroom before summing. All accepted
cost arithmetic is exact: `sum(count[category] * price[category])`, with no category
rounding. Overflow throws; it never clips, wraps, or treats an invalid charge as
zero. A thrown actual-accounting calculation must be handled as an accounting
failure by the caller, never as a free turn. Values already rounded before entering
the API cannot be recovered or detected reliably.

## API

| Function | Result |
| --- | --- |
| `limits()` | Fresh map with `max_tokens`, `max_rate`, `max_units` |
| `rates(input, output, cache_read, cache_write)` | Validated four-category price map; all arguments required |
| `usage(input=0, output=0, cache_read=0, cache_write=0)` | Validated four-category usage map |
| `cost(prices, tokens)` | `{categories: {input, output, cache_read, cache_write}, total_units}` with per-category **costs** |
| `forecast(prices, planned, remaining_units)` | `{kind: "forecast", allowed, estimate, remaining_units, shortfall_units}` |
| `affordable_output(prices, prompt, remaining_units, output_cap)` | `{kind: "forecast", allowed, output_tokens, prompt_units, total_units, remaining_units, shortfall_units}` |
| `account(prices, actual, budget_units, spent_units=0)` | `{kind: "actual", cost, spent_units, within_budget, remaining_units, excess_units}` |

Every call revalidates incoming maps, including maps created by the constructors.
Calls do not mutate inputs; returned records contain no shared caller-owned maps.
Validation reads only the four fields after checking map size. It does not traverse
nested data or stringify rejected values. There is no usage history or log.

`forecast` compares the complete planned cost against the supplied remainder.
Its `remaining_units` is the hypothetical remainder **after the estimate**, zero
when denied. `shortfall_units` is zero when allowed. **A forecast reserves nothing
and does not enforce a remote spending cap.** Reusing the same remaining budget
for concurrent forecasts can overcommit it. The caller must enforce provider limits,
synchronize execution and reconcile actual usage independently.

`affordable_output` requires `prompt.output == 0`; a nonzero value throws instead
of being silently ignored. It prices all three prompt categories, then finds the
largest output count at or below the required caller cap that fits the remainder.
A correction after division protects the integer boundary. The output cap has the
same 1,000,000,000 maximum as a usage count. If output is free, it returns the cap,
not infinity. If the prompt alone exceeds the budget, `allowed` is false,
`output_tokens` is zero, `total_units` remains the prompt cost and `shortfall_units`
reports the deficit. A prompt that fits exactly is allowed even with zero affordable
output. These results describe affordability, not model context-window limits.

`account` uses supplied actual token counts, adds the **full** cost to prior spend
and reports overspend even when it exceeds the budget. A pre-existing overspend is
also retained. `remaining_units` is zero when overspent, and `excess_units` is zero
when within budget. This function does not authenticate usage or execute a payment.
Persist the returned `spent_units` yourself and account each turn once. There is
no idempotency key, mutable ledger, reservation, cancellation or concurrent ownership
protocol in this module.

## Example

```pblang
let B = import("stdlib:ai_turn_budget.sapp");
let prices = B.rates(3, 7, 1, 4);
let prompt = B.usage(10, 0, 20, 2);
let plan = B.affordable_output(prices, prompt, 100, 20);
// Prompt costs 58 units; six output tokens cost 42 more.
assert(plan.output_tokens == 6, "estimated affordable output");
let actual = B.account(prices, B.usage(10, 8, 20, 2), 100);
// Actual cost is 114. The full 14-unit excess remains visible.
assert(actual.spent_units == 114 and actual.excess_units == 14, "actual overspend");
```

Runnable example: `examples/ai_turn_budget/main.sapp`.
Focused tests: `tests/semantics/ai_turn_budget_test.sapp`, including independent
small-domain affordability checks, all cached categories, zero rates/caps, malformed
maps/numbers, exact safe-integer boundaries, overflow, overspending and nonmutation.
This is a source library; it does not install or integrate itself into pbAI,
PB-MCP, a provider SDK or a global launcher. No performance or deployment claim is
implied by the arithmetic contract.
