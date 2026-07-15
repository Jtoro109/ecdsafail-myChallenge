//! Measure the exact Toffoli cost of each modular arithmetic primitive in
//! isolation. Test-only; emits numbers via eprintln for the planner.
//!
//! We don't need these for live correctness — just for honest cost accounting
//! so we can sanity-check SOTA reachability.

#![cfg(test)]

use super::{
    cmod_add_qq, cmod_halve_inplace, cmod_sub_qq, mod_add_qb, mod_add_qc, mod_add_qq,
    mod_add_qq_fast, mod_double_inplace_fast, mod_halve_inplace_fast,
    mod_mul_add_into_acc_schoolbook, mod_mul_sub_qq, mod_mul_write_into_zero_acc_schoolbook,
    mod_neg_inplace_fast, mod_sub_qb, mod_sub_qq_fast, N, SECP256K1_P,
};
use super::{QubitId, B};
use crate::circuit::OperationType;

enum ShiftUndoForCost {
    Doubles(usize),
    Chunk(usize, Vec<QubitId>, QubitId, QubitId),
}

fn shift_tmp_up_for_sparse_const_cost(
    b: &mut B,
    tmp: &[QubitId],
    p: alloy_primitives::U256,
    mut delta: usize,
    undo: &mut Vec<ShiftUndoForCost>,
) {
    while delta >= 22 {
        let (spill, flag_inv, ovf) = super::mod_shift_left_by_k(b, tmp, p, 22);
        undo.push(ShiftUndoForCost::Chunk(22, spill, flag_inv, ovf));
        delta -= 22;
    }
    if delta >= 12 {
        let (spill, flag_inv, ovf) = super::mod_shift_left_by_k(b, tmp, p, delta);
        undo.push(ShiftUndoForCost::Chunk(delta, spill, flag_inv, ovf));
    } else if delta > 0 {
        for _ in 0..delta {
            mod_double_inplace_fast(b, tmp, p);
        }
        undo.push(ShiftUndoForCost::Doubles(delta));
    }
}

fn undo_sparse_const_shifts_for_cost(
    b: &mut B,
    tmp: &[QubitId],
    p: alloy_primitives::U256,
    undo: Vec<ShiftUndoForCost>,
) {
    for item in undo.into_iter().rev() {
        match item {
            ShiftUndoForCost::Doubles(k) => {
                for _ in 0..k {
                    mod_halve_inplace_fast(b, tmp, p);
                }
            }
            ShiftUndoForCost::Chunk(k, spill, flag_inv, ovf) => {
                super::mod_shift_right_by_k(b, tmp, p, k, spill, flag_inv, ovf);
            }
        }
    }
}

fn mul_by_const_acc_chunked_shifts_for_cost(
    b: &mut B,
    x: &[QubitId],
    c: alloy_primitives::U256,
    acc: &[QubitId],
    p: alloy_primitives::U256,
) {
    let n = x.len();
    let tmp = b.alloc_qubits(n);
    for i in 0..n {
        b.cx(x[i], tmp[i]);
    }
    let mut positions = Vec::new();
    for i in 0..256 {
        if super::bit(c, i) {
            positions.push(i);
        }
    }
    let mut undo = Vec::new();
    let mut cur = 0usize;
    for pos in positions {
        shift_tmp_up_for_sparse_const_cost(b, &tmp, p, pos - cur, &mut undo);
        cur = pos;
        mod_add_qq(b, acc, &tmp, p);
    }
    undo_sparse_const_shifts_for_cost(b, &tmp, p, undo);
    for i in 0..n {
        b.cx(x[i], tmp[i]);
    }
    b.free_vec(&tmp);
}

fn mul_by_const_acc_chunked_inplace_src_for_cost(
    b: &mut B,
    x: &[QubitId],
    c: alloy_primitives::U256,
    acc: &[QubitId],
    p: alloy_primitives::U256,
) {
    let mut positions = Vec::new();
    for i in 0..256 {
        if super::bit(c, i) {
            positions.push(i);
        }
    }
    let mut undo = Vec::new();
    let mut cur = 0usize;
    for pos in positions {
        shift_tmp_up_for_sparse_const_cost(b, x, p, pos - cur, &mut undo);
        cur = pos;
        mod_add_qq(b, acc, x, p);
    }
    undo_sparse_const_shifts_for_cost(b, x, p, undo);
}

fn count_ccx(ops: &[crate::circuit::Op]) -> usize {
    ops.iter()
        .filter(|o| matches!(o.kind, OperationType::CCX | OperationType::CCZ))
        .count()
}

fn inv_mod_u64_pow2_for_cost(a: u64, k: usize) -> u64 {
    let mask = (1u64 << k) - 1;
    let mut x = 1u64;
    for _ in 0..6 {
        x = x.wrapping_mul(2u64.wrapping_sub(a.wrapping_mul(x))) & mask;
    }
    x & mask
}

fn xor_fast_modulo_multihalve_threshold_s_for_cost(b: &mut B, h: &[QubitId], s: &[QubitId], k: usize) {
    // For y = h·2^(n-k)+r and p=2^n-(2^32+977), the quotient cleanup bit is
    //   e = [r >= 2^(n-k) - floor((2^32+977)(h+1)/2^k)].
    // For k≤22 the threshold size splits with no carry overlap:
    //   floor((2^32+977)(h+1)/2^k)
    //     = (h+1) << (32-k)  +  floor(977(h+1)/2^k),
    // and the second term fits below bit 10 while the first starts at bit ≥10.
    assert!(k <= 22);
    assert_eq!(h.len(), k);
    assert_eq!(s.len(), 33);
    let u = b.alloc_qubits(k + 1);
    for i in 0..k {
        b.cx(h[i], u[i]);
    }
    super::add_nbit_const_fast(b, &u, alloy_primitives::U256::from(1u64));

    let shift = 32usize - k;
    for i in 0..=k {
        b.cx(u[i], s[i + shift]);
    }

    let prod_bits = k + 10; // 977 * 2^k < 2^(k+10)
    let prod = b.alloc_qubits(prod_bits);
    for i in 0..=k {
        let ci = alloy_primitives::U256::from(977u64) << i;
        super::cadd_nbit_const_direct_fast(b, &prod, ci, u[i]);
    }
    for j in k..prod_bits {
        b.cx(prod[j], s[j - k]);
    }
    for i in (0..=k).rev() {
        let ci = alloy_primitives::U256::from(977u64) << i;
        super::csub_nbit_const_direct_fast(b, &prod, ci, u[i]);
    }
    b.free_vec(&prod);

    super::sub_nbit_const_fast(b, &u, alloy_primitives::U256::from(1u64));
    for i in (0..k).rev() {
        b.cx(h[i], u[i]);
    }
    b.free_vec(&u);
}

