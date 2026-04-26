//! Compact point-add circuit using Fermat inversion (x^{p-2} mod p)
//! with Horner-style modular multiplication.
//!
//! ARCHITECTURE:
//! - Replaces both Kaliski inversions with Fermat x^{p-2}
//! - Uses Horner mul (no 2n workspace, ~2n² CCX per mul)
//! - In-place squaring/multiplication via 3-register Bennett:
//!   C = A; B = A*A (Horner); swap(A,B); B -= C → 0; free B,C
//! - Peak estimate: ~1280-1536 qubits (vs 2716 current)
//! - Toffoli estimate: ~60-130M (vs 4.18M current)
//!
//! This represents the qubit-optimized frontier. Toffoli can be
//! improved later by replacing Fermat with a more efficient inversion
//! once the register layout is proven.

use alloy_primitives::U256;

use super::{
    bit, mod_add_qq_fast, mod_add_qq_fast_from_zero, mod_add_qb,
    mod_double_inplace_fast, mod_halve_inplace_fast, mod_neg_inplace_fast,
    mod_sub_qb, mod_sub_qq_fast, B, QubitId, SECP256K1_P,
};

/// Horner-style modular multiply: acc += x * y mod p.
///
/// Processes x bit-by-bit from LSB. For each set bit x[i],
/// adds y * 2^i mod p to acc. The 2^i factor is tracked by
/// doubling a working copy of y each iteration.
///
/// y is preserved. acc is modified (adds x*y into it).
/// Workspace: 1 working copy of y (n qubits) + n fanout (transient per bit).
/// Toffoli: ~2n² (n bits × n fanout CCX + n add CCX).
pub fn horner_mul_add(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let n = x.len();
    debug_assert_eq!(n, y.len());
    debug_assert_eq!(n, acc.len());

    // Working copy of y that we double each iteration
    let yw = b.alloc_qubits(n);
    for i in 0..n {
        b.cx(y[i], yw[i]);
    }

    for i in 0..n {
        // Controlled add: if x[i], acc += yw
        let fo = b.alloc_qubits(n);
        for j in 0..n {
            b.ccx(x[i], yw[j], fo[j]);
        }
        mod_add_qq_fast(b, acc, &fo, p);
        // HMR uncompute of fanout
        for j in 0..n {
            let m = b.alloc_bit();
            b.hmr(fo[j], m);
            b.cz_if(x[i], yw[j], m);
        }
        b.free_vec(&fo);

        // Double yw for next bit position (except last)
        if i < n - 1 {
            mod_double_inplace_fast(b, &yw, p);
        }
    }

    // Halve yw back to restore original y
    for _ in 0..(n - 1) {
        mod_halve_inplace_fast(b, &yw, p);
    }

    // Uncompute yw
    for i in 0..n {
        b.cx(y[i], yw[i]);
    }
    b.free_vec(&yw);
}

/// Horner-style modular multiply-subtract: acc -= x * y mod p.
pub fn horner_mul_sub(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    y: &[QubitId],
    p: U256,
) {
    let n = x.len();
    let yw = b.alloc_qubits(n);
    for i in 0..n {
        b.cx(y[i], yw[i]);
    }

    for i in 0..n {
        let fo = b.alloc_qubits(n);
        for j in 0..n {
            b.ccx(x[i], yw[j], fo[j]);
        }
        mod_sub_qq_fast(b, acc, &fo, p);
        for j in 0..n {
            let m = b.alloc_bit();
            b.hmr(fo[j], m);
            b.cz_if(x[i], yw[j], m);
        }
        b.free_vec(&fo);

        if i < n - 1 {
            mod_double_inplace_fast(b, &yw, p);
        }
    }

    for _ in 0..(n - 1) {
        mod_halve_inplace_fast(b, &yw, p);
    }

    for i in 0..n {
        b.cx(y[i], yw[i]);
    }
    b.free_vec(&yw);
}

/// In-place modular squaring: a = a² mod p.
///
/// Uses 3-register Bennett pattern:
/// 1. save = a (CX copy)
/// 2. tmp = 0; tmp += a * a (Horner)
/// 3. swap(a, tmp) → a = a², tmp = old_a
/// 4. tmp -= save → tmp = old_a - old_a = 0
/// 5. free tmp, save
///
/// Peak: a + tmp + save + Horner_workspace = 3n + n = 4n
pub fn mod_square_inplace(b: &mut B, a: &[QubitId], p: U256) {
    let n = a.len();

    // Step 1: save = a
    let save = b.alloc_qubits(n);
    for i in 0..n {
        b.cx(a[i], save[i]);
    }

    // Step 2: tmp = a * a (Horner write-into-zero)
    let tmp = b.alloc_qubits(n);
    horner_mul_add(b, &tmp, a, a, p);

    // Step 3: swap a, tmp
    for i in 0..n {
        b.swap(a[i], tmp[i]);
    }

    // Now: a = a², tmp = old_a, save = old_a
    // Step 4: tmp -= save → 0
    mod_sub_qq_fast(b, &tmp, &save, p);

    // Step 5: free tmp, save
    b.free_vec(&tmp);
    b.free_vec(&save);
}

