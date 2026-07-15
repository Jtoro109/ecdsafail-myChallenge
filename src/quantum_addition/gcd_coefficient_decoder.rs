//! Bounded positive quotient decoder for the half-GCD coefficient route.
//!
//! Forward promised relation:
//!
//!   numerator <- numerator mod denominator
//!   quotient  <- floor(numerator_in / denominator), little-endian
//!   denominator is unchanged
//!
//! Preconditions are deliberately narrow and match the coefficient-decoder
//! use case: denominator is positive, quotient targets are clean, quotient
//! fits `q_bits`, and `denominator << (q_bits - 1)` fits the lane width.

use super::{
    cmp_lt_into, cmp_lt_into_borrowed_cin, sub_nbit_qq, sub_nbit_qq_borrowed_cin, B, SECP256K1_P,
};
use crate::circuit::{OperationType, QubitId};
use alloy_primitives::{U256, U512};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HalfGcdCoeffDecoderCost {
    pub width: usize,
    pub q_bits: usize,
    pub peak_qubits: usize,
    pub toffoli_ops: usize,
    pub cx_ops: usize,
    pub x_ops: usize,
    pub r_ops: usize,
    pub hmr_ops: usize,
}

pub(super) fn halfgcd_coeff_decoder_formula(
    width: usize,
    q_bits: usize,
) -> HalfGcdCoeffDecoderCost {
    assert!(width > 0);
    assert!(q_bits > 0);
    HalfGcdCoeffDecoderCost {
        width,
        q_bits,
        peak_qubits: 3 * width + q_bits + 2,
        toffoli_ops: q_bits * (6 * width - 2) - 2 * q_bits * (q_bits - 1),
        cx_ops: q_bits * (10 * width + 1) - 3 * q_bits * (q_bits - 1),
        x_ops: q_bits * (2 * width + 4),
        r_ops: 2 * q_bits + width + 1,
        hmr_ops: 0,
    }
}

pub(super) fn halfgcd_coeff_decoder_bench(width: usize, q_bits: usize) -> HalfGcdCoeffDecoderCost {
    assert!(width > 0);
    assert!(q_bits > 0);

    let mut b = B::new();
    let numerator = b.alloc_qubits(width);
    let denominator = b.alloc_qubits(width);
    let quotient = b.alloc_qubits(q_bits);

    let start = b.ops.len();
    emit_halfgcd_coeff_quotient_decoder(&mut b, &numerator, &denominator, &quotient);
    let slice = &b.ops[start..];

    HalfGcdCoeffDecoderCost {
        width,
        q_bits,
        peak_qubits: b.peak_qubits as usize,
        toffoli_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count(),
        cx_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CX))
            .count(),
        x_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::X))
            .count(),
        r_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::R))
            .count(),
        hmr_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::Hmr))
            .count(),
    }
}

pub(super) fn halfgcd_coeff_decoder_overflow_aware_bench(
    width: usize,
    q_bits: usize,
) -> HalfGcdCoeffDecoderCost {
    assert!(width > 0);
    assert!(q_bits > 0);

    let mut b = B::new();
    let numerator = b.alloc_qubits(width);
    let denominator = b.alloc_qubits(width);
    let quotient = b.alloc_qubits(q_bits);

    let start = b.ops.len();
    emit_halfgcd_coeff_quotient_decoder_overflow_aware(&mut b, &numerator, &denominator, &quotient);
    let slice = &b.ops[start..];

    HalfGcdCoeffDecoderCost {
        width,
        q_bits,
        peak_qubits: b.peak_qubits as usize,
        toffoli_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count(),
        cx_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CX))
            .count(),
        x_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::X))
            .count(),
        r_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::R))
            .count(),
        hmr_ops: slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::Hmr))
            .count(),
    }
}

