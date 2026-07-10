// Grouped implementation file.
use super::*;

// ═══════════════════════════════════════════════════════════════════════════
//  Merged from modular.rs
// ═══════════════════════════════════════════════════════════════════════════

// (refactor) Mechanically extracted from mod.rs. No logic changes.

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


// ═══════════════════════════════════════════════════════════════════════════
//  Merged from cuccaro.rs
// ═══════════════════════════════════════════════════════════════════════════

// (refactor) Mechanically extracted from mod.rs. No logic changes.

// ═══════════════════════════════════════════════════════════════════════════
//  Cuccaro ripple-carry adder
// ═══════════════════════════════════════════════════════════════════════════
//
// Operates on two n-wide qubit registers `a` (addend, unchanged) and
// `acc` (accumulator, becomes a + acc mod 2^n). Also takes:
//   * c_in: one ancilla qubit, = 0 on entry, = 0 on exit (unchanged)
//   * z   : one ancilla qubit, = 0 on entry, = carry_out ⊕ z_in on exit
//           (i.e., the output carry is XORed into z; pass a fresh 0 bit
//           to receive the high bit)
//
// Based on Cuccaro et al. 2004 (arXiv:quant-ph/0410184), Figure 3.
//
// `MAJ(x, y, w)` triple:
//     CX(w, y)        # y ← y ⊕ w
//     CX(w, x)        # x ← x ⊕ w
//     CCX(x, y, w)    # w ← w ⊕ (x·y)        w becomes MAJ(w_old, y_old, x_old)
//
// `UMA(x, y, w)` triple (undoes MAJ, leaves sum bit in y):
//     CCX(x, y, w)
//     CX(w, x)
//     CX(x, y)

pub(crate) fn maj(b: &mut B, x: QubitId, y: QubitId, w: QubitId) {
    b.cx(w, y);
    b.cx(w, x);
    b.ccx(x, y, w);
}

pub(crate) fn uma(b: &mut B, x: QubitId, y: QubitId, w: QubitId) {
    b.ccx(x, y, w);
    b.cx(w, x);
    b.cx(x, y);
}

/// Fast Cuccaro add using carry ancillae + measurement-based UMA.
/// Same interface as `cuccaro_add` but uses n-1 carry ancillae so the
/// UMA sweep costs 0 Toffoli (measurement only). NOT emit_inverse-safe.
pub(crate) fn cuccaro_add_fast(b: &mut B, a: &[QubitId], acc: &[QubitId], c_in: QubitId) {
    let n = a.len();
    assert_eq!(n, acc.len());
    if n == 0 {
        return;
    }
    if n == 1 {
        b.cx(c_in, acc[0]);
        b.cx(a[0], acc[0]);
        return;
    }

    let carries = b.alloc_qubits(n - 1);

    // Forward MAJ sweep with carry ancillae.
    // Step 0: MAJ(c_in, acc[0], a[0]) → carry into carries[0]
    b.cx(a[0], acc[0]);
    b.cx(a[0], c_in);
    b.ccx(c_in, acc[0], carries[0]);
    b.cx(carries[0], a[0]);
    // Steps 1..n-2: MAJ(a[i-1], acc[i], a[i]) → carry into carries[i]
    for i in 1..n - 1 {
        b.cx(a[i], acc[i]);
        b.cx(a[i], a[i - 1]);
        b.ccx(a[i - 1], acc[i], carries[i]);
        b.cx(carries[i], a[i]);
    }

    // Final sum bit (same as original cuccaro_add)
    b.cx(a[n - 2], acc[n - 1]);
    b.cx(a[n - 1], acc[n - 1]);

    // Backward UMA sweep with measurement-based carry uncompute (0 Toffoli).
    for i in (1..n - 1).rev() {
        b.cx(carries[i], a[i]);
        let m = b.alloc_bit();
        b.hmr(carries[i], m);
        b.cz_if(a[i - 1], acc[i], m);
        b.cx(a[i], a[i - 1]);
        b.cx(a[i - 1], acc[i]);
    }
    // Step 0 UMA:
    b.cx(carries[0], a[0]);
    let m0 = b.alloc_bit();
    b.hmr(carries[0], m0);
    b.cz_if(c_in, acc[0], m0);
    b.cx(a[0], c_in);
    b.cx(c_in, acc[0]);

    b.free_vec(&carries);
}

/// Carry-BORROW twin of [`cuccaro_add_fast`]: identical gate sequence, but the
/// n-1 carry qubits are BORROWED from `carry_src` (which MUST be clean |0⟩ on
/// entry and is restored to |0⟩ on exit by the measurement-uncompute) instead
/// of freshly allocated. Flat Toffoli, zero new width at the peak — the carry
/// register is hosted on already-live but idle clean ancilla (e.g. the future
/// Kaliski m_hist transcript bits m_hist[iter+1..], guaranteed |0⟩ until their
/// own iteration writes them). `carry_src.len()` must be >= n-1.
pub(crate) fn cuccaro_add_fast_borrow(
    b: &mut B,
    a: &[QubitId],
    acc: &[QubitId],
    c_in: QubitId,
    carry_src: &[QubitId],
) {
    let n = a.len();
    assert_eq!(n, acc.len());
    if n == 0 {
        return;
    }
    if n == 1 {
        b.cx(c_in, acc[0]);
        b.cx(a[0], acc[0]);
        return;
    }
    assert!(carry_src.len() >= n - 1, "borrow carry_src too short");
    let carries = &carry_src[..n - 1];

    b.cx(a[0], acc[0]);
    b.cx(a[0], c_in);
    b.ccx(c_in, acc[0], carries[0]);
    b.cx(carries[0], a[0]);
    for i in 1..n - 1 {
        b.cx(a[i], acc[i]);
        b.cx(a[i], a[i - 1]);
        b.ccx(a[i - 1], acc[i], carries[i]);
        b.cx(carries[i], a[i]);
    }

    b.cx(a[n - 2], acc[n - 1]);
    b.cx(a[n - 1], acc[n - 1]);

    for i in (1..n - 1).rev() {
        b.cx(carries[i], a[i]);
        let m = b.alloc_bit();
        b.hmr(carries[i], m);
        b.cz_if(a[i - 1], acc[i], m);
        b.cx(a[i], a[i - 1]);
        b.cx(a[i - 1], acc[i]);
    }
    b.cx(carries[0], a[0]);
    let m0 = b.alloc_bit();
    b.hmr(carries[0], m0);
    b.cz_if(c_in, acc[0], m0);
    b.cx(a[0], c_in);
    b.cx(c_in, acc[0]);
    // carries are borrowed: restored to |0> by the measurement-uncompute,
    // NOT freed (they belong to the caller's m_hist register).
}

/// In-place addition `acc += a mod 2^n` on quantum n-bit registers.
/// * `c_in` is a fresh ancilla qubit at 0 on entry and returns to 0.
/// * `a` unchanged; `acc` becomes (a + acc) mod 2^n.
/// Pure mod-2^n: the high carry is discarded (no `z` ancilla). This is
/// honestly reversible because the last MAJ/UMA pair cancel out the
/// carry information on `a[n-1]`.
pub(crate) fn cuccaro_add(b: &mut B, a: &[QubitId], acc: &[QubitId], c_in: QubitId) {
    let n = a.len();
    assert_eq!(n, acc.len());
    if n == 0 {
        return;
    }
    if n == 1 {
        // acc[0] += a[0] + c_in  mod 2 ; c_in → 0
        b.cx(c_in, acc[0]);
        b.cx(a[0], acc[0]);
        return;
    }

    // Forward MAJ sweep.
    maj(b, c_in, acc[0], a[0]);
    for i in 1..n - 1 {
        maj(b, a[i - 1], acc[i], a[i]);
    }

    // Final sum bit: sum[n-1] = acc[n-1] XOR a[n-1] XOR carry_in_to_n-1,
    // where carry_in_to_n-1 is in a[n-2] after the MAJ sweep.
    b.cx(a[n - 2], acc[n - 1]);
    b.cx(a[n - 1], acc[n - 1]);

    // Reverse UMA sweep (skips the final MAJ since we didn't do it).
    for i in (1..n - 1).rev() {
        uma(b, a[i - 1], acc[i], a[i]);
    }
    uma(b, c_in, acc[0], a[0]);
}