fn xor_fast_modulo_multihalve_threshold_flag_for_cost(
    b: &mut B,
    y: &[QubitId],
    k: usize,
    target: QubitId,
) {
    let n = y.len();
    assert_eq!(n, N);
    assert!(k <= 22);
    let w = 33usize;
    let r_len = n - k;
    assert!(r_len > w);

    // z = 2^(n-k)-1-r = bitwise NOT of the low (n-k)-bit tail.  Since the
    // threshold s(h) is < 2^33, e iff z_high==0 and z_low<s(h).
    for i in w..r_len {
        b.x(y[i]);
    }
    let high_zero = b.alloc_qubit();
    super::with_eq_zero_fast(b, &y[w..r_len], high_zero, |b| {
        let z_low = b.alloc_qubits(w);
        for i in 0..w {
            b.cx(y[i], z_low[i]);
            b.x(z_low[i]);
        }
        let s = b.alloc_qubits(w);
        xor_fast_modulo_multihalve_threshold_s_for_cost(b, &y[r_len..], &s, k);
        let lt = b.alloc_qubit();
        super::with_lt(b, &z_low, &s, lt, |b| {
            b.ccx(high_zero, lt, target);
        });
        b.free(lt);
        xor_fast_modulo_multihalve_threshold_s_for_cost(b, &y[r_len..], &s, k);
        b.free_vec(&s);
        for i in (0..w).rev() {
            b.x(z_low[i]);
            b.cx(y[i], z_low[i]);
        }
        b.free_vec(&z_low);
    });
    b.free(high_zero);
    for i in (w..r_len).rev() {
        b.x(y[i]);
    }
}

fn addsub_nbit_qq_fast_sign_for_cost(b: &mut B, a: &[QubitId], acc: &[QubitId], subtract: QubitId) {
    let c_in = b.alloc_qubit();
    b.cx(subtract, c_in);
    for &q in a {
        b.cx(subtract, q);
    }
    super::ripple_adder_add_fast(b, a, acc, c_in);
    for &q in a.iter().rev() {
        b.cx(subtract, q);
    }
    b.cx(subtract, c_in);
    b.free(c_in);
}

fn cmod_addsub_qq_two_cmods_for_cost(
    b: &mut B,
    acc: &[QubitId],
    a: &[QubitId],
    ctrl: QubitId,
    subtract: QubitId,
    p: alloy_primitives::U256,
) {
    let add_ctrl = b.alloc_qubit();
    let sub_ctrl = b.alloc_qubit();

    b.x(subtract);
    b.ccx(ctrl, subtract, add_ctrl);
    b.x(subtract);
    b.ccx(ctrl, subtract, sub_ctrl);

    cmod_add_qq(b, acc, a, add_ctrl, p);
    cmod_sub_qq(b, acc, a, sub_ctrl, p);

    b.ccx(ctrl, subtract, sub_ctrl);
    b.x(subtract);
    b.ccx(ctrl, subtract, add_ctrl);
    b.x(subtract);

    b.free(sub_ctrl);
    b.free(add_ctrl);
}

fn cmod_addsub_qq_fused_negated_add_for_cost(
    b: &mut B,
    acc: &[QubitId],
    a: &[QubitId],
    ctrl: QubitId,
    subtract: QubitId,
    p: alloy_primitives::U256,
) {
    let n = acc.len();
    let f = b.alloc_qubits(n);
    for i in 0..n {
        b.ccx(ctrl, a[i], f[i]);
    }
    super::coordinate_addition_cmod_neg_inplace_fast(b, &f, subtract, p);
    super::mod_add_qq_fast(b, acc, &f, p);
    super::coordinate_addition_cmod_neg_inplace_fast(b, &f, subtract, p);
    for i in 0..n {
        let m = b.alloc_bit();
        b.hmr(f[i], m);
        b.cz_if(ctrl, a[i], m);
    }
    b.free_vec(&f);
}

fn raw_controlled_addsub_nbit_qq_sign_lower_bound_for_cost(
    b: &mut B,
    acc: &[QubitId],
    a: &[QubitId],
    ctrl: QubitId,
    subtract: QubitId,
) {
    let f = b.alloc_qubits(acc.len());
    for i in 0..acc.len() {
        b.ccx(ctrl, a[i], f[i]);
    }
    addsub_nbit_qq_fast_sign_for_cost(b, &f, acc, subtract);
    for i in 0..acc.len() {
        let m = b.alloc_bit();
        b.hmr(f[i], m);
        b.cz_if(ctrl, a[i], m);
    }
    b.free_vec(&f);
}

fn direct_fast_modulo_multihalve_chunk_cost(k: usize) -> (usize, usize, usize, usize) {
    let n = N;
    let p = SECP256K1_P;
    let c = alloy_primitives::U256::MAX
        .wrapping_sub(p)
        .wrapping_add(alloy_primitives::U256::from(1u64));
    let c_u64 = c.as_limbs()[0];
    let mask = (1u64 << k) - 1;
    let c_inv = inv_mod_u64_pow2_for_cost(c_u64 & mask, k);

    let mut b_cur = B::new();
    let v_cur = b_cur.alloc_qubits(n);
    let start_cur = b_cur.ops.len();
    for _ in 0..k {
        mod_halve_inplace_fast(&mut b_cur, &v_cur, p);
    }
    let current = count_ccx(&b_cur.ops[start_cur..]);

    let mut b = B::new();
    let v = b.alloc_qubits(n);
    let t = b.alloc_qubits(k);
    let prod_bits = k + 32;
    let prod = b.alloc_qubits(prod_bits);
    let start = b.ops.len();

    // t += low(x) * c^{-1} (mod 2^k)
    for i in 0..k {
        let ci = ((c_inv as u128) << i) as u64 & mask;
        super::cadd_nbit_const_direct_fast(&mut b, &t, alloy_primitives::U256::from(ci), v[i]);
    }
    // clear low k input bits using low(t*c)
    for i in 0..k {
        let ci = ((c_u64 as u128) << i) as u64 & mask;
        super::csub_nbit_const_direct_fast(&mut b, &v[..k], alloy_primitives::U256::from(ci), t[i]);
    }
    // product scratch prod = t*c, then subtract high(prod) after the free shift.
    for i in 0..k {
        let ci = c << i;
        super::cadd_nbit_const_direct_fast(&mut b, &prod, ci, t[i]);
    }
    let high = b.alloc_qubits(n);
    for j in k..prod_bits {
        b.cx(prod[j], high[j - k]);
    }
    super::sub_nbit_qq_fast(&mut b, &high, &v);
    for j in k..prod_bits {
        b.cx(prod[j], high[j - k]);
    }
    b.free_vec(&high);
    for i in (0..k).rev() {
        let ci = c << i;
        super::csub_nbit_const_direct_fast(&mut b, &prod, ci, t[i]);
    }
    // t -= output_high_k, leaving the single correction bit e in t[0].
    super::sub_nbit_qq_fast(&mut b, &v[n - k..], &t);
    let candidate_without_corr = count_ccx(&b.ops[start..]);
    let threshold_start = b.ops.len();
    xor_fast_modulo_multihalve_threshold_flag_for_cost(&mut b, &v, k, t[0]);
    let threshold_ccx = count_ccx(&b.ops[threshold_start..]);
    let candidate_exact = count_ccx(&b.ops[start..]);
    (
        current,
        candidate_without_corr,
        candidate_exact,
        threshold_ccx,
    )
}

