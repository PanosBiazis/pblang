# Relationship Reasoning Lab

A PBLang-only migration of two small Prolog relationship exercises. The
original facts and query script are represented only by content digests. This
edition uses synthetic identities and does not retain names from the source.

## What it demonstrates

- facts and reusable declarative rules;
- father, mother, sibling, grandparent and recursive ancestor relations;
- ground equality and inequality constraints;
- bounded depth, solver steps and returned solutions;
- explicit truncation for cyclic or oversized searches;
- a local relationship-graph visual plan; and
- one responsive UI tree for Web, native desktop and Android records.

The inequality helper is a deterministic ground-value constraint. Variables
must be bound by earlier goals. It is not presented as a complete Prolog
`dif/2` constraint store. Passing UI and graph-plan tests do not prove that all
platform renderers have been packaged or physically exercised.

## Run

```text
pblang run examples/relationship_reasoning_lab/main.sapp
pblang run examples/relationship_reasoning_lab/test.sapp --allow-all
pblang run examples/relationship_reasoning_lab/benchmark.sapp
```

Normal execution performs no networking, process execution or persistent
storage and has no foreign runtime dependency.
