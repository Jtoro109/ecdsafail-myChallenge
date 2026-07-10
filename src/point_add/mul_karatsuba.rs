//! (refactor r2) Mechanically extracted from mul.rs. No logic changes.
use super::*;

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