pub(super) fn direct_fast_modulo_multihalve_chunk_cost_split(k: usize) -> (usize, usize, usize, usize) {
    let n = N;
    let p = SECP256K1_P;
    let c_low = 977u64;
    let mask = (1u64 << k) - 1;
    let c_inv = inv_mod_u64_pow2_for_cost(c_low & mask, k);

    let mut b_cur = B::new();
    let v_cur = b_cur.alloc_qubits(n);
    let start_cur = b_cur.ops.len();
    for _ in 0..k {
        mod_halve_inplace_fast(&mut b_cur, &v_cur, p);
    }
    let current = count_ccx(&b_cur.ops[start_cur..]);

    let mut b = B::new();
    let v = b.alloc_qubits(n);
    let t = b.alloc_qubits(k);
    let small_prod_bits = k + 10; // t*977 < 2^(k+10)
    let small_prod = b.alloc_qubits(small_prod_bits);
    let start = b.ops.len();

    for i in 0..k {
        let ci = ((c_inv as u128) << i) as u64 & mask;
        super::cadd_nbit_const_direct_fast(&mut b, &t, alloy_primitives::U256::from(ci), v[i]);
    }
    for i in 0..k {
        let ci = ((c_low as u128) << i) as u64 & mask;
        super::csub_nbit_const_direct_fast(&mut b, &v[..k], alloy_primitives::U256::from(ci), t[i]);
    }

    // high(t*c) = t*2^(32-k) + floor(t*977/2^k).  Compute only the small
    // t*977 product, then materialize the combined high addend once.
    for i in 0..k {
        let ci = alloy_primitives::U256::from(c_low) << i;
        super::cadd_nbit_const_direct_fast(&mut b, &small_prod, ci, t[i]);
    }
    let high = b.alloc_qubits(n);
    let shift = 32usize - k;
    for j in 0..k {
        b.cx(t[j], high[j + shift]);
    }
    for j in k..small_prod_bits {
        b.cx(small_prod[j], high[j - k]);
    }
    super::sub_nbit_qq_fast(&mut b, &high, &v);
    for j in k..small_prod_bits {
        b.cx(small_prod[j], high[j - k]);
    }
    for j in 0..k {
        b.cx(t[j], high[j + shift]);
    }
    b.free_vec(&high);
    for i in (0..k).rev() {
        let ci = alloy_primitives::U256::from(c_low) << i;
        super::csub_nbit_const_direct_fast(&mut b, &small_prod, ci, t[i]);
    }
    super::sub_nbit_qq_fast(&mut b, &v[n - k..], &t);
    let candidate_without_corr = count_ccx(&b.ops[start..]);
    let threshold_start = b.ops.len();
    xor_fast_modulo_multihalve_threshold_flag_for_cost(&mut b, &v, k, t[0]);
    let threshold_ccx = count_ccx(&b.ops[threshold_start..]);
    let candidate_exact = count_ccx(&b.ops[start..]);
    (
        current,
        candidate_without_corr,
        candidate_exact,
        threshold_ccx,
    )
}

pub(super) fn direct_fast_modulo_multihalve_chunk_cost_split_peak(
    k: usize,
) -> (usize, usize, usize, usize) {
    let n = N;
    let c_low = 977u64;
    let mask = (1u64 << k) - 1;
    let c_inv = inv_mod_u64_pow2_for_cost(c_low & mask, k);

    let mut b = B::new();
    let v = b.alloc_qubits(n);
    let t = b.alloc_qubits(k);
    let small_prod_bits = k + 10; // t*977 < 2^(k+10)
    let small_prod = b.alloc_qubits(small_prod_bits);
    let start = b.ops.len();

    for i in 0..k {
        let ci = ((c_inv as u128) << i) as u64 & mask;
        super::cadd_nbit_const_direct_fast(&mut b, &t, alloy_primitives::U256::from(ci), v[i]);
    }
    for i in 0..k {
        let ci = ((c_low as u128) << i) as u64 & mask;
        super::csub_nbit_const_direct_fast(&mut b, &v[..k], alloy_primitives::U256::from(ci), t[i]);
    }
    for i in 0..k {
        let ci = alloy_primitives::U256::from(c_low) << i;
        super::cadd_nbit_const_direct_fast(&mut b, &small_prod, ci, t[i]);
    }
    let high = b.alloc_qubits(n);
    let shift = 32usize - k;
    for j in 0..k {
        b.cx(t[j], high[j + shift]);
    }
    for j in k..small_prod_bits {
        b.cx(small_prod[j], high[j - k]);
    }
    super::sub_nbit_qq_fast(&mut b, &high, &v);
    for j in k..small_prod_bits {
        b.cx(small_prod[j], high[j - k]);
    }
    for j in 0..k {
        b.cx(t[j], high[j + shift]);
    }
    b.free_vec(&high);
    for i in (0..k).rev() {
        let ci = alloy_primitives::U256::from(c_low) << i;
        super::csub_nbit_const_direct_fast(&mut b, &small_prod, ci, t[i]);
    }
    b.free_vec(&small_prod);
    super::sub_nbit_qq_fast(&mut b, &v[n - k..], &t);
    let candidate_without_corr = count_ccx(&b.ops[start..]);
    let no_threshold_peak = b.peak_qubits as usize;

    let threshold_start = b.ops.len();
    xor_fast_modulo_multihalve_threshold_flag_for_cost(&mut b, &v, k, t[0]);
    let candidate_exact = count_ccx(&b.ops[start..]);
    let exact_peak = b.peak_qubits as usize;
    let _threshold_ccx = count_ccx(&b.ops[threshold_start..]);
    (
        candidate_without_corr,
        no_threshold_peak,
        candidate_exact,
        exact_peak.max(no_threshold_peak),
    )
}

fn new_builder_with_reg(n: usize) -> (B, Vec<QubitId>) {
    let mut b = B::new();
    let r = b.alloc_qubits(n);
    (b, r)
}

