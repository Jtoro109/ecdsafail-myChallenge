//! (refactor r2) Mechanically extracted from mul.rs. No logic changes.
use super::*;

// ─────────────────────────────────────────────────────────────────────────────────────
// Litinski add-subtract (arXiv:2410.00899) primitives
// ─────────────────────────────────────────────────────────────────────────────────────

/// Controlled add-subtract on (n+1)-bit `acc` with n-bit `x` (padded with 0 at top).
///   ctrl=1 : acc += x  (mod 2^(n+1))
///   ctrl=0 : acc -= x  (mod 2^(n+1))
/// Implementation: conditionally two's-complement (~x + 1) via flip-x plus c_in,
/// then run a single unconditional Gidney/Cuccaro add. Cost = n-1 Toffoli (same as
/// uncontrolled (n+1)-bit add without carry-out).
pub(crate) fn controlled_add_subtract_fast(b: &mut B, x: &[QubitId], acc: &[QubitId], ctrl: QubitId) {
    let n = x.len();
    debug_assert_eq!(acc.len(), n + 1);

    // x_ext: n+1 bits with top pad bit = 0. Only the low n bits of x_ext are flipped
    // when ctrl=0 (two's-complement subtract via ~a + 1). The pad bit stays 0.
    let pad = b.alloc_qubit();
    let mut x_ext = x.to_vec();
    x_ext.push(pad);

    let c_in = b.alloc_qubit();

    // If ctrl=0, we want x_ext[0..n] = ~x and c_in = 1. Encode via x(ctrl) + cx.
    b.x(ctrl);
    for i in 0..n {
        b.cx(ctrl, x_ext[i]);
    }
    b.cx(ctrl, c_in);

    cuccaro_add_fast(b, &x_ext, acc, c_in);

    b.cx(ctrl, c_in);
    for i in 0..n {
        b.cx(ctrl, x_ext[i]);
    }
    b.x(ctrl);

    b.free(c_in);
    b.free(pad);
}

/// Low-peak variant of `controlled_add_subtract_fast` using non-fast
/// Cuccaro (no carry ancillae). Saves ~n qubits of transient peak at the
/// cost of ~n extra Toffolis per call. Useful when called inside the
/// Kaliski-body mul sites where peak is tight.
pub(crate) fn controlled_add_subtract_lowq(b: &mut B, x: &[QubitId], acc: &[QubitId], ctrl: QubitId) {
    let n = x.len();
    debug_assert_eq!(acc.len(), n + 1);

    let pad = b.alloc_qubit();
    let mut x_ext = x.to_vec();
    x_ext.push(pad);

    let c_in = b.alloc_qubit();

    b.x(ctrl);
    for i in 0..n {
        b.cx(ctrl, x_ext[i]);
    }
    b.cx(ctrl, c_in);

    cuccaro_add(b, &x_ext, acc, c_in);

    b.cx(ctrl, c_in);
    for i in 0..n {
        b.cx(ctrl, x_ext[i]);
    }
    b.x(ctrl);

    b.free(c_in);
    b.free(pad);
}

/// Inverse of `controlled_add_subtract_lowq`.
pub(crate) fn controlled_add_subtract_lowq_inverse(b: &mut B, x: &[QubitId], acc: &[QubitId], ctrl: QubitId) {
    let n = x.len();
    debug_assert_eq!(acc.len(), n + 1);

    let pad = b.alloc_qubit();
    let mut x_ext = x.to_vec();
    x_ext.push(pad);

    let c_in = b.alloc_qubit();

    b.x(ctrl);
    for i in 0..n {
        b.cx(ctrl, x_ext[i]);
    }
    b.cx(ctrl, c_in);

    cuccaro_sub(b, &x_ext, acc, c_in);

    b.cx(ctrl, c_in);
    for i in 0..n {
        b.cx(ctrl, x_ext[i]);
    }
    b.x(ctrl);

    b.free(c_in);
    b.free(pad);
}

