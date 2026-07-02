//! (refactor) Mechanical split of bench.rs: by_* / centered_signed_by_* helpers. No logic changes.
use super::*;

pub(crate) fn by_cmod_neg_inplace_fast(b: &mut B, v: &[QubitId], ctrl: QubitId, p: U256) {
    // ctrl ? (p-v) : v.  Like the BY structural tests, this maps v=0 to the
    // noncanonical representative p when ctrl=1; the benchmark scaffold below
    // keeps controls at zero and uses this only to exercise the actual gate
    // body/cost inside the point-add harness.
    for &q in v {
        b.cx(ctrl, q);
    }
    cadd_nbit_const_fast(b, v, p.wrapping_add(U256::from(1u64)), ctrl);
}

pub(crate) fn by_cmod_neg_inplace_canonical_for_bench(b: &mut B, v: &[QubitId], ctrl: QubitId, p: U256) {
    // ctrl ? (-v mod p) : v, preserving the canonical zero representative.  The
    // fast BY negation maps 0 -> p; that is fine inside replay scaffolds but not
    // when the pair2 product-clean path wants to free the slope register after
    // inverse replay.  Nonzeroness is invariant under v -> p-v, so the flag can
    // be uncomputed after the controlled negation.
    let nz = b.alloc_qubit();
    let do_neg = b.alloc_qubit();
    cmp_neq_zero_into(b, v, nz);
    b.ccx(ctrl, nz, do_neg);
    for &q in v {
        b.cx(do_neg, q);
    }
    cadd_nbit_const_fast(b, v, p.wrapping_add(U256::from(1u64)), do_neg);
    b.ccx(ctrl, nz, do_neg);
    cmp_neq_zero_into(b, v, nz);
    b.free(do_neg);
    b.free(nz);
}

pub(crate) fn by_signed_controlled_add_for_bench(b: &mut B, acc: &[QubitId], a: &[QubitId], ctrl: QubitId) {
    let f = b.alloc_qubits(acc.len());
    for i in 0..acc.len() {
        b.ccx(ctrl, a[i], f[i]);
    }
    add_nbit_qq_fast(b, &f, acc);
    for i in 0..acc.len() {
        let m = b.alloc_bit();
        b.hmr(f[i], m);
        b.cz_if(ctrl, a[i], m);
    }
    b.free_vec(&f);
}

pub(crate) fn by_signed_controlled_sub_for_bench(b: &mut B, acc: &[QubitId], a: &[QubitId], ctrl: QubitId) {
    let f = b.alloc_qubits(acc.len());
    for i in 0..acc.len() {
        b.ccx(ctrl, a[i], f[i]);
    }
    sub_nbit_qq_fast(b, &f, acc);
    for i in 0..acc.len() {
        let m = b.alloc_bit();
        b.hmr(f[i], m);
        b.cz_if(ctrl, a[i], m);
    }
    b.free_vec(&f);
}

pub(crate) fn by_twos_cneg_for_bench(b: &mut B, v: &[QubitId], ctrl: QubitId) {
    if std::env::var("BY_CENTERED_REPLAY_DIRECTFAST_CNEG")
        .ok()
        .as_deref()
        == Some("1")
    {
        for &q in v {
            b.cx(ctrl, q);
        }
        cadd_nbit_const_direct_fast(b, v, U256::from(1u64), ctrl);
        return;
    }
    for &q in v {
        b.cx(ctrl, q);
    }
    cadd_nbit_const_fast(b, v, U256::from(1u64), ctrl);
}

pub(crate) fn by_arithmetic_shift_right_even_for_bench(b: &mut B, v: &[QubitId]) {
    for i in 0..v.len() - 1 {
        b.swap(v[i], v[i + 1]);
    }
    b.cx(v[v.len() - 2], v[v.len() - 1]);
}

pub(crate) fn by_centered_halve_live_parity_for_bench(b: &mut B, v: &[QubitId], parity: QubitId, p: U256) {
    let directfast = std::env::var("BY_CENTERED_REPLAY_DIRECTFAST_HALVE")
        .ok()
        .as_deref()
        == Some("1");
    let sign_hist = b.alloc_qubit();
    let add_ctrl = b.alloc_qubit();
    let sub_ctrl = b.alloc_qubit();
    b.cx(v[0], parity);
    b.cx(v[v.len() - 1], sign_hist);
    b.ccx(parity, sign_hist, add_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, sub_ctrl);
    b.x(sign_hist);
    if directfast {
        cadd_nbit_const_direct_fast(b, v, p, add_ctrl);
        csub_nbit_const_direct_fast(b, v, p, sub_ctrl);
    } else {
        cadd_nbit_const_fast(b, v, p, add_ctrl);
        csub_nbit_const_fast(b, v, p, sub_ctrl);
    }
    b.x(sign_hist);
    b.ccx(parity, sign_hist, sub_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, add_ctrl);
    b.free(sub_ctrl);
    b.free(add_ctrl);
    by_arithmetic_shift_right_even_for_bench(b, v);
    b.cx(v[v.len() - 1], sign_hist);
    b.cx(parity, sign_hist);
    b.free(sign_hist);
}

