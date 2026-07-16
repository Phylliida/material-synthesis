---
title: "m0-04 — per-voxel argmin with composite keys"
status: todo
claimed_by:
created: 2026-07-16T22:00:00Z
updated: 2026-07-16T22:00:00Z
---

## Description

For each unclaimed voxel, best seed under key (dist2 << 32) | seed_id (distinct
seed ids => unique argmin, no tie lemmas). Store per-voxel best-key array for the
quota phase. Min-of-prefix loop invariant (tgt exec-loop idioms). GPU note: keep
the (dist2, id) lexicographic-u32-pair reading in a comment — kir-16 consumes
this kernel and WGSL has no u64.

Done when: assign module verifies with ensures: per-voxel key == min over the
phase's seeds, owner recorded, claimed voxels untouched (frame).

## Progress

- (2026-07-16T22:00:00Z) created

## Writeup

