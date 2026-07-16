---
title: "m0-02 — deterministic seed scattering"
status: todo
claimed_by:
created: 2026-07-16T22:00:00Z
updated: 2026-07-16T22:00:00Z
---

## Description

N seeds per phase at positions from hash3(seed, i, phase_tag) reduced via
hash_below per axis. Position collisions across seeds are fine (tie-break is by
seed id downstream). Grain ids: phase_base + seed index (cumulative bases).

Done when: seeds module verifies; seed positions proven in-range (free from
hash_below's ensures); determinism documented (pure fn of params).

## Progress

- (2026-07-16T22:00:00Z) created

## Writeup

