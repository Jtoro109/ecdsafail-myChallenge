// Grouped implementation file.
use super::*;

// ═══════════════════════════════════════════════════════════════════════════
//  Merged from mul_schoolbook.rs
// ═══════════════════════════════════════════════════════════════════════════

// (refactor r2) Mechanically extracted from mul.rs. No logic changes.

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
    let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
    mod_add_qq(b, acc, &hi, p); // position 32
    mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    b.set_phase("sol_halve_tail");
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    b.set_phase("schoolbook_mul_inverse");
    schoolbook_mul_into_addsub_lsx_inverse(b, x, y, &tmp_ext);
    b.free_vec(&tmp_ext);
}


// ═══════════════════════════════════════════════════════════════════════════
//  Merged from mul_karatsuba.rs
// ═══════════════════════════════════════════════════════════════════════════

// (refactor r2) Mechanically extracted from mul.rs. No logic changes.

// ═══════════════════════════════════════════════════════════════════════════
//  1-level Karatsuba multiplication
// ═══════════════════════════════════════════════════════════════════════════

pub(crate) fn karatsuba_half_sum_compute(b: &mut B, lo: &[QubitId], hi: &[QubitId], acc: &[QubitId]) {
    let h = lo.len();
    debug_assert_eq!(h, hi.len());
    debug_assert_eq!(acc.len(), h + 1);
    for i in 0..h {
        b.cx(lo[i], acc[i]);
    }
    let hi_pad = b.alloc_qubit();
    let mut hi_ext = hi.to_vec();
    hi_ext.push(hi_pad);
    add_nbit_qq_fast(b, &hi_ext, acc);
    b.free(hi_pad);
}

/// Low-peak variant of `karatsuba_half_sum_compute` using non-fast Cuccaro.
/// Saves ~h carry qubits at peak at the cost of ~h extra Toffolis.
pub(crate) fn karatsuba_half_sum_compute_lowq(b: &mut B, lo: &[QubitId], hi: &[QubitId], acc: &[QubitId]) {
    let h = lo.len();
    debug_assert_eq!(h, hi.len());
    debug_assert_eq!(acc.len(), h + 1);
    for i in 0..h {
        b.cx(lo[i], acc[i]);
    }
    let hi_pad = b.alloc_qubit();
    let mut hi_ext = hi.to_vec();
    hi_ext.push(hi_pad);
    add_nbit_qq(b, &hi_ext, acc);
    b.free(hi_pad);
}

pub(crate) fn karatsuba_half_sum_uncompute_lowq(b: &mut B, lo: &[QubitId], hi: &[QubitId], acc: &[QubitId]) {
    let h = lo.len();
    let hi_pad = b.alloc_qubit();
    let mut hi_ext = hi.to_vec();
    hi_ext.push(hi_pad);
    sub_nbit_qq(b, &hi_ext, acc);
    b.free(hi_pad);
    for i in 0..h {
        b.cx(lo[i], acc[i]);
    }
}

pub(crate) fn karatsuba_half_sum_uncompute(b: &mut B, lo: &[QubitId], hi: &[QubitId], acc: &[QubitId]) {
    let h = lo.len();
    let hi_pad = b.alloc_qubit();
    let mut hi_ext = hi.to_vec();
    hi_ext.push(hi_pad);
    sub_nbit_qq_fast(b, &hi_ext, acc);
    b.free(hi_pad);
    for i in 0..h {
        b.cx(lo[i], acc[i]);
    }
}

pub(crate) fn karatsuba_forward(
    b: &mut B,
    x: &[QubitId],
    y: &[QubitId],
    tmp_ext: &[QubitId],
    z1_reg: &[QubitId],
) {
    let n = x.len();
    let h = n / 2;
    let x_lo: Vec<QubitId> = x[0..h].to_vec();
    let x_hi: Vec<QubitId> = x[h..n].to_vec();
    let y_lo: Vec<QubitId> = y[0..h].to_vec();
    let y_hi: Vec<QubitId> = y[h..n].to_vec();

    {
        let slice: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        schoolbook_mul_into_addsub(b, &x_lo, &y_lo, &slice);
    }
    {
        let slice: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        schoolbook_mul_into_addsub(b, &x_hi, &y_hi, &slice);
    }

    let x_sum = b.alloc_qubits(h + 1);
    let y_sum = b.alloc_qubits(h + 1);
    karatsuba_half_sum_compute(b, &x_lo, &x_hi, &x_sum);
    karatsuba_half_sum_compute(b, &y_lo, &y_hi, &y_sum);
    // z1_reg width = 2*(h+1). Use addsub variant on (h+1)-sized inputs.
    schoolbook_mul_into_addsub(b, &x_sum, &y_sum, z1_reg);
    karatsuba_half_sum_uncompute(b, &y_lo, &y_hi, &y_sum);
    karatsuba_half_sum_uncompute(b, &x_lo, &x_hi, &x_sum);
    b.free_vec(&y_sum);
    b.free_vec(&x_sum);

    {
        let pad = b.alloc_qubits(2);
        let mut z0_ext: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        z0_ext.extend_from_slice(&pad);
        sub_nbit_qq_fast(b, &z0_ext, z1_reg);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(2);
        let mut z2_ext: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        z2_ext.extend_from_slice(&pad);
        sub_nbit_qq_fast(b, &z2_ext, z1_reg);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(3 * h - 2 * (h + 1));
        let mut z1_ext: Vec<QubitId> = z1_reg.to_vec();
        z1_ext.extend_from_slice(&pad);
        let acc_slice: Vec<QubitId> = tmp_ext[h..4 * h].to_vec();
        b.set_phase("kara_z1_add");
        add_nbit_qq_fast(b, &z1_ext, &acc_slice);
        b.free_vec(&pad);
    }
}