pub(super) fn halfgcd_coeff_decoder_overflow_aware_peak_toffoli(
    width: usize,
    q_bits: usize,
) -> (usize, usize) {
    assert!(width > 0);
    assert!(q_bits > 0);
    assert!(q_bits <= width);

    let mut toffoli_ops = 0usize;
    for shift in 0..q_bits {
        let live_width = width - shift;
        let cmp_twice = 4 * live_width;
        let overflow_or = 2 * shift.saturating_sub(1);
        let guarded_digit = usize::from(shift > 0);
        let gated_subtract = 2 * live_width + (2 * live_width).saturating_sub(2);
        toffoli_ops += cmp_twice + overflow_or + guarded_digit + gated_subtract;
    }

    (3 * width + q_bits + 3, toffoli_ops)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HalfGcdCoeffDecoderStepProfile {
    pub width: usize,
    pub q_bits: usize,
    pub toffoli_ops: usize,
    pub peak_qubits: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HalfGcdCoeffDecoderPrefixProfile {
    pub divisor: U256,
    pub steps: Vec<HalfGcdCoeffDecoderStepProfile>,
    pub total_toffoli_ops: usize,
    pub max_width: usize,
    pub max_q_bits: usize,
    pub max_peak_qubits: usize,
    pub q_bits_over_26_steps: usize,
    pub sum_q_bits: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SignedMagU512 {
    neg: bool,
    mag: U512,
}

fn u512_from_u256(x: U256) -> U512 {
    let l = x.as_limbs();
    U512::from_limbs([l[0], l[1], l[2], l[3], 0, 0, 0, 0])
}

fn u512_bit_len(x: U512) -> usize {
    if x.is_zero() {
        0
    } else {
        512 - x.leading_zeros() as usize
    }
}

fn smag(neg: bool, mag: U512) -> SignedMagU512 {
    SignedMagU512 {
        neg: neg && !mag.is_zero(),
        mag,
    }
}

fn signed_add(a: SignedMagU512, b: SignedMagU512) -> SignedMagU512 {
    if a.mag.is_zero() {
        return b;
    }
    if b.mag.is_zero() {
        return a;
    }
    if a.neg == b.neg {
        smag(a.neg, a.mag + b.mag)
    } else if a.mag >= b.mag {
        smag(a.neg, a.mag - b.mag)
    } else {
        smag(b.neg, b.mag - a.mag)
    }
}

fn signed_neg(x: SignedMagU512) -> SignedMagU512 {
    smag(!x.neg, x.mag)
}

fn signed_mul_mag(x: SignedMagU512, q_neg: bool, q: U512) -> SignedMagU512 {
    smag(x.neg ^ q_neg, x.mag * q)
}

fn signed_sub_scaled(a: SignedMagU512, q: U512, b: SignedMagU512) -> SignedMagU512 {
    signed_add(a, signed_neg(signed_mul_mag(b, false, q)))
}

pub(super) fn halfgcd_coeff_decoder_prefix_profile_round145(
    divisor: U256,
) -> HalfGcdCoeffDecoderPrefixProfile {
    assert!(!divisor.is_zero());
    assert!(divisor < SECP256K1_P);

    let mut u = u512_from_u256(SECP256K1_P);
    let mut v = u512_from_u256(divisor);
    let mut coeff_u = smag(false, U512::ZERO);
    let mut coeff_v = smag(false, U512::from(1u64));
    let mut steps = Vec::new();
    let mut total_toffoli_ops = 0usize;
    let mut max_width = 0usize;
    let mut max_q_bits = 0usize;
    let mut max_peak_qubits = 0usize;
    let mut q_bits_over_26_steps = 0usize;
    let mut sum_q_bits = 0usize;

    while !v.is_zero() && u512_bit_len(u).max(u512_bit_len(v)) > 128 {
        let q = u / v;
        let rem = u - q * v;
        let next_coeff_u = coeff_v;
        let next_coeff_v = signed_sub_scaled(coeff_u, q, coeff_v);

        let numerator = if coeff_u.mag.is_zero() {
            next_coeff_v.mag
        } else {
            assert!(!next_coeff_v.mag.is_zero());
            next_coeff_v.mag - U512::from(1u64)
        };
        let denominator = next_coeff_u.mag;
        assert!(!denominator.is_zero());
        assert_eq!(numerator / denominator, q);

        let q_bits = u512_bit_len(q).max(1);
        let width = u512_bit_len(numerator)
            .max(u512_bit_len(denominator))
            .max(q_bits)
            .max(2);
        let formula = halfgcd_coeff_decoder_formula(width, q_bits);
        steps.push(HalfGcdCoeffDecoderStepProfile {
            width,
            q_bits,
            toffoli_ops: formula.toffoli_ops,
            peak_qubits: formula.peak_qubits,
        });
        total_toffoli_ops += formula.toffoli_ops;
        max_width = max_width.max(width);
        max_q_bits = max_q_bits.max(q_bits);
        max_peak_qubits = max_peak_qubits.max(formula.peak_qubits);
        q_bits_over_26_steps += usize::from(q_bits > 26);
        sum_q_bits += q_bits;

        u = v;
        v = rem;
        coeff_u = next_coeff_u;
        coeff_v = next_coeff_v;
    }

    HalfGcdCoeffDecoderPrefixProfile {
        divisor,
        steps,
        total_toffoli_ops,
        max_width,
        max_q_bits,
        max_peak_qubits,
        q_bits_over_26_steps,
        sum_q_bits,
    }
}

pub(super) fn emit_halfgcd_coeff_quotient_decoder(
    b: &mut B,
    numerator: &[QubitId],
    denominator: &[QubitId],
    quotient: &[QubitId],
) {
    let width = numerator.len();
    assert!(width > 0);
    assert_eq!(denominator.len(), width);
    assert!(!quotient.is_empty());
    assert!(quotient.len() <= width);

    let denominator_scratch = b.alloc_qubits(width);
    let lt_tmp = b.alloc_qubit();

    for shift in (0..quotient.len()).rev() {
        emit_decoder_digit(
            b,
            numerator,
            denominator,
            quotient[shift],
            &denominator_scratch,
            &denominator_scratch,
            lt_tmp,
            shift,
        );
    }

    b.free(lt_tmp);
    b.free_vec(&denominator_scratch);
}

pub(super) fn emit_halfgcd_coeff_quotient_decoder_with_scratch(
    b: &mut B,
    numerator: &[QubitId],
    denominator: &[QubitId],
    quotient: &[QubitId],
    denominator_scratch: &[QubitId],
    lt_tmp: QubitId,
    cmp_cin: QubitId,
) {
    let width = numerator.len();
    assert!(width > 0);
    assert_eq!(denominator.len(), width);
    assert!(!quotient.is_empty());
    assert!(quotient.len() <= width);
    assert_eq!(denominator_scratch.len(), width);

    for shift in (0..quotient.len()).rev() {
        emit_decoder_digit_with_borrowed_cin(
            b,
            numerator,
            denominator,
            quotient[shift],
            denominator_scratch,
            denominator_scratch,
            lt_tmp,
            cmp_cin,
            shift,
        );
    }
}

pub(super) fn emit_halfgcd_coeff_quotient_decoder_overflow_aware(
    b: &mut B,
    numerator: &[QubitId],
    denominator: &[QubitId],
    quotient: &[QubitId],
) {
    let width = numerator.len();
    assert!(width > 0);
    assert_eq!(denominator.len(), width);
    assert!(!quotient.is_empty());
    assert!(quotient.len() <= width);

    let gated_denominator = b.alloc_qubits(width);
    let lt_tmp = b.alloc_qubit();
    let overflow_tmp = b.alloc_qubit();
    let overflow_or_chain = &gated_denominator[..width.saturating_sub(1)];

    for shift in (0..quotient.len()).rev() {
        emit_decoder_digit_overflow_aware(
            b,
            numerator,
            denominator,
            quotient[shift],
            &gated_denominator,
            &overflow_or_chain,
            lt_tmp,
            overflow_tmp,
            shift,
        );
    }

    b.free(overflow_tmp);
    b.free(lt_tmp);
    b.free_vec(&gated_denominator);
}

fn emit_decoder_digit(
    b: &mut B,
    numerator: &[QubitId],
    denominator: &[QubitId],
    qbit: QubitId,
    shifted_denominator: &[QubitId],
    gated_denominator: &[QubitId],
    lt_tmp: QubitId,
    shift: usize,
) {
    let width = numerator.len();
    assert_eq!(denominator.len(), width);
    assert_eq!(shifted_denominator.len(), width);
    assert_eq!(gated_denominator.len(), width);
    assert!(shift < width);

    copy_shifted_denominator(b, denominator, shifted_denominator, shift);

    // lt_tmp = numerator < (denominator << shift). Under the bounded schedule
    // promise the shifted denominator has no truncated high bits.
    cmp_lt_into(b, numerator, shifted_denominator, lt_tmp);

    // qbit = !lt_tmp, then clean lt_tmp using the retained qbit.
    b.x(lt_tmp);
    b.cx(lt_tmp, qbit);
    b.x(lt_tmp);
    b.x(qbit);
    b.cx(qbit, lt_tmp);
    b.x(qbit);

    uncopy_shifted_denominator(b, denominator, shifted_denominator, shift);

    for bit in 0..width - shift {
        b.ccx(qbit, denominator[bit], gated_denominator[bit + shift]);
    }
    sub_nbit_qq(b, &gated_denominator[shift..], &numerator[shift..]);
    for bit in (0..width - shift).rev() {
        b.ccx(qbit, denominator[bit], gated_denominator[bit + shift]);
    }
}

fn emit_decoder_digit_with_borrowed_cin(
    b: &mut B,
    numerator: &[QubitId],
    denominator: &[QubitId],
    qbit: QubitId,
    shifted_denominator: &[QubitId],
    gated_denominator: &[QubitId],
    lt_tmp: QubitId,
    cmp_cin: QubitId,
    shift: usize,
) {
    let width = numerator.len();
    assert_eq!(denominator.len(), width);
    assert_eq!(shifted_denominator.len(), width);
    assert_eq!(gated_denominator.len(), width);
    assert!(shift < width);

    copy_shifted_denominator(b, denominator, shifted_denominator, shift);

    // Same digit test as emit_decoder_digit, with the comparator carry supplied
    // by the caller so envelope-fit benches do not allocate a transient qubit.
    cmp_lt_into_borrowed_cin(b, numerator, shifted_denominator, lt_tmp, cmp_cin);

    // qbit = !lt_tmp, then clean lt_tmp using the retained qbit.
    b.x(lt_tmp);
    b.cx(lt_tmp, qbit);
    b.x(lt_tmp);
    b.x(qbit);
    b.cx(qbit, lt_tmp);
    b.x(qbit);

    uncopy_shifted_denominator(b, denominator, shifted_denominator, shift);

    for bit in 0..width - shift {
        b.ccx(qbit, denominator[bit], gated_denominator[bit + shift]);
    }
    sub_nbit_qq_borrowed_cin(b, &gated_denominator[shift..], &numerator[shift..], cmp_cin);
    for bit in (0..width - shift).rev() {
        b.ccx(qbit, denominator[bit], gated_denominator[bit + shift]);
    }
}

fn emit_decoder_digit_overflow_aware(
    b: &mut B,
    numerator: &[QubitId],
    denominator: &[QubitId],
    qbit: QubitId,
    gated_denominator: &[QubitId],
    overflow_or_chain: &[QubitId],
    lt_tmp: QubitId,
    overflow_tmp: QubitId,
    shift: usize,
) {
    let width = numerator.len();
    assert_eq!(denominator.len(), width);
    assert_eq!(gated_denominator.len(), width);
    assert!(shift < width);

    let overflow_bits = &denominator[width - shift..width];
    xor_nonzero_flag_with_scratch(b, overflow_bits, overflow_or_chain, overflow_tmp);

    let live_width = width - shift;
    let numerator_hi = &numerator[shift..];
    let denominator_lo = &denominator[..live_width];

    // If the shifted denominator fits, numerator < (denominator << shift)
    // is equivalent to numerator[shift..] < denominator[..width-shift].
    // Equality at the high slice still means the trial subtract is allowed,
    // because the shifted denominator has zero low bits.
    cmp_lt_into(b, numerator_hi, denominator_lo, lt_tmp);
    b.x(lt_tmp);
    if shift == 0 {
        b.cx(lt_tmp, qbit);
    } else {
        b.x(overflow_tmp);
        b.ccx(lt_tmp, overflow_tmp, qbit);
        b.x(overflow_tmp);
    }
    b.x(lt_tmp);
    cmp_lt_into(b, numerator_hi, denominator_lo, lt_tmp);

    unxor_nonzero_flag_with_scratch(b, overflow_bits, overflow_or_chain, overflow_tmp);

    for bit in 0..width - shift {
        b.ccx(qbit, denominator[bit], gated_denominator[bit + shift]);
    }
    sub_nbit_qq(b, &gated_denominator[shift..], &numerator[shift..]);
    for bit in (0..width - shift).rev() {
        b.ccx(qbit, denominator[bit], gated_denominator[bit + shift]);
    }
}

fn copy_shifted_denominator(
    b: &mut B,
    denominator: &[QubitId],
    shifted_denominator: &[QubitId],
    shift: usize,
) {
    for bit in 0..denominator.len() - shift {
        b.cx(denominator[bit], shifted_denominator[bit + shift]);
    }
}

fn uncopy_shifted_denominator(
    b: &mut B,
    denominator: &[QubitId],
    shifted_denominator: &[QubitId],
    shift: usize,
) {
    for bit in (0..denominator.len() - shift).rev() {
        b.cx(denominator[bit], shifted_denominator[bit + shift]);
    }
}

fn xor_nonzero_flag_with_scratch(b: &mut B, bits: &[QubitId], scratch: &[QubitId], flag: QubitId) {
    match bits.len() {
        0 => {}
        1 => b.cx(bits[0], flag),
        n => {
            assert!(scratch.len() >= n - 1);
            or_into_clean(b, bits[0], bits[1], scratch[0]);
            for i in 1..n - 1 {
                or_into_clean(b, scratch[i - 1], bits[i + 1], scratch[i]);
            }
            b.cx(scratch[n - 2], flag);
        }
    }
}

fn unxor_nonzero_flag_with_scratch(
    b: &mut B,
    bits: &[QubitId],
    scratch: &[QubitId],
    flag: QubitId,
) {
    match bits.len() {
        0 => {}
        1 => b.cx(bits[0], flag),
        n => {
            assert!(scratch.len() >= n - 1);
            b.cx(scratch[n - 2], flag);
            for i in (1..n - 1).rev() {
                or_into_clean(b, scratch[i - 1], bits[i + 1], scratch[i]);
            }
            or_into_clean(b, bits[0], bits[1], scratch[0]);
        }
    }
}

fn or_into_clean(b: &mut B, x: QubitId, y: QubitId, out: QubitId) {
    b.x(x);
    b.x(y);
    b.ccx(x, y, out);
    b.x(out);
    b.x(y);
    b.x(x);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::{analyze_ops, Op, OperationType};
    use crate::sim::Simulator;
    use sha3::{
        digest::{ExtendableOutput, Update},
        Shake256,
    };
    use std::collections::BTreeSet;

    struct DecoderHarness {
        ops: Vec<Op>,
        num_qubits: usize,
        num_bits: usize,
        numerator: Vec<QubitId>,
        denominator: Vec<QubitId>,
        quotient: Vec<QubitId>,
    }

    fn build_forward_harness(width: usize, q_bits: usize) -> DecoderHarness {
        let mut b = B::new();
        let numerator = b.alloc_qubits(width);
        let denominator = b.alloc_qubits(width);
        let quotient = b.alloc_qubits(q_bits);
        emit_halfgcd_coeff_quotient_decoder(&mut b, &numerator, &denominator, &quotient);
        finish_harness(b, numerator, denominator, quotient)
    }

    fn build_overflow_forward_harness(width: usize, q_bits: usize) -> DecoderHarness {
        let mut b = B::new();
        let numerator = b.alloc_qubits(width);
        let denominator = b.alloc_qubits(width);
        let quotient = b.alloc_qubits(q_bits);
        emit_halfgcd_coeff_quotient_decoder_overflow_aware(
            &mut b,
            &numerator,
            &denominator,
            &quotient,
        );
        finish_harness(b, numerator, denominator, quotient)
    }

    fn build_roundtrip_harness(width: usize, q_bits: usize) -> DecoderHarness {
        let mut b = B::new();
        let numerator = b.alloc_qubits(width);
        let denominator = b.alloc_qubits(width);
        let quotient = b.alloc_qubits(q_bits);
        let start = b.ops.len();
        emit_halfgcd_coeff_quotient_decoder(&mut b, &numerator, &denominator, &quotient);
        let end = b.ops.len();
        append_inverse_of_existing_ops(&mut b, start, end);
        finish_harness(b, numerator, denominator, quotient)
    }

    fn build_overflow_roundtrip_harness(width: usize, q_bits: usize) -> DecoderHarness {
        let mut b = B::new();
        let numerator = b.alloc_qubits(width);
        let denominator = b.alloc_qubits(width);
        let quotient = b.alloc_qubits(q_bits);
        let start = b.ops.len();
        emit_halfgcd_coeff_quotient_decoder_overflow_aware(
            &mut b,
            &numerator,
            &denominator,
            &quotient,
        );
        let end = b.ops.len();
        append_inverse_of_existing_ops(&mut b, start, end);
        finish_harness(b, numerator, denominator, quotient)
    }

    fn build_reused_roundtrip_harness(
        width: usize,
        q_bits: usize,
        passes: usize,
    ) -> DecoderHarness {
        let mut b = B::new();
        let numerator = b.alloc_qubits(width);
        let denominator = b.alloc_qubits(width);
        let quotient = b.alloc_qubits(q_bits);
        for _ in 0..passes {
            let start = b.ops.len();
            emit_halfgcd_coeff_quotient_decoder(&mut b, &numerator, &denominator, &quotient);
            let end = b.ops.len();
            append_inverse_of_existing_ops(&mut b, start, end);
        }
        finish_harness(b, numerator, denominator, quotient)
    }

    fn finish_harness(
        b: B,
        numerator: Vec<QubitId>,
        denominator: Vec<QubitId>,
        quotient: Vec<QubitId>,
    ) -> DecoderHarness {
        let (num_qubits, num_bits, _, _) = analyze_ops(b.ops.iter().copied());
        DecoderHarness {
            ops: b.ops,
            num_qubits: num_qubits as usize,
            num_bits: num_bits as usize,
            numerator,
            denominator,
            quotient,
        }
    }

    fn append_inverse_of_existing_ops(b: &mut B, start: usize, end: usize) {
        let fwd: Vec<_> = b.ops[start..end].to_vec();
        for op in fwd.into_iter().rev() {
            match op.kind {
                OperationType::X
                | OperationType::Z
                | OperationType::CX
                | OperationType::CZ
                | OperationType::CCX
                | OperationType::CCZ
                | OperationType::Swap => b.ops.push(op),
                OperationType::R
                | OperationType::Register
                | OperationType::AppendToRegister
                | OperationType::DebugPrint => {}
                _ => panic!(
                    "append_inverse_of_existing_ops: non-invertible op kind {:?}",
                    op.kind
                ),
            }
        }
    }

    fn execute(
        h: &DecoderHarness,
        numerator: u64,
        denominator: u64,
    ) -> (u64, u64, u64, u64, Vec<u64>) {
        let mut hasher = Shake256::default();
        hasher.update(b"halfgcd-coeff-decoder-test");
        let mut xof = hasher.finalize_xof();
        let mut sim = Simulator::new(h.num_qubits, h.num_bits, &mut xof);
        set_word(&mut sim, &h.numerator, numerator, 0);
        set_word(&mut sim, &h.denominator, denominator, 0);
        sim.apply(&h.ops);
        (
            read_word(&sim, &h.numerator, 0),
            read_word(&sim, &h.denominator, 0),
            read_word(&sim, &h.quotient, 0),
            sim.global_phase() & 1,
            sim.qubits,
        )
    }

    fn set_word<R: sha3::digest::XofReader>(
        sim: &mut Simulator<R>,
        qs: &[QubitId],
        value: u64,
        shot: usize,
    ) {
        for (bit, &q) in qs.iter().enumerate() {
            if ((value >> bit) & 1) != 0 {
                *sim.qubit_mut(q) |= 1u64 << shot;
            } else {
                *sim.qubit_mut(q) &= !(1u64 << shot);
            }
        }
    }

    fn read_word<R: sha3::digest::XofReader>(
        sim: &Simulator<R>,
        qs: &[QubitId],
        shot: usize,
    ) -> u64 {
        let mut out = 0u64;
        for (bit, &q) in qs.iter().enumerate() {
            out |= ((sim.qubit(q) >> shot) & 1) << bit;
        }
        out
    }

    fn promised_cases(width: usize, q_bits: usize) -> Vec<(u64, u64)> {
        let limit = 1u64 << width;
        let quotient_limit = 1u64 << q_bits;
        let mut cases = Vec::new();
        for numerator in 1..limit {
            for denominator in 1..limit {
                if (denominator << (q_bits - 1)) >= limit {
                    continue;
                }
                if numerator / denominator >= quotient_limit {
                    continue;
                }
                cases.push((numerator, denominator));
            }
        }
        cases
    }

    fn full_width_cases(width: usize, q_bits: usize) -> Vec<(u64, u64)> {
        let limit = 1u64 << width;
        let quotient_limit = 1u64 << q_bits;
        let mut cases = Vec::new();
        for numerator in 0..limit {
            for denominator in 1..limit {
                if numerator / denominator < quotient_limit {
                    cases.push((numerator, denominator));
                }
            }
        }
        cases
    }

    fn assert_clean_scratch(h: &DecoderHarness, qubits: &[u64]) {
        let live: BTreeSet<u32> = h
            .numerator
            .iter()
            .chain(h.denominator.iter())
            .chain(h.quotient.iter())
            .map(|q| q.0)
            .collect();
        for (idx, &word) in qubits.iter().enumerate() {
            if !live.contains(&(idx as u32)) {
                assert_eq!(word, 0, "scratch q{} was not clean", idx);
            }
        }
    }

    #[test]
    fn halfgcd_coeff_decoder_forward_toy_widths_are_exact() {
        for (width, q_bits, expected_cases) in [(3usize, 2usize, 17usize), (4, 2, 81)] {
            let h = build_forward_harness(width, q_bits);
            let cases = promised_cases(width, q_bits);
            assert_eq!(cases.len(), expected_cases);
            for (numerator, denominator) in cases {
                let (rem, den, quo, phase, qubits) = execute(&h, numerator, denominator);
                assert_eq!(rem, numerator % denominator);
                assert_eq!(den, denominator);
                assert_eq!(quo, numerator / denominator);
                assert_eq!(phase, 0);
                assert_clean_scratch(&h, &qubits);
            }
        }
    }

    #[test]
    fn halfgcd_coeff_decoder_overflow_aware_handles_full_width_cases() {
        for (width, q_bits) in [(3usize, 3usize), (4, 4)] {
            let h = build_overflow_forward_harness(width, q_bits);
            for (numerator, denominator) in full_width_cases(width, q_bits) {
                let (rem, den, quo, phase, qubits) = execute(&h, numerator, denominator);
                assert_eq!(rem, numerator % denominator);
                assert_eq!(den, denominator);
                assert_eq!(quo, numerator / denominator);
                assert_eq!(phase, 0);
                assert_clean_scratch(&h, &qubits);
            }
        }
    }

    #[test]
    fn halfgcd_coeff_decoder_roundtrip_restores_inputs_and_phase() {
        for (width, q_bits) in [(3usize, 2usize), (4, 3)] {
            let h = build_roundtrip_harness(width, q_bits);
            for (numerator, denominator) in promised_cases(width, q_bits) {
                let (rem, den, quo, phase, qubits) = execute(&h, numerator, denominator);
                assert_eq!(rem, numerator);
                assert_eq!(den, denominator);
                assert_eq!(quo, 0);
                assert_eq!(phase, 0);
                assert_clean_scratch(&h, &qubits);
            }
        }
    }

    #[test]
    fn halfgcd_coeff_decoder_overflow_aware_roundtrip_restores_inputs_and_phase() {
        for (width, q_bits) in [(3usize, 3usize), (4, 4)] {
            let h = build_overflow_roundtrip_harness(width, q_bits);
            for (numerator, denominator) in full_width_cases(width, q_bits) {
                let (rem, den, quo, phase, qubits) = execute(&h, numerator, denominator);
                assert_eq!(rem, numerator);
                assert_eq!(den, denominator);
                assert_eq!(quo, 0);
                assert_eq!(phase, 0);
                assert_clean_scratch(&h, &qubits);
            }
        }
    }

    #[test]
    fn halfgcd_coeff_decoder_exact_cost_bench_matches_formula() {
        for (width, q_bits) in [(3usize, 2usize), (8, 4), (257, 26)] {
            let bench = halfgcd_coeff_decoder_bench(width, q_bits);
            let formula = halfgcd_coeff_decoder_formula(width, q_bits);
            assert_eq!(bench.peak_qubits, formula.peak_qubits);
            assert_eq!(bench.toffoli_ops, formula.toffoli_ops);
            assert_eq!(bench.cx_ops, formula.cx_ops);
            assert_eq!(bench.x_ops, formula.x_ops);
            assert_eq!(bench.r_ops, formula.r_ops);
            assert_eq!(bench.hmr_ops, 0);
        }

        let profile = halfgcd_coeff_decoder_bench(257, 26);
        assert_eq!(profile.peak_qubits, 799);
        assert_eq!(profile.toffoli_ops, 38_740);
        assert_eq!(profile.cx_ops, 64_896);
        assert_eq!(profile.x_ops, 13_468);

        let round145_observed_max = halfgcd_coeff_decoder_bench(141, 13);
        assert_eq!(round145_observed_max.peak_qubits, 438);
        assert_eq!(round145_observed_max.toffoli_ops, 10_660);
    }

    #[test]
    fn halfgcd_coeff_decoder_overflow_aware_cost_is_pinned() {
        let toy = halfgcd_coeff_decoder_overflow_aware_bench(4, 4);
        let (toy_peak, toy_toffoli) = halfgcd_coeff_decoder_overflow_aware_peak_toffoli(4, 4);
        assert_eq!(toy.peak_qubits, toy_peak);
        assert_eq!(toy.toffoli_ops, toy_toffoli);
        assert_eq!(toy.peak_qubits, 19);
        assert_eq!(toy.toffoli_ops, 81);
        assert_eq!(toy.hmr_ops, 0);

        let full = halfgcd_coeff_decoder_overflow_aware_bench(256, 256);
        let (full_peak, full_toffoli) = halfgcd_coeff_decoder_overflow_aware_peak_toffoli(256, 256);
        assert_eq!(full.peak_qubits, full_peak);
        assert_eq!(full.toffoli_ops, full_toffoli);
        assert_eq!(full.peak_qubits, 1027);
        assert_eq!(full.toffoli_ops, 327_681);
        assert_eq!(full.hmr_ops, 0);
    }

    #[test]
    fn halfgcd_coeff_decoder_roundtrip_reuse_keeps_peak_flat() {
        let h = build_reused_roundtrip_harness(4, 3, 4);
        for (numerator, denominator) in promised_cases(4, 3) {
            let (rem, den, quo, phase, qubits) = execute(&h, numerator, denominator);
            assert_eq!(rem, numerator);
            assert_eq!(den, denominator);
            assert_eq!(quo, 0);
            assert_eq!(phase, 0);
            assert_clean_scratch(&h, &qubits);
        }

        let mut b = B::new();
        let numerator = b.alloc_qubits(141);
        let denominator = b.alloc_qubits(141);
        let quotient = b.alloc_qubits(13);
        let start = b.ops.len();
        for _ in 0..4 {
            emit_halfgcd_coeff_quotient_decoder(&mut b, &numerator, &denominator, &quotient);
            emit_halfgcd_coeff_quotient_decoder(&mut b, &numerator, &denominator, &quotient);
        }
        let slice = &b.ops[start..];
        let toffoli_ops = slice
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(
            b.peak_qubits as usize,
            halfgcd_coeff_decoder_formula(141, 13).peak_qubits
        );
        assert_eq!(
            toffoli_ops,
            8 * halfgcd_coeff_decoder_formula(141, 13).toffoli_ops
        );
    }

    #[test]
    fn halfgcd_coeff_decoder_round145_semantic_max_profile_matches_certificate() {
        let divisor = U256::from_str_radix(
            "82302208564988718744202673340416757137332630777895436281211408153252062596056",
            10,
        )
        .unwrap();
        let profile = halfgcd_coeff_decoder_prefix_profile_round145(divisor);
        assert_eq!(profile.total_toffoli_ops, 70_670);
        assert_eq!(profile.max_width, 130);
        assert_eq!(profile.max_q_bits, 5);
        assert_eq!(profile.max_peak_qubits, 395);
        assert_eq!(profile.steps.len(), 105);
        assert_eq!(profile.sum_q_bits, 183);
        assert_eq!(profile.q_bits_over_26_steps, 0);
    }
}
