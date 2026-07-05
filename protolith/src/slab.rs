//! The slab: a thin 3D voxel grid, toroidal in X and Y, open in Z.
//!
//! DESIGN.md §2. This module owns dimension bookkeeping, row-major linear
//! indexing, toroidal wrap arithmetic, and the exact squared torus distance
//! used as the crystallization arrival-time key (M0: unit growth speed, so
//! arrival order == distance order). All arithmetic is integer and proved
//! overflow-safe under the `wf()` dimension bounds.
//!
//! Tactus proof conventions (learned the hard way, see git history):
//! - Everything is Lean-native: exec obligations close via `assert(P) by
//!   { <lean tactics> }` in the body (asserted facts thread forward as
//!   hypotheses), proof fns carry `by { <lean> }` bodies. Verus-style
//!   `proof { }` blocks / lemma calls do NOT lower to the Lean backend.
//! - Linear spec fns are `#[verifier::inline]`: `omega` handles the raw
//!   arithmetic (it sees through casts and let-chains, abstracts products
//!   as atoms; its blind spots are `ite` — use `split_ifs` — and the
//!   symbolic `usize_hi` — case-split `arch_word_bits_valid`).
//! - The ite-shaped distance specs stay opaque; each is unfolded exactly
//!   once, at the assert that proves the exec computation matches it.

use vstd::prelude::*;

