#![cfg(test)]

//! Exact reversible prototype for a very small slice of the "coset/padded"
//! idea: accumulate several classical-register additions in an `(n + c_pad)`-
//! bit non-modular workspace, then canonicalize once at the end by folding the
//! high padding bits into a fresh output register.
//!
//! This is NOT a full coset implementation. Its purpose is to answer a sharp
//! question quickly:
//!   For short chains of modular additions, does "padded non-mod adds + one
//!   exact Bennett-clean canonicalization" beat repeated direct `mod_add_qb`?
//!
//! If not, then the top-level affine constant-correction chain is not a good
//! first landing spot for the coset idea, and we should reserve coset work for
//! much longer arithmetic regions (e.g. inversion internals / QROM-windowed
//! paths).

use alloy_primitives::U256;

use super::{
    add_nbit_qq_fast, cmod_add_qq, load_bits, load_const, mod_add_qb, mod_add_qq_fast,
    sub_nbit_qq_fast, unload_bits, unload_const, QubitId, B, N, SECP256K1_P,
};
use crate::circuit::OperationType;
use crate::sim::Simulator;
use sha3::{
    digest::{ExtendableOutput, Update},
    Shake256,
};

fn count_toffoli(ops: &[crate::circuit::Op]) -> usize {
    ops.iter()
        .filter(|o| matches!(o.kind, OperationType::CCX | OperationType::CCZ))
        .count()
}

fn c_secp256k1() -> U256 {
    U256::MAX
        .wrapping_sub(SECP256K1_P)
        .wrapping_add(U256::from(1u64))
}

fn nonmod_add_bits_padded(b: &mut B, wide: &[QubitId], bits: &[super::BitId]) {
    let cpad = wide.len() - bits.len();
    let a = load_bits(b, bits);
    let zeros = b.alloc_qubits(cpad);
    let mut a_pad = a.clone();
    a_pad.extend_from_slice(&zeros);
    add_nbit_qq_fast(b, &a_pad, wide);
    unload_bits(b, &a, bits);
    b.free_vec(&zeros);
}

fn nonmod_sub_bits_padded(b: &mut B, wide: &[QubitId], bits: &[super::BitId]) {
    let cpad = wide.len() - bits.len();
    let a = load_bits(b, bits);
    let zeros = b.alloc_qubits(cpad);
    let mut a_pad = a.clone();
    a_pad.extend_from_slice(&zeros);
    sub_nbit_qq_fast(b, &a_pad, wide);
    unload_bits(b, &a, bits);
    b.free_vec(&zeros);
}

fn nonmod_add_qureg_padded(b: &mut B, wide: &[QubitId], a: &[QubitId]) {
    let cpad = wide.len() - a.len();
    let zeros = b.alloc_qubits(cpad);
    let mut a_pad = a.to_vec();
    a_pad.extend_from_slice(&zeros);
    add_nbit_qq_fast(b, &a_pad, wide);
    b.free_vec(&zeros);
}

fn nonmod_sub_qureg_padded(b: &mut B, wide: &[QubitId], a: &[QubitId]) {
    let cpad = wide.len() - a.len();
    let zeros = b.alloc_qubits(cpad);
    let mut a_pad = a.to_vec();
    a_pad.extend_from_slice(&zeros);
    sub_nbit_qq_fast(b, &a_pad, wide);
    b.free_vec(&zeros);
}

/// Compute `out = wide mod p` where `wide` is an `(n + c_pad)`-bit padded
/// integer and `p = 2^n - c` is secp256k1's modulus. Uses the exact identity
/// `2^(n+i) ≡ (c << i) (mod p)`.
fn canonicalize_padded_into_fresh(b: &mut B, out: &[QubitId], wide: &[QubitId], p: U256) {
    let n = out.len();
    let cpad = wide.len() - n;
    for i in 0..n {
        b.cx(wide[i], out[i]);
    }
    let c = c_secp256k1();
    for i in 0..cpad {
        let add_const = (c << i) % p;
        let k = load_const(b, n, add_const);
        cmod_add_qq(b, out, &k, wide[n + i], p);
        unload_const(b, &k, add_const);
    }
}

