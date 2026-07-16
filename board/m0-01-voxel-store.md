---
title: "m0-01 — SoA petrofabric store + spec_index injectivity"
status: todo
claimed_by:
created: 2026-07-16T22:00:00Z
updated: 2026-07-16T22:00:00Z
---

## Description

SoA buffers (mineral: Vec<u8>, grain: Vec<u32>, t_impinge: Vec<u32>), lengths ==
dims.total(); mineral 0 = unclaimed convention for M0. Add to slab.rs the one new
lemma the quota argument needs: spec_index injectivity on in-range coords
(index(x1,y1,z1) == index(x2,y2,z2) ==> coords equal, under wf bounds).

Done when: store module + injectivity lemma verify 0 errors.

## Progress

- (2026-07-16T22:00:00Z) created

## Writeup