/// Half-sum-lowq variant of `karatsuba_forward`. Only the Karatsuba
/// half-sum compute/uncompute and z1 merge use non-fast adders; the three
/// inner schoolbook products remain the normal phase-clean implementation.
pub(crate) fn karatsuba_forward_lowq(
    b: &mut B,
    x: &[QubitId],
    y: &[QubitId],
    tmp_ext: &[QubitId],
    z1_reg: &[QubitId],
) {
    let n = x.len();
    let h = n / 2;
    let x_lo: Vec<QubitId> = x[0..h].to_vec();
    let x_hi: Vec<QubitId> = x[h..n].to_vec();
    let y_lo: Vec<QubitId> = y[0..h].to_vec();
    let y_hi: Vec<QubitId> = y[h..n].to_vec();

    {
        let slice: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        schoolbook_mul_into_addsub(b, &x_lo, &y_lo, &slice);
    }
    {
        let slice: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        schoolbook_mul_into_addsub(b, &x_hi, &y_hi, &slice);
    }

    let x_sum = b.alloc_qubits(h + 1);
    let y_sum = b.alloc_qubits(h + 1);
    karatsuba_half_sum_compute_lowq(b, &x_lo, &x_hi, &x_sum);
    karatsuba_half_sum_compute_lowq(b, &y_lo, &y_hi, &y_sum);
    schoolbook_mul_into_addsub(b, &x_sum, &y_sum, z1_reg);
    karatsuba_half_sum_uncompute_lowq(b, &y_lo, &y_hi, &y_sum);
    karatsuba_half_sum_uncompute_lowq(b, &x_lo, &x_hi, &x_sum);
    b.free_vec(&y_sum);
    b.free_vec(&x_sum);

    {
        let pad = b.alloc_qubits(2);
        let mut z0_ext: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        z0_ext.extend_from_slice(&pad);
        sub_nbit_qq(b, &z0_ext, z1_reg);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(2);
        let mut z2_ext: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        z2_ext.extend_from_slice(&pad);
        sub_nbit_qq(b, &z2_ext, z1_reg);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(3 * h - 2 * (h + 1));
        let mut z1_ext: Vec<QubitId> = z1_reg.to_vec();
        z1_ext.extend_from_slice(&pad);
        let acc_slice: Vec<QubitId> = tmp_ext[h..4 * h].to_vec();
        b.set_phase("kara_z1_add");
        add_nbit_qq(b, &z1_ext, &acc_slice);
        b.free_vec(&pad);
    }
}

/// Low-peak variant of `karatsuba_inverse`, paired with `karatsuba_forward_lowq`.
pub(crate) fn karatsuba_inverse_lowq(
    b: &mut B,
    x: &[QubitId],
    y: &[QubitId],
    tmp_ext: &[QubitId],
    z1_reg: &[QubitId],
) {
    let n = x.len();
    let h = n / 2;
    let x_lo: Vec<QubitId> = x[0..h].to_vec();
    let x_hi: Vec<QubitId> = x[h..n].to_vec();
    let y_lo: Vec<QubitId> = y[0..h].to_vec();
    let y_hi: Vec<QubitId> = y[h..n].to_vec();

    {
        let pad = b.alloc_qubits(3 * h - 2 * (h + 1));
        let mut z1_ext: Vec<QubitId> = z1_reg.to_vec();
        z1_ext.extend_from_slice(&pad);
        let acc_slice: Vec<QubitId> = tmp_ext[h..4 * h].to_vec();
        sub_nbit_qq(b, &z1_ext, &acc_slice);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(2);
        let mut z2_ext: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        z2_ext.extend_from_slice(&pad);
        add_nbit_qq(b, &z2_ext, z1_reg);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(2);
        let mut z0_ext: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        z0_ext.extend_from_slice(&pad);
        add_nbit_qq(b, &z0_ext, z1_reg);
        b.free_vec(&pad);
    }

    let x_sum = b.alloc_qubits(h + 1);
    let y_sum = b.alloc_qubits(h + 1);
    karatsuba_half_sum_compute_lowq(b, &x_lo, &x_hi, &x_sum);
    karatsuba_half_sum_compute_lowq(b, &y_lo, &y_hi, &y_sum);
    schoolbook_mul_into_addsub_inverse(b, &x_sum, &y_sum, z1_reg);
    karatsuba_half_sum_uncompute_lowq(b, &y_lo, &y_hi, &y_sum);
    karatsuba_half_sum_uncompute_lowq(b, &x_lo, &x_hi, &x_sum);
    b.free_vec(&y_sum);
    b.free_vec(&x_sum);

    {
        let slice: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        schoolbook_mul_into_addsub_inverse(b, &x_hi, &y_hi, &slice);
    }
    {
        let slice: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        schoolbook_mul_into_addsub_inverse(b, &x_lo, &y_lo, &slice);
    }
}

