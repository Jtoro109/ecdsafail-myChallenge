//! (refactor) Mechanically extracted from kaliski.rs. No logic changes.
use super::*;
/// Optional side-channel coefficient transform used by the tagged-DIV probe.
/// It applies the same linear Kaliski coefficient update to an external
/// `(cr, cs)` pair while the ordinary inverse state still carries the
/// qrisp sentinel needed to uncompute branch flags.
pub(crate) fn coeff_channel_cswap(b: &mut B, ctrl: QubitId, cr: &[QubitId], cs: &[QubitId]) {
    assert_eq!(cr.len(), cs.len());
    for i in 0..cr.len() {
        cswap(b, ctrl, cr[i], cs[i]);
    }
}

pub(crate) fn coeff_channel_cadd(b: &mut B, p: U256, cr: &[QubitId], cs: &[QubitId], ctrl: QubitId) {
    cmod_add_qq(b, cs, cr, ctrl, p);
}

pub(crate) fn coeff_channel_csub(b: &mut B, p: U256, cr: &[QubitId], cs: &[QubitId], ctrl: QubitId) {
    cmod_sub_qq(b, cs, cr, ctrl, p);
}

pub(crate) fn coeff_channel_double(b: &mut B, p: U256, cr: &[QubitId]) {
    // The data coefficient is an arbitrary field element, not the bounded
    // qrisp inverse coefficient, so the early no-correction shift is invalid.
    mod_double_inplace_fast(b, cr, p);
}


pub(crate) fn kaliski_branch_iteration_with_coeff(
    b: &mut B,
    p: U256,
    u: &[QubitId],
    v_w: &[QubitId],
    m_i: QubitId,
    a_i: QubitId,
    f: QubitId,
    coeff: (&[QubitId], &[QubitId]),
) {
    let n = u.len();
    let b_f = b.alloc_qubit();
    let add_f = b.alloc_qubit();
    let _kal_saved_phase = b.phase;

    b.set_phase("br_step0_eqzero");
    with_eq_zero_fast(b, v_w, add_f, |b| {
        b.ccx(f, add_f, m_i);
    });
    b.cx(m_i, f);

    b.set_phase("br_step1");
    b.ccx(f, u[0], b_f);
    b.cx(f, a_i);
    b.cx(b_f, a_i);
    b.x(v_w[0]);
    b.ccx(b_f, v_w[0], m_i);
    b.x(v_w[0]);
    {
        let zm = b.alloc_bit();
        b.hmr(b_f, zm);
        b.cz_if(f, u[0], zm);
    }
    b.cx(a_i, b_f);
    b.cx(m_i, b_f);

    b.set_phase("br_step2");
    let l_gt = b.alloc_qubit();
    with_gt(b, u, v_w, l_gt, |b| {
        b.x(b_f);
        b.ccx(f, l_gt, add_f);
        let t = b.alloc_qubit();
        b.ccx(add_f, b_f, t);
        b.cx(t, a_i);
        b.cx(t, m_i);
        {
            let tm = b.alloc_bit();
            b.hmr(t, tm);
            b.cz_if(add_f, b_f, tm);
        }
        b.free(t);
        {
            let am = b.alloc_bit();
            b.hmr(add_f, am);
            b.cz_if(f, l_gt, am);
        }
        b.x(b_f);
    });
    b.free(l_gt);

    b.set_phase("br_step3_cswap");
    for j in 0..n {
        cswap(b, a_i, u[j], v_w[j]);
    }
    coeff_channel_cswap(b, a_i, coeff.0, coeff.1);

    b.set_phase("br_step4");
    mcx2_polar(b, f, true, b_f, false, add_f);
    cucc_sub_ctrl(b, u, v_w, add_f);
    b.set_phase("br_coeff_step4_add");
    coeff_channel_cadd(b, p, coeff.0, coeff.1, add_f);

    b.set_phase("br_step5");
    b.x(b_f);
    {
        let sm = b.alloc_bit();
        b.hmr(add_f, sm);
        b.cz_if(f, b_f, sm);
    }
    b.x(b_f);
    b.cx(m_i, b_f);
    b.cx(a_i, b_f);
    b.free(add_f);
    b.free(b_f);

    b.set_phase("br_step6_8");
    for i in 0..(n - 1) {
        b.swap(v_w[i], v_w[i + 1]);
    }
    coeff_channel_double(b, p, coeff.0);

    b.set_phase("br_step9_cswap");
    for j in 0..n {
        cswap(b, a_i, u[j], v_w[j]);
    }
    coeff_channel_cswap(b, a_i, coeff.0, coeff.1);

    b.set_phase(_kal_saved_phase);
}

