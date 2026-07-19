# Linear graph traversal and advanced algorithm repair

Both pure-pblang graph APIs copied the remaining queue after every BFS dequeue,
making wide traversal quadratic. Their DFS paths also rebuilt the remaining
stack for every pop or depended on recursion. `stdlib/graph.sapp` and
`stdlib/algorithms_advanced.sapp` now use head-index BFS queues and constant-time
stack pops, giving O(V+E) traversal with O(V) storage.

## Method

`bench/graph_traversal_stages.sapp` constructs a directed 2,000-leaf star before
timing. Each measured process executes and checksum-verifies three complete BFS
traversals. Seven fresh release-runtime processes were pinned to logical CPU 3;
medians are reported. `/usr/bin/time` peak RSS includes startup, imports, the
prebuilt graph, and benchmark state.

| Metric | Baseline | Final | Change |
|---|---:|---:|---:|
| Three verified BFS traversals | 1592.228 ms | 13.802 ms | **99.13% lower** |
| Relative throughput | 1.00x | 115.36x | **115.36x** |
| Median peak RSS | 15,624 KiB | 15,020 KiB | **3.87% lower** |
| BFS complexity | O(V^2+E) | O(V+E) | no queue copies |
| DFS pop | O(V) copy | O(1) mutation | `array_pop` |

The previously untested advanced module was also rewritten into valid,
readable pblang. Its ten exports now have semantic coverage. The pass fixes
fractional midpoint indices, missing-key crashes for leaf nodes, recursion-bound
DFS, O(items*capacity) knapsack storage, and the old discovery-order shortest
path bug. Dijkstra now uses a binary min-heap, stale-entry suppression, and
rejects malformed or negative-weight edges.

`graph_bfs_wide` is a permanent correctness-gated phase metric. These timings
describe the isolated wide-BFS workload on this machine, not every graph shape
or a cross-language performance result.