/// Reverse of `cuccaro_add`: performs `acc -= a mod 2^n`.
/// Implemented as the exact inverse gate sequence of `cuccaro_add`.
pub(crate) fn cuccaro_sub(b: &mut B, a: &[QubitId], acc: &[QubitId], c_in: QubitId) {
    let n = a.len();
    assert_eq!(n, acc.len());
    if n == 0 {
        return;
    }
    if n == 1 {
        // Inverse of (cx c_in acc; cx a acc) is the same two gates in reverse.
        b.cx(a[0], acc[0]);
        b.cx(c_in, acc[0]);
        return;
    }

    // Inverse of `uma(c_in, acc[0], a[0])`, then the rest of UMA sweep
    // in reverse order.
    inv_uma(b, c_in, acc[0], a[0]);
    for i in 1..n - 1 {
        inv_uma(b, a[i - 1], acc[i], a[i]);
    }

    // Inverse of the final sum writes (both CX self-inverse; reverse order).
    b.cx(a[n - 1], acc[n - 1]);
    b.cx(a[n - 2], acc[n - 1]);

    // Inverse of the forward MAJ sweep.
    for i in (1..n - 1).rev() {
        inv_maj(b, a[i - 1], acc[i], a[i]);
    }
    inv_maj(b, c_in, acc[0], a[0]);
}

pub(crate) fn inv_maj(b: &mut B, x: QubitId, y: QubitId, w: QubitId) {
    // maj = CX(w,y); CX(w,x); CCX(x,y,w)
    // inv = CCX(x,y,w); CX(w,x); CX(w,y)
    b.ccx(x, y, w);
    b.cx(w, x);
    b.cx(w, y);
}