pub(crate) fn kaliski_branch_iteration_record(
    b: &mut B,
    u: &[QubitId],
    v_w: &[QubitId],
    m_i: QubitId,
    a_i: QubitId,
    add_i: Option<QubitId>,
    term_bits: Option<(&[QubitId], usize)>,
    f: QubitId,
) {
    let n = u.len();
    let b_f = b.alloc_qubit();
    let add_f = b.alloc_qubit();
    let _kal_saved_phase = b.phase;

    b.set_phase("br_rec_step0_eqzero");
    with_eq_zero_fast(b, v_w, add_f, |b| {
        b.ccx(f, add_f, m_i);
        if let Some((term_bits, iter_idx)) = term_bits {
            for (j, &q) in term_bits.iter().enumerate() {
                if ((iter_idx >> j) & 1) != 0 {
                    b.cx(m_i, q);
                }
            }
        }
    });
    b.cx(m_i, f);

    b.set_phase("br_rec_step1");
    b.ccx(f, u[0], b_f);
    b.cx(f, a_i);
    b.cx(b_f, a_i);
    b.x(v_w[0]);
    b.ccx(b_f, v_w[0], m_i);
    b.x(v_w[0]);
    {
        let zm = b.alloc_bit();
        b.hmr(b_f, zm);
        b.cz_if(f, u[0], zm);
    }
    b.cx(a_i, b_f);
    b.cx(m_i, b_f);

    b.set_phase("br_rec_step2");
    let l_gt = b.alloc_qubit();
    with_gt(b, u, v_w, l_gt, |b| {
        b.x(b_f);
        b.ccx(f, l_gt, add_f);
        let t = b.alloc_qubit();
        b.ccx(add_f, b_f, t);
        b.cx(t, a_i);
        b.cx(t, m_i);
        {
            let tm = b.alloc_bit();
            b.hmr(t, tm);
            b.cz_if(add_f, b_f, tm);
        }
        b.free(t);
        {
            let am = b.alloc_bit();
            b.hmr(add_f, am);
            b.cz_if(f, l_gt, am);
        }
        b.x(b_f);
    });
    b.free(l_gt);

    b.set_phase("br_rec_step3_cswap");
    for j in 0..n {
        cswap(b, a_i, u[j], v_w[j]);
    }

    b.set_phase("br_rec_step4");
    mcx2_polar(b, f, true, b_f, false, add_f);
    if let Some(add_i) = add_i {
        b.cx(add_f, add_i);
    }
    cucc_sub_ctrl(b, u, v_w, add_f);

    b.set_phase("br_rec_step5");
    b.x(b_f);
    {
        let sm = b.alloc_bit();
        b.hmr(add_f, sm);
        b.cz_if(f, b_f, sm);
    }
    b.x(b_f);
    b.cx(m_i, b_f);
    b.cx(a_i, b_f);
    b.free(add_f);
    b.free(b_f);

    b.set_phase("br_rec_step6");
    for i in 0..(n - 1) {
        b.swap(v_w[i], v_w[i + 1]);
    }

    b.set_phase("br_rec_step9_cswap");
    for j in 0..n {
        cswap(b, a_i, u[j], v_w[j]);
    }

    b.set_phase(_kal_saved_phase);
}

pub(crate) fn apply_coeff_channel_from_hist(
    b: &mut B,
    p: U256,
    cr: &[QubitId],
    cs: &[QubitId],
    a_hist: &[QubitId],
    add_hist: &[QubitId],
) {
    assert_eq!(a_hist.len(), add_hist.len());
    for i in 0..a_hist.len() {
        b.set_phase("br_stream_coeff_cswap1");
        coeff_channel_cswap(b, a_hist[i], cr, cs);
        b.set_phase("br_stream_coeff_add");
        coeff_channel_cadd(b, p, cr, cs, add_hist[i]);
        b.set_phase("br_stream_coeff_double");
        coeff_channel_double(b, p, cr);
        b.set_phase("br_stream_coeff_cswap2");
        coeff_channel_cswap(b, a_hist[i], cr, cs);
    }
}

