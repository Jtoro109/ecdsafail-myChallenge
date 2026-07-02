//! (refactor) Mechanically extracted from mod.rs. No logic changes.
use super::*;

// ═══════════════════════════════════════════════════════════════════════════
//  Loading classical operands into a fresh qubit register
// ═══════════════════════════════════════════════════════════════════════════
//
// Cuccaro needs two qubit registers. To add a classical constant or a
// classical bit register to a quantum register, we allocate a fresh
// qubit register, load the classical value into it, run Cuccaro, then
// unload. The load/unload is not counted against Toffolis.

pub(crate) fn load_const(b: &mut B, n: usize, c: U256) -> Vec<QubitId> {
    let qs = b.alloc_qubits(n);
    for i in 0..n {
        if bit(c, i) {
            b.x(qs[i]);
        }
    }
    qs
}

pub(crate) fn unload_const(b: &mut B, qs: &[QubitId], c: U256) {
    for i in 0..qs.len() {
        if bit(c, i) {
            b.x(qs[i]);
        }
    }
    b.free_vec(qs);
}

pub(crate) fn load_bits(b: &mut B, bits: &[BitId]) -> Vec<QubitId> {
    let n = bits.len();
    let qs = b.alloc_qubits(n);
    for i in 0..n {
        // qs[i] ← bits[i] via conditional X
        b.x_if(qs[i], bits[i]);
    }
    qs
}

pub(crate) fn unload_bits(b: &mut B, qs: &[QubitId], bits: &[BitId]) {
    for i in 0..qs.len() {
        b.x_if(qs[i], bits[i]);
    }
    b.free_vec(qs);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Extended registers and modular reduction
// ═══════════════════════════════════════════════════════════════════════════
//
// All modular arithmetic operates on "extended" registers of width n+1
// where bit n is an overflow/sign ancilla. The primitive quantum
// registers handed to us (Px, Py) are exactly n=256 wide; the extension
// bit is a transient ancilla allocated for the duration of a mod-op.

/// Build an (n+1)-bit view by attaching a freshly-allocated 0 ancilla.
pub(crate) fn ext_reg(b: &mut B, reg: &[QubitId]) -> (Vec<QubitId>, QubitId) {
    let ovf = b.alloc_qubit();
    let mut r = reg.to_vec();
    r.push(ovf);
    (r, ovf)
}

/// Release the overflow ancilla (which must be 0 on exit).
pub(crate) fn unext_reg(b: &mut B, ovf: QubitId) {
    b.free(ovf);
}

/// `acc := (acc + a) mod p`. Both `acc` and `a` are n-bit quantum registers
/// with value in [0, p). Solinas reduction using c = 2^n - p: sum ∈ [0, 2p),
/// then add c, branch on top bit to either clear it (reduction) or undo
/// the add (no reduction). Saves one full (n+1)-wide Cuccaro compared to
/// the sub-p/add-p/csub-p pattern.
pub(crate) fn mod_add_qq(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // Step 1: (n+1)-bit add. acc_ext ∈ [0, 2p).
    add_nbit_qq(b, &a_ext, &acc_ext);

    // Step 2: add c. If sum was >= p, the top bit of (sum + c) becomes 1.
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    add_nbit_const(b, &acc_ext, c);

    // Step 3: flag := acc_ovf (= top bit of sum + c).
    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);

    // Step 4: if flag=0 (no reduction needed), undo the add of c.
    b.x(flag);
    csub_nbit_const(b, &acc_ext, c, flag);
    b.x(flag);

    // Step 5: if flag=1, clear the top bit (drops 2^n → yields sum - p).
    b.cx(flag, acc_ovf);

    // Step 6: uncompute flag. Same identity as the old version:
    //   flag == (acc_final < a_orig)
    // because in the flag=1 case acc_final = acc_orig + a - p < a (since acc_orig < p),
    // and in the flag=0 case acc_final = acc_orig + a ≥ a.
    cmp_lt_into(b, &acc_ext[..n], &a_ext[..n], flag);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// Dirty-borrow variant of `mod_add_qq` (KAL_GZ_SOLINAS_LOWSCRATCH): the two
/// 257-wide loaded-constant registers of the `+c` / conditional `-c` reduction
/// steps are replaced by Gidney venting dirty-borrow const adders (2 clean
/// ancilla + a borrowed n-2 DIRTY donor, restored). Used for the shift22
/// position-32 add inside the affine y-mul Solinas fold, where the loaded-const
/// register was the actual 2333 binder. `dirty` must be co-resident, >= n-1
/// wide, disjoint from `acc` and `a`, and is restored to its entry value.
pub(crate) fn mod_add_qq_dirty(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256, dirty: &[QubitId]) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);
    let m = acc_ext.len(); // n+1

    add_nbit_qq(b, &a_ext, &acc_ext);

    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let c_low = c.as_limbs()[0];

    // Step 2: acc_ext += c  (register-free venting dirty-borrow).
    {
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::iadd_dirty_2clean_classical(b, &acc_ext, &dirty[..m - 2], &q_clean2, c_low, false);
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    }

    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);

    // Step 4: if flag=0 undo the +c == conditional -c controlled by !flag.
    b.x(flag);
    {
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(b, &acc_ext, &dirty[..m - 2], &q_clean2, c_low, flag);
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    }
    b.x(flag);

    b.cx(flag, acc_ovf);

    cmp_lt_into(b, &acc_ext[..n], &a_ext[..n], flag);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

