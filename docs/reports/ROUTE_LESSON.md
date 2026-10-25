# Directed route lesson


`route_lesson.analyze(spec, source, target)` accepts exactly `{nodes, edges}`.
There must be 1–32 unique ASCII IDs, 1–16 characters each (letters, digits, `_`,
`-`), and at most 128 exact `{from, to, cost}` records. Costs are integers from
1 to 1000000. Self edges, duplicate directed edges and unknown endpoints reject.
The existing `algorithms_advanced.dijkstra` computes distances. The wrapper
reconstructs a shortest path; ties choose the first declared incoming shortest
edge. Positive costs ensure reconstruction progresses. Maximum simple-path cost
is 31 million, within exact numeric integer representation.

Unreachable targets have `reachable:false`, `cost:nil`, `path:[]`; every declared
node remains in the report. `diagram(spec, source, target)` returns that same
report and passive SVG with curved directed edges, costs and reachability labels.
It performs no filesystem/network operations. Reports retain caller labels.
The fixed four-column layout can overlap on dense graphs; it is not a graph
editor, geographic route engine or real-network observation. Web rendering was
checked for the supplied four-node synthetic example at 900×450 only.

Run `examples/discovery_route_lesson.sapp` for the cost-5 indirect route vs the
cost-9 direct edge, plus an isolated node. `tests/semantics/route_lesson_test.sapp`
covers direction, ties, cycles, unreachable nodes, detached outputs, maximum
chain cost, rejected inputs and agreement between diagram and numeric report.