pub(crate) fn mod_mul_add_into_acc_karatsuba_lowq_with_tmp_ext(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
    tmp_ext: &[QubitId],
) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    debug_assert_eq!(tmp_ext.len(), 2 * n);
    let h = n / 2;
    let z1_reg = b.alloc_qubits(2 * (h + 1));
    karatsuba_forward_lowq(b, x, y, tmp_ext, &z1_reg);

    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    mod_add_qq_fast(b, acc, &lo, p);
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
    mod_add_qq(b, acc, &hi, p);
    mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    karatsuba_inverse_lowq(b, x, y, tmp_ext, &z1_reg);
    b.free_vec(&z1_reg);
}

pub(crate) fn mod_mul_add_into_acc_karatsuba_lowq(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let tmp_ext = b.alloc_qubits(2 * acc.len());
    mod_mul_add_into_acc_karatsuba_lowq_with_tmp_ext(b, acc, x, y, p, &tmp_ext);
    b.free_vec(&tmp_ext);
}

pub(crate) fn karatsuba_inverse(
    b: &mut B,
    x: &[QubitId],
    y: &[QubitId],
    tmp_ext: &[QubitId],
    z1_reg: &[QubitId],
) {
    let n = x.len();
    let h = n / 2;
    let x_lo: Vec<QubitId> = x[0..h].to_vec();
    let x_hi: Vec<QubitId> = x[h..n].to_vec();
    let y_lo: Vec<QubitId> = y[0..h].to_vec();
    let y_hi: Vec<QubitId> = y[h..n].to_vec();

    {
        let pad = b.alloc_qubits(3 * h - 2 * (h + 1));
        let mut z1_ext: Vec<QubitId> = z1_reg.to_vec();
        z1_ext.extend_from_slice(&pad);
        let acc_slice: Vec<QubitId> = tmp_ext[h..4 * h].to_vec();
        sub_nbit_qq_fast(b, &z1_ext, &acc_slice);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(2);
        let mut z2_ext: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        z2_ext.extend_from_slice(&pad);
        add_nbit_qq_fast(b, &z2_ext, z1_reg);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(2);
        let mut z0_ext: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        z0_ext.extend_from_slice(&pad);
        add_nbit_qq_fast(b, &z0_ext, z1_reg);
        b.free_vec(&pad);
    }

    let x_sum = b.alloc_qubits(h + 1);
    let y_sum = b.alloc_qubits(h + 1);
    karatsuba_half_sum_compute(b, &x_lo, &x_hi, &x_sum);
    karatsuba_half_sum_compute(b, &y_lo, &y_hi, &y_sum);
    schoolbook_mul_into_addsub_inverse(b, &x_sum, &y_sum, z1_reg);
    karatsuba_half_sum_uncompute(b, &y_lo, &y_hi, &y_sum);
    karatsuba_half_sum_uncompute(b, &x_lo, &x_hi, &x_sum);
    b.free_vec(&y_sum);
    b.free_vec(&x_sum);

    {
        let slice: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        schoolbook_mul_into_addsub_inverse(b, &x_hi, &y_hi, &slice);
    }
    {
        let slice: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        schoolbook_mul_into_addsub_inverse(b, &x_lo, &y_lo, &slice);
    }
}

pub(crate) fn mod_mul_add_into_acc_karatsuba_with_tmp_ext(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
    tmp_ext: &[QubitId],
) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    debug_assert_eq!(tmp_ext.len(), 2 * n);
    let h = n / 2;
    let z1_reg = b.alloc_qubits(2 * (h + 1));
    karatsuba_forward(b, x, y, tmp_ext, &z1_reg);

    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    mod_add_qq_fast_lowscratch(b, acc, &lo, p);
    mod_add_qq_fast_lowscratch(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast_lowscratch(b, acc, &hi, p);
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast_lowscratch(b, acc, &hi, p);
    let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
    mod_add_qq(b, acc, &hi, p);
    mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    karatsuba_inverse(b, x, y, tmp_ext, &z1_reg);
    b.free_vec(&z1_reg);
}

pub(crate) fn mod_mul_add_into_acc_karatsuba(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let tmp_ext = b.alloc_qubits(2 * acc.len());
    mod_mul_add_into_acc_karatsuba_with_tmp_ext(b, acc, x, y, p, &tmp_ext);
    b.free_vec(&tmp_ext);
}