/// Inverse of controlled_add_subtract_fast: swap add↔sub.
///   ctrl=1 : acc -= x
///   ctrl=0 : acc += x
pub(crate) fn controlled_add_subtract_fast_inverse(b: &mut B, x: &[QubitId], acc: &[QubitId], ctrl: QubitId) {
    let n = x.len();
    debug_assert_eq!(acc.len(), n + 1);

    let pad = b.alloc_qubit();
    let mut x_ext = x.to_vec();
    x_ext.push(pad);

    let c_in = b.alloc_qubit();

    b.x(ctrl);
    for i in 0..n {
        b.cx(ctrl, x_ext[i]);
    }
    b.cx(ctrl, c_in);

    cuccaro_sub_fast(b, &x_ext, acc, c_in);

    b.cx(ctrl, c_in);
    for i in 0..n {
        b.cx(ctrl, x_ext[i]);
    }
    b.x(ctrl);

    b.free(c_in);
    b.free(pad);
}

/// Low-scratch `wide -= x` where `x` is n-bit and `wide` is (2n+1)-bit.
/// Instead of extending `x` to the full (2n+1) width (which allocates ~n+1
/// pad qubits — the dominant transient scratch inside the Litinski multiply),
/// subtract `x` only from the low n bits and ripple the single borrow up the
/// high (n+1) bits with a register-free controlled decrement. Transient
/// scratch ≈ n (one comparator's carries) instead of ~2n. Correct-by-
/// construction from validated primitives (cmp_lt / cuccaro_sub_fast /
/// csub_nbit_const_direct_fast).
///
/// Borrow algebra: borrow = (L_old < x); L_new = (L_old - x) mod 2^n;
/// H_new = H - borrow. Uncompute borrow = (~x < L_new) (proven identity
/// L_old < x  iff  L_new >= 2^n - x  iff  ~x < L_new).
pub(crate) fn correction_sub_x_lowscratch(b: &mut B, x: &[QubitId], wide: &[QubitId]) {
    let n = x.len();
    debug_assert_eq!(wide.len(), 2 * n + 1);
    let lo: Vec<QubitId> = wide[0..n].to_vec();
    let hi: Vec<QubitId> = wide[n..2 * n + 1].to_vec();

    let borrow = b.alloc_qubit();
    // borrow = (L_old < x)
    cmp_lt_into_fast(b, &lo, x, borrow);
    // L -= x  (mod 2^n)
    {
        let c_in = b.alloc_qubit();
        cuccaro_sub_fast(b, x, &lo, c_in);
        b.free(c_in);
    }
    // H -= borrow  (register-free controlled decrement of the high n+1 bits)
    csub_nbit_const_direct_fast(b, &hi, U256::from(1u64), borrow);
    // Uncompute borrow = (~x < L_new): flip x (free), compare, flip back.
    for i in 0..n {
        b.x(x[i]);
    }
    cmp_lt_into_fast(b, x, &lo, borrow);
    for i in 0..n {
        b.x(x[i]);
    }
    b.free(borrow);
}

/// Exact gate-level inverse of `correction_sub_x_lowscratch`: `wide += x`.
pub(crate) fn correction_add_x_lowscratch(b: &mut B, x: &[QubitId], wide: &[QubitId]) {
    let n = x.len();
    debug_assert_eq!(wide.len(), 2 * n + 1);
    let lo: Vec<QubitId> = wide[0..n].to_vec();
    let hi: Vec<QubitId> = wide[n..2 * n + 1].to_vec();

    let borrow = b.alloc_qubit();
    // Reverse the borrow-uncompute: recompute borrow = (~x < L_new).
    for i in 0..n {
        b.x(x[i]);
    }
    cmp_lt_into_fast(b, x, &lo, borrow);
    for i in 0..n {
        b.x(x[i]);
    }
    // Reverse H -= borrow  ->  H += borrow.
    cadd_nbit_const_direct_fast(b, &hi, U256::from(1u64), borrow);
    // Reverse L -= x  ->  L += x.
    {
        let c_in = b.alloc_qubit();
        cuccaro_add_fast(b, x, &lo, c_in);
        b.free(c_in);
    }
    // Reverse borrow compute: borrow = (L_old < x) (now L = L_old again).
    cmp_lt_into_fast(b, &lo, x, borrow);
    b.free(borrow);
}

