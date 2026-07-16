# Task board — protolith M0 (material-synthesis)

One markdown file = one task; same conventions as the tactus boards
(frontmatter: title/status/claimed_by/created/updated; sections: Description /
Progress / Writeup; set status todo -> in_progress -> done as you go).

## Program map

Spec: `../DESIGN.md` (v0.2), milestone M0 (§14): slab infra + JFA-JMAK + 3
minerals + flat optics -> recognizable pseudo-granite; verification deliverable =
B1 partition + exact-quota theorems. Landed so far: `m0-00` (slab + hash).

Rough order: m0-01 store -> m0-02 seeds -> m0-03 tracer-bullet image ->
m0-04 assign -> m0-05 quota -> m0-06 driver theorems -> m0-07 optics + exit.

Cross-link: the GPU transpiler board (verus-gpu-transpiler/board/) consumes this
track — kir-16 (assign pass end-to-end on GPU) depends on m0-04, and m0-07's
goldens seed the differential harness (kir-11).

Files starting with `.` or `_`, plus this README, are ignored by the board.