pub(crate) fn apply_coeff_channel_from_term_roll(
    b: &mut B,
    p: U256,
    cr: &[QubitId],
    cs: &[QubitId],
    a_hist: &[QubitId],
    m_hist: &[QubitId],
    term_bits: &[QubitId],
) {
    assert_eq!(a_hist.len(), m_hist.len());
    let active = b.alloc_qubit();
    b.x(active); // active before the terminal iteration.
    for i in 0..a_hist.len() {
        b.set_phase("br_roll_term_update");
        let eq_i = b.alloc_qubit();
        with_eq_const_fast(b, term_bits, i, eq_i, |b| {
            b.cx(eq_i, active);
        });
        b.free(eq_i);

        b.set_phase("br_roll_coeff_cswap1");
        coeff_channel_cswap(b, a_hist[i], cr, cs);

        b.set_phase("br_roll_coeff_add");
        let same = b.alloc_qubit();
        b.x(same);
        b.cx(a_hist[i], same);
        b.cx(m_hist[i], same); // same = !(a xor m)
        let add_ctrl = b.alloc_qubit();
        b.ccx(active, same, add_ctrl);
        coeff_channel_cadd(b, p, cr, cs, add_ctrl);
        b.ccx(active, same, add_ctrl);
        b.free(add_ctrl);
        b.cx(m_hist[i], same);
        b.cx(a_hist[i], same);
        b.x(same);
        b.free(same);

        b.set_phase("br_roll_coeff_double");
        coeff_channel_double(b, p, cr);
        b.set_phase("br_roll_coeff_cswap2");
        coeff_channel_cswap(b, a_hist[i], cr, cs);
    }
    b.free(active);
}

pub(crate) fn apply_coeff_channel_from_term_roll_inverse(
    b: &mut B,
    p: U256,
    cr: &[QubitId],
    cs: &[QubitId],
    a_hist: &[QubitId],
    m_hist: &[QubitId],
    term_bits: &[QubitId],
) {
    assert_eq!(a_hist.len(), m_hist.len());
    let active = b.alloc_qubit(); // active after the last forward iteration is 0.
    for i in (0..a_hist.len()).rev() {
        b.set_phase("br_roll_inv_coeff_cswap2");
        coeff_channel_cswap(b, a_hist[i], cr, cs);
        b.set_phase("br_roll_inv_coeff_halve");
        mod_halve_inplace_fast(b, cr, p);

        b.set_phase("br_roll_inv_coeff_sub");
        let same = b.alloc_qubit();
        b.x(same);
        b.cx(a_hist[i], same);
        b.cx(m_hist[i], same); // same = !(a xor m)
        let sub_ctrl = b.alloc_qubit();
        b.ccx(active, same, sub_ctrl);
        coeff_channel_csub(b, p, cr, cs, sub_ctrl);
        b.ccx(active, same, sub_ctrl);
        b.free(sub_ctrl);
        b.cx(m_hist[i], same);
        b.cx(a_hist[i], same);
        b.x(same);
        b.free(same);

        b.set_phase("br_roll_inv_coeff_cswap1");
        coeff_channel_cswap(b, a_hist[i], cr, cs);

        b.set_phase("br_roll_inv_term_update");
        let eq_i = b.alloc_qubit();
        with_eq_const_fast(b, term_bits, i, eq_i, |b| {
            b.cx(eq_i, active);
        });
        b.free(eq_i);
    }
    // We have rewound the rolling flag to its pre-iteration-0 value, 1.
    b.x(active);
    b.free(active);
}