/// Variant of `schoolbook_mul_into_addsub` that uses the low-scratch
/// borrow-ripple `-x` correction, dropping the multiply's transient peak by
/// ~n (the full-width x_ext pads). Semantics identical: `tmp_ext += x*y`.
pub(crate) fn schoolbook_mul_into_addsub_lsx(b: &mut B, x: &[QubitId], y: &[QubitId], tmp_ext: &[QubitId]) {
    let n = x.len();
    debug_assert_eq!(y.len(), n);
    debug_assert_eq!(tmp_ext.len(), 2 * n);

    let low = b.alloc_qubit();
    let mut wide: Vec<QubitId> = Vec::with_capacity(2 * n + 1);
    wide.push(low);
    wide.extend_from_slice(tmp_ext);

    for k in 0..n {
        let slice: Vec<QubitId> = wide[k..k + n + 1].to_vec();
        controlled_add_subtract_fast(b, x, &slice, y[k]);
    }

    // +2^n * (y + 1)
    {
        let pad = b.alloc_qubit();
        let mut y_ext = y.to_vec();
        y_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        b.x(c_in);
        cuccaro_add_fast(b, &y_ext, &slice, c_in);
        b.x(c_in);
        b.free(c_in);
        b.free(pad);
    }

    // -2^{2n}
    b.x(wide[2 * n]);

    // -x (low-scratch borrow-ripple, the peak-relevant change).
    correction_sub_x_lowscratch(b, x, &wide);

    // +2^n * x
    {
        let pad = b.alloc_qubit();
        let mut x_ext = x.to_vec();
        x_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        cuccaro_add_fast(b, &x_ext, &slice, c_in);
        b.free(c_in);
        b.free(pad);
    }

    b.free(low);
}

/// Exact gate-level inverse of `schoolbook_mul_into_addsub_lsx`.
pub(crate) fn schoolbook_mul_into_addsub_lsx_inverse(
    b: &mut B,
    x: &[QubitId],
    y: &[QubitId],
    tmp_ext: &[QubitId],
) {
    let n = x.len();
    debug_assert_eq!(y.len(), n);
    debug_assert_eq!(tmp_ext.len(), 2 * n);

    let low = b.alloc_qubit();
    let mut wide: Vec<QubitId> = Vec::with_capacity(2 * n + 1);
    wide.push(low);
    wide.extend_from_slice(tmp_ext);

    // Reverse correction 4: sub x at bit n.
    {
        let pad = b.alloc_qubit();
        let mut x_ext = x.to_vec();
        x_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        cuccaro_sub_fast(b, &x_ext, &slice, c_in);
        b.free(c_in);
        b.free(pad);
    }
    // Reverse correction 3 (-x): add x back via the low-scratch ripple.
    correction_add_x_lowscratch(b, x, &wide);
    // Reverse correction 2: toggle wide[2n].
    b.x(wide[2 * n]);
    // Reverse correction 1: sub (y+1) at bit n.
    {
        let pad = b.alloc_qubit();
        let mut y_ext = y.to_vec();
        y_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        b.x(c_in);
        cuccaro_sub_fast(b, &y_ext, &slice, c_in);
        b.x(c_in);
        b.free(c_in);
        b.free(pad);
    }
    // Reverse n add-subtract rows.
    for k in (0..n).rev() {
        let slice: Vec<QubitId> = wide[k..k + n + 1].to_vec();
        controlled_add_subtract_fast_inverse(b, x, &slice, y[k]);
    }

    b.free(low);
}

