//! Protolith verified core: process-simulating rock texture generation.
//!
//! Everything in this library is verified under the tactus Lean backend
//! (`./check.sh`). The deliberately-unverified surface of the project is
//! exactly `src/main.rs` — the I/O shell (arg parsing, image writing).
//!
//! DESIGN.md §11–§12: pure core, fixed-point/integer arithmetic throughout,
//! prove structure not pixels.

pub mod hash;
pub mod slab;
