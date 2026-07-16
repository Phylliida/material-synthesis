---
title: "m0-00 — LANDED: slab infra + counter hashing, 12/0 closer-free"
status: done
claimed_by:
created: 2026-07-16T22:00:00Z
updated: 2026-07-16T22:00:00Z
---

## Description

Record (commits through 66a42ff): hash.rs (arithmetic-only counter hash, hash3 +
hash_below < n) and slab.rs (SlabDims wf/total/index, wrap_delta, abs_delta,
torus_dist2 < 2^21) verified 12/0 under the tactus Lean backend, crate-local
check.sh, zero tactus_tactic closers. slab.rs header documents the proof idioms
(Lean-native asserts thread as hypotheses; inline spec fns + omega; opaque ite
specs unfolded once; word-width case-split for usize).

## Progress

- (2026-07-16T22:00:00Z) created

## Writeup

