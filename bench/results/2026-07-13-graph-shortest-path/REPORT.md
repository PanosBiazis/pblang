# Heap shortest paths and shared min-priority queue

`graph.dijkstra` previously selected every next vertex by materializing the
unvisited keys and scanning all of them. That made sparse shortest paths
O(V^2+E). It now delegates relaxation to the same validated, pure-pblang binary
min-heap used by `algorithms_advanced.dijkstra`, reducing the path to
O((V+E) log V).

## Method

`bench/graph_shortest_path_stages.sapp` constructs a directed 600-edge chain
before timing. Each measured process executes two complete shortest-path runs
and checks both the source and final distance. Seven fresh release-runtime
processes were pinned to logical CPU 3. `/usr/bin/time` peak RSS includes
startup, imports, the prebuilt graph, and benchmark state. Medians are reported.

| Metric | Baseline | Final | Change |
|---|---:|---:|---:|
| Two verified sparse Dijkstra runs | 443.201 ms | 5.278 ms | **98.81% lower** |
| Relative throughput | 1.00x | 83.97x | **83.97x** |
| Median peak RSS | 11,820 KiB | 12,068 KiB | **2.10% higher** |
| Shortest-path complexity | O(V^2+E) | O((V+E) log V) | heap selection |

The shared queue rejects non-numeric, NaN, and infinite priorities. Both
Dijkstra APIs reject malformed or negative edge weights. The high-level graph
API preserves its existing unreachable-node sentinel. The same pass makes
topological queue consumption O(1) per node and replaces recursive undirected
cycle traversal with an explicit parent stack.

`graph_dijkstra_sparse` is a permanent correctness-gated phase metric. These
numbers describe this isolated workload on this machine; they do not establish
that pblang is universally faster than another language or implementation.