pub(crate) fn inv_uma(b: &mut B, x: QubitId, y: QubitId, w: QubitId) {
    // uma = CCX(x,y,w); CX(w,x); CX(x,y)
    // inv = CX(x,y); CX(w,x); CCX(x,y,w)
    b.cx(x, y);
    b.cx(w, x);
    b.ccx(x, y, w);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Non-modular n-bit primitives
// ═══════════════════════════════════════════════════════════════════════════

/// Fast Cuccaro sub: `acc -= a mod 2^n` with measurement UMA (0 Toffoli
/// for UMA sweep). Exact gate-level inverse of `cuccaro_add_fast`.
pub(crate) fn cuccaro_sub_fast(b: &mut B, a: &[QubitId], acc: &[QubitId], c_in: QubitId) {
    let n = a.len();
    assert_eq!(n, acc.len());
    if n == 0 {
        return;
    }
    if n == 1 {
        b.cx(a[0], acc[0]);
        b.cx(c_in, acc[0]);
        return;
    }

    let carries = b.alloc_qubits(n - 1);

    // Forward inv_UMA sweep with carry ancillae (reversed UMA from cuccaro_sub).
    // Step 0:
    b.cx(c_in, acc[0]);
    b.cx(a[0], c_in);
    b.ccx(c_in, acc[0], carries[0]);
    b.cx(carries[0], a[0]);
    // Steps 1..n-2:
    for i in 1..n - 1 {
        b.cx(a[i - 1], acc[i]);
        b.cx(a[i], a[i - 1]);
        b.ccx(a[i - 1], acc[i], carries[i]);
        b.cx(carries[i], a[i]);
    }

    // Final sum bit (reversed from cuccaro_add)
    b.cx(a[n - 1], acc[n - 1]);
    b.cx(a[n - 2], acc[n - 1]);

    // Backward inv_MAJ sweep with measurement.
    for i in (1..n - 1).rev() {
        b.cx(carries[i], a[i]);
        let m = b.alloc_bit();
        b.hmr(carries[i], m);
        b.cz_if(a[i - 1], acc[i], m);
        b.cx(a[i], a[i - 1]);
        b.cx(a[i], acc[i]);
    }
    b.cx(carries[0], a[0]);
    let m0 = b.alloc_bit();
    b.hmr(carries[0], m0);
    b.cz_if(c_in, acc[0], m0);
    b.cx(a[0], c_in);
    b.cx(a[0], acc[0]);

    b.free_vec(&carries);
}

/// Carry-BORROW twin of [`cuccaro_sub_fast`]: see [`cuccaro_add_fast_borrow`].
/// Borrows `carry_src[..n-1]` (clean |0>, restored to |0>) as the carry block.
pub(crate) fn cuccaro_sub_fast_borrow(
    b: &mut B,
    a: &[QubitId],
    acc: &[QubitId],
    c_in: QubitId,
    carry_src: &[QubitId],
) {
    let n = a.len();
    assert_eq!(n, acc.len());
    if n == 0 {
        return;
    }
    if n == 1 {
        b.cx(a[0], acc[0]);
        b.cx(c_in, acc[0]);
        return;
    }
    assert!(carry_src.len() >= n - 1, "borrow carry_src too short");
    let carries = &carry_src[..n - 1];

    b.cx(c_in, acc[0]);
    b.cx(a[0], c_in);
    b.ccx(c_in, acc[0], carries[0]);
    b.cx(carries[0], a[0]);
    for i in 1..n - 1 {
        b.cx(a[i - 1], acc[i]);
        b.cx(a[i], a[i - 1]);
        b.ccx(a[i - 1], acc[i], carries[i]);
        b.cx(carries[i], a[i]);
    }

    b.cx(a[n - 1], acc[n - 1]);
    b.cx(a[n - 2], acc[n - 1]);

    for i in (1..n - 1).rev() {
        b.cx(carries[i], a[i]);
        let m = b.alloc_bit();
        b.hmr(carries[i], m);
        b.cz_if(a[i - 1], acc[i], m);
        b.cx(a[i], a[i - 1]);
        b.cx(a[i], acc[i]);
    }
    b.cx(carries[0], a[0]);
    let m0 = b.alloc_bit();
    b.hmr(carries[0], m0);
    b.cz_if(c_in, acc[0], m0);
    b.cx(a[0], c_in);
    b.cx(a[0], acc[0]);
    // carries borrowed: restored to |0>, NOT freed.
}

/// Borrow-carry `acc += a mod 2^n`: hosts the fast-Cuccaro carry register on
/// `carry_src` (clean |0>, restored). Flat Toffoli, no new peak width.
pub(crate) fn add_nbit_qq_fast_borrow(b: &mut B, a: &[QubitId], acc: &[QubitId], carry_src: &[QubitId]) {
    assert_eq!(a.len(), acc.len());
    let c_in = b.alloc_qubit();
    cuccaro_add_fast_borrow(b, a, acc, c_in, carry_src);
    b.free(c_in);
}

pub(crate) fn add_nbit_qq_fast_borrowed(b: &mut B, a: &[QubitId], acc: &[QubitId], carry_src: &[QubitId]) {
    add_nbit_qq_fast_borrow(b, a, acc, carry_src);
}

/// Borrow-carry `acc -= a mod 2^n`. Companion to [`add_nbit_qq_fast_borrow`].
pub(crate) fn sub_nbit_qq_fast_borrow(b: &mut B, a: &[QubitId], acc: &[QubitId], carry_src: &[QubitId]) {
    assert_eq!(a.len(), acc.len());
    let c_in = b.alloc_qubit();
    cuccaro_sub_fast_borrow(b, a, acc, c_in, carry_src);
    b.free(c_in);
}

pub(crate) fn sub_nbit_qq_fast_borrowed(b: &mut B, a: &[QubitId], acc: &[QubitId], carry_src: &[QubitId]) {
    sub_nbit_qq_fast_borrow(b, a, acc, carry_src);
}

/// Build a width-(n-1) clean-|0> carry register for a fast-Cuccaro add/sub of
/// width n, hosting as many bits as possible on `m_future` (clean |0> bits that
/// belong to the caller and are restored to |0> on exit), and freshly
/// allocating only the shortfall. Returns (full_carry_vec, fresh_count); the
/// caller must `free` the LAST `fresh_count` entries after the add/sub. Flat
/// Toffoli vs the all-fresh path; peak width drops by `min(n-1, m_future.len())`.
pub(crate) fn borrow_carry_register(
    b: &mut B,
    n: usize,
    m_future: &[QubitId],
) -> (Vec<QubitId>, usize) {
    let need = n.saturating_sub(1);
    let borrowed = need.min(m_future.len());
    let fresh_count = need - borrowed;
    let mut carries: Vec<QubitId> = Vec::with_capacity(need);
    carries.extend_from_slice(&m_future[..borrowed]);
    for _ in 0..fresh_count {
        carries.push(b.alloc_qubit());
    }
    (carries, fresh_count)
}

/// Max fresh carry qubits we will allocate on top of the m_future borrow
/// before falling back to the slow (carry-register-FREE, 1-ancilla in-place
/// Cuccaro). Tuned so the per-step peak stays <= the 2333 shift22 floor:
/// 2333 - (binder carrier 1175 + tmp 256 + init 512 + lam 256 + slack) leaves
/// headroom for ~120 fresh carries. When the m_future pool is too small to
/// cover (n-1) with <= this many fresh bits, we use slow Cuccaro for that one
/// call (flat WIDTH, +~n Toffoli) — only the few late iters with an exhausted
/// pool pay it, keeping the global Toffoli penalty small while still capping
/// the peak at the floor.
pub(crate) fn gz_max_fresh_carries() -> usize {
    // 131 = the max fresh-carry budget that keeps the per-step peak at the 2333
    // shift22 floor (132+ lets kal_bulk_step4 borrow-fast push to 2334+).
    // Swept empirically; minimizes the slow-Cuccaro fallback Toffoli at 2333.
    std::env::var("KAL_GZ_MAX_FRESH")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(131)
}

/// `acc += a mod 2^n`: borrow the carry register from `m_future` (clean |0>);
/// if the shortfall would exceed `gz_max_fresh_carries`, fall back to the slow
/// in-place Cuccaro (no carry register at all) so the peak stays at the floor.
pub(crate) fn add_nbit_qq_fast_mfut(b: &mut B, a: &[QubitId], acc: &[QubitId], m_future: &[QubitId]) {
    assert_eq!(a.len(), acc.len());
    let n = a.len();
    let need = n.saturating_sub(1);
    let borrowed = need.min(m_future.len());
    if need - borrowed > gz_max_fresh_carries() {
        // Pool too small: slow Cuccaro (1 ancilla, no carry register).
        let c_in = b.alloc_qubit();
        cuccaro_add(b, a, acc, c_in);
        b.free(c_in);
        return;
    }
    let (carries, fresh) = borrow_carry_register(b, n, m_future);
    let c_in = b.alloc_qubit();
    cuccaro_add_fast_borrow(b, a, acc, c_in, &carries);
    b.free(c_in);
    for &q in carries[carries.len() - fresh..].iter() {
        b.free(q);
    }
}

/// `acc -= a mod 2^n` with m_future borrow + slow-Cuccaro shortfall fallback.
pub(crate) fn sub_nbit_qq_fast_mfut(b: &mut B, a: &[QubitId], acc: &[QubitId], m_future: &[QubitId]) {
    assert_eq!(a.len(), acc.len());
    let n = a.len();
    let need = n.saturating_sub(1);
    let borrowed = need.min(m_future.len());
    if need - borrowed > gz_max_fresh_carries() {
        let c_in = b.alloc_qubit();
        cuccaro_sub(b, a, acc, c_in);
        b.free(c_in);
        return;
    }
    let (carries, fresh) = borrow_carry_register(b, n, m_future);
    let c_in = b.alloc_qubit();
    cuccaro_sub_fast_borrow(b, a, acc, c_in, &carries);
    b.free(c_in);
    for &q in carries[carries.len() - fresh..].iter() {
        b.free(q);
    }
}

/// Build a width-(n-1) clean-|0> carry register from TWO already-live clean
/// pools (`m_future` first, then `extra`), freshly allocating only the
/// shortfall. Both pools must be |0> on entry; the borrowed bits are restored
/// to |0> by the caller's measurement-uncompute. Returns (carry_vec,
/// fresh_count); the caller frees the LAST `fresh_count` entries. Adds ZERO
/// peak width for every bit drawn from the pools. See [`gz_late_recover`].
pub(crate) fn borrow_carry_register_pool(
    b: &mut B,
    n: usize,
    m_future: &[QubitId],
    extra: &[QubitId],
) -> (Vec<QubitId>, usize) {
    let need = n.saturating_sub(1);
    let mut carries: Vec<QubitId> = Vec::with_capacity(need);
    let take_mf = need.min(m_future.len());
    carries.extend_from_slice(&m_future[..take_mf]);
    if carries.len() < need {
        let take_ex = (need - carries.len()).min(extra.len());
        carries.extend_from_slice(&extra[..take_ex]);
    }
    let fresh_count = need - carries.len();
    for _ in 0..fresh_count {
        carries.push(b.alloc_qubit());
    }
    (carries, fresh_count)
}

/// `acc += a mod 2^n`: borrow carries from `m_future` THEN `extra` (both clean
/// |0>, restored on exit); slow-Cuccaro fallback only if the COMBINED pool is
/// still too small. Recovers the late-iter slow-fallback Toffoli at flat peak.
pub(crate) fn add_nbit_qq_fast_mfut_pool(
    b: &mut B,
    a: &[QubitId],
    acc: &[QubitId],
    m_future: &[QubitId],
    extra: &[QubitId],
) {
    assert_eq!(a.len(), acc.len());
    let n = a.len();
    let need = n.saturating_sub(1);
    let pool = m_future.len() + extra.len();
    let borrowed = need.min(pool);
    if need - borrowed > gz_max_fresh_carries() {
        let c_in = b.alloc_qubit();
        cuccaro_add(b, a, acc, c_in);
        b.free(c_in);
        return;
    }
    let (carries, fresh) = borrow_carry_register_pool(b, n, m_future, extra);
    let c_in = b.alloc_qubit();
    cuccaro_add_fast_borrow(b, a, acc, c_in, &carries);
    b.free(c_in);
    for &q in carries[carries.len() - fresh..].iter() {
        b.free(q);
    }
}

/// `acc -= a mod 2^n` twin of [`add_nbit_qq_fast_mfut_pool`].
pub(crate) fn sub_nbit_qq_fast_mfut_pool(
    b: &mut B,
    a: &[QubitId],
    acc: &[QubitId],
    m_future: &[QubitId],
    extra: &[QubitId],
) {
    assert_eq!(a.len(), acc.len());
    let n = a.len();
    let need = n.saturating_sub(1);
    let pool = m_future.len() + extra.len();
    let borrowed = need.min(pool);
    if need - borrowed > gz_max_fresh_carries() {
        let c_in = b.alloc_qubit();
        cuccaro_sub(b, a, acc, c_in);
        b.free(c_in);
        return;
    }
    let (carries, fresh) = borrow_carry_register_pool(b, n, m_future, extra);
    let c_in = b.alloc_qubit();
    cuccaro_sub_fast_borrow(b, a, acc, c_in, &carries);
    b.free(c_in);
    for &q in carries[carries.len() - fresh..].iter() {
        b.free(q);
    }
}

/// Fast `acc += a mod 2^n` using measurement-based Cuccaro.
pub(crate) fn add_nbit_qq_fast(b: &mut B, a: &[QubitId], acc: &[QubitId]) {
    assert_eq!(a.len(), acc.len());
    let c_in = b.alloc_qubit();
    cuccaro_add_fast(b, a, acc, c_in);
    b.free(c_in);
}

/// Fast `acc -= a mod 2^n` using measurement-based Cuccaro.
pub(crate) fn sub_nbit_qq_fast(b: &mut B, a: &[QubitId], acc: &[QubitId]) {
    assert_eq!(a.len(), acc.len());
    let c_in = b.alloc_qubit();
    cuccaro_sub_fast(b, a, acc, c_in);
    b.free(c_in);
}

/// `acc += a mod 2^n`. Caller must pre-extend both slices if they want the
/// top carry absorbed into the accumulator (i.e. pass n+1-bit slices with
/// top bits 0 to get a full n+1-bit add). The carry-out beyond the slice
/// is discarded via `R` on the `z` ancilla — safe when both inputs fit
/// in n-1 bits (as in our mod-p layer where both < 2p < 2^{n+1}).
pub(crate) fn add_nbit_qq(b: &mut B, a: &[QubitId], acc: &[QubitId]) {
    assert_eq!(a.len(), acc.len());
    let c_in = b.alloc_qubit();
    cuccaro_add(b, a, acc, c_in);
    b.free(c_in);
}

pub(crate) fn sub_nbit_qq(b: &mut B, a: &[QubitId], acc: &[QubitId]) {
    assert_eq!(a.len(), acc.len());
    let c_in = b.alloc_qubit();
    cuccaro_sub(b, a, acc, c_in);
    b.free(c_in);
}

pub(crate) fn centered_restoring_trial_subtract_clean(
    b: &mut B,
    u: &[QubitId],
    v: &[QubitId],
    q_success: QubitId,
) {
    // Trial subtract for a centered-Euclid quotient bit. Compute the borrow,
    // copy out the success bit, then undo with the arithmetic inverse instead
    // of replaying the Cuccaro subtract wrapper through emit_inverse.
    assert_eq!(u.len(), v.len());
    let top_u = b.alloc_qubit();
    let top_v = b.alloc_qubit();
    let mut u_ext = u.to_vec();
    u_ext.push(top_u);
    let mut v_ext = v.to_vec();
    v_ext.push(top_v);
    sub_nbit_qq(b, &v_ext, &u_ext);
    b.cx(top_u, q_success);
    b.x(q_success);
    add_nbit_qq(b, &v_ext, &u_ext);
    b.free(top_v);
    b.free(top_u);
}

pub(crate) fn add_nbit_const(b: &mut B, acc: &[QubitId], c: U256) {
    let n = acc.len();
    let a = load_const(b, n, c);
    add_nbit_qq(b, &a, acc);
    unload_const(b, &a, c);
}

pub(crate) fn sub_nbit_const(b: &mut B, acc: &[QubitId], c: U256) {
    let n = acc.len();
    let a = load_const(b, n, c);
    sub_nbit_qq(b, &a, acc);
    unload_const(b, &a, c);
}

pub(crate) fn csub_nbit_const(b: &mut B, acc: &[QubitId], c: U256, ctrl: QubitId) {
    // acc -= (ctrl ? c : 0). Mirror of cadd_nbit_const.
    let n = acc.len();
    let a = b.alloc_qubits(n);
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, a[i]);
        }
    }
    sub_nbit_qq(b, &a, acc);
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, a[i]);
        }
    }
    b.free_vec(&a);
}

