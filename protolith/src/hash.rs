//! Counter-based deterministic hashing for seed placement.
//!
//! DESIGN.md §11: all randomness comes from counter-based hashes keyed
//! (seed, counter, tag) — identical params reproduce the identical rock
//! bit-for-bit, and the CUDA port must match exactly (integer ops are
//! bit-identical across CPU/GPU).
//!
//! Deliberately all-arithmetic (mul/add/div/mod — no xor/shift): bitwise
//! ops route through the Lean backend's BitVec bridge, while integer
//! arithmetic stays in omega-friendly land. Two rounds of multiply +
//! high-half folding give adequate diffusion for seed scattering; if a
//! later stage needs stronger mixing, upgrade to splitmix64 once the
//! BitVec story is exercised.
//!
//! Proof surface is intentionally minimal: the mixer has no ensures
//! (determinism is inherent in a pure fn); the one proved fact is the
//! one consumers rely on — `hash_below(h, n) < n`.

use vstd::prelude::*;

verus! {

import Mathlib.Tactic.Linarith

/// Wrapping u64 multiply via u128, no bitwise ops.
#[verifier::tactus_auto]
#[verifier::tactus_tactic("tactus_first | tactus_auto | (intros; casesm* _ ∧ _; refine ⟨?_, ?_⟩ <;> nlinarith) | (intros; casesm* _ ∧ _; nlinarith)")]
pub fn wmul(a: u64, b: u64) -> u64 {
    (((a as u128) * (b as u128)) % 0x1_0000_0000_0000_0000u128) as u64
}

/// Wrapping u64 add via u128.
#[verifier::tactus_auto]
pub fn wadd(a: u64, b: u64) -> u64 {
    (((a as u128) + (b as u128)) % 0x1_0000_0000_0000_0000u128) as u64
}

/// Fold the high 32 bits into the low bits (arithmetic form of `z ^= z >> 32`).
#[verifier::tactus_auto]
pub fn fold_high(z: u64) -> u64 {
    wadd(z, wmul(z / 0x1_0000_0000, 0x9E3779B9))
}

/// Counter-based hash keyed (seed, counter, tag). Pure and deterministic.
#[verifier::tactus_auto]
pub fn hash3(seed: u64, counter: u64, tag: u64) -> u64 {
    let z0 = wadd(wadd(wmul(counter, 0x9E3779B97F4A7C15), wmul(tag, 0xBF58476D1CE4E5B9)), seed);
    let z1 = fold_high(wmul(z0, 0x94D049BB133111EB));
    let z2 = fold_high(wmul(z1, 0xD6E8FEB86659FD93));
    z2
}

/// Range reduction: the one fact consumers need.
#[verifier::tactus_auto]
#[verifier::tactus_tactic("tactus_first | tactus_auto | (intros; omega) | (rcases arch_word_bits_valid with h | h <;> simp only [usize_hi, isize_hi, h] at * <;> (intros; omega))")]
pub fn hash_below(h: u64, n: usize) -> (r: usize)
    requires 1 <= n,
    ensures r < n,
{
    let m = h % (n as u64);
    // omega only handles % by constants; mod-by-variable needs the
    // emod bound lemma applied explicitly.
    assert(m < n as u64) by {
        intros
        exact Int.emod_lt_of_pos _ (Int.natCast_pos.mpr (by omega))
    };
    m as usize
}

} // verus!