pub(crate) fn centered_signed_by_microstep_for_bench(
    b: &mut B,
    r: &[QubitId],
    s: &[QubitId],
    odd: QubitId,
    a: QubitId,
    parity: QubitId,
    p: U256,
) {
    let exact_cneg = std::env::var("BY_CENTERED_REPLAY_EXACT_CNEG")
        .ok()
        .as_deref()
        == Some("1");
    let exact_add = std::env::var("BY_CENTERED_REPLAY_EXACT_ADD")
        .ok()
        .as_deref()
        == Some("1");
    let exact_halve = std::env::var("BY_CENTERED_REPLAY_EXACT_HALVE")
        .ok()
        .as_deref()
        == Some("1");
    for i in 0..r.len() {
        cswap(b, a, r[i], s[i]);
    }
    if exact_cneg {
        by_twos_cneg_exact_for_bench(b, s, a);
    } else {
        by_twos_cneg_for_bench(b, s, a);
    }
    if exact_add {
        by_signed_controlled_add_exact_for_bench(b, s, r, odd);
    } else {
        by_signed_controlled_add_for_bench(b, s, r, odd);
    }
    if exact_halve {
        by_centered_halve_live_parity_exact_for_bench(b, s, parity, p);
    } else {
        by_centered_halve_live_parity_for_bench(b, s, parity, p);
    }
}

pub(crate) fn by_signed_controlled_add_exact_for_bench(
    b: &mut B,
    acc: &[QubitId],
    a: &[QubitId],
    ctrl: QubitId,
) {
    let f = b.alloc_qubits(acc.len());
    for i in 0..acc.len() {
        b.ccx(ctrl, a[i], f[i]);
    }
    add_nbit_qq(b, &f, acc);
    for i in 0..acc.len() {
        b.ccx(ctrl, a[i], f[i]);
    }
    b.free_vec(&f);
}

pub(crate) fn by_signed_controlled_sub_exact_for_bench(
    b: &mut B,
    acc: &[QubitId],
    a: &[QubitId],
    ctrl: QubitId,
) {
    let f = b.alloc_qubits(acc.len());
    for i in 0..acc.len() {
        b.ccx(ctrl, a[i], f[i]);
    }
    sub_nbit_qq(b, &f, acc);
    for i in 0..acc.len() {
        b.ccx(ctrl, a[i], f[i]);
    }
    b.free_vec(&f);
}

pub(crate) fn by_twos_cneg_exact_for_bench(b: &mut B, v: &[QubitId], ctrl: QubitId) {
    for &q in v {
        b.cx(ctrl, q);
    }
    cadd_nbit_const(b, v, U256::from(1u64), ctrl);
}

pub(crate) fn by_arithmetic_shift_left_even_inverse_for_bench(b: &mut B, v: &[QubitId]) {
    b.cx(v[v.len() - 2], v[v.len() - 1]);
    for i in (0..v.len() - 1).rev() {
        b.swap(v[i], v[i + 1]);
    }
}

pub(crate) fn by_centered_halve_live_parity_exact_for_bench(
    b: &mut B,
    v: &[QubitId],
    parity: QubitId,
    p: U256,
) {
    let sign_hist = b.alloc_qubit();
    let add_ctrl = b.alloc_qubit();
    let sub_ctrl = b.alloc_qubit();
    b.cx(v[0], parity);
    b.cx(v[v.len() - 1], sign_hist);
    b.ccx(parity, sign_hist, add_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, sub_ctrl);
    b.x(sign_hist);
    cadd_nbit_const(b, v, p, add_ctrl);
    csub_nbit_const(b, v, p, sub_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, sub_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, add_ctrl);
    b.free(sub_ctrl);
    b.free(add_ctrl);
    by_arithmetic_shift_right_even_for_bench(b, v);
    b.cx(v[v.len() - 1], sign_hist);
    b.cx(parity, sign_hist);
    b.free(sign_hist);
}

pub(crate) fn by_centered_unhalve_with_parity_for_bench(b: &mut B, v: &[QubitId], parity: QubitId, p: U256) {
    by_arithmetic_shift_left_even_inverse_for_bench(b, v);
    let sign_hist = b.alloc_qubit();
    let add_ctrl = b.alloc_qubit();
    let sub_ctrl = b.alloc_qubit();
    let sign = v[v.len() - 1];
    b.cx(sign, sign_hist);
    b.ccx(parity, sign_hist, add_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, sub_ctrl);
    b.x(sign_hist);
    cadd_nbit_const_fast(b, v, p, add_ctrl);
    csub_nbit_const_fast(b, v, p, sub_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, sub_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, add_ctrl);
    b.free(sub_ctrl);
    b.free(add_ctrl);
    b.cx(sign, sign_hist);
    b.cx(parity, sign_hist);
    b.free(sign_hist);
}