/// Litinski 2024 add-subtract schoolbook: tmp_ext += x * y.
///
/// Precondition: tmp_ext has 2n bits and holds value A_in.
/// Postcondition: tmp_ext holds A_in + x*y (mod 2^{2n}).
pub(crate) fn schoolbook_mul_into_addsub(b: &mut B, x: &[QubitId], y: &[QubitId], tmp_ext: &[QubitId]) {
    let n = x.len();
    debug_assert_eq!(y.len(), n);
    debug_assert_eq!(tmp_ext.len(), 2 * n);

    // wide = [low, tmp_ext[0], ..., tmp_ext[2n-1]]  =  2n+1 bits.
    // This treats the (2n+1)-bit number `wide` as Litinski's accumulator.
    // After all ops, wide = 2*A_in_shifted + 2*x*y  (i.e. 2*(A_in + xy)).
    // `/2 relabel` reads out xy at wide[1..2n+1] = tmp_ext.
    //
    // To add A_in into the 2*(A_in + xy) result correctly, we need to bring A_in
    // in as `2*A_in` in wide. That is done pre-loop: swap tmp_ext values up one bit.
    // But Litinski's derivation assumes A_in = 0. To support non-zero A_in we'd
    // need to double tmp_ext at the start and halve at the end.
    //
    // Fortunately ALL call sites pass tmp_ext starting at 0 (fresh alloc), so we
    // can just assume A_in = 0.
    let low = b.alloc_qubit();
    let mut wide: Vec<QubitId> = Vec::with_capacity(2 * n + 1);
    wide.push(low);
    wide.extend_from_slice(tmp_ext);

    // n controlled add-subtracts (Litinski Fig 2b).
    for k in 0..n {
        let slice: Vec<QubitId> = wide[k..k + n + 1].to_vec();
        controlled_add_subtract_fast(b, x, &slice, y[k]);
    }

    // Corrections:
    //   Using y as ctrl and x as operand, the intermediate value is:
    //     2xy + 2^{2n} - 2^n (x+y+1) + x
    //   Target: 2xy. So apply +2^n(y+1) + 2^n*x - 2^{2n} - x.

    // +2^n * (y + 1): (n+1)-bit add of y_ext (top=0) into wide[n..2n+1] with c_in=1.
    {
        let pad = b.alloc_qubit();
        let mut y_ext = y.to_vec();
        y_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        b.x(c_in);
        if std::env::var("KAL_VENT_MODADD").ok().as_deref() == Some("1") {
            cuccaro_add(b, &y_ext, &slice, c_in);
        } else {
            cuccaro_add_fast(b, &y_ext, &slice, c_in);
        }
        b.x(c_in);
        b.free(c_in);
        b.free(pad);
    }

    // -2^{2n}: toggle wide[2n].
    b.x(wide[2 * n]);

    // -x as full (2n+1)-bit sub. Use in-place cuccaro_sub (no carry ancillae) to
    // keep peak qubits low during this otherwise-expensive full-width correction.
    // Costs n-1 extra Toffoli vs cuccaro_sub_fast but saves 2n peak qubits.
    {
        let mut x_ext: Vec<QubitId> = x.to_vec();
        while x_ext.len() < 2 * n + 1 {
            x_ext.push(b.alloc_qubit());
        }
        let c_in = b.alloc_qubit();
        cuccaro_sub(b, &x_ext, &wide, c_in);
        b.free(c_in);
        for _ in n..2 * n + 1 {
            let q = x_ext.pop().unwrap();
            b.free(q);
        }
    }

    // +2^n * x: (n+1)-bit add of x_ext into wide[n..2n+1].
    {
        let pad = b.alloc_qubit();
        let mut x_ext = x.to_vec();
        x_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        if std::env::var("KAL_VENT_MODADD").ok().as_deref() == Some("1") {
            cuccaro_add(b, &x_ext, &slice, c_in);
        } else {
            cuccaro_add_fast(b, &x_ext, &slice, c_in);
        }
        b.free(c_in);
        b.free(pad);
    }

    // wide = 2xy. /2 relabel: xy is at wide[1..2n+1] = tmp_ext. wide[0]=low should be 0.
    b.free(low);
}

/// Low-peak variant of `schoolbook_mul_into_addsub`: uses non-fast Cuccaro
/// (`cuccaro_add`) inside the `controlled_add_subtract` core and in the
/// correction adders. Saves roughly `n` transient qubits at peak vs. the
/// `_fast` variant at the cost of ~n extra Toffolis per row. Top-level
/// semantics identical to `schoolbook_mul_into_addsub`.
pub(crate) fn schoolbook_mul_into_addsub_lowq(b: &mut B, x: &[QubitId], y: &[QubitId], tmp_ext: &[QubitId]) {
    let n = x.len();
    debug_assert_eq!(y.len(), n);
    debug_assert_eq!(tmp_ext.len(), 2 * n);

    let low = b.alloc_qubit();
    let mut wide: Vec<QubitId> = Vec::with_capacity(2 * n + 1);
    wide.push(low);
    wide.extend_from_slice(tmp_ext);

    for k in 0..n {
        let slice: Vec<QubitId> = wide[k..k + n + 1].to_vec();
        controlled_add_subtract_lowq(b, x, &slice, y[k]);
    }

    // +2^n * (y + 1)
    {
        let pad = b.alloc_qubit();
        let mut y_ext = y.to_vec();
        y_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        b.x(c_in);
        cuccaro_add(b, &y_ext, &slice, c_in);
        b.x(c_in);
        b.free(c_in);
        b.free(pad);
    }

    // -2^{2n}
    b.x(wide[2 * n]);

    // -x full (2n+1)-bit sub
    {
        let mut x_ext: Vec<QubitId> = x.to_vec();
        while x_ext.len() < 2 * n + 1 {
            x_ext.push(b.alloc_qubit());
        }
        let c_in = b.alloc_qubit();
        cuccaro_sub(b, &x_ext, &wide, c_in);
        b.free(c_in);
        for _ in n..2 * n + 1 {
            let q = x_ext.pop().unwrap();
            b.free(q);
        }
    }

    // +2^n * x
    {
        let pad = b.alloc_qubit();
        let mut x_ext = x.to_vec();
        x_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        cuccaro_add(b, &x_ext, &slice, c_in);
        b.free(c_in);
        b.free(pad);
    }

    b.free(low);
}