#[test]
fn direct_fast_modulo_multihalve_chunk_cost_probe() {
    let (cur22, cand22_no_corr, cand22_exact, thr22) = direct_fast_modulo_multihalve_chunk_cost(22);
    let (cur8, cand8_no_corr, cand8_exact, thr8) = direct_fast_modulo_multihalve_chunk_cost(8);
    let (_cur22s, split22_no_corr, split22_exact, split_thr22) =
        direct_fast_modulo_multihalve_chunk_cost_split(22);
    let (_cur8s, split8_no_corr, split8_exact, split_thr8) =
        direct_fast_modulo_multihalve_chunk_cost_split(8);
    let projected_current_404 = 18 * cur22 + cur8;
    let projected_exact_404 = 18 * cand22_exact + cand8_exact;
    let projected_saving_404 = projected_current_404 as isize - projected_exact_404 as isize;
    let projected_split_exact_404 = 18 * split22_exact + split8_exact;
    let projected_split_saving_404 =
        projected_current_404 as isize - projected_split_exact_404 as isize;
    let projected_split_no_threshold_404 = 18 * split22_no_corr + split8_no_corr;
    let projected_split_no_threshold_saving_404 =
        projected_current_404 as isize - projected_split_no_threshold_404 as isize;
    let projected_split_no_threshold_roundtrip_saving_404 =
        2 * projected_split_no_threshold_saving_404;
    eprintln!(
        "direct FastModulo multihalve cost: cur22={cur22}, cand22_no_corr={cand22_no_corr}, cand22_exact={cand22_exact}, thr22={thr22}, split22_no_corr={split22_no_corr}, split22_exact={split22_exact}, split_thr22={split_thr22}, cur8={cur8}, cand8_no_corr={cand8_no_corr}, cand8_exact={cand8_exact}, thr8={thr8}, split8_no_corr={split8_no_corr}, split8_exact={split8_exact}, split_thr8={split_thr8}, projected_saving_404={projected_saving_404}, projected_split_saving_404={projected_split_saving_404}, projected_split_no_threshold_saving_404={projected_split_no_threshold_saving_404}"
    );
    println!("METRIC fast_modulo_multihalve_cur22_ccx={cur22}");
    println!("METRIC fast_modulo_multihalve_cand22_no_threshold_ccx={cand22_no_corr}");
    println!("METRIC fast_modulo_multihalve_cand22_exact_ccx={cand22_exact}");
    println!("METRIC fast_modulo_multihalve_threshold22_ccx={thr22}");
    println!("METRIC fast_modulo_multihalve_split22_no_threshold_ccx={split22_no_corr}");
    println!("METRIC fast_modulo_multihalve_split22_exact_ccx={split22_exact}");
    println!("METRIC fast_modulo_multihalve_split_threshold22_ccx={split_thr22}");
    println!("METRIC fast_modulo_multihalve_cur8_ccx={cur8}");
    println!("METRIC fast_modulo_multihalve_cand8_no_threshold_ccx={cand8_no_corr}");
    println!("METRIC fast_modulo_multihalve_cand8_exact_ccx={cand8_exact}");
    println!("METRIC fast_modulo_multihalve_threshold8_ccx={thr8}");
    println!("METRIC fast_modulo_multihalve_split8_no_threshold_ccx={split8_no_corr}");
    println!("METRIC fast_modulo_multihalve_split8_exact_ccx={split8_exact}");
    println!("METRIC fast_modulo_multihalve_split_threshold8_ccx={split_thr8}");
    println!("METRIC fast_modulo_multihalve_exact_projected_saving_404_ccx={projected_saving_404}");
    println!("METRIC fast_modulo_multihalve_split_exact_projected_saving_404_ccx={projected_split_saving_404}");
    println!("METRIC fast_modulo_multihalve_split_no_threshold_projected_saving_404_ccx={projected_split_no_threshold_saving_404}");
    println!("METRIC fast_modulo_multihalve_split_no_threshold_roundtrip_saving_404_ccx={projected_split_no_threshold_roundtrip_saving_404}");
    println!(
        "METRIC fast_modulo_multihalve_split_no_threshold_history_bits_404={}",
        19
    );

    let mut split_exact_by_k = vec![0usize; 23];
    let mut split_no_threshold_by_k = vec![0usize; 23];
    let mut best_single_k = 0usize;
    let mut best_single_saving = isize::MIN;
    for k in 1..=22 {
        let (cur, no_corr, exact, threshold) = direct_fast_modulo_multihalve_chunk_cost_split(k);
        split_exact_by_k[k] = exact;
        split_no_threshold_by_k[k] = no_corr;
        let saving = cur as isize - exact as isize;
        eprintln!("  k={k:2}: cur={cur:5}, split_exact={exact:5}, threshold={threshold:5}, saving={saving:6}");
        if saving > best_single_saving {
            best_single_saving = saving;
            best_single_k = k;
        }
    }
    let inf = usize::MAX / 4;
    let best_chunking = |cost_by_k: &[usize], len: usize| -> (usize, isize, [usize; 23]) {
        let mut dp = vec![inf; len + 1];
        let mut prev = vec![0usize; len + 1];
        dp[0] = 0;
        for i in 1..=len {
            for k in 1..=22.min(i) {
                let cand = dp[i - k].saturating_add(cost_by_k[k]);
                if cand < dp[i] {
                    dp[i] = cand;
                    prev[i] = k;
                }
            }
        }
        let mut counts = [0usize; 23];
        let mut i = len;
        while i > 0 {
            let k = prev[i];
            counts[k] += 1;
            i -= k;
        }
        let current = 255usize * len;
        let saving = current as isize - dp[len] as isize;
        (dp[len], saving, counts)
    };
    let (exact_dp_cost, exact_dp_saving, exact_counts) = best_chunking(&split_exact_by_k, 404);
    let (hist_dp_cost, hist_dp_saving, hist_counts) = best_chunking(&split_no_threshold_by_k, 404);
    let (hist401_dp_cost, hist401_dp_saving, hist401_counts) =
        best_chunking(&split_no_threshold_by_k, 401);
    let hist_dp_history_bits: usize = hist_counts.iter().sum();
    let hist401_dp_history_bits: usize = hist401_counts.iter().sum();
    let total_state_a2_roundtrip_saving = 2 * (hist_dp_saving + hist401_dp_saving);
    eprintln!("best exact split chunking for 404 halvings: cost={exact_dp_cost}, saving={exact_dp_saving}, counts={:?}", &exact_counts[1..]);
    eprintln!("best history-carry split chunking for 404 halvings: cost={hist_dp_cost}, saving={hist_dp_saving}, roundtrip_saving={}, history_bits={hist_dp_history_bits}, counts={:?}", 2 * hist_dp_saving, &hist_counts[1..]);
    eprintln!("best history-carry split chunking for 401 halvings/doubles: cost={hist401_dp_cost}, saving={hist401_dp_saving}, roundtrip_saving={}, history_bits={hist401_dp_history_bits}, counts={:?}", 2 * hist401_dp_saving, &hist401_counts[1..]);
    println!("METRIC fast_modulo_multihalve_split_exact_best_single_k={best_single_k}");
    println!("METRIC fast_modulo_multihalve_split_exact_best_single_saving_ccx={best_single_saving}");
    println!("METRIC fast_modulo_multihalve_split_exact_best_dp_cost_404_ccx={exact_dp_cost}");
    println!("METRIC fast_modulo_multihalve_split_exact_best_dp_saving_404_ccx={exact_dp_saving}");
    println!("METRIC fast_modulo_multihalve_split_history_best_dp_cost_404_ccx={hist_dp_cost}");
    println!("METRIC fast_modulo_multihalve_split_history_best_dp_saving_404_ccx={hist_dp_saving}");
    println!(
        "METRIC fast_modulo_multihalve_split_history_best_dp_roundtrip_saving_404_ccx={}",
        2 * hist_dp_saving
    );
    println!(
        "METRIC fast_modulo_multihalve_split_history_best_dp_history_bits_404={hist_dp_history_bits}"
    );
    println!("METRIC fast_modulo_multihalve_split_history_best_dp_cost_401_ccx={hist401_dp_cost}");
    println!("METRIC fast_modulo_multihalve_split_history_best_dp_saving_401_ccx={hist401_dp_saving}");
    println!(
        "METRIC fast_modulo_multihalve_split_history_best_dp_roundtrip_saving_401_ccx={}",
        2 * hist401_dp_saving
    );
    println!("METRIC fast_modulo_multihalve_split_history_best_dp_history_bits_401={hist401_dp_history_bits}");
    println!("METRIC fast_modulo_multihalve_split_history_state_a2_roundtrip_saving_ccx={total_state_a2_roundtrip_saving}");
    println!(
        "METRIC fast_modulo_multihalve_split_history_state_a2_history_bits={}",
        hist_dp_history_bits + hist401_dp_history_bits
    );
    assert!(projected_split_saving_404 > projected_saving_404);
}