pub(crate) fn cadd_nbit_const(b: &mut B, acc: &[QubitId], c: U256, ctrl: QubitId) {
    // Conditional add of constant c, controlled by qubit ctrl.
    // Trick: load c into a qubit register via CX-from-ctrl gates
    // (so the loaded value is (ctrl ? c : 0)), then unconditional add,
    // then unload.
    let n = acc.len();
    let a = b.alloc_qubits(n);
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, a[i]);
        }
    }
    add_nbit_qq(b, &a, acc);
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, a[i]);
        }
    }
    b.free_vec(&a);
}

pub(crate) fn csub_nbit_const_fast(b: &mut B, acc: &[QubitId], c: U256, ctrl: QubitId) {
    let n = acc.len();
    let a = b.alloc_qubits(n);
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, a[i]);
        }
    }
    sub_nbit_qq_fast(b, &a, acc);
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, a[i]);
        }
    }
    b.free_vec(&a);
}

/// Controlled subtract of a classical constant without materializing the
/// `ctrl ? c : 0` addend.  This is the same measurement-uncomputed ripple idea
/// as [`sub_nbit_qq_fast`], but the carry/borrow recurrence is specialized to a
/// classical bit and the external control.  It saves the n-qubit loaded-constant
/// register at Kaliski halve peaks; for sparse secp256k1 `c=2^32+977` the CCX
/// count is essentially unchanged.
pub(crate) fn csub_nbit_const_direct_fast(b: &mut B, acc: &[QubitId], c: U256, ctrl: QubitId) {
    let n = acc.len();
    if n == 0 {
        return;
    }
    if n == 1 {
        if bit(c, 0) {
            b.cx(ctrl, acc[0]);
        }
        return;
    }

    let borrows = b.alloc_qubits(n - 1);

    // Forward borrow sweep. borrow_{i+1} = majority(!acc_i, k_i, borrow_i),
    // where k_i = ctrl when c_i=1 and 0 otherwise.
    for i in 0..n - 1 {
        let target = borrows[i];
        let borrow_in = if i == 0 { None } else { Some(borrows[i - 1]) };
        if bit(c, i) {
            b.x(acc[i]);
            if let Some(bi) = borrow_in {
                if majfold_sub_enabled() {
                    b.cx(bi, acc[i]);
                    b.cx(bi, ctrl);
                    b.ccx(acc[i], ctrl, target);
                    b.cx(bi, target);
                    b.cx(bi, ctrl);
                    b.cx(bi, acc[i]);
                } else {
                    b.ccx(acc[i], bi, target);
                    b.ccx(ctrl, acc[i], target);
                    b.ccx(ctrl, bi, target);
                }
            } else {
                b.ccx(acc[i], ctrl, target);
            }
            b.x(acc[i]);
        } else if let Some(bi) = borrow_in {
            b.x(acc[i]);
            b.ccx(acc[i], bi, target);
            b.x(acc[i]);
        }
    }

    // Difference bits: acc_i ^= k_i ^ borrow_i.
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, acc[i]);
        }
        if i > 0 {
            b.cx(borrows[i - 1], acc[i]);
        }
    }

    // Measurement-uncompute borrows in reverse.  For subtraction the post-sum
    // identity is borrow_{i+1} = majority(acc_i_final, k_i, borrow_i).
    for i in (0..n - 1).rev() {
        let m = b.alloc_bit();
        b.hmr(borrows[i], m);
        let borrow_in = if i == 0 { None } else { Some(borrows[i - 1]) };
        if bit(c, i) {
            if let Some(bi) = borrow_in {
                b.cz_if(acc[i], ctrl, m);
                b.cz_if(acc[i], bi, m);
                b.cz_if(ctrl, bi, m);
            } else {
                b.cz_if(acc[i], ctrl, m);
            }
        } else if let Some(bi) = borrow_in {
            b.cz_if(acc[i], bi, m);
        }
    }

    b.free_vec(&borrows);
}

pub(crate) fn cadd_nbit_const_fast(b: &mut B, acc: &[QubitId], c: U256, ctrl: QubitId) {
    let n = acc.len();
    let a = b.alloc_qubits(n);
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, a[i]);
        }
    }
    add_nbit_qq_fast(b, &a, acc);
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, a[i]);
        }
    }
    b.free_vec(&a);
}

/// Controlled add of a classical constant without a loaded addend register.
/// This is the carry analogue of [`csub_nbit_const_direct_fast`].
pub(crate) fn cadd_nbit_const_direct_fast(b: &mut B, acc: &[QubitId], c: U256, ctrl: QubitId) {
    let n = acc.len();
    if n == 0 {
        return;
    }
    if n == 1 {
        if bit(c, 0) {
            b.cx(ctrl, acc[0]);
        }
        return;
    }

    let carries = b.alloc_qubits(n - 1);

    // Forward carry sweep. carry_{i+1} = majority(acc_i, k_i, carry_i).
    for i in 0..n - 1 {
        let target = carries[i];
        let carry_in = if i == 0 { None } else { Some(carries[i - 1]) };
        if bit(c, i) {
            if let Some(ci) = carry_in {
                if majfold_add_enabled() {
                    b.cx(ci, acc[i]);
                    b.cx(ci, ctrl);
                    b.ccx(acc[i], ctrl, target);
                    b.cx(ci, target);
                    b.cx(ci, ctrl);
                    b.cx(ci, acc[i]);
                } else {
                    b.ccx(acc[i], ci, target);
                    b.ccx(ctrl, acc[i], target);
                    b.ccx(ctrl, ci, target);
                }
            } else {
                b.ccx(acc[i], ctrl, target);
            }
        } else if let Some(ci) = carry_in {
            b.ccx(acc[i], ci, target);
        }
    }

    // Sum bits: acc_i ^= k_i ^ carry_i.
    for i in 0..n {
        if bit(c, i) {
            b.cx(ctrl, acc[i]);
        }
        if i > 0 {
            b.cx(carries[i - 1], acc[i]);
        }
    }

    // Measurement-uncompute carries in reverse.  For addition the post-sum
    // identity is carry_{i+1} = majority(!acc_i_final, k_i, carry_i).
    for i in (0..n - 1).rev() {
        let m = b.alloc_bit();
        b.hmr(carries[i], m);
        let carry_in = if i == 0 { None } else { Some(carries[i - 1]) };
        if bit(c, i) {
            b.x(acc[i]);
            if let Some(ci) = carry_in {
                b.cz_if(acc[i], ctrl, m);
                b.cz_if(acc[i], ci, m);
                b.x(acc[i]);
                b.cz_if(ctrl, ci, m);
            } else {
                b.cz_if(acc[i], ctrl, m);
                b.x(acc[i]);
            }
        } else if let Some(ci) = carry_in {
            b.x(acc[i]);
            b.cz_if(acc[i], ci, m);
            b.x(acc[i]);
        }
    }

    b.free_vec(&carries);
}

pub(crate) fn add_nbit_const_fast(b: &mut B, acc: &[QubitId], c: U256) {
    let n = acc.len();
    let a = load_const(b, n, c);
    add_nbit_qq_fast(b, &a, acc);
    unload_const(b, &a, c);
}

pub(crate) fn sub_nbit_const_fast(b: &mut B, acc: &[QubitId], c: U256) {
    let n = acc.len();
    let a = load_const(b, n, c);
    sub_nbit_qq_fast(b, &a, acc);
    unload_const(b, &a, c);
}


// ═══════════════════════════════════════════════════════════════════════════
//  Merged from solinas.rs
// ═══════════════════════════════════════════════════════════════════════════

// (refactor) Mechanically extracted from mod.rs. No logic changes.

/// Shift v left by k bits mod p. Returns (spill, flag_inv, ovf) which MUST
/// be passed to mod_shift_right_by_k for cleanup. Bennett-pattern: flags
/// stay alive across the body so the inverse can cleanly cancel them.
///
/// k must be small enough that spill·c < p. For k≤22 with secp256k1 this holds.
pub(crate) fn lowq_shift22() -> bool {
    // Qubit-first default: the global LOWQ shift22 path is strict-clean on the
    // current scaffold and lowers the benchmark peak (2736q -> 2715q) at a
    // small Toffoli cost. Keep LOWQ_SHIFT22=0 as an explicit opt-out for
    // Toffoli-first diagnostics and baseline comparisons.
    match std::env::var("LOWQ_SHIFT22") {
        Ok(v) => v != "0",
        Err(_) => true,
    }
}