/// In-place modular multiply: a = a * b mod p.
///
/// Same 3-register Bennett pattern as squaring.
pub fn mod_mul_inplace(b: &mut B, a: &[QubitId], b_reg: &[QubitId], p: U256) {
    let n = a.len();

    let save = b.alloc_qubits(n);
    for i in 0..n {
        b.cx(a[i], save[i]);
    }

    let tmp = b.alloc_qubits(n);
    horner_mul_add(b, &tmp, a, b_reg, p);

    for i in 0..n {
        b.swap(a[i], tmp[i]);
    }

    mod_sub_qq_fast(b, &tmp, &save, p);

    b.free_vec(&tmp);
    b.free_vec(&save);
}

/// Fermat inversion: result = x^{-1} mod p = x^{p-2} mod p.
///
/// Left-to-right square-and-multiply:
///   result = 1
///   for bit i of (p-2) from MSB-1 down to 0:
///     result = result²
///     if bit i is set: result = result * x
///
/// result register must be zero on entry (we load 1 into it).
/// x is preserved.
pub fn fermat_inv(
    b: &mut B,
    x: &[QubitId],
    result: &[QubitId],
    p: U256,
) {
    let n = x.len();
    let exp = p - U256::from(2u64); // p - 2

    // result = 1
    b.x(result[0]);

    // Find the highest set bit in exp
    let mut top_bit = 0usize;
    for i in 0..256 {
        if bit(exp, i) {
            top_bit = i;
        }
    }

    // Left-to-right: process bits from top_bit-1 down to 0
    for i in (0..top_bit).rev() {
        // Square: result = result²
        mod_square_inplace(b, result, p);

        // Conditional multiply: if exp[i] set, result *= x
        if bit(exp, i) {
            mod_mul_inplace(b, result, x, p);
        }
    }
}

/// In-place modular multiply-sub: a -= x * y mod p.
/// Uses 3-register Bennett: save=a; compute a-x*y into tmp; swap; tmp-=save=0.
pub fn mod_mul_sub_inplace(b: &mut B, a: &[QubitId], x: &[QubitId], y: &[QubitId], p: U256) {
    let n = a.len();

    let save = b.alloc_qubits(n);
    for i in 0..n {
        b.cx(a[i], save[i]);
    }

    // tmp = a - x*y. First: tmp = a (copy), then sub x*y.
    let tmp = b.alloc_qubits(n);
    for i in 0..n {
        b.cx(a[i], tmp[i]);
    }
    horner_mul_sub(b, &tmp, x, y, p);

    // swap a, tmp
    for i in 0..n {
        b.swap(a[i], tmp[i]);
    }

    // Now: a = old_a - x*y, tmp = old_a, save = old_a
    // Zero tmp: tmp -= save = 0
    mod_sub_qq_fast(b, &tmp, &save, p);

    b.free_vec(&tmp);
    b.free_vec(&save);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::{analyze_ops, Op};
    use crate::sim::Simulator;
    use crate::weierstrass_elliptic_curve::WeierstrassEllipticCurve;
    use alloy_primitives::U256;
    use sha3::digest::{ExtendableOutput, Update, XofReader};
    use sha3::Shake256;

    fn secp256k1() -> WeierstrassEllipticCurve {
        WeierstrassEllipticCurve {
            modulus: SECP256K1_P,
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

    /// Test Horner mul at small n
    #[test]
    fn test_horner_mul_small() {
        let p = U256::from(13u64);
        // Test: 7 * 5 mod 13 = 35 mod 13 = 9
        let n = 4; // small for speed

        let mut b = B::new();
        let x = b.alloc_qubits(n);
        let y = b.alloc_qubits(n);
        let acc = b.alloc_qubits(n);

        // Load x = 7 = 0111
        b.x(x[0]); b.x(x[1]); b.x(x[2]);
        // Load y = 5 = 0101
        b.x(y[0]); b.x(y[2]);

        horner_mul_add(&mut b, &acc, &x, &y, p);

        // Check acc = 9 = 1001
        let (total_qubits, _num_bits, _num_regs, regs) = analyze_ops(b.ops.iter().copied());

        let mut xof_seed = [0u8; 32];
        let mut xof = Shake256::default().chain(&xof_seed).finalize_xof();
        let mut sim = Simulator::new(total_qubits as usize, _num_bits as usize, &mut xof);

        // Set x=7, y=5 on shot 0
        for i in 0..n {
            if [true, true, true, false][i] {
                *sim.qubit_mut(x[i]) |= 1;
            }
            if [true, false, true, false][i] {
                *sim.qubit_mut(y[i]) |= 1;
            }
        }

        sim.apply(&b.ops);

        let acc_val = (0..n).fold(0u64, |v, i| {
            v | if (*sim.qubit_mut(acc[i]) & 1) != 0 { 1 << i } else { 0 }
        });

        assert_eq!(acc_val, 9, "7 * 5 mod 13 should be 9, got {}", acc_val);
    }
}