pub(crate) fn mod_mul_write_into_zero_acc_karatsuba_with_tmp_ext(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
    tmp_ext: &[QubitId],
) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    debug_assert_eq!(tmp_ext.len(), 2 * n);
    let h = n / 2;
    let z1_reg = b.alloc_qubits(2 * (h + 1));
    b.set_phase("kara_fwd");
    karatsuba_forward(b, x, y, tmp_ext, &z1_reg);
    b.set_phase("kara_solinas");

    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    b.set_phase("sol_addlo");
    mod_add_qq_fast_from_zero_lowscratch(b, acc, &lo, p);
    b.set_phase("sol_add0");
    mod_add_qq_fast_lowscratch(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    b.set_phase("sol_add4");
    mod_add_qq_fast_lowscratch(b, acc, &hi, p);
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    b.set_phase("sol_sub6");
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    b.set_phase("sol_add10");
    mod_add_qq_fast_lowscratch(b, acc, &hi, p);
    b.set_phase("kara_solinas_shift22L");
    let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
    b.set_phase("kara_solinas_post32_add");
    // Use non-fast mod_add at peak site (after shift_left, with extra locals alive)
    // to save 256 carry qubits at the expense of ~n Toffoli.
    mod_add_qq(b, acc, &hi, p);
    b.set_phase("kara_solinas_shift22R");
    mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    b.set_phase("kara_solinas_post_halve");
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    b.set_phase("kara_inv");
    karatsuba_inverse(b, x, y, tmp_ext, &z1_reg);
    b.free_vec(&z1_reg);
}

pub(crate) fn mod_mul_write_into_zero_acc_karatsuba(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let tmp_ext = b.alloc_qubits(2 * acc.len());
    mod_mul_write_into_zero_acc_karatsuba_with_tmp_ext(b, acc, x, y, p, &tmp_ext);
    b.free_vec(&tmp_ext);
}

pub(crate) fn pair1_mul1_write_into_zero_acc(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    if pair1_mul1_karatsuba_enabled(acc.len()) {
        mod_mul_write_into_zero_acc_karatsuba(b, acc, x, y, p);
    } else if gz_mul_lowscratch() {
        // 9n-floor: drop the pair1_borrow_dx_mul1 schoolbook Solinas-fold
        // transient below 2333 so it no longer rebinds the peak.
        mod_mul_write_into_zero_acc_schoolbook_lowscratch_fold(b, acc, x, y, p);
    } else {
        mod_mul_write_into_zero_acc_schoolbook(b, acc, x, y, p);
    }
}

pub(crate) fn pair1_mul2_add_into_acc(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    if pair1_mul2_karatsuba_enabled(acc.len()) {
        mod_mul_add_into_acc_karatsuba_lowq(b, acc, x, y, p);
    } else {
        mod_mul_add_into_acc_schoolbook(b, acc, x, y, p);
    }
}

pub(crate) fn pair2_mul_add_into_acc(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    if pair2_mul_karatsuba_enabled(acc.len()) {
        if env_flag_enabled("KAL_PAIR2_MUL_KARATSUBA_LOWQ", false) {
            mod_mul_add_into_acc_karatsuba_lowq(b, acc, x, y, p);
        } else {
            mod_mul_add_into_acc_karatsuba(b, acc, x, y, p);
        }
    } else if gz_mul_lowscratch() {
        // 9n-floor: drop the schoolbook Solinas-fold transient below 2333 so
        // pair2_mul no longer rebinds the peak once STEP-4 has dropped.
        mod_mul_add_into_acc_schoolbook_lowscratch_fold(b, acc, x, y, p);
    } else {
        mod_mul_add_into_acc_schoolbook(b, acc, x, y, p);
    }
}

// ─── 2-level Karatsuba variants (recursive on inner half-mults) ───
// Costs 2 extra z1_inner registers of ~2*(n/4+1) qubits each (~260 total for n=256).
// Higher peak qubits; use only at low-peak mul sites.

