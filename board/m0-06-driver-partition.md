---
title: "m0-06 — sequential phase driver: partition + exact-quota theorems"
status: todo
claimed_by:
created: 2026-07-16T22:00:00Z
updated: 2026-07-16T22:00:00Z
---

## Description

For each phase in Bowen order (M0: hardcoded 3 phases): scatter -> assign over
unclaimed -> freeze at quota; last phase takes quota = remaining (anhedral
automatically). Precondition: quotas sum to total(). Postconditions (M0
verification deliverable, DESIGN 14): every voxel owned by exactly one
(phase, grain); per-phase counts == quotas exactly.

Done when: driver verifies with both theorems as ensures; 0 errors.

## Progress

- (2026-07-16T22:00:00Z) created

## Writeup