pub(crate) fn by_centered_unhalve_with_parity_exact_for_bench(
    b: &mut B,
    v: &[QubitId],
    parity: QubitId,
    p: U256,
) {
    by_arithmetic_shift_left_even_inverse_for_bench(b, v);
    let sign_hist = b.alloc_qubit();
    let add_ctrl = b.alloc_qubit();
    let sub_ctrl = b.alloc_qubit();
    let sign = v[v.len() - 1];
    // The correction direction is determined by the sign of the doubled value
    // before undoing the ±p correction.  Keep that sign live; the correction
    // flips it when parity=1, so recomputing controls from the post-correction
    // sign leaves dirty controls and R-phase garbage.
    b.cx(sign, sign_hist);
    b.ccx(parity, sign_hist, add_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, sub_ctrl);
    b.x(sign_hist);
    cadd_nbit_const(b, v, p, add_ctrl);
    csub_nbit_const(b, v, p, sub_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, sub_ctrl);
    b.x(sign_hist);
    b.ccx(parity, sign_hist, add_ctrl);
    b.free(sub_ctrl);
    b.free(add_ctrl);
    b.cx(sign, sign_hist);
    b.cx(parity, sign_hist);
    b.free(sign_hist);
}

pub(crate) fn centered_signed_by_microstep_inverse_for_bench(
    b: &mut B,
    r: &[QubitId],
    s: &[QubitId],
    odd: QubitId,
    a: QubitId,
    parity: QubitId,
    p: U256,
) {
    by_centered_unhalve_with_parity_for_bench(b, s, parity, p);
    by_signed_controlled_sub_for_bench(b, s, r, odd);
    by_twos_cneg_for_bench(b, s, a);
    for i in 0..r.len() {
        cswap(b, a, r[i], s[i]);
    }
}

pub(crate) fn centered_signed_by_microstep_all_exact_for_bench(
    b: &mut B,
    r: &[QubitId],
    s: &[QubitId],
    odd: QubitId,
    a: QubitId,
    parity: QubitId,
    p: U256,
) {
    for i in 0..r.len() {
        cswap(b, a, r[i], s[i]);
    }
    by_twos_cneg_exact_for_bench(b, s, a);
    by_signed_controlled_add_exact_for_bench(b, s, r, odd);
    by_centered_halve_live_parity_exact_for_bench(b, s, parity, p);
}

pub(crate) fn centered_signed_by_microstep_inverse_all_exact_for_bench(
    b: &mut B,
    r: &[QubitId],
    s: &[QubitId],
    odd: QubitId,
    a: QubitId,
    parity: QubitId,
    p: U256,
) {
    by_centered_unhalve_with_parity_exact_for_bench(b, s, parity, p);
    by_signed_controlled_sub_exact_for_bench(b, s, r, odd);
    by_twos_cneg_exact_for_bench(b, s, a);
    for i in 0..r.len() {
        cswap(b, a, r[i], s[i]);
    }
}

pub(crate) fn centered_signed_by_clear_parity_after_inverse_for_bench(
    b: &mut B,
    r: &[QubitId],
    s: &[QubitId],
    odd: QubitId,
    parity: QubitId,
) {
    b.cx(s[0], parity);
    b.ccx(odd, r[0], parity);
}

pub(crate) fn by_logical_shift_right_even_for_bench(b: &mut B, v: &[QubitId]) {
    for i in 0..v.len() - 1 {
        b.swap(v[i], v[i + 1]);
    }
}

pub(crate) fn by_logical_shift_left_even_inverse_for_bench(b: &mut B, v: &[QubitId]) {
    for i in (0..v.len() - 1).rev() {
        b.swap(v[i], v[i + 1]);
    }
}

pub(crate) fn by_delta_positive_into_for_bench(b: &mut B, delta: &[QubitId], flag: QubitId) {
    let nz = b.alloc_qubit();
    cmp_neq_zero_into(b, delta, nz);
    let sign = delta[delta.len() - 1];
    b.x(sign);
    b.ccx(nz, sign, flag);
    b.x(sign);
    cmp_neq_zero_into(b, delta, nz);
    b.free(nz);
}

pub(crate) fn by_2adic_branch_step_for_bench(
    b: &mut B,
    f: &[QubitId],
    g: &[QubitId],
    delta: &[QubitId],
    odd_out: QubitId,
    a_out: QubitId,
) {
    b.cx(g[0], odd_out);
    let positive = b.alloc_qubit();
    by_delta_positive_into_for_bench(b, delta, positive);
    b.ccx(odd_out, positive, a_out);
    by_delta_positive_into_for_bench(b, delta, positive);
    b.free(positive);

    for i in 0..f.len() {
        cswap(b, a_out, f[i], g[i]);
    }
    by_twos_cneg_for_bench(b, g, a_out);
    cucc_add_ctrl(b, f, g, odd_out);
    by_logical_shift_right_even_for_bench(b, g);

    by_twos_cneg_for_bench(b, delta, a_out);
    add_nbit_const_fast(b, delta, U256::from(1u64));
}