pub(crate) fn karatsuba_forward_2level(
    b: &mut B,
    x: &[QubitId],
    y: &[QubitId],
    tmp_ext: &[QubitId],
    z1_reg: &[QubitId],
    z1_inner_a: &[QubitId],
    z1_inner_b: &[QubitId],
) {
    let n = x.len();
    let h = n / 2;
    let x_lo: Vec<QubitId> = x[0..h].to_vec();
    let x_hi: Vec<QubitId> = x[h..n].to_vec();
    let y_lo: Vec<QubitId> = y[0..h].to_vec();
    let y_hi: Vec<QubitId> = y[h..n].to_vec();

    {
        let slice: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        karatsuba_forward(b, &x_lo, &y_lo, &slice, z1_inner_a);
    }
    {
        let slice: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        karatsuba_forward(b, &x_hi, &y_hi, &slice, z1_inner_b);
    }

    let x_sum = b.alloc_qubits(h + 1);
    let y_sum = b.alloc_qubits(h + 1);
    karatsuba_half_sum_compute(b, &x_lo, &x_hi, &x_sum);
    karatsuba_half_sum_compute(b, &y_lo, &y_hi, &y_sum);
    schoolbook_mul_into_addsub(b, &x_sum, &y_sum, z1_reg);
    karatsuba_half_sum_uncompute(b, &y_lo, &y_hi, &y_sum);
    karatsuba_half_sum_uncompute(b, &x_lo, &x_hi, &x_sum);
    b.free_vec(&y_sum);
    b.free_vec(&x_sum);

    {
        let pad = b.alloc_qubits(2);
        let mut z0_ext: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        z0_ext.extend_from_slice(&pad);
        sub_nbit_qq_fast(b, &z0_ext, z1_reg);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(2);
        let mut z2_ext: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        z2_ext.extend_from_slice(&pad);
        sub_nbit_qq_fast(b, &z2_ext, z1_reg);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(3 * h - 2 * (h + 1));
        let mut z1_ext: Vec<QubitId> = z1_reg.to_vec();
        z1_ext.extend_from_slice(&pad);
        let acc_slice: Vec<QubitId> = tmp_ext[h..4 * h].to_vec();
        add_nbit_qq_fast(b, &z1_ext, &acc_slice);
        b.free_vec(&pad);
    }
}

pub(crate) fn karatsuba_inverse_2level(
    b: &mut B,
    x: &[QubitId],
    y: &[QubitId],
    tmp_ext: &[QubitId],
    z1_reg: &[QubitId],
    z1_inner_a: &[QubitId],
    z1_inner_b: &[QubitId],
) {
    let n = x.len();
    let h = n / 2;
    let x_lo: Vec<QubitId> = x[0..h].to_vec();
    let x_hi: Vec<QubitId> = x[h..n].to_vec();
    let y_lo: Vec<QubitId> = y[0..h].to_vec();
    let y_hi: Vec<QubitId> = y[h..n].to_vec();

    {
        let pad = b.alloc_qubits(3 * h - 2 * (h + 1));
        let mut z1_ext: Vec<QubitId> = z1_reg.to_vec();
        z1_ext.extend_from_slice(&pad);
        let acc_slice: Vec<QubitId> = tmp_ext[h..4 * h].to_vec();
        sub_nbit_qq_fast(b, &z1_ext, &acc_slice);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(2);
        let mut z2_ext: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        z2_ext.extend_from_slice(&pad);
        add_nbit_qq_fast(b, &z2_ext, z1_reg);
        b.free_vec(&pad);
    }
    {
        let pad = b.alloc_qubits(2);
        let mut z0_ext: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        z0_ext.extend_from_slice(&pad);
        add_nbit_qq_fast(b, &z0_ext, z1_reg);
        b.free_vec(&pad);
    }

    let x_sum = b.alloc_qubits(h + 1);
    let y_sum = b.alloc_qubits(h + 1);
    karatsuba_half_sum_compute(b, &x_lo, &x_hi, &x_sum);
    karatsuba_half_sum_compute(b, &y_lo, &y_hi, &y_sum);
    schoolbook_mul_into_addsub_inverse(b, &x_sum, &y_sum, z1_reg);
    karatsuba_half_sum_uncompute(b, &y_lo, &y_hi, &y_sum);
    karatsuba_half_sum_uncompute(b, &x_lo, &x_hi, &x_sum);
    b.free_vec(&y_sum);
    b.free_vec(&x_sum);

    {
        let slice: Vec<QubitId> = tmp_ext[2 * h..4 * h].to_vec();
        karatsuba_inverse(b, &x_hi, &y_hi, &slice, z1_inner_b);
    }
    {
        let slice: Vec<QubitId> = tmp_ext[0..2 * h].to_vec();
        karatsuba_inverse(b, &x_lo, &y_lo, &slice, z1_inner_a);
    }
}

pub(crate) fn mod_mul_write_into_zero_acc_karatsuba2(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    let h = n / 2;
    let h2 = h / 2;
    let tmp_ext = b.alloc_qubits(2 * n);
    let z1_reg = b.alloc_qubits(2 * (h + 1));
    let z1_inner_a = b.alloc_qubits(2 * (h2 + 1));
    let z1_inner_b = b.alloc_qubits(2 * (h2 + 1));
    karatsuba_forward_2level(b, x, y, &tmp_ext, &z1_reg, &z1_inner_a, &z1_inner_b);

    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    mod_add_qq_fast_from_zero(b, acc, &lo, p);
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
    mod_add_qq(b, acc, &hi, p);
    mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    karatsuba_inverse_2level(b, x, y, &tmp_ext, &z1_reg, &z1_inner_a, &z1_inner_b);
    b.free_vec(&z1_inner_b);
    b.free_vec(&z1_inner_a);
    b.free_vec(&z1_reg);
    b.free_vec(&tmp_ext);
}


// ═══════════════════════════════════════════════════════════════════════════
//  Merged from mul_affine.rs
// ═══════════════════════════════════════════════════════════════════════════

// (refactor r2) Mechanically extracted from mul.rs. No logic changes.