#[test]
fn cost_mul_write_schoolbook_n256() {
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let x = b.alloc_qubits(N);
    let y = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_mul_write_into_zero_acc_schoolbook(&mut b, &acc, &x, &y, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_mul_write_into_zero_acc_schoolbook(n=256): {} CCX", ccx);
}

#[test]
fn cost_mul_add_schoolbook_n256() {
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let x = b.alloc_qubits(N);
    let y = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_mul_add_into_acc_schoolbook(&mut b, &acc, &x, &y, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_mul_add_into_acc_schoolbook(n=256): {} CCX", ccx);
}

#[test]
fn cost_mul_sub_qq_n256() {
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let x = b.alloc_qubits(N);
    let y = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_mul_sub_qq(&mut b, &acc, &x, &y, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_mul_sub_qq(n=256): {} CCX", ccx);
}

#[test]
fn cost_sub_qb_n256() {
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let bits = b.alloc_bits(N);
    let start = b.ops.len();
    mod_sub_qb(&mut b, &acc, &bits, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_sub_qb(n=256): {} CCX", ccx);
}

#[test]
fn cost_neg_inplace_fast_n256() {
    let (mut b, r) = new_builder_with_reg(N);
    let p = SECP256K1_P;
    let start = b.ops.len();
    mod_neg_inplace_fast(&mut b, &r, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_neg_inplace_fast(n=256): {} CCX", ccx);
}
#[test]
fn cost_squaring_sub_n256() {
    use super::*;
    use crate::circuit::OperationType;
    fn count_ccx(ops: &[crate::circuit::Op]) -> usize {
        ops.iter()
            .filter(|o| matches!(o.kind, OperationType::CCX | OperationType::CCZ))
            .count()
    }
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let x = b.alloc_qubits(N);
    let start = b.ops.len();
    // mod_mul_sub_qq with same register is a squaring
    mod_mul_sub_qq(&mut b, &acc, &x, &x, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("squaring via mod_mul_sub_qq: {} CCX", ccx);
}

#[test]
fn fermat_fixed_chain_inversion_floor_misses_sota_by_order() {
    // Branchless inversion by Fermat/exponentiation is the obvious way to avoid
    // Euclidean branch histories.  But even an unrealistically optimal addition
    // chain for an exponent near 2^256 needs at least 255 modular
    // square/multiply layers (each layer can at most double the exponent).  With
    // the measured current n=256 modular-square floor, this is already tens of
    // millions of CCX per inverse before any Bennett cleanup, scratch pressure,
    // or the second point-add denominator.  So fixed-sequence exponentiation is
    // not the missing SOTA-shaped DIV/IMUL primitive.
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let x = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_mul_sub_qq(&mut b, &acc, &x, &x, p);
    let square_ccx = count_ccx(&b.ops[start..]);
    let chain_layer_lower_bound = 255usize;
    let inv_floor = square_ccx * chain_layer_lower_bound;
    println!("METRIC fermat_inv_square_floor_ccx={square_ccx}");
    println!("METRIC fermat_inv_chain_floor_ccx={inv_floor}");
    eprintln!(
        "Fermat inversion floor: square_ccx={square_ccx}, layers>={chain_layer_lower_bound}, inv_floor={inv_floor}"
    );
    assert!(inv_floor > 30_000_000);
}

#[test]
fn cost_halve_double_n256() {
    let mut b = B::new();
    let p = SECP256K1_P;
    let v = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_halve_inplace_fast(&mut b, &v, p);
    let mid = b.ops.len();
    mod_double_inplace_fast(&mut b, &v, p);
    let end = b.ops.len();
    let halve_ccx = count_ccx(&b.ops[start..mid]);
    let double_ccx = count_ccx(&b.ops[mid..end]);
    eprintln!("mod_halve_inplace_fast(n=256): {} CCX", halve_ccx);
    eprintln!("mod_double_inplace_fast(n=256): {} CCX", double_ccx);
}

#[test]
fn round700_selected_dyadic_primitive_costs_n256() {
    let p = SECP256K1_P;

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_add_qq_fast(&mut b, &acc, &a, p);
    let mod_add_fast = count_ccx(&b.ops[start..]);

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_sub_qq_fast(&mut b, &acc, &a, p);
    let mod_sub_fast = count_ccx(&b.ops[start..]);

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let start = b.ops.len();
    cmod_add_qq(&mut b, &acc, &a, ctrl, p);
    let cmod_add_hmr = count_ccx(&b.ops[start..]);

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let start = b.ops.len();
    cmod_sub_qq(&mut b, &acc, &a, ctrl, p);
    let cmod_sub_hmr = count_ccx(&b.ops[start..]);

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let start = b.ops.len();
    super::coordinate_addition_cmod_add_qq_exact_for_bench(&mut b, &acc, &a, ctrl, p);
    let cmod_add_exact = count_ccx(&b.ops[start..]);

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let start = b.ops.len();
    super::coordinate_addition_cmod_sub_qq_exact_for_bench(&mut b, &acc, &a, ctrl, p);
    let cmod_sub_exact = count_ccx(&b.ops[start..]);

    let mut b = B::new();
    let v = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let start = b.ops.len();
    cmod_halve_inplace(&mut b, &v, p, ctrl);
    let cmod_halve = count_ccx(&b.ops[start..]);

    println!("METRIC round700_mod_add_qq_fast_ccx={mod_add_fast}");
    println!("METRIC round700_mod_sub_qq_fast_ccx={mod_sub_fast}");
    println!("METRIC round700_cmod_add_qq_hmr_ccx={cmod_add_hmr}");
    println!("METRIC round700_cmod_sub_qq_hmr_ccx={cmod_sub_hmr}");
    println!("METRIC round700_cmod_add_qq_exact_ccx={cmod_add_exact}");
    println!("METRIC round700_cmod_sub_qq_exact_ccx={cmod_sub_exact}");
    println!("METRIC round700_cmod_halve_inplace_ccx={cmod_halve}");

    assert_eq!(cmod_add_hmr, mod_add_fast + N);
    assert_eq!(cmod_sub_hmr, mod_sub_fast + N);
    assert!(cmod_add_hmr < cmod_add_exact);
    assert!(cmod_sub_hmr < cmod_sub_exact);
}

#[test]
fn round702_sign_selected_nonmodular_addsub_is_toffoli_free_over_add() {
    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let subtract = b.alloc_qubit();
    let start = b.ops.len();
    addsub_nbit_qq_fast_sign_for_cost(&mut b, &a, &acc, subtract);
    let addsub = count_ccx(&b.ops[start..]);

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let start = b.ops.len();
    super::add_nbit_qq_fast(&mut b, &a, &acc);
    let add = count_ccx(&b.ops[start..]);

    println!("METRIC round702_addsub_nbit_qq_fast_sign_ccx={addsub}");
    println!("METRIC round702_add_nbit_qq_fast_ccx={add}");
    assert_eq!(add, N - 1);
    assert_eq!(addsub, add);
}

#[test]
fn round704_sign_selected_cmod_addsub_two_cmods_is_correct_on_basis() {
    use crate::sim::Simulator;
    use sha3::{
        digest::{ExtendableOutput, Update, XofReader},
        Shake128,
    };

    fn set_reg<R: XofReader>(
        sim: &mut Simulator<'_, R>,
        qs: &[QubitId],
        val: alloy_primitives::U256,
        shot: usize,
    ) {
        let limbs = val.as_limbs();
        for (i, &q) in qs.iter().enumerate() {
            if ((limbs[i / 64] >> (i % 64)) & 1) != 0 {
                *sim.qubit_mut(q) |= 1u64 << shot;
            } else {
                *sim.qubit_mut(q) &= !(1u64 << shot);
            }
        }
    }

    fn get_reg<R: XofReader>(
        sim: &Simulator<'_, R>,
        qs: &[QubitId],
        shot: usize,
    ) -> alloy_primitives::U256 {
        let mut limbs = [0u64; 4];
        for (i, &q) in qs.iter().enumerate() {
            limbs[i / 64] |= ((sim.qubit(q) >> shot) & 1) << (i % 64);
        }
        alloy_primitives::U256::from_limbs(limbs)
    }

    fn case_acc(i: usize, p: alloy_primitives::U256) -> alloy_primitives::U256 {
        match i % 8 {
            0 => alloy_primitives::U256::ZERO,
            1 => alloy_primitives::U256::from(1u64),
            2 => alloy_primitives::U256::from(17u64),
            3 => p - alloy_primitives::U256::from(1u64),
            4 => p - alloy_primitives::U256::from(2u64),
            5 => p - alloy_primitives::U256::from(1000u64),
            6 => {
                alloy_primitives::U256::from_limbs([
                    0x9e37_79b9_7f4a_7c15,
                    0xd1b5_4a32_d192_ed03,
                    0x94d0_49bb_1331_11eb,
                    0x2545_f491_4f6c_dd1d,
                ]) % p
            }
            _ => {
                alloy_primitives::U256::from_limbs([
                    0xbf58_476d_1ce4_e5b9,
                    0x94d0_49bb_1331_11eb,
                    0xdbe6_d5d5_fe4c_ce2f,
                    0x9e37_79b9_7f4a_7c15,
                ]) % p
            }
        }
    }

    fn case_a(i: usize, p: alloy_primitives::U256) -> alloy_primitives::U256 {
        match i % 8 {
            0 => alloy_primitives::U256::ZERO,
            1 => alloy_primitives::U256::from(1u64),
            2 => alloy_primitives::U256::from(29u64),
            3 => p - alloy_primitives::U256::from(1u64),
            4 => alloy_primitives::U256::from(1001u64),
            5 => p - alloy_primitives::U256::from(999u64),
            6 => {
                alloy_primitives::U256::from_limbs([
                    0x2545_f491_4f6c_dd1d,
                    0x9e37_79b9_7f4a_7c15,
                    0xd1b5_4a32_d192_ed03,
                    0x94d0_49bb_1331_11eb,
                ]) % p
            }
            _ => {
                alloy_primitives::U256::from_limbs([
                    0xdbe6_d5d5_fe4c_ce2f,
                    0xbf58_476d_1ce4_e5b9,
                    0x9e37_79b9_7f4a_7c15,
                    0xd1b5_4a32_d192_ed03,
                ]) % p
            }
        }
    }

    fn expected(
        acc: alloy_primitives::U256,
        a: alloy_primitives::U256,
        ctrl: u64,
        subtract: u64,
        p: alloy_primitives::U256,
    ) -> alloy_primitives::U256 {
        if ctrl == 0 {
            acc
        } else if subtract == 0 {
            let neg_a = if a.is_zero() {
                alloy_primitives::U256::ZERO
            } else {
                p - a
            };
            if acc >= neg_a {
                acc - neg_a
            } else {
                acc + a
            }
        } else if acc >= a {
            acc - a
        } else {
            p - (a - acc)
        }
    }

    let p = SECP256K1_P;
    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let subtract = b.alloc_qubit();
    cmod_addsub_qq_two_cmods_for_cost(&mut b, &acc, &a, ctrl, subtract, p);

    let mut seed = Shake128::default();
    seed.update(b"round704-sign-selected-cmod-addsub");
    let mut xof = seed.finalize_xof();
    let mut sim = Simulator::new(b.next_qubit as usize, b.next_bit as usize, &mut xof);
    for shot in 0..64usize {
        let acc_v = case_acc(shot, p);
        let a_v = case_a(shot / 2, p);
        let ctrl_v = (shot & 1) as u64;
        let subtract_v = ((shot >> 1) & 1) as u64;
        set_reg(&mut sim, &acc, acc_v, shot);
        set_reg(&mut sim, &a, a_v, shot);
        if ctrl_v != 0 {
            *sim.qubit_mut(ctrl) |= 1u64 << shot;
        }
        if subtract_v != 0 {
            *sim.qubit_mut(subtract) |= 1u64 << shot;
        }
    }
    sim.apply(&b.ops);
    assert_eq!(
        sim.global_phase(),
        0,
        "sign-selected cmod addsub left phase garbage"
    );
    for shot in 0..64usize {
        let acc_v = case_acc(shot, p);
        let a_v = case_a(shot / 2, p);
        let ctrl_v = (shot & 1) as u64;
        let subtract_v = ((shot >> 1) & 1) as u64;
        assert_eq!(
            get_reg(&sim, &acc, shot),
            expected(acc_v, a_v, ctrl_v, subtract_v, p),
            "acc shot {shot}"
        );
        assert_eq!(get_reg(&sim, &a, shot), a_v, "a shot {shot}");
        assert_eq!((sim.qubit(ctrl) >> shot) & 1, ctrl_v, "ctrl shot {shot}");
        assert_eq!(
            (sim.qubit(subtract) >> shot) & 1,
            subtract_v,
            "subtract shot {shot}"
        );
    }
}

#[test]
fn round710_sign_selected_cmod_addsub_fused_negated_add_is_correct_on_basis() {
    use crate::sim::Simulator;
    use sha3::{
        digest::{ExtendableOutput, Update, XofReader},
        Shake128,
    };

    fn set_reg<R: XofReader>(
        sim: &mut Simulator<'_, R>,
        qs: &[QubitId],
        val: alloy_primitives::U256,
        shot: usize,
    ) {
        let limbs = val.as_limbs();
        for (i, &q) in qs.iter().enumerate() {
            if ((limbs[i / 64] >> (i % 64)) & 1) != 0 {
                *sim.qubit_mut(q) |= 1u64 << shot;
            } else {
                *sim.qubit_mut(q) &= !(1u64 << shot);
            }
        }
    }

    fn get_reg<R: XofReader>(
        sim: &Simulator<'_, R>,
        qs: &[QubitId],
        shot: usize,
    ) -> alloy_primitives::U256 {
        let mut limbs = [0u64; 4];
        for (i, &q) in qs.iter().enumerate() {
            limbs[i / 64] |= ((sim.qubit(q) >> shot) & 1) << (i % 64);
        }
        alloy_primitives::U256::from_limbs(limbs)
    }

    fn case_acc(i: usize, p: alloy_primitives::U256) -> alloy_primitives::U256 {
        match i % 8 {
            0 => alloy_primitives::U256::ZERO,
            1 => alloy_primitives::U256::from(1u64),
            2 => alloy_primitives::U256::from(17u64),
            3 => p - alloy_primitives::U256::from(1u64),
            4 => p - alloy_primitives::U256::from(2u64),
            5 => p - alloy_primitives::U256::from(1000u64),
            6 => {
                alloy_primitives::U256::from_limbs([
                    0x9e37_79b9_7f4a_7c15,
                    0xd1b5_4a32_d192_ed03,
                    0x94d0_49bb_1331_11eb,
                    0x2545_f491_4f6c_dd1d,
                ]) % p
            }
            _ => {
                alloy_primitives::U256::from_limbs([
                    0xbf58_476d_1ce4_e5b9,
                    0x94d0_49bb_1331_11eb,
                    0xdbe6_d5d5_fe4c_ce2f,
                    0x9e37_79b9_7f4a_7c15,
                ]) % p
            }
        }
    }

    fn case_a(i: usize, p: alloy_primitives::U256) -> alloy_primitives::U256 {
        match i % 8 {
            0 => alloy_primitives::U256::ZERO,
            1 => alloy_primitives::U256::from(1u64),
            2 => alloy_primitives::U256::from(29u64),
            3 => p - alloy_primitives::U256::from(1u64),
            4 => alloy_primitives::U256::from(1001u64),
            5 => p - alloy_primitives::U256::from(999u64),
            6 => {
                alloy_primitives::U256::from_limbs([
                    0x2545_f491_4f6c_dd1d,
                    0x9e37_79b9_7f4a_7c15,
                    0xd1b5_4a32_d192_ed03,
                    0x94d0_49bb_1331_11eb,
                ]) % p
            }
            _ => {
                alloy_primitives::U256::from_limbs([
                    0xdbe6_d5d5_fe4c_ce2f,
                    0xbf58_476d_1ce4_e5b9,
                    0x9e37_79b9_7f4a_7c15,
                    0xd1b5_4a32_d192_ed03,
                ]) % p
            }
        }
    }

    fn expected(
        acc: alloy_primitives::U256,
        a: alloy_primitives::U256,
        ctrl: u64,
        subtract: u64,
        p: alloy_primitives::U256,
    ) -> alloy_primitives::U256 {
        if ctrl == 0 {
            acc
        } else if subtract == 0 {
            let neg_a = if a.is_zero() {
                alloy_primitives::U256::ZERO
            } else {
                p - a
            };
            if acc >= neg_a {
                acc - neg_a
            } else {
                acc + a
            }
        } else if acc >= a {
            acc - a
        } else {
            p - (a - acc)
        }
    }

    let p = SECP256K1_P;
    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let subtract = b.alloc_qubit();
    cmod_addsub_qq_fused_negated_add_for_cost(&mut b, &acc, &a, ctrl, subtract, p);

    let mut seed = Shake128::default();
    seed.update(b"round710-sign-selected-cmod-addsub-fused-negated-add");
    let mut xof = seed.finalize_xof();
    let mut sim = Simulator::new(b.next_qubit as usize, b.next_bit as usize, &mut xof);
    for shot in 0..64usize {
        let acc_v = case_acc(shot, p);
        let a_v = case_a(shot / 2, p);
        let ctrl_v = (shot & 1) as u64;
        let subtract_v = ((shot >> 1) & 1) as u64;
        set_reg(&mut sim, &acc, acc_v, shot);
        set_reg(&mut sim, &a, a_v, shot);
        if ctrl_v != 0 {
            *sim.qubit_mut(ctrl) |= 1u64 << shot;
        }
        if subtract_v != 0 {
            *sim.qubit_mut(subtract) |= 1u64 << shot;
        }
    }
    sim.apply(&b.ops);
    assert_eq!(
        sim.global_phase(),
        0,
        "fused sign-selected cmod addsub left phase garbage"
    );
    for shot in 0..64usize {
        let acc_v = case_acc(shot, p);
        let a_v = case_a(shot / 2, p);
        let ctrl_v = (shot & 1) as u64;
        let subtract_v = ((shot >> 1) & 1) as u64;
        assert_eq!(
            get_reg(&sim, &acc, shot),
            expected(acc_v, a_v, ctrl_v, subtract_v, p),
            "acc shot {shot}"
        );
        assert_eq!(get_reg(&sim, &a, shot), a_v, "a shot {shot}");
        assert_eq!((sim.qubit(ctrl) >> shot) & 1, ctrl_v, "ctrl shot {shot}");
        assert_eq!(
            (sim.qubit(subtract) >> shot) & 1,
            subtract_v,
            "subtract shot {shot}"
        );
    }
}

#[test]
fn round704_sign_selected_cmod_addsub_cost_probe() {
    let p = SECP256K1_P;

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let subtract = b.alloc_qubit();
    let start = b.ops.len();
    cmod_addsub_qq_two_cmods_for_cost(&mut b, &acc, &a, ctrl, subtract, p);
    let two_cmods = count_ccx(&b.ops[start..]);
    let two_cmods_peak = b.peak_qubits as usize;

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let subtract = b.alloc_qubit();
    let start = b.ops.len();
    cmod_addsub_qq_fused_negated_add_for_cost(&mut b, &acc, &a, ctrl, subtract, p);
    let fused_negated_add = count_ccx(&b.ops[start..]);
    let fused_negated_add_peak = b.peak_qubits as usize;

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let subtract = b.alloc_qubit();
    let start = b.ops.len();
    raw_controlled_addsub_nbit_qq_sign_lower_bound_for_cost(&mut b, &acc, &a, ctrl, subtract);
    let raw_lower_bound = count_ccx(&b.ops[start..]);
    let raw_lower_bound_peak = b.peak_qubits as usize;

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let start = b.ops.len();
    cmod_add_qq(&mut b, &acc, &a, ctrl, p);
    let cmod_add = count_ccx(&b.ops[start..]);

    let mut b = B::new();
    let acc = b.alloc_qubits(N);
    let a = b.alloc_qubits(N);
    let ctrl = b.alloc_qubit();
    let start = b.ops.len();
    cmod_sub_qq(&mut b, &acc, &a, ctrl, p);
    let cmod_sub = count_ccx(&b.ops[start..]);

    let selector_exact_ccx = 4usize;
    println!("METRIC round704_sign_selected_cmod_addsub_two_cmods_ccx={two_cmods}");
    println!("METRIC round704_sign_selected_cmod_addsub_two_cmods_peak_qubits={two_cmods_peak}");
    println!("METRIC round710_sign_selected_cmod_addsub_fused_negated_add_ccx={fused_negated_add}");
    println!(
        "METRIC round710_sign_selected_cmod_addsub_fused_negated_add_peak_qubits={fused_negated_add_peak}"
    );
    println!("METRIC round704_sign_selected_cmod_addsub_selector_exact_ccx={selector_exact_ccx}");
    println!("METRIC round704_raw_controlled_addsub_sign_lower_bound_ccx={raw_lower_bound}");
    println!(
        "METRIC round704_raw_controlled_addsub_sign_lower_bound_peak_qubits={raw_lower_bound_peak}"
    );
    println!("METRIC round704_raw_controlled_addsub_sign_missing_modular_correction=1");

    assert_eq!(two_cmods, cmod_add + cmod_sub + selector_exact_ccx);
    assert!(fused_negated_add < two_cmods);
    assert_eq!(raw_lower_bound, N + (N - 1));
}

#[test]
fn chunked_shift_prescaler_reopens_small_scale_absorption_but_not_qubit_gate() {
    // Scale absorption deletes a ~iters-long halve/double correction loop if we
    // initialize Eea with 2^iters*x.  The constants are sparse for secp256k1,
    // e.g. 2^404 = 2^148(2^32+977), so try a custom constant multiplier that
    // jumps between sparse set-bit positions with the FastModulo k-bit shifter
    // instead of walking through every intermediate double.  This beats the old
    // mixed prescaler locally and is just below the correction-loop cost for the
    // current state_a/state_b iteration counts.  Borrowing the source register as
    // the moving shift lane removes the field-sized temp and lowers folded
    // integration from 3153q to 2897q at the same 4,065,906 average executed
    // Toffoli.  Reusing prescaler scratch as Eea state is phase-unsafe, so
    // this remains an env-gated primitive rather than a promotable default path.
    use super::*;
    let p = SECP256K1_P;
    let x = B::new();
    drop(x);
    for &(iters, label) in &[(404usize, "state_a"), (400usize, "state_b")] {
        let scale = pow_mod_2_k(p, iters);
        let mut b = B::new();
        let src = b.alloc_qubits(N);
        let acc = b.alloc_qubits(N);
        let start = b.ops.len();
        mul_by_const_acc_exact_adds_fast_shifts(&mut b, &src, scale, &acc, p, false);
        let mixed_ccx = count_ccx(&b.ops[start..]);
        let mixed_peak = b.peak_qubits as usize;

        let mut b = B::new();
        let src = b.alloc_qubits(N);
        let acc = b.alloc_qubits(N);
        let start = b.ops.len();
        mul_by_const_acc_chunked_shifts_for_cost(&mut b, &src, scale, &acc, p);
        let chunked_ccx = count_ccx(&b.ops[start..]);
        let chunked_peak = b.peak_qubits as usize;

        let mut b = B::new();
        let src = b.alloc_qubits(N);
        let acc = b.alloc_qubits(N);
        let start = b.ops.len();
        mul_by_const_acc_chunked_inplace_src_for_cost(&mut b, &src, scale, &acc, p);
        let inplace_ccx = count_ccx(&b.ops[start..]);
        let inplace_peak = b.peak_qubits as usize;

        let mut b = B::new();
        let v = b.alloc_qubits(N);
        let start = b.ops.len();
        for _ in 0..iters {
            if label == "state_a" {
                mod_halve_inplace_fast(&mut b, &v, p);
            } else {
                mod_double_inplace_fast(&mut b, &v, p);
            }
        }
        let correction_loop_ccx = count_ccx(&b.ops[start..]);
        let projected_delta = 2isize * inplace_ccx as isize - correction_loop_ccx as isize;
        eprintln!(
            "{label} scale prescaler: mixed_ccx={mixed_ccx}, chunked_ccx={chunked_ccx}, inplace_ccx={inplace_ccx}, correction_loop_ccx={correction_loop_ccx}, projected_delta={projected_delta}"
        );
        println!("METRIC scale_absorb_{label}_mixed_prescale_ccx={mixed_ccx}");
        println!("METRIC scale_absorb_{label}_mixed_prescale_peak_qubits={mixed_peak}");
        println!("METRIC scale_absorb_{label}_chunked_prescale_ccx={chunked_ccx}");
        println!("METRIC scale_absorb_{label}_chunked_prescale_peak_qubits={chunked_peak}");
        println!("METRIC scale_absorb_{label}_chunked_inplace_prescale_ccx={inplace_ccx}");
        println!("METRIC scale_absorb_{label}_chunked_inplace_prescale_peak_qubits={inplace_peak}");
        println!("METRIC scale_absorb_{label}_correction_loop_ccx={correction_loop_ccx}");
        println!("METRIC scale_absorb_{label}_chunked_inplace_projected_delta={projected_delta}");
        assert!(
            inplace_ccx < mixed_ccx / 2,
            "chunked sparse shifts should strongly improve the local prescaler"
        );
        assert!(
            inplace_peak < chunked_peak,
            "in-place source schedule should remove the tmp-register peak"
        );
        assert!(
            projected_delta < 0,
            "chunked compute+uncompute should beat the deleted correction loop locally"
        );
    }
}

#[test]
fn profile_ec_add_by_phase() {
    use crate::circuit::OperationType;
    use std::collections::HashMap;
    let mut b = B::new();
    let p = SECP256K1_P;
    let n = 256;
    let tx = b.alloc_qubits(n);
    let ty = b.alloc_qubits(n);
    let ox = b.alloc_bits(n);
    let oy = b.alloc_bits(n);
    super::build_standard_ec_add(&mut b, &tx, &ty, &ox, &oy, p);

    let mut phase_ccx: HashMap<&str, usize> = HashMap::new();
    let mut current_phase: &str = "(none)";
    let trans = &b.phase_transitions;
    let mut ti = 0;
    for (idx, op) in b.ops.iter().enumerate() {
        while ti < trans.len() && trans[ti].0 <= idx {
            current_phase = trans[ti].1;
            ti += 1;
        }
        if matches!(op.kind, OperationType::CCX | OperationType::CCZ) {
            *phase_ccx.entry(current_phase).or_insert(0) += 1;
        }
    }

    let mut entries: Vec<_> = phase_ccx.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    let mut total = 0usize;
    eprintln!("\n=== Point Add Toffoli Profile by Phase ===");
    for (phase, ccx) in &entries {
        total += ccx;
        eprintln!("{:>10} {}", ccx, phase);
    }
    eprintln!("{:>10} TOTAL", total);
}