/// Exact Bennett-clean fresh-output prototype for `out = x + reps * bits mod p`.
///
/// Keeps `x` unchanged. The padded workspace is fully uncomputed at the end.
fn coset_add_qb_repeated_into_fresh(
    b: &mut B,
    out: &[QubitId],
    x: &[QubitId],
    bits: &[super::BitId],
    reps: usize,
    cpad: usize,
    p: U256,
) {
    let n = x.len();
    let wide = b.alloc_qubits(n + cpad);

    for i in 0..n {
        b.cx(x[i], wide[i]);
    }
    for _ in 0..reps {
        nonmod_add_bits_padded(b, &wide, bits);
    }
    canonicalize_padded_into_fresh(b, out, &wide, p);
    for _ in 0..reps {
        nonmod_sub_bits_padded(b, &wide, bits);
    }
    for i in 0..n {
        b.cx(x[i], wide[i]);
    }
    b.free_vec(&wide);
}

fn coset_add_qq_repeated_into_fresh(
    b: &mut B,
    out: &[QubitId],
    x: &[QubitId],
    a: &[QubitId],
    reps: usize,
    cpad: usize,
    p: U256,
) {
    let n = x.len();
    let wide = b.alloc_qubits(n + cpad);

    for i in 0..n {
        b.cx(x[i], wide[i]);
    }
    for _ in 0..reps {
        nonmod_add_qureg_padded(b, &wide, a);
    }
    canonicalize_padded_into_fresh(b, out, &wide, p);
    for _ in 0..reps {
        nonmod_sub_qureg_padded(b, &wide, a);
    }
    for i in 0..n {
        b.cx(x[i], wide[i]);
    }
    b.free_vec(&wide);
}

fn get_u256<R: sha3::digest::XofReader>(sim: &Simulator<'_, R>, reg: &[QubitId]) -> U256 {
    let mut out = U256::ZERO;
    for i in 0..reg.len() {
        if (sim.qubit(reg[i]) & 1) != 0 {
            out |= U256::from(1u64) << i;
        }
    }
    out
}

fn set_u256<R: sha3::digest::XofReader>(sim: &mut Simulator<'_, R>, reg: &[QubitId], x: U256) {
    for i in 0..reg.len() {
        if x.bit(i) {
            *sim.qubit_mut(reg[i]) |= 1;
        }
    }
}

#[test]
fn coset_repeated_add_qb_matches_direct_for_secp256k1_n256() {
    let p = SECP256K1_P;
    let reps = 3usize;
    let cpad = 2usize;

    // Direct circuit.
    let mut b1 = B::new();
    let x1 = b1.alloc_qubits(N);
    let bits1 = b1.alloc_bits(N);
    let out1 = b1.alloc_qubits(N);
    for i in 0..N {
        b1.cx(x1[i], out1[i]);
    }
    for _ in 0..reps {
        mod_add_qb(&mut b1, &out1, &bits1, p);
    }
    let ops1 = b1.ops.clone();
    let nq1 = b1.next_qubit as usize;
    let nb1 = b1.next_bit as usize;

    // Coset/padded fresh-output circuit.
    let mut b2 = B::new();
    let x2 = b2.alloc_qubits(N);
    let bits2 = b2.alloc_bits(N);
    let out2 = b2.alloc_qubits(N);
    coset_add_qb_repeated_into_fresh(&mut b2, &out2, &x2, &bits2, reps, cpad, p);
    let ops2 = b2.ops.clone();
    let nq2 = b2.next_qubit as usize;
    let nb2 = b2.next_bit as usize;

    let mut rng = 0x1234_5678_9abc_def0u64;
    for trial in 0..8 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let x = U256::from(rng) | (U256::from(rng.rotate_left(17)) << 64);
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let b_raw = U256::from(rng) | (U256::from(rng.rotate_left(29)) << 64);
        let x = x % p;
        let b: U256 = b_raw % p;

        let mut seed = Shake256::default();
        seed.update(b"coset-direct");
        seed.update(&(trial as u32).to_le_bytes());
        let mut xof = seed.finalize_xof();
        let mut sim1 = Simulator::new(nq1, nb1, &mut xof);
        set_u256(&mut sim1, &x1, x);
        for i in 0..N {
            if b.bit(i) {
                *sim1.bit_mut(bits1[i]) = 1;
            }
        }
        sim1.apply(&ops1);
        let got1 = get_u256(&sim1, &out1);

        let mut seed = Shake256::default();
        seed.update(b"coset-proto");
        seed.update(&(trial as u32).to_le_bytes());
        let mut xof = seed.finalize_xof();
        let mut sim2 = Simulator::new(nq2, nb2, &mut xof);
        set_u256(&mut sim2, &x2, x);
        for i in 0..N {
            if b.bit(i) {
                *sim2.bit_mut(bits2[i]) = 1;
            }
        }
        sim2.apply(&ops2);
        let got2 = get_u256(&sim2, &out2);

        assert_eq!(
            got2, got1,
            "trial {trial}: coset proto disagrees with direct modular chain"
        );
        assert_eq!(
            got1,
            (x + b + b + b) % p,
            "trial {trial}: direct chain wrong"
        );
    }
}

