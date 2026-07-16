---
title: "m0-05 — exact quota via threshold binary search (the M0 proof meat)"
status: todo
claimed_by:
created: 2026-07-16T22:00:00Z
updated: 2026-07-16T22:00:00Z
---

## Description

Freeze keys K_v = (dist2 << 32) | linear_index(v): injective across voxels via
m0-01's index injectivity. count_le(keys, k) recursive spec + monotonicity +
steps-by-at-most-1 (injectivity) => unique threshold K* with count == quota
exactly. Exec: binary search over key space (~53 iterations x O(V) count pass),
then freeze writes (mineral, grain, t_impinge) to voxels with K <= K*, frame on
already-claimed.

Done when: freeze ensures claimed-count increase == quota exactly; only-unclaimed-
written; 0 errors. This is the flagship B1 quota theorem's engine (DESIGN 12.1).

## Progress

- (2026-07-16T22:00:00Z) created

## Writeup

