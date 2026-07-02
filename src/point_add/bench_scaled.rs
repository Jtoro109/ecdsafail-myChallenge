//! (refactor) Mechanical split of bench.rs: scaled_by_* microsteps. No logic changes.
use super::*;

pub(crate) fn scaled_by_controlled_microstep(
    b: &mut B,
    r: &[QubitId],
    s: &[QubitId],
    odd: QubitId,
    a: QubitId,
    p: U256,
) {
    // Direct scaled Bernstein-Yang tagged-DIV microstep:
    //   C: (r,s) -> (r, s/2)
    //   B: (r,s) -> (r, (s+r)/2)
    //   A: (r,s) -> (s, (s-r)/2)
    // A is emitted as swap, neg(second row), selected add, halve.
    for i in 0..r.len() {
        cswap(b, a, r[i], s[i]);
    }
    by_cmod_neg_inplace_fast(b, s, a, p);
    cmod_add_qq(b, s, r, odd, p);
    mod_halve_inplace_fast(b, s, p);
}

pub(crate) fn scaled_by_controlled_microstep_inverse_negr_for_bench(
    b: &mut B,
    u_neg_r: &[QubitId],
    s: &[QubitId],
    odd: QubitId,
    a: QubitId,
    p: U256,
) {
    // Inverse scaled BY step in the sign-flipped frame u=-r:
    //   C: (u,s) -> (u, 2s)
    //   B: (u,s) -> (u, 2s+u)
    //   A: (u,s) -> (u+2s, -u)
    // This product-clean path avoids centered parity history entirely.  Use the
    // canonical controlled negation so a logically-zero final u can be freed.
    mod_double_inplace_fast(b, s, p);
    cmod_add_qq(b, s, u_neg_r, odd, p);
    for i in 0..u_neg_r.len() {
        cswap(b, a, u_neg_r[i], s[i]);
    }
    by_cmod_neg_inplace_canonical_for_bench(b, s, a, p);
}