pub(crate) fn apply_coeff_channel_from_term_index(
    b: &mut B,
    p: U256,
    cr: &[QubitId],
    cs: &[QubitId],
    a_hist: &[QubitId],
    m_hist: &[QubitId],
    term_bits: &[QubitId],
) {
    assert_eq!(a_hist.len(), m_hist.len());
    for i in 0..a_hist.len() {
        b.set_phase("br_term_coeff_cswap1");
        coeff_channel_cswap(b, a_hist[i], cr, cs);

        // add is true for UG: (a,m)=(1,1).
        b.set_phase("br_term_coeff_add_ug");
        let ug_ctrl = b.alloc_qubit();
        b.ccx(a_hist[i], m_hist[i], ug_ctrl);
        coeff_channel_cadd(b, p, cr, cs, ug_ctrl);
        {
            let um = b.alloc_bit();
            b.hmr(ug_ctrl, um);
            b.cz_if(a_hist[i], m_hist[i], um);
        }
        b.free(ug_ctrl);

        // add is also true for active VG: (a,m)=(0,0) before the terminal
        // iteration. The terminal index is written once during branch record.
        b.set_phase("br_term_coeff_add_vg");
        let active = b.alloc_qubit();
        let ci = load_const(b, term_bits.len(), U256::from(i as u64));
        cmp_gt_into(b, term_bits, &ci, active); // active = term_idx > i
        let vg_ctrl = b.alloc_qubit();
        let scratch = b.alloc_qubit();
        mcx3_polar(
            b, active, true, a_hist[i], false, m_hist[i], false, vg_ctrl, scratch,
        );
        coeff_channel_cadd(b, p, cr, cs, vg_ctrl);
        mcx3_polar(
            b, active, true, a_hist[i], false, m_hist[i], false, vg_ctrl, scratch,
        );
        b.free(scratch);
        b.free(vg_ctrl);
        cmp_gt_into(b, term_bits, &ci, active);
        unload_const(b, &ci, U256::from(i as u64));
        b.free(active);

        b.set_phase("br_term_coeff_double");
        coeff_channel_double(b, p, cr);
        b.set_phase("br_term_coeff_cswap2");
        coeff_channel_cswap(b, a_hist[i], cr, cs);
    }
}

pub(crate) fn kaliski_branch_iteration_backward_recorded(
    b: &mut B,
    u: &[QubitId],
    v_w: &[QubitId],
    m_i: QubitId,
    a_i: QubitId,
    add_i: QubitId,
    f: QubitId,
) {
    let n = u.len();
    let b_f = b.alloc_qubit();
    let add_f = b.alloc_qubit();
    let _kal_saved_phase = b.phase;

    b.cx(a_i, b_f);
    b.cx(m_i, b_f);
    mcx2_polar(b, f, true, b_f, false, add_f);

    b.set_phase("br_rec_bk_step9_cswap");
    for j in (0..n).rev() {
        cswap(b, a_i, u[j], v_w[j]);
    }

    b.set_phase("br_rec_bk_step6");
    for i in (0..(n - 1)).rev() {
        b.swap(v_w[i], v_w[i + 1]);
    }

    b.set_phase("br_rec_bk_step4");
    cucc_add_ctrl(b, u, v_w, add_f);
    b.cx(add_f, add_i);

    b.set_phase("br_rec_bk_step5_unadd");
    b.x(b_f);
    {
        let sm = b.alloc_bit();
        b.hmr(add_f, sm);
        b.cz_if(f, b_f, sm);
    }
    b.x(b_f);

    b.set_phase("br_rec_bk_step3_cswap");
    for j in (0..n).rev() {
        cswap(b, a_i, u[j], v_w[j]);
    }

    b.set_phase("br_rec_bk_step2");
    let l_gt = b.alloc_qubit();
    with_gt(b, u, v_w, l_gt, |b| {
        b.x(b_f);
        b.ccx(f, l_gt, add_f);
        let t = b.alloc_qubit();
        b.ccx(add_f, b_f, t);
        b.cx(t, m_i);
        b.cx(t, a_i);
        {
            let tm = b.alloc_bit();
            b.hmr(t, tm);
            b.cz_if(add_f, b_f, tm);
        }
        b.free(t);
        {
            let am = b.alloc_bit();
            b.hmr(add_f, am);
            b.cz_if(f, l_gt, am);
        }
        b.x(b_f);
    });
    b.free(l_gt);

    b.set_phase("br_rec_bk_step1");
    b.cx(m_i, b_f);
    b.cx(a_i, b_f);
    b.ccx(f, u[0], b_f);
    b.x(v_w[0]);
    b.ccx(b_f, v_w[0], m_i);
    b.x(v_w[0]);
    b.cx(b_f, a_i);
    b.cx(f, a_i);
    {
        let zm = b.alloc_bit();
        b.hmr(b_f, zm);
        b.cz_if(f, u[0], zm);
    }

    b.set_phase("br_rec_bk_step0_eqzero");
    b.cx(m_i, f);
    with_eq_zero_fast(b, v_w, add_f, |b| {
        b.ccx(f, add_f, m_i);
    });

    b.free(add_f);
    b.free(b_f);
    b.set_phase(_kal_saved_phase);
}