pub(crate) fn mod_shift_left_by_k(
    b: &mut B,
    v: &[QubitId],
    p: U256,
    k: usize,
) -> (Vec<QubitId>, QubitId, QubitId) {
    let n = v.len();
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));

    let spill = b.alloc_qubits(k);
    let ovf = b.alloc_qubit();
    let flag_inv = b.alloc_qubit();

    // Step 1: k rounds of shift-by-1, capturing top bits into spill.
    for shift_i in 0..k {
        b.swap(v[n - 1], spill[k - 1 - shift_i]);
        for i in (0..n - 1).rev() {
            b.swap(v[i], v[i + 1]);
        }
    }

    // Step 2: add spill · c to v_ext (using ovf as bit n).
    // c = 2^32 + 977 = 2^32 + 2^10 - 2^6 + 2^4 + 2^0.
    // Consolidate 4 bits (6,7,8,9) of 977 into 2^10 - 2^6: saves 2 Cuccaros per shift.
    // Op list: ADD at 0, 4, 10, 32; SUB at 6. Total 5 ops instead of 7.
    let mut v_ext = v.to_vec();
    v_ext.push(ovf);
    let cuccaro_op = |b: &mut B, pos: usize, is_sub: bool| {
        let pad_width = n + 1 - pos;
        let padded = b.alloc_qubits(pad_width);
        for i in 0..k.min(pad_width) {
            b.cx(spill[i], padded[i]);
        }
        let v_slice: Vec<QubitId> = v_ext[pos..n + 1].to_vec();
        let c_in = b.alloc_qubit();
        if lowq_shift22() {
            if is_sub {
                cuccaro_sub(b, &padded, &v_slice, c_in);
            } else {
                cuccaro_add(b, &padded, &v_slice, c_in);
            }
        } else if is_sub {
            // Fast cuccaro: saves ~n CCX per op. Peak during this op (~514
            // transient) is still below the mod_add_qq_fast peak (517) inside
            // the enclosing Solinas, so no global peak increase.
            cuccaro_sub_fast(b, &padded, &v_slice, c_in);
        } else {
            cuccaro_add_fast(b, &padded, &v_slice, c_in);
        }
        b.free(c_in);
        for i in 0..k.min(pad_width) {
            b.cx(spill[i], padded[i]);
        }
        b.free_vec(&padded);
    };
    b.set_phase("shift22_cuccaro_op_0");
    cuccaro_op(b, 0, false);
    b.set_phase("shift22_cuccaro_op_4");
    cuccaro_op(b, 4, false);
    b.set_phase("shift22_cuccaro_op_6");
    cuccaro_op(b, 6, true);
    b.set_phase("shift22_cuccaro_op_10");
    cuccaro_op(b, 10, false);
    b.set_phase("shift22_cuccaro_op_32");
    cuccaro_op(b, 32, false);

    // Step 3: const add.
    b.set_phase("shift22_step3");
    if lowq_shift22() {
        add_nbit_const(b, &v_ext, c);
    } else {
        add_nbit_const_fast(b, &v_ext, c);
    }
    b.x(ovf);
    b.cx(ovf, flag_inv); // flag_inv = NOT(top_bit_after_add) = (value < p)
    b.x(ovf);

    // Step 4: conditional const sub.
    b.set_phase("shift22_step4");
    if lowq_shift22() {
        csub_nbit_const(b, &v_ext, c, flag_inv);
    } else {
        csub_nbit_const_fast(b, &v_ext, c, flag_inv);
    }
    b.x(flag_inv);
    b.cx(flag_inv, ovf);
    b.x(flag_inv);

    (spill, flag_inv, ovf)
}

/// Gate-level inverse of mod_shift_left_by_k.
pub(crate) fn mod_shift_right_by_k(
    b: &mut B,
    v: &[QubitId],
    p: U256,
    k: usize,
    spill: Vec<QubitId>,
    flag_inv: QubitId,
    ovf: QubitId,
) {
    let n = v.len();
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));

    let mut v_ext = v.to_vec();
    v_ext.push(ovf);

    // Reverse step 4.
    b.x(flag_inv);
    b.cx(flag_inv, ovf);
    b.x(flag_inv);
    b.set_phase("rshift22_rev_step4");
    if lowq_shift22() {
        cadd_nbit_const(b, &v_ext, c, flag_inv);
    } else {
        cadd_nbit_const_fast(b, &v_ext, c, flag_inv);
    }

    // Reverse step 3.
    b.x(ovf);
    b.cx(ovf, flag_inv);
    b.x(ovf);
    b.set_phase("rshift22_rev_step3");
    if lowq_shift22() {
        sub_nbit_const(b, &v_ext, c);
    } else {
        sub_nbit_const_fast(b, &v_ext, c);
    }
    b.free(flag_inv);
    b.set_phase("rshift22_rev_step2");

    // Reverse step 2: inverse of the consolidated op list (5 ops, in reverse order, flipped signs).
    let cuccaro_op = |b: &mut B, pos: usize, is_sub: bool| {
        let pad_width = n + 1 - pos;
        let padded = b.alloc_qubits(pad_width);
        for i in 0..k.min(pad_width) {
            b.cx(spill[i], padded[i]);
        }
        let v_slice: Vec<QubitId> = v_ext[pos..n + 1].to_vec();
        let c_in = b.alloc_qubit();
        if lowq_shift22() {
            if is_sub {
                cuccaro_sub(b, &padded, &v_slice, c_in);
            } else {
                cuccaro_add(b, &padded, &v_slice, c_in);
            }
        } else if is_sub {
            cuccaro_sub_fast(b, &padded, &v_slice, c_in);
        } else {
            cuccaro_add_fast(b, &padded, &v_slice, c_in);
        }
        b.free(c_in);
        for i in 0..k.min(pad_width) {
            b.cx(spill[i], padded[i]);
        }
        b.free_vec(&padded);
    };
    // Reverse: undo ADD at 32, 10; undo SUB at 6; undo ADD at 4, 0.
    cuccaro_op(b, 32, true); // undo +spill·2^32
    cuccaro_op(b, 10, true); // undo +spill·2^10
    cuccaro_op(b, 6, false); // undo -spill·2^6
    cuccaro_op(b, 4, true); // undo +spill·2^4
    cuccaro_op(b, 0, true); // undo +spill·2^0

    // Reverse step 1: reverse swap cascades.
    for shift_i in (0..k).rev() {
        for i in 0..n - 1 {
            b.swap(v[i], v[i + 1]);
        }
        b.swap(v[n - 1], spill[k - 1 - shift_i]);
    }

    b.free(ovf);
    b.free_vec(&spill);
}

/// Low-scratch spill add `v_ext[pos..] += spill*2^pos` (KAL_GZ_SOLINAS_LOWSCRATCH).
/// `spill` is a k-bit quantum value; the full-width add it represents has nonzero
/// addend bits only in [pos, pos+k). Instead of the ~(n+1-pos)-wide `padded`
/// carry-scratch register, this adds the k-bit chunk with a captured carry-out,
/// propagates that single carry through the high bits via a Gidney venting
/// controlled-increment (2 clean ancilla + a borrowed DIRTY donor, restored),
/// then uncomputes the carry-out via the exact comparator identity
/// `carry == (low_sum < spill)`. Net transient ~k+5 instead of ~n.
pub(crate) fn shift22_spill_op_dirty(
    b: &mut B,
    v_ext: &[QubitId],
    spill: &[QubitId],
    pos: usize,
    k: usize,
    is_sub: bool,
    dirty: &[QubitId],
) {
    let total = v_ext.len(); // n+1
    let w = total - pos; // width of the affected window
    debug_assert!(w >= k);
    let v_slice: Vec<QubitId> = v_ext[pos..total].to_vec();
    let low: Vec<QubitId> = v_slice[..k].to_vec();
    let hi: Vec<QubitId> = v_slice[k..].to_vec(); // width w-k

    // Step A: add/sub the k-bit spill into the low window, capturing carry/borrow.
    let carry = b.alloc_qubit();
    let zpad = b.alloc_qubit(); // |0> top bit of the (k+1)-bit addend
    let mut addend = spill.to_vec();
    addend.push(zpad);
    let mut acc = low.clone();
    acc.push(carry);
    let c_in = b.alloc_qubit();
    if is_sub {
        cuccaro_sub(b, &addend, &acc, c_in);
    } else {
        cuccaro_add(b, &addend, &acc, c_in);
    }
    b.free(c_in);
    b.free(zpad); // restored to |0> by cuccaro (addend register is preserved)

    // Step B: propagate the single carry/borrow into the high window.
    // add: hi += carry. sub: hi -= carry == controlled-decrement (invert,+1,invert).
    if w - k >= 5 {
        let dlen = (w - k).saturating_sub(2);
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        if is_sub {
            for &q in hi.iter() {
                b.cx(carry, q); // controlled-invert hi (no-op when carry=0)
            }
            venting::ciadd_dirty_2clean_classical(
                b, &hi, &dirty[..dlen], &q_clean2, 1, carry, false,
            );
            for &q in hi.iter() {
                b.cx(carry, q);
            }
        } else {
            venting::ciadd_dirty_2clean_classical(
                b, &hi, &dirty[..dlen], &q_clean2, 1, carry, false,
            );
        }
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    } else {
        let c = U256::from(1u64);
        if is_sub {
            csub_nbit_const_direct_fast(b, &hi, c, carry);
        } else {
            cadd_nbit_const_direct_fast(b, &hi, c, carry);
        }
    }

    // Step C: uncompute `carry`. add: carry == (low_new < spill).
    // sub: borrow == ((~low_new) < spill).
    if is_sub {
        for &q in low.iter() {
            b.x(q);
        }
        cmp_lt_into(b, &low, spill, carry);
        for &q in low.iter() {
            b.x(q);
        }
    } else {
        cmp_lt_into(b, &low, spill, carry);
    }
    b.free(carry);
}