/// Exact gate-level inverse of `schoolbook_mul_into_addsub_lowq`.
pub(crate) fn schoolbook_mul_into_addsub_lowq_inverse(
    b: &mut B,
    x: &[QubitId],
    y: &[QubitId],
    tmp_ext: &[QubitId],
) {
    let n = x.len();
    debug_assert_eq!(y.len(), n);
    debug_assert_eq!(tmp_ext.len(), 2 * n);

    let low = b.alloc_qubit();
    let mut wide: Vec<QubitId> = Vec::with_capacity(2 * n + 1);
    wide.push(low);
    wide.extend_from_slice(tmp_ext);

    // Reverse correction 4: sub x at bit n.
    {
        let pad = b.alloc_qubit();
        let mut x_ext = x.to_vec();
        x_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        cuccaro_sub(b, &x_ext, &slice, c_in);
        b.free(c_in);
        b.free(pad);
    }
    // Reverse correction 3.
    {
        let mut x_ext: Vec<QubitId> = x.to_vec();
        while x_ext.len() < 2 * n + 1 {
            x_ext.push(b.alloc_qubit());
        }
        let c_in = b.alloc_qubit();
        cuccaro_add(b, &x_ext, &wide, c_in);
        b.free(c_in);
        for _ in n..2 * n + 1 {
            let q = x_ext.pop().unwrap();
            b.free(q);
        }
    }
    // Reverse correction 2.
    b.x(wide[2 * n]);
    // Reverse correction 1.
    {
        let pad = b.alloc_qubit();
        let mut y_ext = y.to_vec();
        y_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        b.x(c_in);
        cuccaro_sub(b, &y_ext, &slice, c_in);
        b.x(c_in);
        b.free(c_in);
        b.free(pad);
    }
    for k in (0..n).rev() {
        let slice: Vec<QubitId> = wide[k..k + n + 1].to_vec();
        controlled_add_subtract_lowq_inverse(b, x, &slice, y[k]);
    }

    b.free(low);
}

/// Exact gate-level inverse of `schoolbook_mul_into_addsub`.
pub(crate) fn schoolbook_mul_into_addsub_inverse(
    b: &mut B,
    x: &[QubitId],
    y: &[QubitId],
    tmp_ext: &[QubitId],
) {
    let n = x.len();
    debug_assert_eq!(y.len(), n);
    debug_assert_eq!(tmp_ext.len(), 2 * n);

    let low = b.alloc_qubit();
    let mut wide: Vec<QubitId> = Vec::with_capacity(2 * n + 1);
    wide.push(low);
    wide.extend_from_slice(tmp_ext);

    // Reverse correction 4: sub x at bit n.
    {
        let pad = b.alloc_qubit();
        let mut x_ext = x.to_vec();
        x_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        cuccaro_sub_fast(b, &x_ext, &slice, c_in);
        b.free(c_in);
        b.free(pad);
    }
    // Reverse correction 3 (sub x full-width): add x back with borrow propagation.
    // Use in-place cuccaro_add (no carries) to keep peak low, matching forward.
    {
        let mut x_ext: Vec<QubitId> = x.to_vec();
        while x_ext.len() < 2 * n + 1 {
            x_ext.push(b.alloc_qubit());
        }
        let c_in = b.alloc_qubit();
        cuccaro_add(b, &x_ext, &wide, c_in);
        b.free(c_in);
        for _ in n..2 * n + 1 {
            let q = x_ext.pop().unwrap();
            b.free(q);
        }
    }
    // Reverse correction 2: toggle wide[2n].
    b.x(wide[2 * n]);
    // Reverse correction 1: sub (y+1) at bit n.
    {
        let pad = b.alloc_qubit();
        let mut y_ext = y.to_vec();
        y_ext.push(pad);
        let slice: Vec<QubitId> = wide[n..2 * n + 1].to_vec();
        let c_in = b.alloc_qubit();
        b.x(c_in);
        cuccaro_sub_fast(b, &y_ext, &slice, c_in);
        b.x(c_in);
        b.free(c_in);
        b.free(pad);
    }
    // Reverse n add-subtract rows.
    for k in (0..n).rev() {
        let slice: Vec<QubitId> = wide[k..k + n + 1].to_vec();
        controlled_add_subtract_fast_inverse(b, x, &slice, y[k]);
    }

    b.free(low);
}

