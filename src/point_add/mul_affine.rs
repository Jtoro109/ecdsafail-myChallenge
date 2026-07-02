//! (refactor r2) Mechanically extracted from mul.rs. No logic changes.
use super::*;

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