fn shift22_compute_m_977(b: &mut B, m: &[QubitId], spill: &[QubitId], k: usize, undo: bool) {
    debug_assert_eq!(m.len(), 32);
    let terms: [(usize, bool); 4] = [(0, false), (4, false), (6, true), (10, false)];
    let term_op = |b: &mut B, pos: usize, is_sub: bool| {
        let w = 32 - pos;
        let m_slice: Vec<QubitId> = m[pos..32].to_vec();
        let pad = b.alloc_qubits(w - k);
        let mut addend = spill.to_vec();
        addend.extend_from_slice(&pad);
        let c_in = b.alloc_qubit();
        if is_sub {
            cuccaro_sub(b, &addend, &m_slice, c_in);
        } else {
            cuccaro_add(b, &addend, &m_slice, c_in);
        }
        b.free(c_in);
        b.free_vec(&pad);
    };
    if !undo {
        for &(pos, is_sub) in terms.iter() {
            term_op(b, pos, is_sub);
        }
    } else {
        for &(pos, is_sub) in terms.iter().rev() {
            term_op(b, pos, !is_sub);
        }
    }
}

/// KAL_GZ_SOLINAS_LOWSCRATCH forward shift22: same arithmetic as
/// `mod_shift_left_by_k` but STEP-2 spill ops use the dirty-borrow narrow
/// spill-add (no ~257-wide `padded`) and STEP-3/STEP-4 const-add /
/// conditional-const-sub use Gidney venting dirty-borrow const adders (no
/// ~257-wide loaded-constant register). `dirty` is a co-resident DIRTY donor
/// register (>= n-2 wide, restored to its entry value on exit).
pub(crate) fn mod_shift_left_by_k_dirty(
    b: &mut B,
    v: &[QubitId],
    p: U256,
    k: usize,
    dirty: &[QubitId],
) -> (Vec<QubitId>, QubitId, QubitId) {
    let n = v.len();
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let c_low = c.as_limbs()[0];

    let spill = b.alloc_qubits(k);
    let ovf = b.alloc_qubit();
    let flag_inv = b.alloc_qubit();

    for shift_i in 0..k {
        b.swap(v[n - 1], spill[k - 1 - shift_i]);
        for i in (0..n - 1).rev() {
            b.swap(v[i], v[i + 1]);
        }
    }

    let mut v_ext = v.to_vec();
    v_ext.push(ovf);

    if shift22_collapse() && k <= 22 {
        // SHIFT22_COLLAPSE: compute m=spill*977 (fits 32 bits), fold at pos 0
        // and pos 32 instead of 5 dirty spill ops. Reversal mirrors this.
        let m = b.alloc_qubits(32);
        shift22_compute_m_977(b, &m, &spill, k, false);
        b.set_phase("shift22_cuccaro_op_0");
        shift22_spill_op_dirty(b, &v_ext, &m, 0, 32, false, dirty);
        b.set_phase("shift22_cuccaro_op_32");
        shift22_spill_op_dirty(b, &v_ext, &spill, 32, k, false, dirty);
        shift22_compute_m_977(b, &m, &spill, k, true);
        b.free_vec(&m);
    } else {
        b.set_phase("shift22_cuccaro_op_0");
        shift22_spill_op_dirty(b, &v_ext, &spill, 0, k, false, dirty);
        b.set_phase("shift22_cuccaro_op_4");
        shift22_spill_op_dirty(b, &v_ext, &spill, 4, k, false, dirty);
        b.set_phase("shift22_cuccaro_op_6");
        shift22_spill_op_dirty(b, &v_ext, &spill, 6, k, true, dirty);
        b.set_phase("shift22_cuccaro_op_10");
        shift22_spill_op_dirty(b, &v_ext, &spill, 10, k, false, dirty);
        b.set_phase("shift22_cuccaro_op_32");
        shift22_spill_op_dirty(b, &v_ext, &spill, 32, k, false, dirty);
    }

    // Step 3: unconditional const add of c (register-free venting dirty-borrow).
    b.set_phase("shift22_step3");
    {
        let m = v_ext.len();
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::iadd_dirty_2clean_classical(
            b, &v_ext, &dirty[..m - 2], &q_clean2, c_low, false,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    }
    b.x(ovf);
    b.cx(ovf, flag_inv); // flag_inv = NOT(top_bit_after_add) = (value < p)
    b.x(ovf);

    // Step 4: conditional const sub of c (register-free venting dirty-borrow).
    b.set_phase("shift22_step4");
    {
        let m = v_ext.len();
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::cisub_dirty_2clean_classical(
            b, &v_ext, &dirty[..m - 2], &q_clean2, c_low, flag_inv,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    }
    b.x(flag_inv);
    b.cx(flag_inv, ovf);
    b.x(flag_inv);

    (spill, flag_inv, ovf)
}

/// Gate-level inverse of `mod_shift_left_by_k_dirty`.
pub(crate) fn mod_shift_right_by_k_dirty(
    b: &mut B,
    v: &[QubitId],
    p: U256,
    k: usize,
    spill: Vec<QubitId>,
    flag_inv: QubitId,
    ovf: QubitId,
    dirty: &[QubitId],
) {
    let n = v.len();
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));
    let c_low = c.as_limbs()[0];

    let mut v_ext = v.to_vec();
    v_ext.push(ovf);

    // Reverse step 4.
    b.x(flag_inv);
    b.cx(flag_inv, ovf);
    b.x(flag_inv);
    b.set_phase("rshift22_rev_step4");
    {
        // inverse of cisub(c, flag_inv) is ciadd(c, flag_inv): if flag_inv: x += c.
        let m = v_ext.len();
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        venting::ciadd_dirty_2clean_classical(
            b, &v_ext, &dirty[..m - 2], &q_clean2, c_low, flag_inv, false,
        );
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    }

    // Reverse step 3.
    b.x(ovf);
    b.cx(ovf, flag_inv);
    b.x(ovf);
    b.set_phase("rshift22_rev_step3");
    {
        // inverse of iadd(c) is isub(c) == invert; iadd(c); invert.
        let m = v_ext.len();
        let q_clean2: [QubitId; 2] = [b.alloc_qubit(), b.alloc_qubit()];
        for &q in v_ext.iter() {
            b.x(q);
        }
        venting::iadd_dirty_2clean_classical(
            b, &v_ext, &dirty[..m - 2], &q_clean2, c_low, false,
        );
        for &q in v_ext.iter() {
            b.x(q);
        }
        b.free(q_clean2[0]);
        b.free(q_clean2[1]);
    }
    b.free(flag_inv);
    b.set_phase("rshift22_rev_step2");

    // Reverse step 2: undo the spill ops in reverse order with flipped signs.
    if shift22_collapse() && k <= 22 {
        // Reverse the COLLAPSE: recompute m, undo pos 32 then pos 0, uncompute m.
        let m = b.alloc_qubits(32);
        shift22_compute_m_977(b, &m, &spill, k, false);
        shift22_spill_op_dirty(b, &v_ext, &spill, 32, k, true, dirty); // undo +spill*2^32
        shift22_spill_op_dirty(b, &v_ext, &m, 0, 32, true, dirty); // undo +m=spill*977
        shift22_compute_m_977(b, &m, &spill, k, true);
        b.free_vec(&m);
    } else {
        shift22_spill_op_dirty(b, &v_ext, &spill, 32, k, true, dirty); // undo +2^32
        shift22_spill_op_dirty(b, &v_ext, &spill, 10, k, true, dirty); // undo +2^10
        shift22_spill_op_dirty(b, &v_ext, &spill, 6, k, false, dirty); // undo -2^6
        shift22_spill_op_dirty(b, &v_ext, &spill, 4, k, true, dirty); // undo +2^4
        shift22_spill_op_dirty(b, &v_ext, &spill, 0, k, true, dirty); // undo +2^0
    }

    // Reverse step 1: reverse swap cascades.
    for shift_i in (0..k).rev() {
        for i in 0..n - 1 {
            b.swap(v[i], v[i + 1]);
        }
        b.swap(v[n - 1], spill[k - 1 - shift_i]);
    }

    b.free(ovf);
    b.free_vec(&spill);
}