pub(crate) fn mod_sub_qq(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    // mod_add_qq is a bijection on (acc, a): (acc, a) ↦ (acc + a mod p, a).
    // Its gate-level inverse therefore acts as (acc, a) ↦ (acc - a mod p, a),
    // which is exactly what we want. emit_inverse replays the forward's gates
    // reversed, skipping R markers — valid because mod_add_qq is clean
    // (every ancilla is driven to |0⟩ before its R).
    let a_copy: Vec<QubitId> = a.to_vec();
    emit_inverse(b, move |b| mod_add_qq(b, acc, &a_copy, p));
}

/// Fast `acc := (acc - a) mod p`. Direct sub + conditional add-p + flag
/// uncompute via neg+cmp_lt+neg. All ops use measurement-based Cuccaro.
pub(crate) fn mod_sub_qq_fast(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // Step 1: (n+1)-bit sub.
    sub_nbit_qq_fast(b, &a_ext, &acc_ext);

    // Step 2: flag = acc_ovf (=1 iff underflow, i.e. acc < a).
    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    // We only need the borrow as a separate flag; the low register is
    // corrected modulo 2^n, so clear the extension bit immediately.
    b.cx(flag, acc_ovf);

    // Step 3: underflow correction. With p = 2^n - c, the wrapped 256-bit
    // subtraction needs only a conditional subtract of c on the low register.
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    if std::env::var("KAL_VENT_MODADD").ok().as_deref() == Some("1") {
        // Use venting cisub with a_ext as dirty qubits.
        let c_low = c.as_limbs()[0];
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(
            b,
            &acc_ext[..n],
            &a_ext[..n - 2],
            &q_clean2,
            c_low,
            flag,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else {
        csub_nbit_const_fast(b, &acc_ext[..n], c, flag);
    }

    // Step 4: uncompute flag. Identity: flag = NOT(acc_final < (p - a)).
    // Negate a in place, compare, un-negate.
    b.x(flag);
    mod_neg_inplace_fast(b, &a_ext[..n], p);
    cmp_lt_into_fast(b, &acc_ext[..n], &a_ext[..n], flag);
    mod_neg_inplace_fast(b, &a_ext[..n], p);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// Fast mod_neg using measurement-based Cuccaro for the addition.
pub(crate) fn mod_neg_inplace_fast(b: &mut B, v: &[QubitId], p: U256) {
    for &q in v {
        b.x(q);
    }
    let n = v.len();
    let ca = load_const(b, n, p.wrapping_add(U256::from(1)));
    add_nbit_qq_fast(b, &ca, v);
    unload_const(b, &ca, p.wrapping_add(U256::from(1)));
}

/// Register-free mod_neg: `v := (p - v) mod p` via flip + direct const-add of
/// (p+1). Avoids the n-bit `load_const` register of `mod_neg_inplace_fast`,
/// so it adds no n-wide transient scratch (only the direct carry sweep's
/// ancillae). Used in the low-scratch Solinas fold.
pub(crate) fn mod_neg_inplace_direct(b: &mut B, v: &[QubitId], p: U256) {
    for &q in v {
        b.x(q);
    }
    let one = b.alloc_qubit();
    b.x(one);
    cadd_nbit_const_direct_fast(b, v, p.wrapping_add(U256::from(1)), one);
    b.x(one);
    b.free(one);
}

/// Carry-free + register-free `acc := (acc + a) mod p`. Uses the no-carry
/// Cuccaro (`add_nbit_qq`), the register-free direct const adders, and the
/// no-carry comparator (`cmp_lt_into`). Holds ~0 wide transient scratch (only
/// 2 ext-ovf + 1 flag), at +~n Toffoli per call vs the fast variant. Drops the
/// Solinas-fold instant by ~256 (the cuccaro carry register) — the dominant
/// fold-scratch at the affine y-mul binder.
pub(crate) fn mod_add_qq_lowq_lowscratch(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    add_nbit_qq(b, &a_ext, &acc_ext);

    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let one = b.alloc_qubit();
    b.x(one);
    cadd_nbit_const_direct_fast(b, &acc_ext, c, one);
    b.x(one);
    b.free(one);

    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    b.x(flag);
    csub_nbit_const_direct_fast(b, &acc_ext, c, flag);
    b.x(flag);
    b.cx(flag, acc_ovf);
    cmp_lt_into(b, &acc_ext[..n], &a_ext[..n], flag);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// Carry-free + register-free `acc := (acc - a) mod p`. Companion to
/// `mod_add_qq_lowq_lowscratch`.
pub(crate) fn mod_sub_qq_lowq_lowscratch(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    sub_nbit_qq(b, &a_ext, &acc_ext);

    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    b.cx(flag, acc_ovf);

    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    csub_nbit_const_direct_fast(b, &acc_ext[..n], c, flag);

    b.x(flag);
    mod_neg_inplace_direct(b, &a_ext[..n], p);
    cmp_lt_into(b, &acc_ext[..n], &a_ext[..n], flag);
    mod_neg_inplace_direct(b, &a_ext[..n], p);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

pub(crate) fn mod_add_qc(b: &mut B, acc: &[QubitId], c: U256, p: U256) {
    // acc := (acc + c) mod p. c is a compile-time constant.
    let n = acc.len();
    let a = load_const(b, n, c);
    mod_add_qq_fast(b, acc, &a, p);
    unload_const(b, &a, c);
}

pub(crate) fn mod_add_qb(b: &mut B, acc: &[QubitId], bits: &[BitId], p: U256) {
    // acc := (acc + bits) mod p. `bits` is a classical bit register.
    let a = load_bits(b, bits);
    mod_add_qq_fast(b, acc, &a, p);
    unload_bits(b, &a, bits);
}

pub(crate) fn mod_add_double_qb(b: &mut B, acc: &[QubitId], bits: &[BitId], p: U256) {
    // acc := acc + 2*bits mod p. Reuse a single loaded copy of the classical
    // point and walk it through the cheap secp256k1 double/halve pair.
    let a = load_bits(b, bits);
    mod_double_inplace_fast(b, &a, p);
    mod_add_qq_fast(b, acc, &a, p);
    mod_halve_inplace_fast(b, &a, p);
    unload_bits(b, &a, bits);
}

pub(crate) fn mod_sub_double_qb(b: &mut B, acc: &[QubitId], bits: &[BitId], p: U256) {
    // acc := acc - 2*bits mod p. Mirror of mod_add_double_qb.
    let a = load_bits(b, bits);
    mod_double_inplace_fast(b, &a, p);
    mod_sub_qq_fast(b, acc, &a, p);
    mod_halve_inplace_fast(b, &a, p);
    unload_bits(b, &a, bits);
}

pub(crate) fn mod_sub_qb(b: &mut B, acc: &[QubitId], bits: &[BitId], p: U256) {
    // acc -= bits mod p. Uses fast mod_sub_qq via neg+add+neg.
    let a = load_bits(b, bits);
    mod_sub_qq_fast(b, acc, &a, p);
    unload_bits(b, &a, bits);
}


// ═══════════════════════════════════════════════════════════════════════════
//  Modular multiplication
// ═══════════════════════════════════════════════════════════════════════════
//
// Shift-and-add, MSB-to-LSB. `acc += x*y mod p`. Iteration:
//
//     for i from n-1 down to 0:
//         acc := 2*acc mod p
//         if y[i]:  acc := acc + x mod p
//
// For q*q mul, y[i] is a qubit; we implement the conditional add by
// CCX-copying x (gated on y[i]) into a temporary, adding, and
// uncopying. For q*b mul, y[i] is a classical bit and the copy is
// done with CX_if gates.

/// `v := 2*v mod p`. In-place via shift-left (swap cascade) + Solinas-style
/// mod reduction. For secp256k1, p = 2^n - c with c = 2^32 + 977, so
/// `T - p = T + c - 2^n`. The reduction becomes: add c, branch on the top
/// bit of the (n+1)-wide shifted register — if set, clear it; else undo
/// the add. Costs two full (n+1)-wide Cuccaro adds instead of three.
pub(crate) fn mod_double_inplace(b: &mut B, v: &[QubitId], p: U256) {
    let n = v.len();
    let ovf = b.alloc_qubit();

    // Shift left by 1 via swaps: introduces a 0 into v[0], pushes v[n-1] → ovf.
    b.swap(v[n - 1], ovf);
    for i in (0..n - 1).rev() {
        b.swap(v[i], v[i + 1]);
    }

    let mut v_ext: Vec<QubitId> = v.to_vec();
    v_ext.push(ovf);

    // c = 2^n - p (= 2^32 + 977 for secp256k1). Assumes n == 256 so that
    // 2^n wraps cleanly in U256::MAX + 1 arithmetic.
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));

    // S := T + c. Fits in n+1 bits.
    add_nbit_const(b, &v_ext, c);

    // flag := (S >= 2^n) = S[n]. S[n]==1 iff we need the reduction.
    let flag = b.alloc_qubit();
    b.cx(ovf, flag);

    // If flag=0, undo the add (we didn't need to reduce).
    b.x(flag);
    csub_nbit_const(b, &v_ext, c, flag);
    b.x(flag);

    // If flag=1, clear the top bit (drops the 2^n from S, giving T - p).
    b.cx(flag, ovf);

    // Uncompute flag via parity: flag == v[0] after the operation.
    // Case flag=0: v = T = 2*v_orig (even) → v[0]=0.
    // Case flag=1: v = T - p. T even, p odd → v is odd → v[0]=1.
    b.cx(v[0], flag);
    b.free(flag);
    b.free(ovf);
}

/// Fast `v := 2*v mod p` using measurement-based Cuccaro.
pub(crate) fn mod_double_inplace_fast(b: &mut B, v: &[QubitId], p: U256) {
    let n = v.len();
    let ovf = b.alloc_qubit();
    b.swap(v[n - 1], ovf);
    for i in (0..n - 1).rev() {
        b.swap(v[i], v[i + 1]);
    }
    debug_assert_eq!(n, 256);
    // For secp256k1, p = 2^n - c. After the shift, the old top bit is in
    // `ovf` and the low register holds T mod 2^n for T = 2*v. If ovf=1 then
    // T = 2^n + low and T mod p = low + c; otherwise T mod p = low.
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    if std::env::var("KAL_DIRECT_CONST_DOUBLE").ok().as_deref() == Some("1") {
        cadd_nbit_const_direct_fast(b, v, c, ovf);
    } else {
        cadd_nbit_const_fast(b, v, c, ovf);
    }
    // Result parity equals the old top bit: even if ovf=0, odd if ovf=1.
    b.cx(v[0], ovf);
    b.free(ovf);
}

/// `v := 2*v mod p` using the register-free direct const-add (no `load_const`
/// addend register and no measurement-Cuccaro carry register held alongside
/// it). Transient scratch ~n/2 less than `mod_double_inplace_fast`'s
/// `cadd_nbit_const_fast` (which holds a 256-bit const register + 256 add
/// carries simultaneously). Same value semantics; used in the low-scratch
/// Solinas fold where the mod_double instant is the affine y-mul binder.
pub(crate) fn mod_double_inplace_direct(b: &mut B, v: &[QubitId], p: U256) {
    let n = v.len();
    let ovf = b.alloc_qubit();
    b.swap(v[n - 1], ovf);
    for i in (0..n - 1).rev() {
        b.swap(v[i], v[i + 1]);
    }
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    cadd_nbit_const_direct_fast(b, v, c, ovf);
    b.cx(v[0], ovf);
    b.free(ovf);
}

/// `v := 2*v` assuming v[n-1] = 0 (no wrap). Just a shift-left cascade.
/// 0 Toffoli. Used in Kaliski STEP 7+8 for small iters where r[255]=0 guaranteed.
pub(crate) fn mod_double_no_corr(b: &mut B, v: &[QubitId]) {
    let n = v.len();
    for i in (0..n - 1).rev() {
        b.swap(v[i], v[i + 1]);
    }
}

/// `v := v/2` assuming v[0] = 0 (v was even after corresponding no-corr double).
/// Exact inverse of `mod_double_no_corr`. 0 Toffoli.
pub(crate) fn mod_halve_no_corr(b: &mut B, v: &[QubitId]) {
    let n = v.len();
    for i in 0..n - 1 {
        b.swap(v[i], v[i + 1]);
    }
}


/// Fast `v := v/2 mod p`. Explicit reverse of `mod_double_inplace` with
/// measurement-based Cuccaro (not emit_inverse).
pub(crate) fn mod_halve_inplace_fast(b: &mut B, v: &[QubitId], p: U256) {
    mod_halve_inplace_fast_with_dirty(b, v, p, None)
}

/// Variant of `mod_halve_inplace_fast` that optionally borrows `dirty_src`
/// qubits for the controlled-sub step, using Gidney's venting
/// `cisub_dirty_2clean_classical`. Saves n transient qubits at the peak
/// when dirty qubits are available from the caller.
pub(crate) fn mod_halve_inplace_fast_with_dirty(
    b: &mut B,
    v: &[QubitId],
    p: U256,
    dirty_src: Option<&[QubitId]>,
) {
    let n = v.len();
    let ovf = b.alloc_qubit();
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    b.cx(v[0], ovf);
    // If caller provided enough dirty qubits AND c fits in u64 (it does
    // for secp256k1: c = 2^32 + 977), use the venting variant.
    let use_venting = std::env::var("KAL_VENT_HALVE").ok().as_deref() == Some("1")
        && dirty_src.map_or(false, |d| d.len() >= n - 2);
    if use_venting {
        // c as u64 (it fits: c = 0x1000003D1).
        // For n=256, we still need to pass the full 256-bit constant via u64.
        // Since c only has 33 bits, u64 is fine.
        let c_u64: u64 = c.as_limbs()[0] | (c.as_limbs()[1] << 32); // hack for U256
                                                                    // Actually, U256 limbs are u64[4]. Bit 32 of U256 is limbs[0] bit 32.
                                                                    // limbs[0] holds bits 0..64. So just take limbs[0] for bits < 64.
        let c_low = c.as_limbs()[0];
        let dirty = dirty_src.unwrap();
        let dirty_slice = &dirty[..n - 2];
        // We need 2 clean ancilla. Alloc them fresh.
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(b, v, dirty_slice, &q_clean2, c_low, ovf);
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
        let _ = c_u64; // unused, c_low is the right value
    } else if direct_const_halve_enabled() {
        csub_nbit_const_direct_fast(b, v, c, ovf);
    } else {
        csub_nbit_const_fast(b, v, c, ovf);
    }
    for i in 0..n - 1 {
        b.swap(v[i], v[i + 1]);
    }
    b.swap(v[n - 1], ovf);
    b.free(ovf);
}

/// `v := v/2 mod p`. Gate-inverse of `mod_double_inplace`.
pub(crate) fn mod_halve_inplace(b: &mut B, v: &[QubitId], p: U256) {
    let v_copy: Vec<QubitId> = v.to_vec();
    emit_inverse(b, move |b| mod_double_inplace(b, &v_copy, p));
}

// ═══════════════════════════════════════════════════════════════════════════
//  Conditional modular add/sub helpers
// ═══════════════════════════════════════════════════════════════════════════
//
// Used by the multipliers. Each variant loads `(ctrl ? a : 0)` into a
// fresh temporary via CCX or CX_if, runs the unconditional mod_add_qq /
// mod_sub_qq, then unloads.

/// Like `cmp_lt_into` but uses carry-ancilla + measurement-based uncompute
/// for the inv_MAJ sweep. Saves n CCX. NOT emit_inverse-safe.
pub(crate) fn cmp_lt_into_fast(b: &mut B, u: &[QubitId], v: &[QubitId], flag: QubitId) {
    // KAL_VENT_MODADD=1 uses the slow (no-carries) comparator which
    // saves n peak qubits at cost of ~n CCX per call.
    if std::env::var("KAL_VENT_MODADD").ok().as_deref() == Some("1") {
        cmp_lt_into(b, u, v, flag);
        return;
    }
    let n = u.len();
    assert_eq!(n, v.len());
    let c_in = b.alloc_qubit();
    let carries = b.alloc_qubits(n);
    for i in 0..n {
        b.x(u[i]);
    }

    // Forward MAJ sweep with carry ancillae
    b.cx(u[0], v[0]);
    b.cx(u[0], c_in);
    b.ccx(c_in, v[0], carries[0]);
    b.cx(carries[0], u[0]);
    for i in 1..n {
        b.cx(u[i], v[i]);
        b.cx(u[i], u[i - 1]);
        b.ccx(u[i - 1], v[i], carries[i]);
        b.cx(carries[i], u[i]);
    }

    b.cx(u[n - 1], flag);

    // Backward inv_MAJ with measurement
    for i in (1..n).rev() {
        b.cx(carries[i], u[i]);
        let m = b.alloc_bit();
        b.hmr(carries[i], m);
        b.cz_if(u[i - 1], v[i], m);
        b.cx(u[i], u[i - 1]);
        b.cx(u[i], v[i]);
    }
    b.cx(carries[0], u[0]);
    let m0 = b.alloc_bit();
    b.hmr(carries[0], m0);
    b.cz_if(c_in, v[0], m0);
    b.cx(u[0], c_in);
    b.cx(u[0], v[0]);

    for i in 0..n {
        b.x(u[i]);
    }
    b.free_vec(&carries);
    b.free(c_in);
}

/// Like `mod_add_qq` but uses `cmp_lt_into_fast` for the flag uncompute.
/// NOT safe inside emit_inverse blocks.
pub(crate) fn mod_add_qq_fast(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // Use fast (measurement-based) Cuccaro everywhere.
    add_nbit_qq_fast(b, &a_ext, &acc_ext);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    // add_nbit_const with fast Cuccaro OR venting (using `a` as dirty).
    let use_vent = std::env::var("KAL_VENT_MODADD").ok().as_deref() == Some("1");
    if use_vent {
        let n1 = acc_ext.len();
        // Use `a_ext` as dirty qubits (it was just used as add operand,
        // its value is preserved through the venting sub-protocol).
        let c_low = c.as_limbs()[0];
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::iadd_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            false,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else {
        let n1 = acc_ext.len();
        let ca = load_const(b, n1, c);
        add_nbit_qq_fast(b, &ca, &acc_ext);
        unload_const(b, &ca, c);
    }
    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    b.x(flag);
    // csub_nbit_const with fast Cuccaro OR venting.
    if use_vent {
        let c_low = c.as_limbs()[0];
        let n1 = acc_ext.len();
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            flag,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else {
        let n1 = acc_ext.len();
        let ca = b.alloc_qubits(n1);
        for i in 0..n1 {
            if bit(c, i) {
                b.cx(flag, ca[i]);
            }
        }
        sub_nbit_qq_fast(b, &ca, &acc_ext);
        for i in 0..n1 {
            if bit(c, i) {
                b.cx(flag, ca[i]);
            }
        }
        b.free_vec(&ca);
    }
    b.x(flag);
    b.cx(flag, acc_ovf);
    cmp_lt_into_fast(b, &acc_ext[..n], &a_ext[..n], flag);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// Specialization of mod_add_qq_fast when acc = 0 on entry. Replaces the
/// initial Cuccaro add with CX-copy (0 CCX instead of n-1 CCX).
/// Saves 255 CCX per call.
pub(crate) fn mod_add_qq_fast_from_zero(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // acc is 0 on entry. CX-copy a into acc (0 CCX). Top bits both 0.
    for i in 0..n {
        b.cx(a[i], acc[i]);
    }
    // acc_ovf and a_ovf are both 0 (both freshly allocated as 0 by ext_reg).

    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let use_vent = std::env::var("KAL_VENT_MODADD").ok().as_deref() == Some("1");
    if use_vent {
        let n1 = acc_ext.len();
        let c_low = c.as_limbs()[0];
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::iadd_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            false,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else {
        let n1 = acc_ext.len();
        let ca = load_const(b, n1, c);
        add_nbit_qq_fast(b, &ca, &acc_ext);
        unload_const(b, &ca, c);
    }
    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    b.x(flag);
    if use_vent {
        let c_low = c.as_limbs()[0];
        let n1 = acc_ext.len();
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(
            b,
            &acc_ext,
            &a_ext[..n1 - 2],
            &q_clean2,
            c_low,
            flag,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else {
        let n1 = acc_ext.len();
        let ca = b.alloc_qubits(n1);
        for i in 0..n1 {
            if bit(c, i) {
                b.cx(flag, ca[i]);
            }
        }
        sub_nbit_qq_fast(b, &ca, &acc_ext);
        for i in 0..n1 {
            if bit(c, i) {
                b.cx(flag, ca[i]);
            }
        }
        b.free_vec(&ca);
    }
    b.x(flag);
    b.cx(flag, acc_ovf);
    cmp_lt_into_fast(b, &acc_ext[..n], &a_ext[..n], flag);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// Low-scratch peak variant of `mod_add_qq_fast`. Identical arithmetic, but the
/// Solinas `+c` / `-c` corrections (c = 2^256 - p = 2^32 + 977, sparse) are done
/// with the register-free direct const adders (`cadd_/csub_nbit_const_direct_fast`)
/// instead of `load_const` + a full-width q-q add. This drops the per-call scratch
/// from ~(n+1 loaded-const register + n carries) to ~(n carries), saving ~256
/// qubits at the call site. Toffoli is ~neutral for sparse c (the direct const
/// adders carry the same length sweep; only the loaded register disappears).
/// Used at the Karatsuba Solinas fold, which owns the 2710 peak.
pub(crate) fn mod_add_qq_fast_lowscratch(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // Step 1: (n+1)-bit operand add (measurement-based Cuccaro, unchanged).
    add_nbit_qq_fast(b, &a_ext, &acc_ext);

    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    // Step 2: unconditional +c via register-free direct const add. A |1> control
    // makes the controlled primitive unconditional (X/CX/CZ are free Cliffords).
    let one = b.alloc_qubit();
    b.x(one);
    cadd_nbit_const_direct_fast(b, &acc_ext, c, one);
    b.x(one);
    b.free(one);

    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    b.x(flag);
    // Step 3: if flag=0, undo the +c (register-free conditional const sub).
    csub_nbit_const_direct_fast(b, &acc_ext, c, flag);
    b.x(flag);
    b.cx(flag, acc_ovf);
    cmp_lt_into_fast(b, &acc_ext[..n], &a_ext[..n], flag);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// Low-scratch peak variant of `mod_add_qq_fast_from_zero` (acc == 0 on entry).
/// Same register-free Solinas correction as `mod_add_qq_fast_lowscratch`.
pub(crate) fn mod_add_qq_fast_from_zero_lowscratch(b: &mut B, acc: &[QubitId], a: &[QubitId], p: U256) {
    let n = acc.len();
    assert_eq!(n, a.len());
    debug_assert_eq!(n, 256);

    let (acc_ext, acc_ovf) = ext_reg(b, acc);
    let (a_ext, a_ovf) = ext_reg(b, a);

    // acc is 0 on entry. CX-copy a into acc (0 CCX). Top bits both 0.
    for i in 0..n {
        b.cx(a[i], acc[i]);
    }

    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let one = b.alloc_qubit();
    b.x(one);
    cadd_nbit_const_direct_fast(b, &acc_ext, c, one);
    b.x(one);
    b.free(one);

    let flag = b.alloc_qubit();
    b.cx(acc_ovf, flag);
    b.x(flag);
    csub_nbit_const_direct_fast(b, &acc_ext, c, flag);
    b.x(flag);
    b.cx(flag, acc_ovf);
    cmp_lt_into_fast(b, &acc_ext[..n], &a_ext[..n], flag);
    b.free(flag);

    unext_reg(b, a_ovf);
    unext_reg(b, acc_ovf);
    let _ = (acc_ext, a_ext);
}

/// Low-peak variant of `mod_mul_write_into_zero_acc_schoolbook`: uses
/// `schoolbook_mul_into_addsub_lowq` + `_inverse_lowq` instead of the fast
/// variants, saving ~n qubits at peak at the cost of ~n extra Toffolis per
/// row.
///
/// NOTE: microbench (n=256) shows this DOES NOT reduce the local peak
/// (schoolbook_fast 1797 = schoolbook_lowq 1797); the Solinas reduction +
/// acc lifetimes already dominate, and the lowq carry saving is hidden
/// underneath. We also observed a deterministic phase-garbage batch when
/// wiring this in at pair1_mul1 (1/20480 shots, ALT_SEED tag=5, across
/// two runs), so this helper is currently DEAD CODE kept only as a paper
/// trail for the negative result. See `autoresearch.ideas.md`.
#[allow(dead_code)]
pub(crate) fn mod_mul_write_into_zero_acc_schoolbook_lowq(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let n = acc.len();
    debug_assert_eq!(n, 256);

    let tmp_ext = b.alloc_qubits(2 * n);
    schoolbook_mul_into_addsub_lowq(b, x, y, &tmp_ext);

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

    schoolbook_mul_into_addsub_lowq_inverse(b, x, y, &tmp_ext);
    b.free_vec(&tmp_ext);
}

/// Specialization of mod_mul_add_into_acc_schoolbook when acc = 0 on entry.
/// Uses mod_add_qq_fast_from_zero for the first Solinas reduction step.
/// Saves ~255 CCX per call.
pub(crate) fn mod_mul_write_into_zero_acc_schoolbook(
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
    // First add: acc is known to be 0, so use the fast-from-zero variant.
    mod_add_qq_fast_from_zero(b, acc, &lo, p);
    let _ = c;
    // 977 = 2^10 - 2^6 + 2^4 + 2^0 consolidation. 5 ops instead of 7.
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
    b.set_phase("sol_halve_tail");
    for _ in 0..10 {
        mod_halve_inplace_fast(b, &hi, p);
    }

    b.set_phase("schoolbook_mul_inverse");
    schoolbook_mul_into_addsub_inverse(b, x, y, &tmp_ext);
    b.free_vec(&tmp_ext);
}

pub(crate) fn cmod_add_qq(b: &mut B, acc: &[QubitId], a: &[QubitId], ctrl: QubitId, p: U256) {
    let n = acc.len();
    let f = b.alloc_qubits(n);
    for i in 0..n {
        b.ccx(ctrl, a[i], f[i]);
    }
    mod_add_qq_fast(b, acc, &f, p);
    // Gidney measurement-based AND uncomputation: f[i] = ctrl AND a[i],
    // which is unchanged by mod_add_qq (Cuccaro restores the addend).
    // HMR + classically-conditioned CZ costs 0 Toffoli vs 256 CCX.
    for i in 0..n {
        let m = b.alloc_bit();
        b.hmr(f[i], m);
        b.cz_if(ctrl, a[i], m);
    }
    b.free_vec(&f);
}

pub(crate) fn cmod_sub_qq(b: &mut B, acc: &[QubitId], a: &[QubitId], ctrl: QubitId, p: U256) {
    let n = acc.len();
    let f = b.alloc_qubits(n);
    for i in 0..n {
        b.ccx(ctrl, a[i], f[i]);
    }
    mod_sub_qq_fast(b, acc, &f, p);
    for i in 0..n {
        let m = b.alloc_bit();
        b.hmr(f[i], m);
        b.cz_if(ctrl, a[i], m);
    }
    b.free_vec(&f);
}