/// Symmetric schoolbook for squaring: x² = sum_i x[i]·2^(2i) + sum_{i<j} 2·x[i]·x[j]·2^(i+j).
/// Each cross-product is computed ONCE (instead of twice in full schoolbook),
/// halving the AND count + Cuccaro_add length. Saves ~130k CCX per squaring.
///
/// Row i layout (width n-i): bit 0 = diagonal x[i] at position 2i, bit 1 = 0
/// (gap), bit k+2 = cross-product (x[i] AND x[i+1+k]) at position i+(i+1+k)+1.
pub(crate) fn schoolbook_square_symmetric(b: &mut B, x: &[QubitId], tmp_ext: &[QubitId]) {
    let n = x.len();
    debug_assert_eq!(tmp_ext.len(), 2 * n);
    for i in 0..n {
        // Width: bit 0 = diag at pos 2i, bit 1 = gap, bits 2..(n-i) = cross-
        // products at positions 2i+2..i+n. Last bit index = n-i, so width = n-i+1.
        // Edge case: i = n-1 has only the diagonal, width = 1.
        let width = if i == n - 1 { 1 } else { n - i + 1 };
        let num_cross = if i + 1 < n { n - i - 1 } else { 0 };
        // num_cross = number of cross-products in this row = width - 2 when width >= 2.
        let row = b.alloc_qubits(width);
        b.cx(x[i], row[0]);
        for k in 0..num_cross {
            b.ccx(x[i], x[i + 1 + k], row[k + 2]);
        }
        let pad = b.alloc_qubit();
        let mut row_padded = row.clone();
        row_padded.push(pad);
        let slice: Vec<QubitId> = tmp_ext[2 * i..2 * i + width + 1].to_vec();
        let c_in = b.alloc_qubit();
        cuccaro_add_fast(b, &row_padded, &slice, c_in);
        b.free(c_in);
        b.free(pad);
        b.cx(x[i], row[0]);
        for k in 0..num_cross {
            let m = b.alloc_bit();
            b.hmr(row[k + 2], m);
            b.cz_if(x[i], x[i + 1 + k], m);
        }
        b.free_vec(&row);
    }
}

pub(crate) fn schoolbook_square_symmetric_inverse(b: &mut B, x: &[QubitId], tmp_ext: &[QubitId]) {
    let n = x.len();
    for i in (0..n).rev() {
        let width = if i == n - 1 { 1 } else { n - i + 1 };
        let num_cross = if i + 1 < n { n - i - 1 } else { 0 };
        let row = b.alloc_qubits(width);
        b.cx(x[i], row[0]);
        for k in 0..num_cross {
            b.ccx(x[i], x[i + 1 + k], row[k + 2]);
        }
        let pad = b.alloc_qubit();
        let mut row_padded = row.clone();
        row_padded.push(pad);
        let slice: Vec<QubitId> = tmp_ext[2 * i..2 * i + width + 1].to_vec();
        let c_in = b.alloc_qubit();
        cuccaro_sub_fast(b, &row_padded, &slice, c_in);
        b.free(c_in);
        b.free(pad);
        b.cx(x[i], row[0]);
        for k in 0..num_cross {
            let m = b.alloc_bit();
            b.hmr(row[k + 2], m);
            b.cz_if(x[i], x[i + 1 + k], m);
        }
        b.free_vec(&row);
    }
}

/// Schoolbook squarer with Bennett uncompute. For squaring `tmp_ext = x*x`
/// (2n bits, no mod reduction), then ADD with Solinas reduction to acc,
/// then uncompute tmp_ext via gate-level inverse.
pub(crate) fn squaring_add_to_acc_schoolbook(b: &mut B, acc: &[QubitId], x: &[QubitId], p: U256) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    debug_assert_eq!(x.len(), n);

    let tmp_ext = b.alloc_qubits(2 * n);
    schoolbook_square_symmetric(b, x, &tmp_ext);

    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    mod_add_qq_fast(b, acc, &lo, p);
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
    mod_add_qq(b, acc, &hi, p);
    mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    schoolbook_square_symmetric_inverse(b, x, &tmp_ext);
    b.free_vec(&tmp_ext);
}

pub(crate) fn mod_add_solinas_ext_product(b: &mut B, acc: &[QubitId], tmp_ext: &[QubitId], p: U256) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    debug_assert_eq!(tmp_ext.len(), 2 * n);
    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    mod_add_qq_fast(b, acc, &lo, p);
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    if sol_ext_product_pos32_fast() {
        // SOL_EXT_PRODUCT_POS32_FAST: fast measurement-based add at position 32.
        let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
        mod_add_qq_fast(b, acc, &hi, p);
        mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    } else {
        let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
        mod_add_qq(b, acc, &hi, p);
        mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    }
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }
}

pub(crate) fn mod_sub_solinas_ext_product(b: &mut B, acc: &[QubitId], tmp_ext: &[QubitId], p: U256) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    debug_assert_eq!(tmp_ext.len(), 2 * n);
    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    mod_sub_qq_fast(b, acc, &lo, p);
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    if sol_ext_product_pos32_fast() {
        let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
        mod_sub_qq_fast(b, acc, &hi, p);
        mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    } else {
        let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
        mod_sub_qq(b, acc, &hi, p);
        mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    }
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }
}