pub(crate) fn by_2adic_branch_step_reverse_for_bench(
    b: &mut B,
    f: &[QubitId],
    g: &[QubitId],
    delta: &[QubitId],
    odd_hist: QubitId,
    a_hist: QubitId,
) {
    sub_nbit_const_fast(b, delta, U256::from(1u64));
    by_twos_cneg_for_bench(b, delta, a_hist);
    by_logical_shift_left_even_inverse_for_bench(b, g);
    cucc_sub_ctrl(b, f, g, odd_hist);
    by_twos_cneg_for_bench(b, g, a_hist);
    for i in 0..f.len() {
        cswap(b, a_hist, f[i], g[i]);
    }

    let positive = b.alloc_qubit();
    by_delta_positive_into_for_bench(b, delta, positive);
    b.ccx(odd_hist, positive, a_hist);
    by_delta_positive_into_for_bench(b, delta, positive);
    b.free(positive);
    b.cx(g[0], odd_hist);
}

pub(crate) fn by_signed_branch_step_for_bench(
    b: &mut B,
    f: &[QubitId],
    g: &[QubitId],
    delta: &[QubitId],
    odd_out: QubitId,
    a_out: QubitId,
) {
    b.cx(g[0], odd_out);
    let positive = b.alloc_qubit();
    by_delta_positive_into_for_bench(b, delta, positive);
    b.ccx(odd_out, positive, a_out);
    by_delta_positive_into_for_bench(b, delta, positive);
    b.free(positive);

    for i in 0..f.len() {
        cswap(b, a_out, f[i], g[i]);
    }
    by_twos_cneg_for_bench(b, g, a_out);
    cucc_add_ctrl(b, f, g, odd_out);
    by_arithmetic_shift_right_even_for_bench(b, g);

    by_twos_cneg_for_bench(b, delta, a_out);
    add_nbit_const_fast(b, delta, U256::from(1u64));
}

pub(crate) fn by_signed_branch_step_reverse_for_bench(
    b: &mut B,
    f: &[QubitId],
    g: &[QubitId],
    delta: &[QubitId],
    odd_hist: QubitId,
    a_hist: QubitId,
) {
    sub_nbit_const_fast(b, delta, U256::from(1u64));
    by_twos_cneg_for_bench(b, delta, a_hist);
    by_arithmetic_shift_left_even_inverse_for_bench(b, g);
    cucc_sub_ctrl(b, f, g, odd_hist);
    by_twos_cneg_for_bench(b, g, a_hist);
    for i in 0..f.len() {
        cswap(b, a_hist, f[i], g[i]);
    }

    let positive = b.alloc_qubit();
    by_delta_positive_into_for_bench(b, delta, positive);
    b.ccx(odd_hist, positive, a_hist);
    by_delta_positive_into_for_bench(b, delta, positive);
    b.free(positive);
    b.cx(g[0], odd_hist);
}

pub(crate) fn by_signed_branch_apply_step_for_bench(
    b: &mut B,
    f: &[QubitId],
    g: &[QubitId],
    delta: &[QubitId],
    odd: QubitId,
    a: QubitId,
) {
    for i in 0..f.len() {
        cswap(b, a, f[i], g[i]);
    }
    by_twos_cneg_for_bench(b, g, a);
    cucc_add_ctrl(b, f, g, odd);
    by_arithmetic_shift_right_even_for_bench(b, g);

    by_twos_cneg_for_bench(b, delta, a);
    add_nbit_const_fast(b, delta, U256::from(1u64));
}

pub(crate) fn by_signed_branch_apply_step_reverse_for_bench(
    b: &mut B,
    f: &[QubitId],
    g: &[QubitId],
    delta: &[QubitId],
    odd: QubitId,
    a: QubitId,
) {
    sub_nbit_const_fast(b, delta, U256::from(1u64));
    by_twos_cneg_for_bench(b, delta, a);
    by_arithmetic_shift_left_even_inverse_for_bench(b, g);
    cucc_sub_ctrl(b, f, g, odd);
    by_twos_cneg_for_bench(b, g, a);
    for i in 0..f.len() {
        cswap(b, a, f[i], g[i]);
    }
}

pub(crate) fn by_copy_lowword_sign_extended_for_bench(
    b: &mut B,
    src: &[QubitId],
    dst: &[QubitId],
    low_bits: usize,
) {
    assert!(dst.len() >= low_bits);
    assert!(src.len() >= low_bits);
    for i in 0..low_bits {
        b.cx(src[i], dst[i]);
    }
    for i in low_bits..dst.len() {
        b.cx(src[low_bits - 1], dst[i]);
    }
}