/// Add x*y mod p to acc, via schoolbook into a wide accumulator + Solinas
/// reduction + Bennett uncompute. Saves ~100k CCX vs Horner-on-acc per call.
pub(crate) fn mod_mul_add_into_acc_schoolbook(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));

    let tmp_ext = b.alloc_qubits(2 * n);
    schoolbook_mul_into_addsub(b, x, y, &tmp_ext);

    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    let _ = c;
    mod_add_qq_fast(b, acc, &lo, p);
    // Solinas with 977 = 2^10 - 2^6 + 2^4 + 2^0. c = 2^32 + 977 = {+2^0, +2^4, -2^6, +2^10, +2^32}.
    // 5 ops instead of 7 (saves 2 per call). Use shift_left_by_22 for the 10→32 gap.
    mod_add_qq_fast(b, acc, &hi, p); // position 0
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p); // position 4
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p); // position 6 (SUB because of 977 consolidation)
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p); // position 10
    let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
    mod_add_qq(b, acc, &hi, p); // position 32
    mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    b.set_phase("sol_halve_tail");
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    b.set_phase("schoolbook_mul_inverse");
    schoolbook_mul_into_addsub_inverse(b, x, y, &tmp_ext);
    b.free_vec(&tmp_ext);
}

/// Peak-minimized affine y-mul `acc += x*y mod p`. Drops the y-mul binder from
/// 2565 to 2459 (the next cluster) by removing the ~256-wide transient scratch
/// that the schoolbook MAC holds ON TOP of its 512-bit product `tmp_ext` while
/// the lam² square's 512-bit `tmp_ext` co-resides. Three independent scratch
/// cuts, each replacing a register-allocating primitive with its carry/register-
/// free equivalent (all near-Toffoli-neutral, measured +0.10% total):
///   1. forward/inverse mul: `schoolbook_mul_into_addsub_lsx` — the `-x`
///      correction's full-width x_ext pads (~n) -> a 1-qubit borrow ripple.
///   2. Solinas fold adds/sub: `mod_*_qq_lowq_lowscratch` — carry-free Cuccaro
///      + register-free direct const adders + carry-free comparator.
///   3. Solinas fold doublings: `mod_double_inplace_direct` — register-free
///      direct const-add (no `load_const` register + 256 add carries co-live).
/// The binding instant inside the schoolbook fold was the `mod_double` step
/// (cadd_nbit_const_fast holds a 256-bit const register AND 256 add carries =
/// ~512 transient); cut #3 is the dominant lever. Validated 9024-shot clean.
pub(crate) fn mod_mul_add_into_acc_schoolbook_lowscratch_fold(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let n = acc.len();
    debug_assert_eq!(n, 256);

    let tmp_ext = b.alloc_qubits(2 * n);
    schoolbook_mul_into_addsub_lsx(b, x, y, &tmp_ext);

    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    mod_add_qq_lowq_lowscratch(b, acc, &lo, p);
    mod_add_qq_lowq_lowscratch(b, acc, &hi, p); // position 0
    for _ in 0..4 {
        mod_double_inplace_direct(b, &hi, p);
    }
    mod_add_qq_lowq_lowscratch(b, acc, &hi, p); // position 4
    for _ in 0..2 {
        mod_double_inplace_direct(b, &hi, p);
    }
    mod_sub_qq_lowq_lowscratch(b, acc, &hi, p); // position 6
    for _ in 0..4 {
        mod_double_inplace_direct(b, &hi, p);
    }
    mod_add_qq_lowq_lowscratch(b, acc, &hi, p); // position 10
    if gz_solinas_lowscratch() {
        // The shift22 at this Solinas fold is the affine y-mul binder (2333).
        // Borrow the co-resident dirty product `lo` half (restored on exit) as
        // the venting dirty donor so the shift22 reduction holds ~k+5 scratch
        // instead of ~257, dropping the binder below the bk_step4 floor (2309).
        let (spill, flag_inv, ovf) = mod_shift_left_by_k_dirty(b, &hi, p, 22, &lo);
        b.set_phase("shift22_pos32_dirty");
        mod_add_qq_dirty(b, acc, &hi, p, &lo); // position 32 (venting dirty-borrow)
        mod_shift_right_by_k_dirty(b, &hi, p, 22, spill, flag_inv, ovf, &lo);
    } else {
        let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
        mod_add_qq(b, acc, &hi, p); // position 32
        mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    }
    b.set_phase("sol_halve_tail");
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    b.set_phase("schoolbook_mul_inverse");
    schoolbook_mul_into_addsub_lsx_inverse(b, x, y, &tmp_ext);
    b.free_vec(&tmp_ext);
}