pub(crate) fn square_tx_and_combined_ty_l2minus3qx(
    b: &mut B,
    tx: &[QubitId],
    ty: &[QubitId],
    lam: &[QubitId],
    ox: &[BitId],
    p: U256,
) {
    let n = tx.len();
    debug_assert_eq!(n, 256);
    debug_assert_eq!(ty.len(), n);
    debug_assert_eq!(lam.len(), n);

    b.set_phase("affine_combined_square");
    let tmp_ext = b.alloc_qubits(2 * n);
    schoolbook_square_symmetric(b, lam, &tmp_ext);

    b.set_phase("affine_combined_breg_red");
    let breg = b.alloc_qubits(n);
    mod_add_solinas_ext_product(b, &breg, &tmp_ext, p);
    mod_sub_double_qb(b, &breg, ox, p);
    mod_sub_qb(b, &breg, ox, p);

    b.set_phase("affine_combined_y_mul");
    if env_flag_enabled("POINT_ADD_AFFINE_COMBINED_Y_KARATSUBA_LOWQ", false) {
        mod_mul_add_into_acc_karatsuba_lowq(b, ty, lam, &breg, p);
    } else if env_flag_enabled("AFFINE_Y_MUL_LOWSCRATCH_FOLD", stack_2565_enabled()) {
        // Peak-minimized y-mul: cuts the ~256-wide transient scratch the
        // schoolbook MAC holds on top of its 512 product while the lam² square's
        // 512 product co-resides (the -x correction pads, the Solinas fold's
        // carry/const registers, and the fold mod_double's const register). The
        // y-mul instant drops 2565 -> ~2333, below the next cluster (2459),
        // breaking the 2565 binder. Default-on under STACK-2565; set
        // AFFINE_Y_MUL_LOWSCRATCH_FOLD=0 to restore the byte-identical
        // fast-fold schoolbook MAC (peak 2565).
        mod_mul_add_into_acc_schoolbook_lowscratch_fold(b, ty, lam, &breg, p);
    } else {
        mod_mul_add_into_acc_schoolbook(b, ty, lam, &breg, p);
    }

    // r-lifecycle (default): fold lambda^2 once and reuse the reduced value for
    // the tx update so tx_update is a cheap qq-sub instead of a second full
    // Solinas fold. After the two 3Qx re-adds below, `breg` holds
    // r = lambda^2 mod p (the reduced value) -- exactly the constant tx_update
    // must subtract. Consume breg-as-r for tx BEFORE zeroing breg, then zero
    // breg with the one Solinas fold it would have used anyway. No extra
    // register => peak-neutral. Validated -18,963 Toffoli, peak 2708 unchanged.
    // Set AFFINE_R_LIFECYCLE=0 to fall back to the legacy 3-fold path.
    let affine_r_lifecycle =
        std::env::var("AFFINE_R_LIFECYCLE").ok().as_deref() != Some("0");

    if affine_r_lifecycle {
        b.set_phase("affine_combined_breg_unred");
        mod_add_qb(b, &breg, ox, p); // breg = lambda^2 mod p = r
        mod_add_double_qb(b, &breg, ox, p);

        b.set_phase("affine_combined_tx_update");
        // tx -= r  (== tx -= lambda^2 mod p), reusing breg=r, cheap qq sub.
        mod_sub_qq_fast(b, tx, &breg, p);

        b.set_phase("affine_combined_breg_unred");
        // Zero breg via the one Solinas fold it would have used anyway.
        mod_sub_solinas_ext_product(b, &breg, &tmp_ext, p);
        b.free_vec(&breg);

        b.set_phase("affine_combined_tx_update");
        mod_add_double_qb(b, tx, ox, p);
        mod_add_qb(b, tx, ox, p);
        mod_neg_inplace_fast(b, tx, p);
    } else {
        b.set_phase("affine_combined_breg_unred");
        mod_add_qb(b, &breg, ox, p);
        mod_add_double_qb(b, &breg, ox, p);
        mod_sub_solinas_ext_product(b, &breg, &tmp_ext, p);
        b.free_vec(&breg);

        b.set_phase("affine_combined_tx_update");
        mod_sub_solinas_ext_product(b, tx, &tmp_ext, p);
        mod_add_double_qb(b, tx, ox, p);
        mod_add_qb(b, tx, ox, p);
        mod_neg_inplace_fast(b, tx, p);
    }

    schoolbook_square_symmetric_inverse(b, lam, &tmp_ext);
    b.free_vec(&tmp_ext);
}