pub(crate) fn kaliski_branch_iteration_backward(
    b: &mut B,
    u: &[QubitId],
    v_w: &[QubitId],
    m_i: QubitId,
    a_i: QubitId,
    term_bits: Option<(&[QubitId], usize)>,
    f: QubitId,
) {
    let n = u.len();
    let b_f = b.alloc_qubit();
    let add_f = b.alloc_qubit();
    let _kal_saved_phase = b.phase;

    b.cx(a_i, b_f);
    b.cx(m_i, b_f);
    mcx2_polar(b, f, true, b_f, false, add_f);

    b.set_phase("br_bk_step9_cswap");
    for j in (0..n).rev() {
        cswap(b, a_i, u[j], v_w[j]);
    }

    b.set_phase("br_bk_step6");
    for i in (0..(n - 1)).rev() {
        b.swap(v_w[i], v_w[i + 1]);
    }

    b.set_phase("br_bk_step4");
    cucc_add_ctrl(b, u, v_w, add_f);

    b.set_phase("br_bk_step5_unadd");
    b.x(b_f);
    {
        let sm = b.alloc_bit();
        b.hmr(add_f, sm);
        b.cz_if(f, b_f, sm);
    }
    b.x(b_f);

    b.set_phase("br_bk_step3_cswap");
    for j in (0..n).rev() {
        cswap(b, a_i, u[j], v_w[j]);
    }

    b.set_phase("br_bk_step2");
    let l_gt = b.alloc_qubit();
    with_gt(b, u, v_w, l_gt, |b| {
        b.x(b_f);
        b.ccx(f, l_gt, add_f);
        let t = b.alloc_qubit();
        b.ccx(add_f, b_f, t);
        b.cx(t, m_i);
        b.cx(t, a_i);
        {
            let tm = b.alloc_bit();
            b.hmr(t, tm);
            b.cz_if(add_f, b_f, tm);
        }
        b.free(t);
        {
            let am = b.alloc_bit();
            b.hmr(add_f, am);
            b.cz_if(f, l_gt, am);
        }
        b.x(b_f);
    });
    b.free(l_gt);

    b.set_phase("br_bk_step1");
    b.cx(m_i, b_f);
    b.cx(a_i, b_f);
    b.ccx(f, u[0], b_f);
    b.x(v_w[0]);
    b.ccx(b_f, v_w[0], m_i);
    b.x(v_w[0]);
    b.cx(b_f, a_i);
    b.cx(f, a_i);
    {
        let zm = b.alloc_bit();
        b.hmr(b_f, zm);
        b.cz_if(f, u[0], zm);
    }

    b.set_phase("br_bk_step0_eqzero");
    if let Some((term_bits, iter_idx)) = term_bits {
        for (j, &q) in term_bits.iter().enumerate() {
            if ((iter_idx >> j) & 1) != 0 {
                b.cx(m_i, q);
            }
        }
    }
    b.cx(m_i, f);
    with_eq_zero_fast(b, v_w, add_f, |b| {
        b.ccx(f, add_f, m_i);
    });

    b.free(add_f);
    b.free(b_f);
    b.set_phase(_kal_saved_phase);
}

pub(crate) fn kaliski_branch_forward_with_coeff(
    b: &mut B,
    v_in: &[QubitId],
    st: &KaliskiBranchState,
    p: U256,
    iters: usize,
    coeff: (&[QubitId], &[QubitId]),
) {
    let n = v_in.len();
    for i in 0..n {
        if bit(p, i) {
            b.x(st.u[i]);
        }
        b.cx(v_in[i], st.v_w[i]);
    }
    b.x(st.f_flag);
    for i in 0..iters {
        kaliski_branch_iteration_with_coeff(
            b,
            p,
            &st.u,
            &st.v_w,
            st.m_hist[i],
            st.a_hist[i],
            st.f_flag,
            coeff,
        );
    }
}

pub(crate) fn kaliski_branch_backward(
    b: &mut B,
    v_in: &[QubitId],
    st: &KaliskiBranchState,
    p: U256,
    iters: usize,
) {
    let n = v_in.len();
    for i in (0..iters).rev() {
        kaliski_branch_iteration_backward(
            b,
            &st.u,
            &st.v_w,
            st.m_hist[i],
            st.a_hist[i],
            None,
            st.f_flag,
        );
    }
    b.x(st.f_flag);
    for i in 0..n {
        b.cx(v_in[i], st.v_w[i]);
        if bit(p, i) {
            b.x(st.u[i]);
        }
    }
}