/// From-zero (acc == 0 on entry) twin of
/// `mod_mul_add_into_acc_schoolbook_lowscratch_fold`. Same three scratch cuts
/// (lsx mul, lowscratch Solinas folds, register-free direct doubles) but the
/// first lo-add uses the from-zero CX-copy path. Used for the pair1_borrow_dx
/// mul1 binder under the 9n-floor flag.
pub(crate) fn mod_mul_write_into_zero_acc_schoolbook_lowscratch_fold(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let n = acc.len();
    debug_assert_eq!(n, 256);

    let tmp_ext = b.alloc_qubits(2 * n);
    schoolbook_mul_into_addsub_lsx(b, x, y, &tmp_ext);

    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    // acc == 0 on entry: first lo-add is a CX-copy + register-free correction.
    mod_add_qq_fast_from_zero_lowscratch(b, acc, &lo, p);
    mod_add_qq_lowq_lowscratch(b, acc, &hi, p); // position 0
    for _ in 0..4 {
        mod_double_inplace_direct(b, &hi, p);
    }
    mod_add_qq_lowq_lowscratch(b, acc, &hi, p); // position 4
    for _ in 0..2 {
        mod_double_inplace_direct(b, &hi, p);
    }
    mod_sub_qq_lowq_lowscratch(b, acc, &hi, p); // position 6
    for _ in 0..4 {
        mod_double_inplace_direct(b, &hi, p);
    }
    mod_add_qq_lowq_lowscratch(b, acc, &hi, p); // position 10
    // SHIFT22_FOLD_DIRTY (default ON): the shift22 inside this Solinas fold is the
    // pair1_mul1 peak binder (base 972 carrier+init + tmp_ext 512 + acc 256 = 1740;
    // the plain shift22's ~257-wide CLEAN `padded` transient lifts it to 2025 — the
    // GLOBAL peak). The product's LOW half `lo` (tmp_ext[0..n]) is DEAD here: it was
    // consumed read-only by the from_zero lo-add above and is not touched again until
    // the multiply uncompute (`schoolbook_mul_into_addsub_lsx_inverse`). So `lo` is a
    // valid co-resident DIRTY donor for `mod_shift_left_by_k_dirty`, which vents the
    // spill folds + const add/sub through the dirty borrow instead of allocating the
    // 257-wide clean padded — removing the shift22 transient from the binder. The
    // venting restores `lo` to its entry value, so the multiply uncompute is exact.
    // Set SHIFT22_FOLD_DIRTY=0 to restore the byte-identical clean-padded shift22.
    let shift22_fold_dirty = env_flag_enabled("SHIFT22_FOLD_DIRTY", true);
    if shift22_fold_dirty {
        let (spill, flag_inv, ovf) = mod_shift_left_by_k_dirty(b, &hi, p, 22, &lo);
        b.set_phase("shift22_pos32_dirty");
        mod_add_qq_dirty(b, acc, &hi, p, &lo); // position 32 (venting dirty-borrow)
        mod_shift_right_by_k_dirty(b, &hi, p, 22, spill, flag_inv, ovf, &lo);
    } else {
        let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
        mod_add_qq(b, acc, &hi, p); // position 32
        mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    }
    b.set_phase("sol_halve_tail");
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    b.set_phase("schoolbook_mul_inverse");
    schoolbook_mul_into_addsub_lsx_inverse(b, x, y, &tmp_ext);
    b.free_vec(&tmp_ext);
}
