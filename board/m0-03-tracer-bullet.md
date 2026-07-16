---
title: "m0-03 — tracer bullet: one phase claims everything -> first image"
status: todo
claimed_by:
created: 2026-07-16T22:00:00Z
updated: 2026-07-16T22:00:00Z
---

## Description

Derisk the visual pipeline before the quota machinery exists: single phase,
argmin over its seeds for EVERY voxel (degenerate full-slab Voronoi), flat
per-grain color from hash(grain_id), polished z-slice, unverified shell
(src/main.rs, outside check.sh scope) writing PPM or PNG.

Build gotcha (2026-07-02 session): plain `cargo` cannot compile the core —
rustc can't parse the Lean by-blocks — use `verus --compile` or cargo-verus
for the binary.

Done when: an image of a full-slab Voronoi tessellation exists; shell is
enumerated as the deliberately-unverified surface (DESIGN 11).

## Progress

- (2026-07-16T22:00:00Z) created

## Writeup