pub(crate) fn by_signed_lowword_window_xor_controls_for_bench(
    b: &mut B,
    f_full: &[QubitId],
    g_full: &[QubitId],
    delta_full: &[QubitId],
    odd_hist: &[QubitId],
    a_hist: &[QubitId],
    q_hist: Option<(&[QubitId], &[QubitId])>,
    start: usize,
) {
    // Window selector primitive for the centered-BY denominator path.  The next
    // 16 BY branch decisions depend only on the low 16 bits of the current
    // signed denominator pair plus delta.  Compute them in a narrow local
    // 2-adic simulator, xor them into the persistent odd/A histories, and then
    // reverse the simulator.  The full-width denominator state is updated by a
    // separate selected-control application below; this first hook deliberately
    // wires the lowword-window control source into the real pair replacement.
    const W: usize = 16;
    const QBITS: usize = 34;
    let f = b.alloc_qubits(QBITS);
    let g = b.alloc_qubits(QBITS);
    let delta = b.alloc_qubits(delta_full.len());
    let odd_tmp = b.alloc_qubits(W);
    let a_tmp = b.alloc_qubits(W);

    by_copy_lowword_sign_extended_for_bench(b, f_full, &f, W);
    by_copy_lowword_sign_extended_for_bench(b, g_full, &g, W);
    for i in 0..delta_full.len() {
        b.cx(delta_full[i], delta[i]);
    }

    for j in 0..W {
        by_signed_branch_step_for_bench(b, &f, &g, &delta, odd_tmp[j], a_tmp[j]);
    }
    for j in 0..W {
        b.cx(odd_tmp[j], odd_hist[start + j]);
        b.cx(a_tmp[j], a_hist[start + j]);
    }
    if let Some((q0_hist, q1_hist)) = q_hist {
        let windows = odd_hist.len() / W;
        assert_eq!(q0_hist.len(), q1_hist.len());
        assert_eq!(q0_hist.len() % windows, 0);
        let qhist_bits = q0_hist.len() / windows;
        assert!(qhist_bits <= QBITS);
        let q_start = (start / W) * qhist_bits;
        // After the local signed divsteps, these narrow rows are exactly the
        // lowword quotient corrections q=(P·low)/2^16.  Persist only the
        // bounded signed payload bits (18); the local simulator still uses 34
        // bits to make the signed divsteps reversible.  The same helper is
        // called in reverse to xor the payload clean again.
        for i in 0..qhist_bits {
            b.cx(f[i], q0_hist[q_start + i]);
            b.cx(g[i], q1_hist[q_start + i]);
        }
    }
    for j in (0..W).rev() {
        by_signed_branch_step_reverse_for_bench(b, &f, &g, &delta, odd_tmp[j], a_tmp[j]);
    }

    for i in (0..delta_full.len()).rev() {
        b.cx(delta_full[i], delta[i]);
    }
    by_copy_lowword_sign_extended_for_bench(b, g_full, &g, W);
    by_copy_lowword_sign_extended_for_bench(b, f_full, &f, W);
    b.free_vec(&a_tmp);
    b.free_vec(&odd_tmp);
    b.free_vec(&delta);
    b.free_vec(&g);
    b.free_vec(&f);
}

pub(crate) fn by_window_controls_enabled_for_bench() -> bool {
    std::env::var("BY_CENTERED_WINDOW_DENOM_REPLACE")
        .ok()
        .as_deref()
        == Some("1")
        || by_window_q_payload_enabled_for_bench()
}

pub(crate) fn by_window_q_payload_enabled_for_bench() -> bool {
    std::env::var("BY_CENTERED_WINDOW_Q_DENOM_REPLACE")
        .ok()
        .as_deref()
        == Some("1")
}

pub(crate) fn by_generate_signed_controls_for_bench(
    b: &mut B,
    f: &[QubitId],
    g: &[QubitId],
    delta: &[QubitId],
    odd: &[QubitId],
    a_ctrl: &[QubitId],
    q_hist: Option<(&[QubitId], &[QubitId])>,
) {
    if by_window_controls_enabled_for_bench() {
        const W: usize = 16;
        assert_eq!(odd.len() % W, 0);
        for start in (0..odd.len()).step_by(W) {
            by_signed_lowword_window_xor_controls_for_bench(
                b, f, g, delta, odd, a_ctrl, q_hist, start,
            );
            for j in 0..W {
                by_signed_branch_apply_step_for_bench(
                    b,
                    f,
                    g,
                    delta,
                    odd[start + j],
                    a_ctrl[start + j],
                );
            }
        }
    } else {
        for i in 0..odd.len() {
            by_signed_branch_step_for_bench(b, f, g, delta, odd[i], a_ctrl[i]);
        }
    }
}

pub(crate) fn by_reverse_signed_controls_for_bench(
    b: &mut B,
    f: &[QubitId],
    g: &[QubitId],
    delta: &[QubitId],
    odd: &[QubitId],
    a_ctrl: &[QubitId],
    q_hist: Option<(&[QubitId], &[QubitId])>,
) {
    if by_window_controls_enabled_for_bench() {
        const W: usize = 16;
        assert_eq!(odd.len() % W, 0);
        for start in (0..odd.len()).step_by(W).rev() {
            for j in (0..W).rev() {
                by_signed_branch_apply_step_reverse_for_bench(
                    b,
                    f,
                    g,
                    delta,
                    odd[start + j],
                    a_ctrl[start + j],
                );
            }
            by_signed_lowword_window_xor_controls_for_bench(
                b, f, g, delta, odd, a_ctrl, q_hist, start,
            );
        }
    } else {
        for i in (0..odd.len()).rev() {
            by_signed_branch_step_reverse_for_bench(b, f, g, delta, odd[i], a_ctrl[i]);
        }
    }
}

