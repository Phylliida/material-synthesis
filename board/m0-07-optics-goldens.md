---
title: "m0-07 — flat optics, pseudo-granite exit, golden outputs"
status: todo
claimed_by:
created: 2026-07-16T22:00:00Z
updated: 2026-07-16T22:00:00Z
---

## Description

Polished mode: z = const slice, per-texel albedo = 3-entry mineral table +
per-grain value jitter from hash(grain_id) (without jitter it reads cartoon-flat).
Channel range proofs trivial at M0. Shell renders the image; eyeball check =
recognizable pseudo-granite = M0 exit (DESIGN 14).

Also: create golden/ and record the first golden outputs (buffers + image) — the
differential-testing suite starts at M0 per DESIGN 11, and the GPU harness
(kir-11) consumes these.

Done when: pseudo-granite image committed; golden/ populated; M0 declared done in
this board + memory updated.

## Progress

- (2026-07-16T22:00:00Z) created

## Writeup