#[test]
fn coset_proto_cost_repeated_add_qb_n256() {
    let p = SECP256K1_P;
    for (reps, cpad) in [
        (3usize, 2usize),
        (8usize, 4usize),
        (12usize, 4usize),
        (16usize, 5usize),
        (32usize, 6usize),
        (64usize, 7usize),
        (256usize, 9usize),
    ] {
        let mut direct = B::new();
        let x = direct.alloc_qubits(N);
        let bits = direct.alloc_bits(N);
        let out = direct.alloc_qubits(N);
        for i in 0..N {
            direct.cx(x[i], out[i]);
        }
        let start = direct.ops.len();
        for _ in 0..reps {
            mod_add_qb(&mut direct, &out, &bits, p);
        }
        let direct_ccx = count_toffoli(&direct.ops[start..]);
        let direct_peak = direct.peak_qubits;

        let mut coset = B::new();
        let x = coset.alloc_qubits(N);
        let bits = coset.alloc_bits(N);
        let out = coset.alloc_qubits(N);
        let start = coset.ops.len();
        coset_add_qb_repeated_into_fresh(&mut coset, &out, &x, &bits, reps, cpad, p);
        let coset_ccx = count_toffoli(&coset.ops[start..]);
        let coset_peak = coset.peak_qubits;

        eprintln!(
            "coset_proto qb reps={} cpad={} | direct_ccx={} direct_peak={} | coset_ccx={} coset_peak={} | delta_ccx={:+} delta_peak={:+}",
            reps,
            cpad,
            direct_ccx,
            direct_peak,
            coset_ccx,
            coset_peak,
            coset_ccx as i64 - direct_ccx as i64,
            coset_peak as i64 - direct_peak as i64,
        );
    }
}

#[test]
fn coset_proto_cost_repeated_add_qq_n256() {
    let p = SECP256K1_P;
    for (reps, cpad) in [
        (3usize, 2usize),
        (8usize, 4usize),
        (12usize, 4usize),
        (16usize, 5usize),
        (32usize, 6usize),
        (64usize, 7usize),
        (256usize, 9usize),
    ] {
        let mut direct = B::new();
        let x = direct.alloc_qubits(N);
        let a = direct.alloc_qubits(N);
        let out = direct.alloc_qubits(N);
        for i in 0..N {
            direct.cx(x[i], out[i]);
        }
        let start = direct.ops.len();
        for _ in 0..reps {
            mod_add_qq_fast(&mut direct, &out, &a, p);
        }
        let direct_ccx = count_toffoli(&direct.ops[start..]);
        let direct_peak = direct.peak_qubits;

        let mut coset = B::new();
        let x = coset.alloc_qubits(N);
        let a = coset.alloc_qubits(N);
        let out = coset.alloc_qubits(N);
        let start = coset.ops.len();
        coset_add_qq_repeated_into_fresh(&mut coset, &out, &x, &a, reps, cpad, p);
        let coset_ccx = count_toffoli(&coset.ops[start..]);
        let coset_peak = coset.peak_qubits;

        eprintln!(
            "coset_proto qq reps={} cpad={} | direct_ccx={} direct_peak={} | coset_ccx={} coset_peak={} | delta_ccx={:+} delta_peak={:+}",
            reps,
            cpad,
            direct_ccx,
            direct_peak,
            coset_ccx,
            coset_peak,
            coset_ccx as i64 - direct_ccx as i64,
            coset_peak as i64 - direct_peak as i64,
        );
    }
}