pub(crate) fn kaliski_branch_record_forward(
    b: &mut B,
    v_in: &[QubitId],
    st: &KaliskiBranchState,
    p: U256,
    iters: usize,
) {
    let n = v_in.len();
    for i in 0..n {
        if bit(p, i) {
            b.x(st.u[i]);
        }
        b.cx(v_in[i], st.v_w[i]);
    }
    b.x(st.f_flag);
    for i in 0..iters {
        kaliski_branch_iteration_record(
            b,
            &st.u,
            &st.v_w,
            st.m_hist[i],
            st.a_hist[i],
            Some(st.add_hist[i]),
            None,
            st.f_flag,
        );
    }
}

pub(crate) fn kaliski_branch_record_backward(
    b: &mut B,
    v_in: &[QubitId],
    st: &KaliskiBranchState,
    p: U256,
    iters: usize,
) {
    let n = v_in.len();
    for i in (0..iters).rev() {
        kaliski_branch_iteration_backward_recorded(
            b,
            &st.u,
            &st.v_w,
            st.m_hist[i],
            st.a_hist[i],
            st.add_hist[i],
            st.f_flag,
        );
    }
    b.x(st.f_flag);
    for i in 0..n {
        b.cx(v_in[i], st.v_w[i]);
        if bit(p, i) {
            b.x(st.u[i]);
        }
    }
}

pub(crate) fn kaliski_branch_record_forward_term(
    b: &mut B,
    v_in: &[QubitId],
    st: &KaliskiBranchState,
    term_bits: &[QubitId],
    p: U256,
    iters: usize,
) {
    let n = v_in.len();
    for i in 0..n {
        if bit(p, i) {
            b.x(st.u[i]);
        }
        b.cx(v_in[i], st.v_w[i]);
    }
    b.x(st.f_flag);
    for i in 0..iters {
        kaliski_branch_iteration_record(
            b,
            &st.u,
            &st.v_w,
            st.m_hist[i],
            st.a_hist[i],
            None,
            Some((term_bits, i)),
            st.f_flag,
        );
    }
}

pub(crate) fn kaliski_branch_record_backward_term(
    b: &mut B,
    v_in: &[QubitId],
    st: &KaliskiBranchState,
    term_bits: &[QubitId],
    p: U256,
    iters: usize,
) {
    let n = v_in.len();
    for i in (0..iters).rev() {
        kaliski_branch_iteration_backward(
            b,
            &st.u,
            &st.v_w,
            st.m_hist[i],
            st.a_hist[i],
            Some((term_bits, i)),
            st.f_flag,
        );
    }
    b.x(st.f_flag);
    for i in 0..n {
        b.cx(v_in[i], st.v_w[i]);
        if bit(p, i) {
            b.x(st.u[i]);
        }
    }
}

pub(crate) fn with_kal_branch_inv_raw_roll<F: FnOnce(&mut B, &[QubitId])>(
    b: &mut B,
    v_in: &[QubitId],
    p: U256,
    iters: usize,
    body: F,
) {
    let n = v_in.len();
    let mut st = alloc_kaliski_branch_state_no_add(b, n, iters);
    let term_bits = b.alloc_qubits(9);
    kaliski_branch_record_forward_term(b, v_in, &st, &term_bits, p, iters);

    // Final denominator state is known when iters is beyond the convergence
    // tail. Free it so coefficient replay carries only histories + inv coeffs.
    b.x(st.u[0]);
    b.free_vec(&st.u);
    b.free_vec(&st.v_w);
    b.free(st.f_flag);

    let inv_raw = b.alloc_qubits(n);
    let coeff_s = b.alloc_qubits(n);
    b.x(coeff_s[0]);
    apply_coeff_channel_from_term_roll(
        b, p, &inv_raw, &coeff_s, &st.a_hist, &st.m_hist, &term_bits,
    );

    body(b, &inv_raw);

    apply_coeff_channel_from_term_roll_inverse(
        b, p, &inv_raw, &coeff_s, &st.a_hist, &st.m_hist, &term_bits,
    );
    b.x(coeff_s[0]);
    b.free_vec(&coeff_s);
    b.free_vec(&inv_raw);

    st.u = b.alloc_qubits(n);
    st.v_w = b.alloc_qubits(n);
    st.f_flag = b.alloc_qubit();
    b.x(st.u[0]);
    kaliski_branch_record_backward_term(b, v_in, &st, &term_bits, p, iters);
    b.free_vec(&term_bits);
    free_kaliski_branch_state(b, st);
}