verus! {

import Mathlib.Tactic.Linarith

/// Slab dimensions. X and Y wrap (torus); Z is open depth.
#[derive(Clone, Copy)]
pub struct SlabDims {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}

impl SlabDims {
    /// Well-formedness: nonzero and bounded so that every derived quantity
    /// below fits comfortably: total() <= 2^30, dist2 < 2^21, and the
    /// composite selection key (dist2 << 32 | index) fits in u64 with room.
    #[verifier::inline]
    pub open spec fn wf(self) -> bool {
        &&& 1 <= self.nx <= 1024
        &&& 1 <= self.ny <= 1024
        &&& 1 <= self.nz <= 1024
    }

    #[verifier::inline]
    pub open spec fn spec_total(self) -> int {
        (self.nx * self.ny) * self.nz
    }

    /// Row-major linear index.
    #[verifier::inline]
    pub open spec fn spec_index(self, x: int, y: int, z: int) -> int {
        x + y * self.nx + z * (self.nx * self.ny)
    }

    /// Slab voxel count as exec, with the 2^30 bound proved.
    #[verifier::tactus_auto]
    pub fn total(&self) -> (t: usize)
        requires self.wf(),
        ensures
            t == self.spec_total(),
            t <= 0x4000_0000,
            t >= 1,
    {
        assert(1 <= (self.nx as int) * (self.ny as int)
            && (self.nx as int) * (self.ny as int) <= 0x10_0000) by {
            intros
            casesm* _ ∧ _
            refine ⟨?_, ?_⟩ <;> nlinarith
        };
        assert(1 <= (self.nx as int) * (self.ny as int) * (self.nz as int)
            && (self.nx as int) * (self.ny as int) * (self.nz as int) <= 0x4000_0000) by {
            intros
            casesm* _ ∧ _
            refine ⟨?_, ?_⟩ <;> nlinarith
        };
        // usize overflow for the two products needs the word-width case-split
        // (tactus_auto can't case-split usize_hi); the ℤ bounds above supply
        // the magnitudes after zify / push_cast.
        assert(self.nx * self.ny <= usize::MAX
            && (self.nx * self.ny) * self.nz <= usize::MAX) by {
            intros
            rcases arch_word_bits_valid with h | h <;>
                simp only [usize_hi, isize_hi, h] at * <;>
                (try zify at *; try push_cast at *; omega)
        };
        // ℕ→ℤ bridge for the `t == spec_total` postcondition.
        assert(((self.nx * self.ny) * self.nz) as int
            == (self.nx as int) * (self.ny as int) * (self.nz as int)) by {
            intros
            push_cast
            omega
        };
        (self.nx * self.ny) * self.nz
    }

    /// Linear index of an in-range coordinate triple.
    #[verifier::tactus_auto]
    pub fn index(&self, x: usize, y: usize, z: usize) -> (i: usize)
        requires
            self.wf(),
            x < self.nx,
            y < self.ny,
            z < self.nz,
        ensures
            i == self.spec_index(x as int, y as int, z as int),
            i < self.spec_total(),
    {
        assert(1 <= (self.nx as int) * (self.ny as int)
            && (self.nx as int) * (self.ny as int) <= 0x10_0000) by {
            intros
            casesm* _ ∧ _
            refine ⟨?_, ?_⟩ <;> nlinarith
        };
        assert(0 <= (y as int) * (self.nx as int)
            && (y as int) * (self.nx as int) <= 0x10_0000) by {
            intros
            casesm* _ ∧ _
            refine ⟨?_, ?_⟩ <;> nlinarith
        };
        // The in-range crux, subtraction-free (Nat-sub is poison for
        // linarith): x + y*nx < nx*ny, then (z+1)*(nx*ny) <= nz*(nx*ny)
        // stated additively, then the sum. nlinarith ring-normalizes the
        // associativity/commutativity differences between the pieces.
        assert((x as int) + (y as int) * (self.nx as int)
            < (self.nx as int) * (self.ny as int)) by {
            intros
            casesm* _ ∧ _
            nlinarith
        };
        assert((z as int) * ((self.nx as int) * (self.ny as int))
                + (self.nx as int) * (self.ny as int)
            <= (self.nz as int) * ((self.nx as int) * (self.ny as int))) by {
            intros
            casesm* _ ∧ _
            nlinarith
        };
        assert(0 <= (z as int) * ((self.nx as int) * (self.ny as int))
            && (z as int) * ((self.nx as int) * (self.ny as int)) <= 0x4000_0000) by {
            intros
            casesm* _ ∧ _
            refine ⟨?_, ?_⟩ <;> nlinarith
        };
        assert((x as int) + (y as int) * (self.nx as int)
                + (z as int) * ((self.nx as int) * (self.ny as int))
            < (self.nx as int) * (self.ny as int) * (self.nz as int)) by {
            intros
            casesm* _ ∧ _
            nlinarith
        };
        // usize overflow for every sub-expression of the index, via the
        // word-width case-split; the ℤ bounds above supply the magnitudes.
        assert(self.nx * self.ny <= usize::MAX
            && y * self.nx <= usize::MAX
            && x + y * self.nx <= usize::MAX
            && z * (self.nx * self.ny) <= usize::MAX
            && x + y * self.nx + z * (self.nx * self.ny) <= usize::MAX) by {
            intros
            rcases arch_word_bits_valid with h | h <;>
                simp only [usize_hi, isize_hi, h] at * <;>
                (try zify at *; try push_cast at *; omega)
        };
        // ℕ→ℤ bridge for the `i == spec_index` postcondition.
        assert((x + y * self.nx + z * (self.nx * self.ny)) as int
            == (x as int) + (y as int) * (self.nx as int)
                + (z as int) * ((self.nx as int) * (self.ny as int))) by {
            intros
            push_cast
            omega
        };
        x + y * self.nx + z * (self.nx * self.ny)
    }

}

/// Toroidal separation along one wrapped axis: the shorter way around.
/// Kept opaque (NOT inline): the ite structure would blind `omega` at
/// every use site; `wrap_delta` proves the connection once.
pub open spec fn spec_wrap_delta(a: int, b: int, n: int) -> int {
    let d = if a >= b { a - b } else { b - a };
    if d <= n - d { d } else { n - d }
}

/// Exec toroidal separation. Result is at most n/2.
#[verifier::tactus_auto]
pub fn wrap_delta(a: usize, b: usize, n: usize) -> (d: usize)
    requires
        1 <= n <= 1024,
        a < n,
        b < n,
    ensures
        d == spec_wrap_delta(a as int, b as int, n as int),
        d <= n / 2,
{
    let raw = if a >= b { a - b } else { b - a };
    let d = if raw <= n - raw { raw } else { n - raw };
    assert(d == spec_wrap_delta(a as int, b as int, n as int)) by {
        intros
        simp_all (config := { zetaDelta := true }) [slab.spec_wrap_delta] <;> omega
    };
    d
}

/// Open-axis separation (Z): plain absolute difference. Opaque and
/// int-typed for the same defeq reason as `spec_wrap_delta` — the
/// torus-distance connection proof needs the exec value substituted as
/// exactly this application.
pub open spec fn spec_abs_delta(a: int, b: int) -> int {
    if a >= b { a - b } else { b - a }
}

#[verifier::tactus_auto]
pub fn abs_delta(a: usize, b: usize, n: usize) -> (d: usize)
    requires
        1 <= n <= 1024,
        a < n,
        b < n,
    ensures
        d == spec_abs_delta(a as int, b as int),
        d < n,
{
    let d = if a >= b { a - b } else { b - a };
    assert(d == spec_abs_delta(a as int, b as int)) by {
        intros
        simp_all (config := { zetaDelta := true }) [slab.spec_abs_delta] <;> omega
    };
    d
}

/// Squared torus distance between two in-range voxel coordinates:
/// wrapped in X and Y, open in Z. Fits u32 with slack (< 2^21).
pub open spec fn spec_torus_dist2(
    dims: SlabDims,
    x1: int, y1: int, z1: int,
    x2: int, y2: int, z2: int,
) -> int {
    let dx = spec_wrap_delta(x1, x2, dims.nx as int);
    let dy = spec_wrap_delta(y1, y2, dims.ny as int);
    let dz = spec_abs_delta(z1, z2);
    dx * dx + dy * dy + dz * dz
}

// Sum of three squared deltas as u32. The deltas are small (dx, dy <= 512 —
// half the maximum torus width — and dz <= 1023), so the sum of squares is
// < 2^21 and the u32 cast is lossless. Kept as its own fn so the squaring /
// overflow / cast `assert`s stay scoped here; `torus_dist2`'s postconditions
// then follow directly from this `ensures`, without wading through those
// hypotheses (a flattened body would pile them onto every later obligation).
#[verifier::tactus_auto]
fn sum_sq3(dx: usize, dy: usize, dz: usize) -> (r: u32)
    requires dx <= 512, dy <= 512, dz <= 1023,
    ensures
        r as int == (dx as int) * (dx as int) + (dy as int) * (dy as int)
            + (dz as int) * (dz as int),
        r < 0x20_0000,
{
    // Each squared delta fits (nonlinear); `omega` then bounds the sum < 2^21,
    // so the usize products, the sum, and the u32 cast below are all in range.
    assert(dx * dx <= 0x4_0000) by { intros; nlinarith };
    assert(dy * dy <= 0x4_0000) by { intros; nlinarith };
    assert(dz * dz <= 0x10_0000) by { intros; nlinarith };
    assert(dx * dx + dy * dy + dz * dz < 0x20_0000) by {
        intros
        omega
    };
    // The usize overflow checks compare against `usize::MAX`, which the Lean
    // backend renders via the symbolic word-width `usize_hi` (>= 2^32 on any
    // supported width). `tactus_auto` can't case-split the word size, so
    // discharge those bounds here (products/sum are far below 2^32 above).
    assert(dx * dx <= usize::MAX && dy * dy <= usize::MAX && dz * dz <= usize::MAX
        && dx * dx + dy * dy + dz * dz <= usize::MAX) by {
        intros
        rcases arch_word_bits_valid with h | h <;>
            simp only [usize_hi, isize_hi, h] at * <;> omega
    };
    // ℕ→ℤ bridge: the exec sum, cast to int, equals the ℤ sum-of-squares.
    assert((dx * dx + dy * dy + dz * dz) as int
        == (dx as int) * (dx as int) + (dy as int) * (dy as int)
            + (dz as int) * (dz as int)) by {
        intros
        push_cast
        omega
    };
    (dx * dx + dy * dy + dz * dz) as u32
}

#[verifier::tactus_auto]
pub fn torus_dist2(
    dims: &SlabDims,
    x1: usize, y1: usize, z1: usize,
    x2: usize, y2: usize, z2: usize,
) -> (d2: u32)
    requires
        dims.wf(),
        x1 < dims.nx, y1 < dims.ny, z1 < dims.nz,
        x2 < dims.nx, y2 < dims.ny, z2 < dims.nz,
    ensures
        d2 == spec_torus_dist2(*dims,
            x1 as int, y1 as int, z1 as int,
            x2 as int, y2 as int, z2 as int),
        d2 < 0x20_0000,
{
    let dx = wrap_delta(x1, x2, dims.nx);
    let dy = wrap_delta(y1, y2, dims.ny);
    let dz = abs_delta(z1, z2, dims.nz);
    assert(dx <= 512 && dy <= 512 && dz <= 1023) by {
        intros
        omega
    };
    let r = sum_sq3(dx, dy, dz);
    // `r as int == dx² + dy² + dz²` (sum_sq3), and each delta equals its spec
    // value (`dx == spec_wrap_delta …` etc. from wrap_delta / abs_delta), so
    // `simp_all` rewrites the sum-of-squares into the distance spec.
    assert(r as int == spec_torus_dist2(*dims,
        x1 as int, y1 as int, z1 as int,
        x2 as int, y2 as int, z2 as int)) by {
        intros
        simp_all (config := { zetaDelta := true }) [slab.spec_torus_dist2]
    };
    r
}

} // verus!