pub(crate) fn mod_shift_left_by_k_lowq(
    b: &mut B,
    v: &[QubitId],
    p: U256,
    k: usize,
) -> (Vec<QubitId>, QubitId, QubitId) {
    let n = v.len();
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));

    let spill = b.alloc_qubits(k);
    let ovf = b.alloc_qubit();
    let flag_inv = b.alloc_qubit();

    for shift_i in 0..k {
        b.swap(v[n - 1], spill[k - 1 - shift_i]);
        for i in (0..n - 1).rev() {
            b.swap(v[i], v[i + 1]);
        }
    }

    let mut v_ext = v.to_vec();
    v_ext.push(ovf);
    let cuccaro_op = |b: &mut B, pos: usize, is_sub: bool| {
        let pad_width = n + 1 - pos;
        let padded = b.alloc_qubits(pad_width);
        for i in 0..k.min(pad_width) {
            b.cx(spill[i], padded[i]);
        }
        let v_slice: Vec<QubitId> = v_ext[pos..n + 1].to_vec();
        let c_in = b.alloc_qubit();
        if is_sub {
            cuccaro_sub(b, &padded, &v_slice, c_in);
        } else {
            cuccaro_add(b, &padded, &v_slice, c_in);
        }
        b.free(c_in);
        for i in 0..k.min(pad_width) {
            b.cx(spill[i], padded[i]);
        }
        b.free_vec(&padded);
    };
    cuccaro_op(b, 0, false);
    cuccaro_op(b, 4, false);
    cuccaro_op(b, 6, true);
    cuccaro_op(b, 10, false);
    cuccaro_op(b, 32, false);

    add_nbit_const(b, &v_ext, c);
    b.x(ovf);
    b.cx(ovf, flag_inv);
    b.x(ovf);
    csub_nbit_const(b, &v_ext, c, flag_inv);
    b.x(flag_inv);
    b.cx(flag_inv, ovf);
    b.x(flag_inv);

    (spill, flag_inv, ovf)
}

pub(crate) fn mod_shift_right_by_k_lowq(
    b: &mut B,
    v: &[QubitId],
    p: U256,
    k: usize,
    spill: Vec<QubitId>,
    flag_inv: QubitId,
    ovf: QubitId,
) {
    let n = v.len();
    debug_assert_eq!(n, 256);
    let c = U256::MAX.wrapping_sub(p).wrapping_add(U256::from(1));

    let mut v_ext = v.to_vec();
    v_ext.push(ovf);

    b.x(flag_inv);
    b.cx(flag_inv, ovf);
    b.x(flag_inv);
    cadd_nbit_const(b, &v_ext, c, flag_inv);

    b.x(ovf);
    b.cx(ovf, flag_inv);
    b.x(ovf);
    sub_nbit_const(b, &v_ext, c);
    b.free(flag_inv);

    let cuccaro_op = |b: &mut B, pos: usize, is_sub: bool| {
        let pad_width = n + 1 - pos;
        let padded = b.alloc_qubits(pad_width);
        for i in 0..k.min(pad_width) {
            b.cx(spill[i], padded[i]);
        }
        let v_slice: Vec<QubitId> = v_ext[pos..n + 1].to_vec();
        let c_in = b.alloc_qubit();
        if is_sub {
            cuccaro_sub(b, &padded, &v_slice, c_in);
        } else {
            cuccaro_add(b, &padded, &v_slice, c_in);
        }
        b.free(c_in);
        for i in 0..k.min(pad_width) {
            b.cx(spill[i], padded[i]);
        }
        b.free_vec(&padded);
    };
    cuccaro_op(b, 32, true);
    cuccaro_op(b, 10, true);
    cuccaro_op(b, 6, false);
    cuccaro_op(b, 4, true);
    cuccaro_op(b, 0, true);

    for shift_i in (0..k).rev() {
        for i in 0..n - 1 {
            b.swap(v[i], v[i + 1]);
        }
        b.swap(v[n - 1], spill[k - 1 - shift_i]);
    }

    b.free(ovf);
    b.free_vec(&spill);
}


// ═══════════════════════════════════════════════════════════════════════════
//  Merged from compare.rs
// ═══════════════════════════════════════════════════════════════════════════

// (refactor) Mechanically extracted from mod.rs. No logic changes.

// ═══════════════════════════════════════════════════════════════════════════
//  Kaliski almost-inverse
// ═══════════════════════════════════════════════════════════════════════════

/// Fredkin (controlled swap): swap (a, t) if ctrl. Decomposed as CX/CCX/CX.
pub(crate) fn cswap(b: &mut B, ctrl: QubitId, a: QubitId, t: QubitId) {
    b.cx(t, a);
    b.ccx(ctrl, a, t);
    b.cx(t, a);
}