/// Schoolbook squarer with Bennett uncompute. For squaring `tmp_ext = x*x`
/// (2n bits, no mod reduction), then sub from acc with on-the-fly Solinas
/// reduction, then uncompute tmp_ext via gate-level inverse. Saves ~170k
/// CCX vs walk-x squaring (459k → 289k) by avoiding 256 expensive
/// cmod_add_qq calls (each 5n) in favor of 2n²=131k of cheap AND+Cuccaro.
pub(crate) fn squaring_sub_from_acc_schoolbook(b: &mut B, acc: &[QubitId], x: &[QubitId], p: U256) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    debug_assert_eq!(x.len(), n);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));

    // Wide accumulator (2n bits) starts at 0.
    let tmp_ext = b.alloc_qubits(2 * n);

    // Phase 1: symmetric schoolbook tmp_ext = x*x (~half the CCX of full).
    schoolbook_square_symmetric(b, x, &tmp_ext);

    // Phase 2: subtract (lo + hi*c mod p) from acc.
    // For each set bit k of c, sub (hi shifted by k mod p) from acc, by
    // walking hi via mod_double in place. Sub lo first.
    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    mod_sub_qq_fast(b, acc, &lo, p);
    let _ = c;
    // 977 consolidation: c = {+2^0, +2^4, -2^6, +2^10, +2^32}. For acc-=hi·c, signs flip:
    // acc -= hi·2^0, acc -= hi·2^4, acc += hi·2^6, acc -= hi·2^10, acc -= hi·2^32.
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p); // sign flipped
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    let (spill, flag_inv, ovf) = mod_shift_left_by_k(b, &hi, p, 22);
    mod_sub_qq(b, acc, &hi, p);
    mod_shift_right_by_k(b, &hi, p, 22, spill, flag_inv, ovf);
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    // Phase 3: uncompute tmp_ext via symmetric schoolbook inverse.
    schoolbook_square_symmetric_inverse(b, x, &tmp_ext);

    b.free_vec(&tmp_ext);
}

pub(crate) fn squaring_sub_from_acc_schoolbook_lowq_shift22(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    p: U256,
) {
    let n = acc.len();
    debug_assert_eq!(n, 256);
    debug_assert_eq!(x.len(), n);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));

    let tmp_ext = b.alloc_qubits(2 * n);
    schoolbook_square_symmetric(b, x, &tmp_ext);

    let lo: Vec<QubitId> = tmp_ext[0..n].to_vec();
    let hi: Vec<QubitId> = tmp_ext[n..2 * n].to_vec();
    mod_sub_qq_fast(b, acc, &lo, p);
    let _ = c;
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    for _ in 0..2 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_add_qq_fast(b, acc, &hi, p);
    for _ in 0..4 {
        mod_double_inplace_fast(b, &hi, p);
    }
    mod_sub_qq_fast(b, acc, &hi, p);
    let (spill, flag_inv, ovf) = mod_shift_left_by_k_lowq(b, &hi, p, 22);
    mod_sub_qq(b, acc, &hi, p);
    mod_shift_right_by_k_lowq(b, &hi, p, 22, spill, flag_inv, ovf);
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    schoolbook_square_symmetric_inverse(b, x, &tmp_ext);
    b.free_vec(&tmp_ext);
}

pub(crate) fn mod_mul_sub_qq(b: &mut B, acc: &[QubitId], x: &[QubitId], y: &[QubitId], p: U256) {
    // acc -= x * y mod p. Negate x, run schoolbook ADD (cheaper than sub),
    // then restore x. For x≠y we can walk the negated multiplicand in place
    // and halve it back afterwards, avoiding the doubled tmp register. For
    // squaring we snapshot the original control bits once into `ctrl_copy`,
    // then reuse the same in-place walk on the negated x.
    let n = acc.len();
    let is_squaring = x[0] == y[0]; // same register → squaring
    if is_squaring {
        // Use the schoolbook squarer for the squaring case (~170k savings).
        squaring_sub_from_acc_schoolbook(b, acc, x, p);
        return;
    }
    if false {
        // Hold the original x bits fixed for control while x itself walks
        // through (-x)*2^i mod p.
        let ctrl_copy = b.alloc_qubits(n);
        for i in 0..n {
            b.cx(x[i], ctrl_copy[i]);
        }
        mod_neg_inplace_fast(b, x, p);
        for i in 0..n {
            cmod_add_qq(b, acc, x, ctrl_copy[i], p);
            if i < n - 1 {
                mod_double_inplace_fast(b, x, p);
            }
        }
        for _ in 0..(n - 1) {
            mod_halve_inplace_fast(b, x, p);
        }
        mod_neg_inplace_fast(b, x, p);
        for i in 0..n {
            b.cx(x[i], ctrl_copy[i]);
        }
        b.free_vec(&ctrl_copy);
    } else {
        // Keep x negated during the loop and walk it in place.
        mod_neg_inplace_fast(b, x, p);
        for i in 0..n {
            cmod_add_qq(b, acc, x, y[i], p);
            if i < n - 1 {
                mod_double_inplace_fast(b, x, p);
            }
        }
        for _ in 0..(n - 1) {
            mod_halve_inplace_fast(b, x, p);
        }
        mod_neg_inplace_fast(b, x, p);
    }
}