pub(crate) fn init_small_const_reg(b: &mut B, reg: &[QubitId], value: u64) {
    for (i, &q) in reg.iter().enumerate() {
        if ((value >> i) & 1) != 0 {
            b.x(q);
        }
    }
}

pub(crate) fn by_copy_signed_mod_p_for_bench(b: &mut B, signed: &[QubitId], out: &[QubitId], p: U256) {
    assert!(signed.len() > out.len());
    for i in 0..out.len() {
        b.cx(signed[i], out[i]);
    }
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1u64));
    csub_nbit_const(b, out, c, signed[signed.len() - 1]);
}

pub(crate) fn by_uncopy_signed_mod_p_for_bench(b: &mut B, signed: &[QubitId], out: &[QubitId], p: U256) {
    assert!(signed.len() > out.len());
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1u64));
    cadd_nbit_const(b, out, c, signed[signed.len() - 1]);
    for i in 0..out.len() {
        b.cx(signed[i], out[i]);
    }
}

pub(crate) fn by_add_neg_quotient_from_centered_r_for_bench(
    b: &mut B,
    acc: &[QubitId],
    r: &[QubitId],
    f_neg: QubitId,
    p: U256,
) {
    // Tagged recovery is q = sign(f)*r - 1.  Add -q = 1 - sign(f)*r to acc.
    mod_add_qc(b, acc, U256::from(1u64), p);
    let r_mod = b.alloc_qubits(acc.len());
    by_copy_signed_mod_p_for_bench(b, r, &r_mod, p);
    let f_pos = b.alloc_qubit();
    b.x(f_pos);
    b.cx(f_neg, f_pos);
    cmod_sub_qq(b, acc, &r_mod, f_pos, p);
    cmod_add_qq(b, acc, &r_mod, f_neg, p);
    b.cx(f_neg, f_pos);
    b.x(f_pos);
    b.free(f_pos);
    by_uncopy_signed_mod_p_for_bench(b, r, &r_mod, p);
    b.free_vec(&r_mod);
}

pub(crate) fn by_write_neg_quotient_from_centered_r_for_bench(
    b: &mut B,
    lam: &[QubitId],
    r: &[QubitId],
    f_neg: QubitId,
    p: U256,
) {
    by_add_neg_quotient_from_centered_r_for_bench(b, lam, r, f_neg, p);
}

pub(crate) fn by_load_centered_copy_for_bench(
    b: &mut B,
    src: &[QubitId],
    dst: &[QubitId],
    p: U256,
) -> QubitId {
    assert!(dst.len() >= src.len());
    for i in 0..src.len() {
        b.cx(src[i], dst[i]);
    }
    let center_flag = b.alloc_qubit();
    let half_p = p >> 1usize;
    let half = load_const(b, src.len(), half_p);
    cmp_lt_into(b, &half, &dst[..src.len()], center_flag);
    unload_const(b, &half, half_p);
    csub_nbit_const(b, dst, p, center_flag);
    center_flag
}

pub(crate) fn by_unload_centered_copy_for_bench(
    b: &mut B,
    src: &[QubitId],
    dst: &[QubitId],
    p: U256,
    center_flag: QubitId,
) {
    assert!(dst.len() >= src.len());
    cadd_nbit_const(b, dst, p, center_flag);
    let half_p = p >> 1usize;
    let half = load_const(b, src.len(), half_p);
    cmp_lt_into(b, &half, &dst[..src.len()], center_flag);
    unload_const(b, &half, half_p);
    for i in 0..src.len() {
        b.cx(src[i], dst[i]);
    }
    b.free(center_flag);
}