/// Run `body` with `flag` holding (u < v), then uncompute the flag and
/// restore u, v. Uses carry-ancilla + measurement-based uncomputation
/// for the inv_MAJ sweep (0 Toffoli instead of n CCX).
/// Cost ≈ n CCX (forward MAJ) + body + 0 CCX (measurement inv_MAJ).
pub(crate) fn with_lt<F: FnOnce(&mut B)>(b: &mut B, u: &[QubitId], v: &[QubitId], flag: QubitId, body: F) {
    let n = u.len();
    assert_eq!(n, v.len());
    let c_in = b.alloc_qubit();
    let carries = b.alloc_qubits(n);
    for i in 0..n {
        b.x(u[i]);
    }

    // Forward MAJ sweep with separate carry ancillae.
    // maj_with_carry: CX(w,y); CX(w,x); CCX(x_new,y_new,carry); CX(carry,w)
    // Step 0: (x=c_in, y=v[0], w=u[0])
    b.cx(u[0], v[0]);
    b.cx(u[0], c_in);
    b.ccx(c_in, v[0], carries[0]);
    b.cx(carries[0], u[0]);
    // Steps 1..n-1: (x=u[i-1], y=v[i], w=u[i])
    for i in 1..n {
        b.cx(u[i], v[i]);
        b.cx(u[i], u[i - 1]);
        b.ccx(u[i - 1], v[i], carries[i]);
        b.cx(carries[i], u[i]);
    }

    b.cx(u[n - 1], flag);
    body(b);
    b.cx(u[n - 1], flag);

    // Backward inv_MAJ sweep with measurement-based carry uncompute (0 Toffoli).
    // inv_maj_with_carry: CX(carry,w); HMR+CZ(carry,x,y); CX(w,x); CX(w,y)
    for i in (1..n).rev() {
        b.cx(carries[i], u[i]); // restore w = u[i]
        let m = b.alloc_bit();
        b.hmr(carries[i], m); // measure carry
        b.cz_if(u[i - 1], v[i], m); // phase correction
        b.cx(u[i], u[i - 1]); // restore x = u[i-1]
        b.cx(u[i], v[i]); // restore y = v[i]
    }
    // Step 0: (x=c_in, y=v[0], w=u[0])
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

/// Symmetric helper: runs `body` with `flag` holding (u > v).
pub(crate) fn with_gt<F: FnOnce(&mut B)>(b: &mut B, u: &[QubitId], v: &[QubitId], flag: QubitId, body: F) {
    with_lt(b, v, u, flag, body)
}

/// flag ^= (u < v).  Non-destructive on u and v.
///
/// Uses a MAJ-only carry chain instead of the full sub+add pattern.
/// Identity: u < v iff carry-out of (~u + v) = 1, since
///   ~u + v = (2^n - 1 - u) + v = (v - u) + (2^n - 1)
/// which overflows 2^n iff v - u ≥ 1 iff v > u. We negate u in place,
/// run a forward MAJ sweep over (~u, v, c_in=0), capture u[n-1] (which
/// holds the high carry after the chain), then run the inverse MAJ
/// sweep + un-negate to restore u and v. Cost ≈ 2n CCX, half of the
/// previous sub+add (≈ 4n CCX).
pub(crate) fn cmp_lt_into(b: &mut B, u: &[QubitId], v: &[QubitId], flag: QubitId) {
    let n = u.len();
    assert_eq!(n, v.len());

    let c_in = b.alloc_qubit();

    // ~u in place (X is free in the metric).
    for i in 0..n {
        b.x(u[i]);
    }

    // Forward MAJ sweep — n MAJs (one more than cuccaro_add, which omits
    // the top one because it doesn't need the carry-out).
    maj(b, c_in, v[0], u[0]);
    for i in 1..n {
        maj(b, u[i - 1], v[i], u[i]);
    }
    // u[n-1] now holds the high carry = (u < v).
    b.cx(u[n - 1], flag);

    // Inverse sweep restores u and v to their (negated u) state.
    for i in (1..n).rev() {
        inv_maj(b, u[i - 1], v[i], u[i]);
    }
    inv_maj(b, c_in, v[0], u[0]);

    // Un-negate u.
    for i in 0..n {
        b.x(u[i]);
    }

    b.free(c_in);
}

/// flag ^= (v != 0). Computes OR of all bits of v into a scratch ancilla,
/// CXs into flag, then properly uncomputes the scratch.
///
/// We use the simple chain: `or[0] = v[0]`, `or[i] = or[i-1] OR v[i]`.
/// OR via de Morgan: `or[i] = NOT((NOT or[i-1]) AND (NOT v[i]))`, i.e.
///   x(or[i-1]); x(v[i]); ccx(or[i-1], v[i], or[i]); x(or[i]);
///   x(v[i]); x(or[i-1]);
/// Each `or[i]` is a fresh ancilla. We compute the chain, CX `or[n-1]`
/// into `flag`, then reverse the chain to return every ancilla to |0⟩.
pub(crate) fn cmp_neq_zero_into(b: &mut B, v: &[QubitId], flag: QubitId) {
    let n = v.len();
    assert!(n > 0);
    if n == 1 {
        b.cx(v[0], flag);
        return;
    }

    let or_chain: Vec<QubitId> = b.alloc_qubits(n - 1);
    // or_chain[0] = v[0] OR v[1]
    or_step(b, v[0], v[1], or_chain[0]);
    for i in 1..n - 1 {
        or_step(b, or_chain[i - 1], v[i + 1], or_chain[i]);
    }

    // flag ^= or_chain[n-2]
    b.cx(or_chain[n - 2], flag);

    // Uncompute.
    for i in (1..n - 1).rev() {
        or_step(b, or_chain[i - 1], v[i + 1], or_chain[i]);
    }
    or_step(b, v[0], v[1], or_chain[0]);

    b.free_vec(&or_chain);
}

/// out ^= (x OR y). `out` starts 0. Uses the de-Morgan form:
///   x(x); x(y); ccx(x, y, out); x(out); x(y); x(x);
/// After this, out = x OR y (assuming out started at 0). Its inverse is
/// the same gate sequence run in reverse — since it's symmetric (all gates
/// involutions, palindromic structure), running the exact same helper
/// again uncomputes it.
pub(crate) fn or_step(b: &mut B, x: QubitId, y: QubitId, out: QubitId) {
    b.x(x);
    b.x(y);
    b.ccx(x, y, out);
    b.x(out);
    b.x(y);
    b.x(x);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Primitives for the Kaliski port (qrisp-style)
// ═══════════════════════════════════════════════════════════════════════════

/// 2-controlled X with per-control polarity. `polarity=true` means positive
/// control; `false` means anti-control (ctrl=0 triggers).
pub(crate) fn mcx2_polar(b: &mut B, c1: QubitId, p1: bool, c2: QubitId, p2: bool, target: QubitId) {
    if !p1 {
        b.x(c1);
    }
    if !p2 {
        b.x(c2);
    }
    b.ccx(c1, c2, target);
    if !p2 {
        b.x(c2);
    }
    if !p1 {
        b.x(c1);
    }
}

/// 3-controlled X with per-control polarity. Uses a borrowed scratch qubit
/// (must be supplied clean, returns clean).
pub(crate) fn mcx3_polar(
    b: &mut B,
    c1: QubitId,
    p1: bool,
    c2: QubitId,
    p2: bool,
    c3: QubitId,
    p3: bool,
    target: QubitId,
    scratch: QubitId,
) {
    if !p1 {
        b.x(c1);
    }
    if !p2 {
        b.x(c2);
    }
    if !p3 {
        b.x(c3);
    }
    b.ccx(c1, c2, scratch);
    b.ccx(scratch, c3, target);
    b.ccx(c1, c2, scratch);
    if !p3 {
        b.x(c3);
    }
    if !p2 {
        b.x(c2);
    }
    if !p1 {
        b.x(c1);
    }
}

/// flag ^= (u > v).  Symmetric to cmp_lt_into(v, u, flag).
pub(crate) fn cmp_gt_into(b: &mut B, u: &[QubitId], v: &[QubitId], flag: QubitId) {
    cmp_lt_into(b, v, u, flag);
}

/// Controlled n-bit subtract mod 2^n: if ctrl, acc -= a. Both are n-wide
/// qubit slices. Not a mod-p operation.
pub(crate) fn cucc_sub_ctrl(b: &mut B, a: &[QubitId], acc: &[QubitId], ctrl: QubitId) {
    let n = a.len();
    let tmp = b.alloc_qubits(n);
    for i in 0..n {
        b.ccx(ctrl, a[i], tmp[i]);
    }
    sub_nbit_qq(b, &tmp, acc);
    for i in 0..n {
        b.ccx(ctrl, a[i], tmp[i]);
    }
    b.free_vec(&tmp);
}

/// Controlled n-bit add mod 2^n: if ctrl, acc += a.
pub(crate) fn cucc_add_ctrl(b: &mut B, a: &[QubitId], acc: &[QubitId], ctrl: QubitId) {
    let n = a.len();
    let tmp = b.alloc_qubits(n);
    for i in 0..n {
        b.ccx(ctrl, a[i], tmp[i]);
    }
    add_nbit_qq(b, &tmp, acc);
    for i in 0..n {
        b.ccx(ctrl, a[i], tmp[i]);
    }
    b.free_vec(&tmp);
}

/// Inverse of `shift_left_1`: shifts an (n+1)-bit register right by 1.
/// ASSUMES r[0]=0 before the shift (i.e., was even).
#[allow(dead_code)]
pub(crate) fn shift_right_1(b: &mut B, r: &[QubitId]) {
    let n1 = r.len();
    for i in 2..n1 {
        b.swap(r[i], r[i - 1]);
    }
    b.swap(r[n1 - 1], r[0]);
}

/// Classical modular inverse via Fermat's little theorem. Used ONLY at
/// circuit-construction time to compute correction constants.
#[allow(dead_code)]
pub(crate) fn classical_modinv(a: U256, p: U256) -> U256 {
    // a^(p-2) mod p via square-and-multiply.
    let exponent = p.wrapping_sub(U256::from(2));
    let mut result = U256::from(1);
    let mut base = a % p;
    for i in 0..256 {
        if exponent.bit(i) {
            result = mulmod(result, base, p);
        }
        base = mulmod(base, base, p);
    }
    result
}

/// Classical modular multiplication used to compute correction constants
/// at build time.
pub(crate) fn mulmod(a: U256, b: U256, p: U256) -> U256 {
    // Naive (a * b) mod p — both < p < 2^256, so the product may overflow
    // 256 bits. Use U256's widening mul if available; else do it in u512
    // via chunks. alloy's U256 has `mul_mod`.
    a.mul_mod(b, p)
}


/// Classical: compute `2^k mod p`.
pub(crate) fn pow_mod_2_k(p: U256, k: usize) -> U256 {
    let mut r = U256::from(1);
    let two = U256::from(2);
    for _ in 0..k {
        r = mulmod(r, two, p);
    }
    r
}

#[allow(dead_code)]
pub(crate) fn secp256k1_curve() -> WeierstrassEllipticCurve {
    WeierstrassEllipticCurve {
        modulus: U256::from_str_radix(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F",
            16,
        )
        .unwrap(),
        a: U256::from(0),
        b: U256::from(7),
        gx: U256::from_str_radix(
            "79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798",
            16,
        )
        .unwrap(),
        gy: U256::from_str_radix(
            "483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8",
            16,
        )
        .unwrap(),
        order: U256::from_str_radix(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141",
            16,
        )
        .unwrap(),
    }
}

#[allow(dead_code)]
pub(crate) fn alt_seed_xof(ops: &[Op], tag: u64) -> sha3::Shake256Reader {
    let mut hasher = Shake256::default();
    hasher.update(b"quantum_ecc-alt-seed-v1");
    hasher.update(&tag.to_le_bytes());
    hasher.update(&(ops.len() as u64).to_le_bytes());
    for op in ops {
        hasher.update(&[op.kind as u8]);
        hasher.update(&op.q_control2.0.to_le_bytes());
        hasher.update(&op.q_control1.0.to_le_bytes());
        hasher.update(&op.q_target.0.to_le_bytes());
        hasher.update(&op.c_target.0.to_le_bytes());
        hasher.update(&op.c_condition.0.to_le_bytes());
        hasher.update(&op.r_target.0.to_le_bytes());
    }
    hasher.finalize_xof()
}


