# Local offer and workload scenarios

`offer_plan.sapp` is a pure, bounded companion to `money.sapp`. PBOffer owns the
product UI; the reusable library owns amount parsing and monthly scenario logic.
It introduces no billing API, tax rules, credentials, clock, network or file I/O.

```sapp
let P=import("stdlib:offer_plan.sapp");
let result=P.calculate(map{
    target_cents:200000,fixed_cost_cents:10000,fixed_minutes:600,available_minutes:1200,
    offers:[map{id:"kit",price_cents:14900,unit_cost_cents:500,fee_bps:1000,
        fee_fixed_cents:50,units:17,minutes_per_sale:20}]
});
// 208620 cents after entered costs, 940 minutes, 17 solo sales needed.
```

`calculate` accepts exactly the five plan fields and seven offer fields above.
IDs are unique 1–32 character lowercase ASCII slugs (`a-z`, `0-9`, `-`, `_`).
All numeric inputs must be finite nonnegative ordinary integers. There are
1–16 offers; price/cost/fixed fee are <= 100,000,000 cents, fee <= 10,000 basis
points, units <= 100,000, minutes/sale <= 10,000, target/fixed cost <=
1,000,000,000 cents, fixed/available time <= 44,640 minutes. Products and totals
are below 2^53; at the maximum admitted portfolio gross is 160,000,000,000,000
cents. The arithmetic performs O(number of offers) work and does not loop over
sales or repeatedly search prices.

`parse_amount` reads bounded unsigned decimal text directly into integer cents,
rejecting more than two decimal places. `parse_integer` reads ASCII integer text
up to an explicit bound (default 1,000,000,000). No locale guessing, exponent
parsing, currency signs, surrounding spaces or silent rounding is applied.

Per-sale fee = ceil(price * basis_points / 10000) + fixed_fee.
Per-sale contribution = price - fee - entered per-sale cost.
Monthly contribution = sum(contribution * units) - shared fixed cost.
Required solo sales = ceil((target + fixed cost) / contribution) for a positive
contribution, limited to the admitted unit cap. A zero required amount needs zero
sales. Nonpositive contribution and an excessive required count return `nil`
with distinct status strings. Required workload includes fixed minutes.

`offers[].solo_units_for_target` is an independent single-offer scenario, not the
incremental amount required on top of other offers. Output rows follow input
order and do not alias inputs. Losses are reported, not clamped away. Only
shortfall is clamped to zero. Fees are rounded per sale, not on the portfolio
total; this is an assumption, not payment-provider compatibility.

`scenario_only:true`, `demand_verified:false`, `tax_included:false`,
`owner_labor_cost_included:false` and `safe_to_log:false` describe scope.
The library does not infer taxes or an owner's hourly wage; a caller may supply
additional modeled costs. Workload fit does not establish market feasibility.
No automatic external effects or claims about future income follow from a plan.

Source direction: `POLYGLOT_MIGRATION_CATALOG_2026-08-20.md` budgeting/reporting
categories and the explicit portfolio/revenue request. No vendor code or scanned
private file contents were copied. PBOffer's README owns product examples and
edition status; the semantic suite checks arithmetic, limits and rejection.