pub(crate) fn compute_pair1_lam_with_centered_by_bench(
    b: &mut B,
    tx: &[QubitId],
    ty: &[QubitId],
    p: U256,
) -> Vec<QubitId> {
    // Functional pair1 experiment: compute lam=-dy/dx using denominator-derived
    // BY controls and centered tagged numerator replay.  This is Bennett-style:
    // copy the recovered lam, then reverse replay/control generation so only lam
    // remains.  The caller can use the ordinary mul2 cleanup to zero ty.
    const STEPS: usize = 576;
    const DBITS: usize = 12;
    const WIDE: usize = N + 4;
    // Lowword q corrections are bounded below 2^17 in the sampled window
    // algebra, so 18 signed bits are enough for the raw payload history. The
    // local simulator remains 34 bits wide for reversible signed divsteps.
    const WINDOW_QBITS: usize = 18;
    b.set_phase("pair1_by_centered_alloc");
    let f = b.alloc_qubits(STEPS);
    let g = b.alloc_qubits(STEPS);
    let delta = b.alloc_qubits(DBITS);
    let odd = b.alloc_qubits(STEPS);
    let a_ctrl = b.alloc_qubits(STEPS);
    let parity = b.alloc_qubits(STEPS);
    let q_hist = if by_window_q_payload_enabled_for_bench() {
        Some((
            b.alloc_qubits((STEPS / 16) * WINDOW_QBITS),
            b.alloc_qubits((STEPS / 16) * WINDOW_QBITS),
        ))
    } else {
        None
    };
    let r = b.alloc_qubits(WIDE);
    let s = b.alloc_qubits(WIDE);
    let num = b.alloc_qubits(N);
    let lam = b.alloc_qubits(N);

    for i in 0..N {
        if bit(p, i) {
            b.x(f[i]);
        }
        b.cx(tx[i], g[i]);
        b.cx(ty[i], num[i]);
    }
    b.x(delta[0]);
    mod_add_qq_fast(b, &num, tx, p); // tagged numerator: dy + dx
    let center_flag = by_load_centered_copy_for_bench(b, &num, &s, p);

    b.set_phase("pair1_by_centered_generate");
    // Full-width denominator evolution preserves the final f sign needed by
    // tagged quotient recovery.  With BY_CENTERED_WINDOW_DENOM_REPLACE=1 the
    // branch decisions are sourced from 16-step lowword window oracles, then
    // applied to this full-width state; otherwise this is the original direct
    // per-step generator.
    let q_hist_slices = q_hist
        .as_ref()
        .map(|(q0, q1)| (q0.as_slice(), q1.as_slice()));
    by_generate_signed_controls_for_bench(b, &f, &g, &delta, &odd, &a_ctrl, q_hist_slices);

    b.set_phase("pair1_by_centered_forward");
    for i in 0..STEPS {
        centered_signed_by_microstep_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
    }

    b.set_phase("pair1_by_centered_copy_lam");
    by_write_neg_quotient_from_centered_r_for_bench(b, &lam, &r, f[STEPS - 1], p);

    b.set_phase("pair1_by_centered_inverse_replay");
    for i in (0..STEPS).rev() {
        centered_signed_by_microstep_inverse_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
        centered_signed_by_clear_parity_after_inverse_for_bench(b, &r, &s, odd[i], parity[i]);
    }

    b.set_phase("pair1_by_centered_reverse_den");
    let q_hist_slices = q_hist
        .as_ref()
        .map(|(q0, q1)| (q0.as_slice(), q1.as_slice()));
    by_reverse_signed_controls_for_bench(b, &f, &g, &delta, &odd, &a_ctrl, q_hist_slices);

    b.set_phase("pair1_by_centered_clear");
    by_unload_centered_copy_for_bench(b, &num, &s, p, center_flag);
    mod_sub_qq_fast(b, &num, tx, p);
    for i in 0..N {
        b.cx(ty[i], num[i]);
        b.cx(tx[i], g[i]);
        if bit(p, i) {
            b.x(f[i]);
        }
    }
    b.x(delta[0]);
    b.free_vec(&num);
    b.free_vec(&s);
    b.free_vec(&r);
    b.free_vec(&parity);
    if let Some((q0_hist, q1_hist)) = q_hist {
        b.free_vec(&q1_hist);
        b.free_vec(&q0_hist);
    }
    b.free_vec(&a_ctrl);
    b.free_vec(&odd);
    b.free_vec(&delta);
    b.free_vec(&g);
    b.free_vec(&f);
    lam
}

pub(crate) fn write_pair2_product_and_clean_lam_with_scaled_by_bench(
    b: &mut B,
    lam: &[QubitId],
    denom: &[QubitId],
    product: &[QubitId],
    p: U256,
) {
    // Last-shot BY architecture: use scaled BY inverse/product-clean directly
    // for pair2.  Given q=lam and denominator x, the inverse scaled replay maps
    // (sign(f)*q, 0) -> (0, q*x).  In the u=-r frame the input is
    // u = -sign(f)*q, so f>0 selects -q and f<0 leaves q.  This deletes pair2's
    // old q*x multiplication and avoids centered parity history; it still uses
    // the direct 576-step denominator generator and is therefore a correctness
    // probe, not yet SOTA-shaped.
    const STEPS: usize = 576;
    const DBITS: usize = 12;
    b.set_phase("pair2_by_scaled_product_alloc");
    let f = b.alloc_qubits(STEPS);
    let g = b.alloc_qubits(STEPS);
    let delta = b.alloc_qubits(DBITS);
    let odd = b.alloc_qubits(STEPS);
    let a_ctrl = b.alloc_qubits(STEPS);

    for i in 0..N {
        if bit(p, i) {
            b.x(f[i]);
        }
        b.cx(denom[i], g[i]);
    }
    b.x(delta[0]);

    b.set_phase("pair2_by_scaled_product_generate");
    by_generate_signed_controls_for_bench(b, &f, &g, &delta, &odd, &a_ctrl, None);

    b.set_phase("pair2_by_scaled_product_frame");
    let f_pos = b.alloc_qubit();
    b.x(f_pos);
    b.cx(f[STEPS - 1], f_pos);
    by_cmod_neg_inplace_canonical_for_bench(b, lam, f_pos, p);

    b.set_phase("pair2_by_scaled_product_inverse");
    for i in (0..STEPS).rev() {
        scaled_by_controlled_microstep_inverse_negr_for_bench(
            b, lam, product, odd[i], a_ctrl[i], p,
        );
    }

    b.set_phase("pair2_by_scaled_product_clear_frame");
    b.cx(f[STEPS - 1], f_pos);
    b.x(f_pos);
    b.free(f_pos);

    b.set_phase("pair2_by_scaled_product_reverse_den");
    by_reverse_signed_controls_for_bench(b, &f, &g, &delta, &odd, &a_ctrl, None);

    b.set_phase("pair2_by_scaled_product_clear");
    for i in 0..N {
        b.cx(denom[i], g[i]);
        if bit(p, i) {
            b.x(f[i]);
        }
    }
    b.x(delta[0]);
    b.free_vec(&a_ctrl);
    b.free_vec(&odd);
    b.free_vec(&delta);
    b.free_vec(&g);
    b.free_vec(&f);
}

