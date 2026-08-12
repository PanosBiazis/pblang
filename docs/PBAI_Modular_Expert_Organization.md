# pbAI modular expert organization

PBLang will support a lightweight native pbAI family built around one compact
English+PBLang core model and a group of narrow expert, router, planner,
security-review, verifier, tool, memory, and coordinator models. Only the
smallest required group should be loaded for a request.

The canonical product roadmap is currently maintained in
`products/pbai/docs/Modular_Expert_Organization.md`. Until the duplicated pbAI
trees are consolidated, implementations must not be copied independently into
both locations. The future PBLang embedding must be generated from one canonical
source and verified by hash.

Required architecture properties are versioned typed inter-model messages,
least-privilege capabilities, owner-isolated memory, provenance, independent
verification, bounded compute/token/memory/thermal budgets, cancellation,
deterministic fallback, and measured CPU/GPU/NPU routing. Untrusted model output
is data and never directly grants tool, process, filesystem, network, or secret
access.

Completion requires reproducible licensed datasets, compile/test-based PBLang
evaluations, English quality and abstention tests, adversarial multi-model
security tests, and per-device latency/RSS/storage/battery/thermal benchmarks.