pub(crate) fn with_kal_branch_term_roll_tagged_div<F: FnOnce(&mut B)>(
    b: &mut B,
    v_in: &[QubitId],
    p: U256,
    iters: usize,
    coeff: (&[QubitId], &[QubitId]),
    body: F,
) {
    let n = v_in.len();
    let mut st = alloc_kaliski_branch_state_no_add(b, n, iters);
    let term_bits = b.alloc_qubits(9);
    kaliski_branch_record_forward_term(b, v_in, &st, &term_bits, p, iters);

    b.x(st.u[0]);
    b.free_vec(&st.u);
    b.free_vec(&st.v_w);
    b.free(st.f_flag);

    apply_coeff_channel_from_term_roll(b, p, coeff.0, coeff.1, &st.a_hist, &st.m_hist, &term_bits);
    body(b);

    st.u = b.alloc_qubits(n);
    st.v_w = b.alloc_qubits(n);
    st.f_flag = b.alloc_qubit();
    b.x(st.u[0]);
    kaliski_branch_record_backward_term(b, v_in, &st, &term_bits, p, iters);
    b.free_vec(&term_bits);
    free_kaliski_branch_state(b, st);
}

pub(crate) fn with_kal_branch_term_tagged_div<F: FnOnce(&mut B)>(
    b: &mut B,
    v_in: &[QubitId],
    p: U256,
    iters: usize,
    coeff: (&[QubitId], &[QubitId]),
    body: F,
) {
    let n = v_in.len();
    let mut st = alloc_kaliski_branch_state_no_add(b, n, iters);
    let term_bits = b.alloc_qubits(9);
    kaliski_branch_record_forward_term(b, v_in, &st, &term_bits, p, iters);

    b.x(st.u[0]);
    b.free_vec(&st.u);
    b.free_vec(&st.v_w);
    b.free(st.f_flag);

    apply_coeff_channel_from_term_index(b, p, coeff.0, coeff.1, &st.a_hist, &st.m_hist, &term_bits);
    body(b);

    st.u = b.alloc_qubits(n);
    st.v_w = b.alloc_qubits(n);
    st.f_flag = b.alloc_qubit();
    b.x(st.u[0]);
    kaliski_branch_record_backward_term(b, v_in, &st, &term_bits, p, iters);
    b.free_vec(&term_bits);
    free_kaliski_branch_state(b, st);
}

pub(crate) fn with_kal_branch_stream_tagged_div<F: FnOnce(&mut B)>(
    b: &mut B,
    v_in: &[QubitId],
    p: U256,
    iters: usize,
    coeff: (&[QubitId], &[QubitId]),
    body: F,
) {
    let n = v_in.len();
    let mut st = alloc_kaliski_branch_state(b, n, iters);
    kaliski_branch_record_forward(b, v_in, &st, p, iters);

    // At sufficient iteration count the denominator state is known `(u,v,f)=(1,0,0)`.
    // Free it before the coefficient replay so the replay peak is history + coeff,
    // not history + denominator + coeff.
    b.x(st.u[0]);
    b.free_vec(&st.u);
    b.free_vec(&st.v_w);
    b.free(st.f_flag);

    apply_coeff_channel_from_hist(b, p, coeff.0, coeff.1, &st.a_hist, &st.add_hist);
    body(b);

    st.u = b.alloc_qubits(n);
    st.v_w = b.alloc_qubits(n);
    st.f_flag = b.alloc_qubit();
    b.x(st.u[0]);
    kaliski_branch_record_backward(b, v_in, &st, p, iters);
    free_kaliski_branch_state(b, st);
}

pub(crate) fn with_kal_branch_tagged_div_coeff<F: FnOnce(&mut B)>(
    b: &mut B,
    v_in: &[QubitId],
    p: U256,
    iters: usize,
    coeff: (&[QubitId], &[QubitId]),
    body: F,
) {
    let st = alloc_kaliski_branch_state(b, v_in.len(), iters);
    kaliski_branch_forward_with_coeff(b, v_in, &st, p, iters, coeff);
    body(b);
    kaliski_branch_backward(b, v_in, &st, p, iters);
    free_kaliski_branch_state(b, st);
}