pub(crate) fn add_neg_quotient_into_acc_with_centered_by_bench(
    b: &mut B,
    acc: &[QubitId],
    denom: &[QubitId],
    numer: &[QubitId],
    p: U256,
) {
    // Functional pair2-style experiment: add -(numer/denom) into an existing
    // accumulator, then Bennett-clean the BY denominator/replay scratch.  For
    // pair2, acc is lam and numer = lam*denom, so this zeros lam without a
    // separate quotient output register that would need uncomputation.
    const STEPS: usize = 576;
    const DBITS: usize = 12;
    const WIDE: usize = N + 4;
    const WINDOW_QBITS: usize = 18;
    b.set_phase("by_centered_accquot_alloc");
    let f = b.alloc_qubits(STEPS);
    let g = b.alloc_qubits(STEPS);
    let delta = b.alloc_qubits(DBITS);
    let odd = b.alloc_qubits(STEPS);
    let a_ctrl = b.alloc_qubits(STEPS);
    let parity = b.alloc_qubits(STEPS);
    let q_hist = if by_window_q_payload_enabled_for_bench() {
        Some((
            b.alloc_qubits((STEPS / 16) * WINDOW_QBITS),
            b.alloc_qubits((STEPS / 16) * WINDOW_QBITS),
        ))
    } else {
        None
    };
    let r = b.alloc_qubits(WIDE);
    let s = b.alloc_qubits(WIDE);
    let num = b.alloc_qubits(N);

    for i in 0..N {
        if bit(p, i) {
            b.x(f[i]);
        }
        b.cx(denom[i], g[i]);
        b.cx(numer[i], num[i]);
    }
    b.x(delta[0]);
    mod_add_qq_fast(b, &num, denom, p);
    let center_flag = by_load_centered_copy_for_bench(b, &num, &s, p);

    b.set_phase("by_centered_accquot_generate");
    let q_hist_slices = q_hist
        .as_ref()
        .map(|(q0, q1)| (q0.as_slice(), q1.as_slice()));
    by_generate_signed_controls_for_bench(b, &f, &g, &delta, &odd, &a_ctrl, q_hist_slices);

    b.set_phase("by_centered_accquot_forward");
    for i in 0..STEPS {
        centered_signed_by_microstep_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
    }

    b.set_phase("by_centered_accquot_add");
    by_add_neg_quotient_from_centered_r_for_bench(b, acc, &r, f[STEPS - 1], p);

    b.set_phase("by_centered_accquot_inverse_replay");
    for i in (0..STEPS).rev() {
        centered_signed_by_microstep_inverse_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
        centered_signed_by_clear_parity_after_inverse_for_bench(b, &r, &s, odd[i], parity[i]);
    }

    b.set_phase("by_centered_accquot_reverse_den");
    let q_hist_slices = q_hist
        .as_ref()
        .map(|(q0, q1)| (q0.as_slice(), q1.as_slice()));
    by_reverse_signed_controls_for_bench(b, &f, &g, &delta, &odd, &a_ctrl, q_hist_slices);

    b.set_phase("by_centered_accquot_clear");
    by_unload_centered_copy_for_bench(b, &num, &s, p, center_flag);
    mod_sub_qq_fast(b, &num, denom, p);
    for i in 0..N {
        b.cx(numer[i], num[i]);
        b.cx(denom[i], g[i]);
        if bit(p, i) {
            b.x(f[i]);
        }
    }
    b.x(delta[0]);
    b.free_vec(&num);
    b.free_vec(&s);
    b.free_vec(&r);
    b.free_vec(&parity);
    if let Some((q0_hist, q1_hist)) = q_hist {
        b.free_vec(&q1_hist);
        b.free_vec(&q0_hist);
    }
    b.free_vec(&a_ctrl);
    b.free_vec(&odd);
    b.free_vec(&delta);
    b.free_vec(&g);
    b.free_vec(&f);
}

