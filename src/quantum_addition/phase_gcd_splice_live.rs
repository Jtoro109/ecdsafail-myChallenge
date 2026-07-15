//! Round158 live-denominator half-GCD splice attempt.
//!
//! This is intentionally an implementation artifact, not a planning note.  It
//! wires the Round145 coefficient quotient decoder to a live denominator lane,
//! updates the Euclid residual pair and the second-column coefficient state, and
//! leaves the exact quotient stream as an explicit tail.  That is still not a
//! complete point-addition candidate: the tail and prefix state are not erased
//! back to the four Google ABI registers.

use super::{gcd_coefficient_decoder, load_const, B, N, SECP256K1_P};
use crate::circuit::{Op, OperationType, QubitId};
use alloy_primitives::{U256, U512};
use std::collections::BTreeSet;
use std::fmt::Write as _;

pub(super) const ROUND158_HALFGCD_LIVE_PREFIX_PA_ROUTE_ENV: &str =
    "ROUND158_HALFGCD_LIVE_PREFIX_PA_ROUTE";
const GOOGLE_RELAXED_Q_TARGET: usize = 2_100;
const GOOGLE_RELAXED_T_TARGET: usize = 3_100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SignedMag {
    neg: bool,
    mag: U512,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Round158PrefixStep {
    q: U256,
    q_bits: usize,
    u_bits: usize,
    v_bits: usize,
    coeff_bits_before: usize,
    coeff_bits_after: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Round158PrefixProfile {
    lane_width: usize,
    stop_bits: usize,
    divisor: U256,
    steps: Vec<Round158PrefixStep>,
    total_q_bits: usize,
    max_q_bits: usize,
    coeff_width: usize,
    final_u: U256,
    final_v: U256,
    final_b: SignedMag,
    final_d: SignedMag,
}

#[derive(Clone, Debug)]
struct Round158LivePrefixEmission {
    u: Vec<QubitId>,
    v: Vec<QubitId>,
    coeff_b: Vec<QubitId>,
    coeff_d: Vec<QubitId>,
    q_tail: Vec<QubitId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ProfileRejection {
    QuotientBitsTooWide {
        step: usize,
        required: usize,
        provided: usize,
    },
    ShiftedDenominatorOverflows {
        step: usize,
        q_bits: usize,
        v_bits: usize,
        lane_width: usize,
    },
    RanOutOfSteps {
        remaining_u_bits: usize,
        remaining_v_bits: usize,
    },
}

pub(super) fn round158_live_prefix_pa_route_enabled() -> bool {
    std::env::var(ROUND158_HALFGCD_LIVE_PREFIX_PA_ROUTE_ENV)
        .ok()
        .as_deref()
        == Some("1")
}

pub(super) fn round158_live_prefix_pa_route_blocker_message(
    tx: &[QubitId],
    ty: &[QubitId],
    ox: &[crate::circuit::BitId],
    oy: &[crate::circuit::BitId],
    p: U256,
) -> String {
    let (x0, y0, x1, y1) = first_secp_x_pair_with_delta_one(512);
    let divisor = x1 - x0;
    let required_first_q_bits = u256_bit_len(p / divisor).max(1);
    let semantic_profile = round158_prefix_profile(p, round146_semantic_max_divisor(), N, 128);
    let semantic_rejection =
        profile_rejection_for_live_divisor(p, divisor, N, 128, &semantic_profile);
    let first_step_regular_decoder_t =
        gcd_coefficient_decoder::halfgcd_coeff_decoder_formula(N, required_first_q_bits).toffoli_ops;
    let (first_step_overflow_decoder_q, first_step_overflow_decoder_t) =
        gcd_coefficient_decoder::halfgcd_coeff_decoder_overflow_aware_peak_toffoli(
            N,
            required_first_q_bits,
        );
    let widened_first_step_overflows_regular_denominator =
        u256_bit_len(round146_semantic_max_divisor()) + required_first_q_bits.saturating_sub(1) > N;

    let mut msg = String::new();
    let _ = write!(
        msg,
        "{env}=1 cannot be promoted to build_standard_ec_add: \
         at the PA splice point tx is the live quantum denominator dx=target_x-offset_x, \
         ty is dy=target_y-offset_y, and ox/oy are still the public addend bits \
         (wire widths tx={}, ty={}, ox={}, oy={}). ",
        tx.len(),
        ty.len(),
        ox.len(),
        oy.len(),
        env = ROUND158_HALFGCD_LIVE_PREFIX_PA_ROUTE_ENV,
    );
    let _ = write!(
        msg,
        "A valid secp256k1 ABI input has x0={}, y0={}, x1={}, y1={} so dx=x1-x0=1. \
         The Round144/Round146 semantic profile starts with q_bits={} and total_q_tail_bits={}, \
         but that witness needs step0 q=p/dx with {} bits; rejection={:?}. ",
        x0,
        y0,
        x1,
        y1,
        semantic_profile.steps[0].q_bits,
        semantic_profile.total_q_bits,
        required_first_q_bits,
        semantic_rejection,
    );
    let _ = write!(
        msg,
        "The regular Round144 decoder still requires denominator << (q_bits-1) to fit the {}-bit lane \
         and overflows for the ordinary {}-bit semantic denominator; widened_regular_first_step_decoder_t_floor={}. \
         The direct-slice overflow-aware decoder removes that shift-fit blocker for a {}-bit first slot \
         at decoder floor Q={}, T={}. ",
        N,
        u256_bit_len(round146_semantic_max_divisor()),
        first_step_regular_decoder_t,
        required_first_q_bits,
        first_step_overflow_decoder_q,
        first_step_overflow_decoder_t,
    );
    let _ = write!(
        msg,
        "Current Round158 emission also leaves u/v residuals, coeff_b/coeff_d, and q_tail live; \
         it never consumes dy into lambda, restores tx/ty to affine output, or erases those lanes back to \
         the four Google ABI registers. This is a wire/state blocker, not a missing KMX dumper. \
         targets Q<{}, T<{}; widened_slot_overflows_regular_denominator={}.",
        GOOGLE_RELAXED_Q_TARGET,
        GOOGLE_RELAXED_T_TARGET,
        widened_first_step_overflows_regular_denominator,
    );
    msg
}

pub(super) fn abort_round158_live_prefix_pa_route(
    tx: &[QubitId],
    ty: &[QubitId],
    ox: &[crate::circuit::BitId],
    oy: &[crate::circuit::BitId],
    p: U256,
) -> ! {
    panic!(
        "{}",
        round158_live_prefix_pa_route_blocker_message(tx, ty, ox, oy, p)
    );
}

fn u512_from_u256(x: U256) -> U512 {
    let l = x.as_limbs();
    U512::from_limbs([l[0], l[1], l[2], l[3], 0, 0, 0, 0])
}

fn u256_bit_len(x: U256) -> usize {
    if x.is_zero() {
        0
    } else {
        256 - x.leading_zeros() as usize
    }
}

fn u512_bit_len(x: U512) -> usize {
    if x.is_zero() {
        0
    } else {
        512 - x.leading_zeros() as usize
    }
}

fn smag(neg: bool, mag: U512) -> SignedMag {
    SignedMag {
        neg: neg && !mag.is_zero(),
        mag,
    }
}

fn signed_add(a: SignedMag, b: SignedMag) -> SignedMag {
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

fn signed_neg(x: SignedMag) -> SignedMag {
    smag(!x.neg, x.mag)
}

fn signed_mul_mag(x: SignedMag, q: U256) -> SignedMag {
    smag(x.neg, x.mag * u512_from_u256(q))
}

fn signed_sub_scaled(a: SignedMag, q: U256, b: SignedMag) -> SignedMag {
    signed_add(a, signed_neg(signed_mul_mag(b, q)))
}

fn signed_bits(x: SignedMag) -> usize {
    u512_bit_len(x.mag) + usize::from(!x.mag.is_zero())
}

fn round146_semantic_max_divisor() -> U256 {
    U256::from_str_radix(
        "82302208564988718744202673340416757137332630777895436281211408153252062596056",
        10,
    )
    .unwrap()
}

fn round158_prefix_profile(
    p: U256,
    divisor: U256,
    lane_width: usize,
    stop_bits: usize,
) -> Round158PrefixProfile {
    assert!(lane_width <= 256);
    assert!(!divisor.is_zero());
    assert!(divisor < p);

    let mut u = p;
    let mut v = divisor;
    let mut b = smag(false, U512::ZERO);
    let mut d = smag(false, U512::from(1u64));
    let mut steps = Vec::new();
    let mut total_q_bits = 0usize;
    let mut max_q_bits = 0usize;
    let mut max_coeff_mag_bits = 1usize;

    while !v.is_zero() && u256_bit_len(u).max(u256_bit_len(v)) > stop_bits {
        let q = u / v;
        let rem = u - q * v;
        let next_b = d;
        let next_d = signed_sub_scaled(b, q, d);
        let q_bits = u256_bit_len(q).max(1);
        let coeff_bits_before = signed_bits(b).max(signed_bits(d)).max(1);
        let coeff_bits_after = signed_bits(next_b).max(signed_bits(next_d)).max(1);
        steps.push(Round158PrefixStep {
            q,
            q_bits,
            u_bits: u256_bit_len(u),
            v_bits: u256_bit_len(v),
            coeff_bits_before,
            coeff_bits_after,
        });
        total_q_bits += q_bits;
        max_q_bits = max_q_bits.max(q_bits);
        max_coeff_mag_bits = max_coeff_mag_bits
            .max(u512_bit_len(b.mag))
            .max(u512_bit_len(d.mag))
            .max(u512_bit_len(next_b.mag))
            .max(u512_bit_len(next_d.mag));
        u = v;
        v = rem;
        b = next_b;
        d = next_d;
    }

    Round158PrefixProfile {
        lane_width,
        stop_bits,
        divisor,
        steps,
        total_q_bits,
        max_q_bits,
        coeff_width: max_coeff_mag_bits + 1,
        final_u: u,
        final_v: v,
        final_b: b,
        final_d: d,
    }
}

fn profile_rejection_for_live_divisor(
    p: U256,
    divisor: U256,
    lane_width: usize,
    stop_bits: usize,
    profile: &Round158PrefixProfile,
) -> Option<ProfileRejection> {
    let mut u = p;
    let mut v = divisor;
    for (step, spec) in profile.steps.iter().enumerate() {
        if v.is_zero() || u256_bit_len(u).max(u256_bit_len(v)) <= stop_bits {
            return None;
        }
        let q = u / v;
        let required = u256_bit_len(q).max(1);
        if required > spec.q_bits {
            return Some(ProfileRejection::QuotientBitsTooWide {
                step,
                required,
                provided: spec.q_bits,
            });
        }
        let v_bits = u256_bit_len(v);
        if v_bits + spec.q_bits.saturating_sub(1) > lane_width {
            return Some(ProfileRejection::ShiftedDenominatorOverflows {
                step,
                q_bits: spec.q_bits,
                v_bits,
                lane_width,
            });
        }
        let rem = u - q * v;
        u = v;
        v = rem;
    }

    if !v.is_zero() && u256_bit_len(u).max(u256_bit_len(v)) > stop_bits {
        Some(ProfileRejection::RanOutOfSteps {
            remaining_u_bits: u256_bit_len(u),
            remaining_v_bits: u256_bit_len(v),
        })
    } else {
        None
    }
}

fn emit_sub_q_times_twos_complement(
    b: &mut B,
    q: &[QubitId],
    multiplicand: &[QubitId],
    acc: &[QubitId],
) {
    assert_eq!(multiplicand.len(), acc.len());
    assert!(q.len() <= acc.len());
    let width = acc.len();

    for (shift, &qbit) in q.iter().enumerate() {
        let term = b.alloc_qubits(width);
        for src in 0..width.saturating_sub(shift) {
            b.ccx(qbit, multiplicand[src], term[src + shift]);
        }
        super::sub_nbit_qq(b, &term, acc);
        for src in (0..width.saturating_sub(shift)).rev() {
            b.ccx(qbit, multiplicand[src], term[src + shift]);
        }
        b.free_vec(&term);
    }
}

fn swap_regs(b: &mut B, a: &[QubitId], c: &[QubitId]) {
    assert_eq!(a.len(), c.len());
    for (&qa, &qc) in a.iter().zip(c.iter()) {
        b.swap(qa, qc);
    }
}

fn copy_reg_xor(b: &mut B, src: &[QubitId], dst: &[QubitId]) {
    assert_eq!(src.len(), dst.len());
    for (&s, &d) in src.iter().zip(dst.iter()) {
        b.cx(s, d);
    }
}

fn replay_inverse_of_clean_forward_ops(b: &mut B, fwd: &[Op]) {
    for &op in fwd.iter().rev() {
        match op.kind {
            OperationType::X
            | OperationType::Z
            | OperationType::CX
            | OperationType::CZ
            | OperationType::CCX
            | OperationType::CCZ
            | OperationType::Swap => b.ops.push(op),
            OperationType::R => {}
            OperationType::Register
            | OperationType::AppendToRegister
            | OperationType::DebugPrint => {}
            _ => panic!(
                "replay_inverse_of_clean_forward_ops: non-invertible op kind {:?}",
                op.kind
            ),
        }
    }
}

fn remap_qubit_id(q: QubitId, from: &[QubitId], to: &[QubitId]) -> QubitId {
    for (&src, &dst) in from.iter().zip(to.iter()) {
        if q == src {
            return dst;
        }
    }
    q
}

fn remap_op_qubits(mut op: Op, from: &[QubitId], to: &[QubitId]) -> Op {
    op.q_control2 = remap_qubit_id(op.q_control2, from, to);
    op.q_control1 = remap_qubit_id(op.q_control1, from, to);
    op.q_target = remap_qubit_id(op.q_target, from, to);
    op
}

fn replay_inverse_of_clean_forward_ops_remap_q(
    b: &mut B,
    fwd: &[Op],
    from: &[QubitId],
    to: &[QubitId],
) {
    assert_eq!(from.len(), to.len());
    for &op in fwd.iter().rev() {
        let op = remap_op_qubits(op, from, to);
        match op.kind {
            OperationType::X
            | OperationType::Z
            | OperationType::CX
            | OperationType::CZ
            | OperationType::CCX
            | OperationType::CCZ
            | OperationType::Swap => b.ops.push(op),
            OperationType::R => {}
            OperationType::Register
            | OperationType::AppendToRegister
            | OperationType::DebugPrint => {}
            _ => panic!(
                "replay_inverse_of_clean_forward_ops_remap_q: non-invertible op kind {:?}",
                op.kind
            ),
        }
    }
}

fn emit_abs_twos_complement_into_zero(b: &mut B, signed: &[QubitId], magnitude: &[QubitId]) {
    assert_eq!(signed.len(), magnitude.len());
    assert!(!signed.is_empty());
    let sign = signed[signed.len() - 1];
    for (&src, &dst) in signed.iter().zip(magnitude.iter()) {
        b.cx(src, dst);
    }
    for &dst in magnitude {
        b.cx(sign, dst);
    }
    super::cadd_nbit_const(b, magnitude, U256::from(1u64), sign);
}

fn emit_abs_twos_complement_into_zero_inverse(
    b: &mut B,
    signed: &[QubitId],
    magnitude: &[QubitId],
) {
    assert_eq!(signed.len(), magnitude.len());
    assert!(!signed.is_empty());
    let sign = signed[signed.len() - 1];
    super::csub_nbit_const(b, magnitude, U256::from(1u64), sign);
    for &dst in magnitude.iter().rev() {
        b.cx(sign, dst);
    }
    for (&src, &dst) in signed.iter().zip(magnitude.iter()).rev() {
        b.cx(src, dst);
    }
}

fn emit_neg_low_twos_complement_into_zero(b: &mut B, signed: &[QubitId], out: &[QubitId]) {
    assert!(!signed.is_empty());
    assert!(!out.is_empty());
    assert!(out.len() <= signed.len());
    for (&src, &dst) in signed.iter().zip(out.iter()) {
        b.cx(src, dst);
    }
    for &dst in out {
        b.x(dst);
    }
    super::add_nbit_const(b, out, U256::from(1u64));
}

fn emit_neg_low_twos_complement_into_zero_inverse(b: &mut B, signed: &[QubitId], out: &[QubitId]) {
    assert!(!signed.is_empty());
    assert!(!out.is_empty());
    assert!(out.len() <= signed.len());
    super::sub_nbit_const(b, out, U256::from(1u64));
    for &dst in out.iter().rev() {
        b.x(dst);
    }
    for (&src, &dst) in signed.iter().zip(out.iter()).rev() {
        b.cx(src, dst);
    }
}

pub(super) fn emit_round158_numeric_endpoint_step(
    b: &mut B,
    u: &[QubitId],
    v: &[QubitId],
    coeff_b: &[QubitId],
    coeff_d: &[QubitId],
    q: &[QubitId],
) {
    emit_round158_numeric_endpoint_step_with_decoder(b, u, v, coeff_b, coeff_d, q, true);
}

fn emit_round158_numeric_endpoint_step_with_decoder(
    b: &mut B,
    u: &[QubitId],
    v: &[QubitId],
    coeff_b: &[QubitId],
    coeff_d: &[QubitId],
    q: &[QubitId],
    overflow_aware: bool,
) {
    assert_eq!(u.len(), v.len());
    assert_eq!(coeff_b.len(), coeff_d.len());
    assert!(!u.is_empty());
    assert!(!q.is_empty());
    assert!(q.len() <= u.len());
    assert!(q.len() <= coeff_b.len());

    if overflow_aware {
        gcd_coefficient_decoder::emit_halfgcd_coeff_quotient_decoder_overflow_aware(b, u, v, q);
    } else {
        gcd_coefficient_decoder::emit_halfgcd_coeff_quotient_decoder(b, u, v, q);
    }
    emit_sub_q_times_twos_complement(b, q, coeff_d, coeff_b);
    swap_regs(b, u, v);
    swap_regs(b, coeff_b, coeff_d);
}

pub(super) fn emit_round197_numeric_endpoint_step_copy_clean(
    b: &mut B,
    u: &[QubitId],
    v: &[QubitId],
    coeff_b: &[QubitId],
    coeff_d: &[QubitId],
    q: &[QubitId],
    u_out: &[QubitId],
    v_out: &[QubitId],
    coeff_b_out: &[QubitId],
    coeff_d_out: &[QubitId],
) {
    assert_eq!(u.len(), u_out.len());
    assert_eq!(v.len(), v_out.len());
    assert_eq!(coeff_b.len(), coeff_b_out.len());
    assert_eq!(coeff_d.len(), coeff_d_out.len());

    let start = b.ops.len();
    emit_round158_numeric_endpoint_step(b, u, v, coeff_b, coeff_d, q);
    let fwd = b.ops[start..].to_vec();

    copy_reg_xor(b, u, u_out);
    copy_reg_xor(b, v, v_out);
    copy_reg_xor(b, coeff_b, coeff_b_out);
    copy_reg_xor(b, coeff_d, coeff_d_out);

    replay_inverse_of_clean_forward_ops(b, &fwd);
}

pub(super) fn emit_round197_numeric_endpoint_step_clean_q_from_coeff(
    b: &mut B,
    u: &[QubitId],
    v: &[QubitId],
    coeff_b: &[QubitId],
    coeff_d: &[QubitId],
    q: &[QubitId],
    initial_endpoint_step: bool,
) {
    let overflow_aware = std::env::var("ROUND197_CLEAN_Q_REGULAR_DECODER")
        .ok()
        .as_deref()
        != Some("1");
    emit_round197_numeric_endpoint_step_clean_q_from_coeff_with_decoder(
        b,
        u,
        v,
        coeff_b,
        coeff_d,
        q,
        initial_endpoint_step,
        overflow_aware,
    );
}

pub(super) fn emit_round197_numeric_endpoint_step_clean_q_from_coeff_with_decoder(
    b: &mut B,
    u: &[QubitId],
    v: &[QubitId],
    coeff_b: &[QubitId],
    coeff_d: &[QubitId],
    q: &[QubitId],
    initial_endpoint_step: bool,
    overflow_aware: bool,
) {
    assert_eq!(u.len(), v.len());
    assert_eq!(coeff_b.len(), coeff_d.len());
    assert!(!q.is_empty());
    assert!(q.len() <= u.len());
    assert!(q.len() <= coeff_b.len());

    emit_round158_numeric_endpoint_step_with_decoder(b, u, v, coeff_b, coeff_d, q, overflow_aware);

    let q_calc = b.alloc_qubits(q.len());

    if initial_endpoint_step {
        emit_neg_low_twos_complement_into_zero(b, coeff_d, &q_calc);
        for (&src, &dst) in q_calc.iter().zip(q.iter()) {
            b.cx(src, dst);
        }
        emit_neg_low_twos_complement_into_zero_inverse(b, coeff_d, &q_calc);
        b.free_vec(&q_calc);
        return;
    }

    let numerator = b.alloc_qubits(coeff_d.len());
    let denominator = b.alloc_qubits(coeff_b.len());
    emit_abs_twos_complement_into_zero(b, coeff_d, &numerator);
    emit_abs_twos_complement_into_zero(b, coeff_b, &denominator);
    super::sub_nbit_const(b, &numerator, U256::from(1u64));

    let decoder_start = b.ops.len();
    if overflow_aware {
        gcd_coefficient_decoder::emit_halfgcd_coeff_quotient_decoder_overflow_aware(
            b,
            &numerator,
            &denominator,
            &q_calc,
        );
    } else {
        gcd_coefficient_decoder::emit_halfgcd_coeff_quotient_decoder(
            b,
            &numerator,
            &denominator,
            &q_calc,
        );
    }
    let decoder_ops = b.ops[decoder_start..].to_vec();
    for (&src, &dst) in q_calc.iter().zip(q.iter()) {
        b.cx(src, dst);
    }
    replay_inverse_of_clean_forward_ops(b, &decoder_ops);

    super::add_nbit_const(b, &numerator, U256::from(1u64));
    emit_abs_twos_complement_into_zero_inverse(b, coeff_b, &denominator);
    emit_abs_twos_complement_into_zero_inverse(b, coeff_d, &numerator);
    b.free_vec(&denominator);
    b.free_vec(&numerator);
    b.free_vec(&q_calc);
}

pub(super) fn round198_semantic_coeff_clean_sequence_widths() -> (usize, usize, usize) {
    let profile = round158_prefix_profile(SECP256K1_P, round146_semantic_max_divisor(), N, 128);
    (profile.lane_width, profile.coeff_width, profile.max_q_bits)
}

pub(super) fn emit_round198_semantic_coeff_clean_prefix_sequence(
    b: &mut B,
    u: &[QubitId],
    v: &[QubitId],
    coeff_b: &[QubitId],
    coeff_d: &[QubitId],
    q: &[QubitId],
    overflow_aware: bool,
) {
    let profile = round158_prefix_profile(SECP256K1_P, round146_semantic_max_divisor(), N, 128);
    assert_eq!(u.len(), profile.lane_width);
    assert_eq!(v.len(), profile.lane_width);
    assert_eq!(coeff_b.len(), profile.coeff_width);
    assert_eq!(coeff_d.len(), profile.coeff_width);
    assert!(q.len() >= profile.max_q_bits);

    for (idx, step) in profile.steps.iter().enumerate() {
        b.set_phase(match idx {
            0 => "round198_semantic_coeff_clean_step0",
            1 => "round198_semantic_coeff_clean_step1",
            2 => "round198_semantic_coeff_clean_step2",
            3 => "round198_semantic_coeff_clean_step3",
            _ => "round198_semantic_coeff_clean_step_ge4",
        });
        emit_round197_numeric_endpoint_step_clean_q_from_coeff_with_decoder(
            b,
            u,
            v,
            coeff_b,
            coeff_d,
            &q[..step.q_bits],
            idx == 0,
            overflow_aware,
        );
    }
}

pub(super) fn round199_semantic_full_gcd_prefix_widths() -> (usize, usize, usize, usize, usize) {
    let profile = round158_prefix_profile(SECP256K1_P, round146_semantic_max_divisor(), N, 1);
    (
        profile.lane_width,
        profile.coeff_width,
        profile.total_q_bits,
        profile.max_q_bits,
        profile.steps.len(),
    )
}

pub(super) fn emit_round199_semantic_full_gcd_prefix_sequence(
    b: &mut B,
    u: &[QubitId],
    v: &[QubitId],
    coeff_b: &[QubitId],
    coeff_d: &[QubitId],
    q_tail: &[QubitId],
) {
    let profile = round158_prefix_profile(SECP256K1_P, round146_semantic_max_divisor(), N, 1);
    assert_eq!(u.len(), profile.lane_width);
    assert_eq!(v.len(), profile.lane_width);
    assert_eq!(coeff_b.len(), profile.coeff_width);
    assert_eq!(coeff_d.len(), profile.coeff_width);
    assert_eq!(q_tail.len(), profile.total_q_bits);

    let mut q_offset = 0usize;
    for (idx, step) in profile.steps.iter().enumerate() {
        b.set_phase(match idx {
            0 => "round199_semantic_full_gcd_prefix_step0",
            1 => "round199_semantic_full_gcd_prefix_step1",
            2 => "round199_semantic_full_gcd_prefix_step2",
            3 => "round199_semantic_full_gcd_prefix_step3",
            _ => "round199_semantic_full_gcd_prefix_step_ge4",
        });
        let q = &q_tail[q_offset..q_offset + step.q_bits];
        gcd_coefficient_decoder::emit_halfgcd_coeff_quotient_decoder(b, u, v, q);
        emit_sub_q_times_twos_complement(b, q, coeff_d, coeff_b);
        swap_regs(b, u, v);
        swap_regs(b, coeff_b, coeff_d);
        q_offset += step.q_bits;
    }
    assert_eq!(q_offset, q_tail.len());
}

pub(super) fn emit_round199_semantic_full_gcd_prefix_roundtrip(
    b: &mut B,
    u: &[QubitId],
    v: &[QubitId],
    coeff_b: &[QubitId],
    coeff_d: &[QubitId],
    q_tail: &[QubitId],
) {
    let start = b.ops.len();
    emit_round199_semantic_full_gcd_prefix_sequence(b, u, v, coeff_b, coeff_d, q_tail);
    let fwd = b.ops[start..].to_vec();
    b.set_phase("round199_semantic_full_gcd_prefix_inverse_replay");
    replay_inverse_of_clean_forward_ops(b, &fwd);
}

pub(super) fn replay_round199_semantic_full_gcd_prefix_inverse_from_ops(
    b: &mut B,
    forward_ops: &[Op],
) {
    b.set_phase("round199_semantic_full_gcd_prefix_inverse_replay");
    replay_inverse_of_clean_forward_ops(b, forward_ops);
}

fn emit_round158_live_prefix_splice(
    b: &mut B,
    live_denominator: &[QubitId],
    p: U256,
    profile: &Round158PrefixProfile,
) -> Round158LivePrefixEmission {
    assert_eq!(live_denominator.len(), profile.lane_width);
    assert!(profile.coeff_width >= profile.max_q_bits);

    b.set_phase("round158_load_live_denominator");
    let u = load_const(b, profile.lane_width, p);
    let v = b.alloc_qubits(profile.lane_width);
    for (&src, &dst) in live_denominator.iter().zip(v.iter()) {
        b.cx(src, dst);
    }

    let coeff_b = b.alloc_qubits(profile.coeff_width);
    let coeff_d = b.alloc_qubits(profile.coeff_width);
    b.x(coeff_d[0]);
    let q_tail = b.alloc_qubits(profile.total_q_bits);

    let mut q_offset = 0usize;
    for (idx, step) in profile.steps.iter().enumerate() {
        b.set_phase(match idx {
            0 => "round158_prefix_step0",
            1 => "round158_prefix_step1",
            2 => "round158_prefix_step2",
            3 => "round158_prefix_step3",
            _ => "round158_prefix_step_ge4",
        });
        let q = &q_tail[q_offset..q_offset + step.q_bits];
        gcd_coefficient_decoder::emit_halfgcd_coeff_quotient_decoder(b, &u, &v, q);
        emit_sub_q_times_twos_complement(b, q, &coeff_d, &coeff_b);
        swap_regs(b, &u, &v);
        swap_regs(b, &coeff_b, &coeff_d);
        q_offset += step.q_bits;
    }
    assert_eq!(q_offset, q_tail.len());

    Round158LivePrefixEmission {
        u,
        v,
        coeff_b,
        coeff_d,
        q_tail,
    }
}

fn count_toffoli(ops: &[crate::circuit::Op]) -> usize {
    ops.iter()
        .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
        .count()
}

fn set_word<R: sha3::digest::XofReader>(
    sim: &mut crate::sim::Simulator<R>,
    qs: &[QubitId],
    value: u64,
) {
    for (bit, &q) in qs.iter().enumerate() {
        if ((value >> bit) & 1) != 0 {
            *sim.qubit_mut(q) |= 1;
        } else {
            *sim.qubit_mut(q) &= !1;
        }
    }
}

fn read_word<R: sha3::digest::XofReader>(sim: &crate::sim::Simulator<R>, qs: &[QubitId]) -> u64 {
    let mut out = 0u64;
    for (bit, &q) in qs.iter().enumerate().take(64) {
        out |= ((sim.qubit(q) & 1) as u64) << bit;
    }
    out
}

fn set_word_shot<R: sha3::digest::XofReader>(
    sim: &mut crate::sim::Simulator<R>,
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

fn read_word_shot<R: sha3::digest::XofReader>(
    sim: &crate::sim::Simulator<R>,
    qs: &[QubitId],
    shot: usize,
) -> u64 {
    let mut out = 0u64;
    for (bit, &q) in qs.iter().enumerate().take(64) {
        out |= ((sim.qubit(q) >> shot) & 1) << bit;
    }
    out
}

fn twos_complement_word(x: SignedMag, width: usize) -> u64 {
    assert!(width < 64);
    let mask = (1u128 << width) - 1;
    let mag = x.mag.as_limbs()[0] as u128;
    assert!(mag < (1u128 << (width - 1)));
    let raw = if x.neg {
        ((!mag).wrapping_add(1)) & mask
    } else {
        mag & mask
    };
    raw as u64
}

fn expected_tail_word(profile: &Round158PrefixProfile) -> u64 {
    assert!(profile.total_q_bits <= 64);
    let mut out = 0u64;
    let mut offset = 0usize;
    for step in &profile.steps {
        for bit in 0..step.q_bits {
            if step.q.bit(bit) {
                out |= 1u64 << (offset + bit);
            }
        }
        offset += step.q_bits;
    }
    out
}

fn assert_only_named_live_qubits_nonzero<R: sha3::digest::XofReader>(
    sim: &crate::sim::Simulator<R>,
    num_qubits: usize,
    live: &[&[QubitId]],
) {
    let live: BTreeSet<u64> = live.iter().flat_map(|qs| qs.iter().map(|q| q.0)).collect();
    for idx in 0..num_qubits {
        if !live.contains(&(idx as u64)) {
            assert_eq!(
                sim.qubit(QubitId(idx as u64)) & 1,
                0,
                "scratch q{idx} leaked"
            );
        }
    }
}

fn secp_rhs(x: U256) -> U256 {
    x.mul_mod(x, SECP256K1_P)
        .mul_mod(x, SECP256K1_P)
        .add_mod(U256::from(7u64), SECP256K1_P)
}

fn secp_sqrt_if_square(rhs: U256) -> Option<U256> {
    let exp = SECP256K1_P.wrapping_add(U256::from(1u64)) >> 2usize;
    let y = rhs.pow_mod(exp, SECP256K1_P);
    if y.mul_mod(y, SECP256K1_P) == rhs {
        Some(y)
    } else {
        None
    }
}

fn first_secp_x_pair_with_delta_one(limit: u64) -> (U256, U256, U256, U256) {
    for x0 in 1..limit {
        let x1 = x0 + 1;
        let x0 = U256::from(x0);
        let x1 = U256::from(x1);
        if let (Some(y0), Some(y1)) = (
            secp_sqrt_if_square(secp_rhs(x0)),
            secp_sqrt_if_square(secp_rhs(x1)),
        ) {
            return (x0, y0, x1, y1);
        }
    }
    panic!("no delta-one secp x-coordinate pair found below {limit}");
}

#[test]
fn round158_numeric_endpoint_step_toy_exact_updates_live_registers() {
    use sha3::digest::{ExtendableOutput, Update};

    const W: usize = 4;
    const CW: usize = 8;
    const BATCH: usize = 64;

    let mut b = B::new();
    let u = b.alloc_qubits(W);
    let v = b.alloc_qubits(W);
    let coeff_b = b.alloc_qubits(CW);
    let coeff_d = b.alloc_qubits(CW);
    let q = b.alloc_qubits(W);
    let start = b.ops.len();
    emit_round158_numeric_endpoint_step(&mut b, &u, &v, &coeff_b, &coeff_d, &q);
    let toffoli = count_toffoli(&b.ops[start..]);
    let num_qubits = b.next_qubit as usize;
    let num_bits = b.next_bit as usize;
    let ops = b.ops.clone();

    let mut cases = Vec::new();
    for start_u in 0u64..(1 << W) {
        for start_v in 1u64..(1 << W) {
            for start_b in 0u64..16 {
                for start_d in 0u64..16 {
                    cases.push((start_u, start_v, start_b, start_d));
                }
            }
        }
    }

    let mut checked = 0usize;
    for batch in cases.chunks(BATCH) {
        let mut hasher = sha3::Shake128::default();
        hasher.update(b"round158-numeric-endpoint-step");
        let mut xof = hasher.finalize_xof();
        let mut sim = crate::sim::Simulator::new(num_qubits, num_bits, &mut xof);

        for (shot, &(start_u, start_v, start_b, start_d)) in batch.iter().enumerate() {
            set_word_shot(&mut sim, &u, start_u, shot);
            set_word_shot(&mut sim, &v, start_v, shot);
            set_word_shot(&mut sim, &coeff_b, start_b, shot);
            set_word_shot(&mut sim, &coeff_d, start_d, shot);
        }

        sim.apply(&ops);

        for (shot, &(start_u, start_v, start_b, start_d)) in batch.iter().enumerate() {
            let quo = start_u / start_v;
            let rem = start_u % start_v;
            let next_b = start_d;
            let next_d = start_b.wrapping_sub(quo.wrapping_mul(start_d)) & ((1 << CW) - 1);

            assert_eq!(read_word_shot(&sim, &u, shot), start_v);
            assert_eq!(read_word_shot(&sim, &v, shot), rem);
            assert_eq!(read_word_shot(&sim, &coeff_b, shot), next_b);
            assert_eq!(read_word_shot(&sim, &coeff_d, shot), next_d);
            assert_eq!(read_word_shot(&sim, &q, shot), quo);
        }

        assert_eq!(sim.global_phase(), 0);
        assert_only_named_live_qubits_nonzero(&sim, num_qubits, &[&u, &v, &coeff_b, &coeff_d, &q]);
        checked += batch.len();
    }

    assert_eq!(checked, 61_440);
    println!("METRIC round158_numeric_endpoint_step_toy_cases={checked}");
    println!("METRIC round158_numeric_endpoint_step_toy_toffoli={toffoli}");
    println!(
        "METRIC round158_numeric_endpoint_step_toy_peak_q={}",
        b.peak_qubits
    );
}

#[test]
fn round197_numeric_endpoint_step_copy_clean_restores_source_and_q() {
    use sha3::digest::{ExtendableOutput, Update};

    const W: usize = 4;
    const CW: usize = 8;
    const BATCH: usize = 64;

    let mut b = B::new();
    let u = b.alloc_qubits(W);
    let v = b.alloc_qubits(W);
    let coeff_b = b.alloc_qubits(CW);
    let coeff_d = b.alloc_qubits(CW);
    let q = b.alloc_qubits(W);
    let u_out = b.alloc_qubits(W);
    let v_out = b.alloc_qubits(W);
    let coeff_b_out = b.alloc_qubits(CW);
    let coeff_d_out = b.alloc_qubits(CW);
    let start = b.ops.len();
    emit_round197_numeric_endpoint_step_copy_clean(
        &mut b,
        &u,
        &v,
        &coeff_b,
        &coeff_d,
        &q,
        &u_out,
        &v_out,
        &coeff_b_out,
        &coeff_d_out,
    );
    let toffoli = count_toffoli(&b.ops[start..]);
    let num_qubits = b.next_qubit as usize;
    let num_bits = b.next_bit as usize;
    let ops = b.ops.clone();

    let mut cases = Vec::new();
    for start_u in 0u64..(1 << W) {
        for start_v in 1u64..(1 << W) {
            for start_b in 0u64..16 {
                for start_d in 0u64..16 {
                    cases.push((start_u, start_v, start_b, start_d));
                }
            }
        }
    }

    let mut checked = 0usize;
    for batch in cases.chunks(BATCH) {
        let mut hasher = sha3::Shake128::default();
        hasher.update(b"round197-numeric-endpoint-step-copy-clean");
        let mut xof = hasher.finalize_xof();
        let mut sim = crate::sim::Simulator::new(num_qubits, num_bits, &mut xof);

        for (shot, &(start_u, start_v, start_b, start_d)) in batch.iter().enumerate() {
            set_word_shot(&mut sim, &u, start_u, shot);
            set_word_shot(&mut sim, &v, start_v, shot);
            set_word_shot(&mut sim, &coeff_b, start_b, shot);
            set_word_shot(&mut sim, &coeff_d, start_d, shot);
        }

        sim.apply(&ops);

        for (shot, &(start_u, start_v, start_b, start_d)) in batch.iter().enumerate() {
            let quo = start_u / start_v;
            let rem = start_u % start_v;
            let next_b = start_d;
            let next_d = start_b.wrapping_sub(quo.wrapping_mul(start_d)) & ((1 << CW) - 1);

            assert_eq!(read_word_shot(&sim, &u, shot), start_u);
            assert_eq!(read_word_shot(&sim, &v, shot), start_v);
            assert_eq!(read_word_shot(&sim, &coeff_b, shot), start_b);
            assert_eq!(read_word_shot(&sim, &coeff_d, shot), start_d);
            assert_eq!(read_word_shot(&sim, &q, shot), 0);
            assert_eq!(read_word_shot(&sim, &u_out, shot), start_v);
            assert_eq!(read_word_shot(&sim, &v_out, shot), rem);
            assert_eq!(read_word_shot(&sim, &coeff_b_out, shot), next_b);
            assert_eq!(read_word_shot(&sim, &coeff_d_out, shot), next_d);
        }

        assert_eq!(sim.global_phase(), 0);
        assert_only_named_live_qubits_nonzero(
            &sim,
            num_qubits,
            &[
                &u,
                &v,
                &coeff_b,
                &coeff_d,
                &u_out,
                &v_out,
                &coeff_b_out,
                &coeff_d_out,
            ],
        );
        checked += batch.len();
    }

    assert_eq!(checked, 61_440);
    println!("METRIC round197_numeric_endpoint_step_copy_clean_toy_cases={checked}");
    println!("METRIC round197_numeric_endpoint_step_copy_clean_toy_toffoli={toffoli}");
    println!(
        "METRIC round197_numeric_endpoint_step_copy_clean_toy_peak_q={}",
        b.peak_qubits
    );
}

#[test]
fn round197_numeric_endpoint_step_cleans_q_from_second_column_coefficients() {
    use sha3::digest::{ExtendableOutput, Update};

    const W: usize = 4;
    const CW: usize = 8;
    const BATCH: usize = 64;

    let raw_from_i64 = |x: i64| -> u64 { ((x as i128) & ((1i128 << CW) - 1)) as u64 };
    let i64_from_raw = |raw: u64| -> i64 {
        let raw = raw & ((1u64 << CW) - 1);
        if (raw & (1u64 << (CW - 1))) != 0 {
            raw as i64 - (1i64 << CW)
        } else {
            raw as i64
        }
    };

    let mut one = B::new();
    let u1 = one.alloc_qubits(W);
    let v1 = one.alloc_qubits(W);
    let b1 = one.alloc_qubits(CW);
    let d1 = one.alloc_qubits(CW);
    let q1 = one.alloc_qubits(W);
    let one_start = one.ops.len();
    emit_round197_numeric_endpoint_step_clean_q_from_coeff(&mut one, &u1, &v1, &b1, &d1, &q1, true);
    let one_toffoli = count_toffoli(&one.ops[one_start..]);
    let one_num_qubits = one.next_qubit as usize;
    let one_num_bits = one.next_bit as usize;
    let one_ops = one.ops.clone();

    let one_cases: Vec<_> = (0u64..(1 << W))
        .flat_map(|u0| (1u64..(1 << W)).map(move |v0| (u0, v0)))
        .collect();
    let mut checked_one = 0usize;
    for batch in one_cases.chunks(BATCH) {
        let mut hasher = sha3::Shake128::default();
        hasher.update(b"round197-clean-q-from-coeff-one-step");
        let mut xof = hasher.finalize_xof();
        let mut sim = crate::sim::Simulator::new(one_num_qubits, one_num_bits, &mut xof);
        for (shot, &(u0, v0)) in batch.iter().enumerate() {
            set_word_shot(&mut sim, &u1, u0, shot);
            set_word_shot(&mut sim, &v1, v0, shot);
            set_word_shot(&mut sim, &d1, 1, shot);
        }
        sim.apply(&one_ops);
        for (shot, &(u0, v0)) in batch.iter().enumerate() {
            let q = (u0 / v0) as i64;
            let rem = u0 % v0;
            assert_eq!(read_word_shot(&sim, &u1, shot), v0);
            assert_eq!(read_word_shot(&sim, &v1, shot), rem);
            assert_eq!(i64_from_raw(read_word_shot(&sim, &b1, shot)), 1);
            assert_eq!(i64_from_raw(read_word_shot(&sim, &d1, shot)), -q);
            assert_eq!(
                read_word_shot(&sim, &q1, shot),
                0,
                "q not cleaned for one-step u0={u0} v0={v0}"
            );
        }
        assert_eq!(sim.global_phase(), 0);
        assert_only_named_live_qubits_nonzero(&sim, one_num_qubits, &[&u1, &v1, &b1, &d1]);
        checked_one += batch.len();
    }

    let mut two = B::new();
    let u2 = two.alloc_qubits(W);
    let v2 = two.alloc_qubits(W);
    let b2 = two.alloc_qubits(CW);
    let d2 = two.alloc_qubits(CW);
    let q2 = two.alloc_qubits(W);
    let two_start = two.ops.len();
    emit_round197_numeric_endpoint_step_clean_q_from_coeff(&mut two, &u2, &v2, &b2, &d2, &q2, true);
    emit_round197_numeric_endpoint_step_clean_q_from_coeff(
        &mut two, &u2, &v2, &b2, &d2, &q2, false,
    );
    let two_toffoli = count_toffoli(&two.ops[two_start..]);
    let two_num_qubits = two.next_qubit as usize;
    let two_num_bits = two.next_bit as usize;
    let two_ops = two.ops.clone();

    let two_cases: Vec<_> = one_cases
        .iter()
        .copied()
        .filter(|(u0, v0)| u0 >= v0 && u0 % v0 != 0)
        .collect();
    let mut checked_two = 0usize;
    for batch in two_cases.chunks(BATCH) {
        let mut hasher = sha3::Shake128::default();
        hasher.update(b"round197-clean-q-from-coeff-two-step");
        let mut xof = hasher.finalize_xof();
        let mut sim = crate::sim::Simulator::new(two_num_qubits, two_num_bits, &mut xof);
        for (shot, &(u0, v0)) in batch.iter().enumerate() {
            set_word_shot(&mut sim, &u2, u0, shot);
            set_word_shot(&mut sim, &v2, v0, shot);
            set_word_shot(&mut sim, &d2, 1, shot);
        }
        sim.apply(&two_ops);
        for (shot, &(u0, v0)) in batch.iter().enumerate() {
            let q0 = (u0 / v0) as i64;
            let mut eu = v0;
            let mut ev = u0 % v0;
            let mut eb = 1i64;
            let mut ed = -q0;
            let q = (eu / ev) as i64;
            let rem = eu % ev;
            let next_b = ed;
            let next_d = eb - q * ed;
            eu = ev;
            ev = rem;
            eb = next_b;
            ed = next_d;

            assert_eq!(read_word_shot(&sim, &u2, shot), eu);
            assert_eq!(read_word_shot(&sim, &v2, shot), ev);
            assert_eq!(i64_from_raw(read_word_shot(&sim, &b2, shot)), eb);
            assert_eq!(i64_from_raw(read_word_shot(&sim, &d2, shot)), ed);
            assert_eq!(
                read_word_shot(&sim, &q2, shot),
                0,
                "q not cleaned for two-step u0={u0} v0={v0}"
            );
        }
        assert_eq!(sim.global_phase(), 0);
        assert_only_named_live_qubits_nonzero(&sim, two_num_qubits, &[&u2, &v2, &b2, &d2]);
        checked_two += batch.len();
    }

    assert_eq!(checked_one, 240);
    assert!(checked_two > 0);
    println!("METRIC round197_clean_q_coeff_one_step_cases={checked_one}");
    println!("METRIC round197_clean_q_coeff_two_step_cases={checked_two}");
    println!("METRIC round197_clean_q_coeff_one_step_toffoli={one_toffoli}");
    println!(
        "METRIC round197_clean_q_coeff_one_step_peak_q={}",
        one.peak_qubits
    );
    println!("METRIC round197_clean_q_coeff_two_step_toffoli={two_toffoli}");
    println!(
        "METRIC round197_clean_q_coeff_two_step_peak_q={}",
        two.peak_qubits
    );
    let _ = raw_from_i64;
}

#[test]
fn round198_semantic_coeff_clean_regular_profile_shift_fits() {
    let profile = round158_prefix_profile(SECP256K1_P, round146_semantic_max_divisor(), N, 128);
    let mut u = SECP256K1_P;
    let mut v = profile.divisor;
    let mut coeff_b = smag(false, U512::ZERO);
    let mut coeff_d = smag(false, U512::from(1u64));
    let mut max_coeff_decoder_width = 0usize;
    let mut max_coeff_shift_sum = 0usize;
    let mut max_residual_shift_sum = 0usize;

    for (idx, step) in profile.steps.iter().enumerate() {
        let q = u / v;
        assert_eq!(q, step.q, "profile q drift at step {idx}");
        let q_bits = u256_bit_len(q).max(1);
        assert_eq!(q_bits, step.q_bits, "profile q width drift at step {idx}");

        let residual_shift_sum = u256_bit_len(v) + q_bits.saturating_sub(1);
        max_residual_shift_sum = max_residual_shift_sum.max(residual_shift_sum);
        assert!(
            residual_shift_sum <= profile.lane_width,
            "regular residual decoder shift overflow at step {idx}: v_bits={} q_bits={} lane={}",
            u256_bit_len(v),
            q_bits,
            profile.lane_width
        );

        let next_b = coeff_d;
        let next_d = signed_sub_scaled(coeff_b, q, coeff_d);
        let numerator = if coeff_b.mag.is_zero() {
            next_d.mag
        } else {
            assert!(
                !next_d.mag.is_zero(),
                "zero coefficient numerator at step {idx}"
            );
            next_d.mag - U512::from(1u64)
        };
        let denominator = next_b.mag;
        assert!(
            !denominator.is_zero(),
            "zero coefficient denominator at step {idx}"
        );
        assert_eq!(
            numerator / denominator,
            u512_from_u256(q),
            "coefficient reverse quotient formula drift at step {idx}"
        );
        let decoder_width = u512_bit_len(numerator)
            .max(u512_bit_len(denominator))
            .max(1);
        let coeff_shift_sum = u512_bit_len(denominator) + q_bits.saturating_sub(1);
        max_coeff_decoder_width = max_coeff_decoder_width.max(decoder_width);
        max_coeff_shift_sum = max_coeff_shift_sum.max(coeff_shift_sum);
        assert!(
            coeff_shift_sum <= decoder_width,
            "regular coefficient decoder shift overflow at step {idx}: denom_bits={} q_bits={} width={decoder_width}",
            u512_bit_len(denominator),
            q_bits
        );

        let rem = u - q * v;
        u = v;
        v = rem;
        coeff_b = next_b;
        coeff_d = next_d;
    }

    println!(
        "METRIC round198_semantic_regular_steps={}",
        profile.steps.len()
    );
    println!(
        "METRIC round198_semantic_regular_max_q_bits={}",
        profile.max_q_bits
    );
    println!("METRIC round198_semantic_regular_max_residual_shift_sum={max_residual_shift_sum}");
    println!("METRIC round198_semantic_regular_max_coeff_decoder_width={max_coeff_decoder_width}");
    println!("METRIC round198_semantic_regular_max_coeff_shift_sum={max_coeff_shift_sum}");
    assert_eq!(profile.steps.len(), 105);
    assert_eq!(profile.max_q_bits, 5);
    assert_eq!(max_residual_shift_sum, 256);
    assert_eq!(max_coeff_decoder_width, 130);
}

#[test]
fn round199_semantic_full_gcd_profile_reaches_inverse_coefficient() {
    let profile = round158_prefix_profile(SECP256K1_P, round146_semantic_max_divisor(), N, 1);
    let mut max_residual_shift_sum = 0usize;
    let mut max_coeff_bits_after = 0usize;

    for (idx, step) in profile.steps.iter().enumerate() {
        let residual_shift_sum = step.v_bits + step.q_bits.saturating_sub(1);
        max_residual_shift_sum = max_residual_shift_sum.max(residual_shift_sum);
        max_coeff_bits_after = max_coeff_bits_after.max(step.coeff_bits_after);
        assert!(
            residual_shift_sum <= profile.lane_width,
            "regular residual decoder shift overflow at full-gcd step {idx}: v_bits={} q_bits={} lane={}",
            step.v_bits,
            step.q_bits,
            profile.lane_width
        );
    }

    let signed_mod_p = |x: SignedMag| -> U256 {
        let reduced = x.mag % u512_from_u256(SECP256K1_P);
        let limbs = reduced.as_limbs();
        let mag = U256::from_limbs([limbs[0], limbs[1], limbs[2], limbs[3]]);
        if x.neg && !mag.is_zero() {
            SECP256K1_P - mag
        } else {
            mag
        }
    };
    let inv = signed_mod_p(profile.final_b);
    let final_b_limbs = profile.final_b.mag.as_limbs();
    let final_b_mag_low = U256::from_limbs([
        final_b_limbs[0],
        final_b_limbs[1],
        final_b_limbs[2],
        final_b_limbs[3],
    ]);
    let final_b_low_twos = if profile.final_b.neg {
        U256::ZERO.wrapping_sub(final_b_mag_low)
    } else {
        final_b_mag_low
    };
    let final_b_folded = if profile.final_b.neg {
        final_b_low_twos.wrapping_sub(U256::from(4_294_968_273u64))
    } else {
        final_b_low_twos
    };

    println!(
        "METRIC round199_semantic_full_gcd_steps={}",
        profile.steps.len()
    );
    println!(
        "METRIC round199_semantic_full_gcd_total_q_bits={}",
        profile.total_q_bits
    );
    println!(
        "METRIC round199_semantic_full_gcd_max_q_bits={}",
        profile.max_q_bits
    );
    println!(
        "METRIC round199_semantic_full_gcd_coeff_width={}",
        profile.coeff_width
    );
    println!("METRIC round199_semantic_full_gcd_max_residual_shift_sum={max_residual_shift_sum}");
    println!("METRIC round199_semantic_full_gcd_max_coeff_bits_after={max_coeff_bits_after}");

    assert_eq!(profile.steps.len(), 170);
    assert_eq!(profile.total_q_bits, 343);
    assert_eq!(profile.max_q_bits, 13);
    assert_eq!(profile.coeff_width, 257);
    assert_eq!(profile.final_u, U256::from(1u64));
    assert_eq!(profile.final_v, U256::ZERO);
    assert_eq!(
        inv.mul_mod(profile.divisor, SECP256K1_P),
        U256::from(1u64),
        "final second-column coefficient is not dx inverse mod p"
    );
    assert_eq!(
        final_b_folded, inv,
        "Round200 low-256/sign-fold coefficient mapping is not the inverse"
    );
}

#[test]
fn round158_toy_live_prefix_splice_updates_residual_coefficients_and_tail() {
    use sha3::digest::{ExtendableOutput, Update};

    let p = U256::from(251u64);
    let divisor = U256::from(157u64);
    let profile = round158_prefix_profile(p, divisor, 8, 0);
    assert_eq!(profile.final_u, U256::from(1u64));
    assert_eq!(profile.final_v, U256::ZERO);
    assert!(profile.total_q_bits <= 64);

    let mut b = B::new();
    let live_denominator = b.alloc_qubits(8);
    let start = b.ops.len();
    let emitted = emit_round158_live_prefix_splice(&mut b, &live_denominator, p, &profile);
    let toffoli = count_toffoli(&b.ops[start..]);
    let num_qubits = b.next_qubit as usize;
    let num_bits = b.next_bit as usize;
    let ops = b.ops.clone();

    let mut hasher = sha3::Shake128::default();
    hasher.update(b"round158-toy-live-prefix-splice");
    let mut xof = hasher.finalize_xof();
    let mut sim = crate::sim::Simulator::new(num_qubits, num_bits, &mut xof);
    set_word(&mut sim, &live_denominator, 157);
    sim.apply(&ops);

    assert_eq!(read_word(&sim, &emitted.u), 1);
    assert_eq!(read_word(&sim, &emitted.v), 0);
    assert_eq!(
        read_word(&sim, &emitted.coeff_b),
        twos_complement_word(profile.final_b, profile.coeff_width)
    );
    assert_eq!(
        read_word(&sim, &emitted.coeff_d),
        twos_complement_word(profile.final_d, profile.coeff_width)
    );
    assert_eq!(
        read_word(&sim, &emitted.q_tail),
        expected_tail_word(&profile)
    );
    assert_eq!(sim.global_phase() & 1, 0);
    assert_only_named_live_qubits_nonzero(
        &sim,
        num_qubits,
        &[
            &live_denominator,
            &emitted.u,
            &emitted.v,
            &emitted.coeff_b,
            &emitted.coeff_d,
            &emitted.q_tail,
        ],
    );

    println!("METRIC round158_toy_splice_steps={}", profile.steps.len());
    println!(
        "METRIC round158_toy_splice_tail_bits={}",
        profile.total_q_bits
    );
    println!("METRIC round158_toy_splice_toffoli={toffoli}");
    println!("METRIC round158_toy_splice_peak_q={}", b.peak_qubits);
}

#[test]
fn round158_secp_semantic_live_prefix_bench_is_stateful_but_not_abi_clean() {
    let divisor = round146_semantic_max_divisor();
    let profile = round158_prefix_profile(SECP256K1_P, divisor, N, 128);
    assert_eq!(profile.steps.len(), 105);
    assert_eq!(profile.total_q_bits, 183);
    assert_eq!(profile.max_q_bits, 5);

    let mut b = B::new();
    let tx = b.alloc_qubits(N);
    b.declare_qubit_register(&tx);
    let ty = b.alloc_qubits(N);
    b.declare_qubit_register(&ty);
    let ox = b.alloc_bits(N);
    b.declare_bit_register(&ox);
    let oy = b.alloc_bits(N);
    b.declare_bit_register(&oy);

    let start = b.ops.len();
    let emitted = emit_round158_live_prefix_splice(&mut b, &tx, SECP256K1_P, &profile);
    let toffoli = count_toffoli(&b.ops[start..]);
    let complete_ec_addition_candidate = false;
    let under_local_resource_envelope = (b.peak_qubits as usize) < 2_000 && toffoli < 3_000_000;

    println!(
        "METRIC round158_secp_semantic_splice_steps={}",
        profile.steps.len()
    );
    println!(
        "METRIC round158_secp_semantic_splice_tail_bits={}",
        emitted.q_tail.len()
    );
    println!(
        "METRIC round158_secp_semantic_splice_coeff_width={}",
        profile.coeff_width
    );
    println!("METRIC round158_secp_semantic_splice_toffoli={toffoli}");
    println!(
        "METRIC round158_secp_semantic_splice_peak_q={}",
        b.peak_qubits
    );
    println!(
        "METRIC round158_secp_semantic_splice_under_q2000_t3m={}",
        under_local_resource_envelope as u8
    );
    println!(
        "METRIC round158_secp_semantic_splice_complete_ec_addition_candidate={}",
        complete_ec_addition_candidate as u8
    );

    assert!(
        !emitted.q_tail.is_empty(),
        "splice did not expose quotient tail state"
    );
    assert!(!complete_ec_addition_candidate);
    assert!(toffoli > 0, "bench did not emit arithmetic");
    let _ = (ty, ox, oy, emitted);
}

#[test]
fn round158_round146_fixed_sequence_is_math_blocked_for_google_abi_denominator() {
    let (x0, y0, x1, y1) = first_secp_x_pair_with_delta_one(512);
    assert_eq!(x1 - x0, U256::from(1u64));
    assert_eq!(secp_rhs(x0), y0.mul_mod(y0, SECP256K1_P));
    assert_eq!(secp_rhs(x1), y1.mul_mod(y1, SECP256K1_P));

    let divisor = x1 - x0;
    let first_q = SECP256K1_P / divisor;
    let required_first_q_bits = u256_bit_len(first_q).max(1);
    let semantic_profile =
        round158_prefix_profile(SECP256K1_P, round146_semantic_max_divisor(), N, 128);
    let rejection =
        profile_rejection_for_live_divisor(SECP256K1_P, divisor, N, 128, &semantic_profile)
            .expect("delta-one denominator should not satisfy the Round146 fixed sequence");

    println!("METRIC round158_valid_curve_delta_one_x0={x0}");
    println!("METRIC round158_valid_curve_delta_one_x1={x1}");
    println!("METRIC round158_delta_one_required_first_q_bits={required_first_q_bits}");
    println!(
        "METRIC round158_round146_semantic_first_q_bits={}",
        semantic_profile.steps[0].q_bits
    );
    println!("METRIC round158_round146_live_exact_rejected=1");

    assert_eq!(required_first_q_bits, 256);
    assert_eq!(
        rejection,
        ProfileRejection::QuotientBitsTooWide {
            step: 0,
            required: 256,
            provided: semantic_profile.steps[0].q_bits,
        }
    );
}

#[test]
fn round158_build_standard_route_gate_reports_wire_state_blocker() {
    let mut b = B::new();
    let tx = b.alloc_qubits(N);
    let ty = b.alloc_qubits(N);
    let ox = b.alloc_bits(N);
    let oy = b.alloc_bits(N);

    let blocker = round158_live_prefix_pa_route_blocker_message(&tx, &ty, &ox, &oy, SECP256K1_P);
    println!("METRIC round158_build_standard_route_gate_blocked=1");
    println!(
        "METRIC round158_build_standard_route_gate_msg_len={}",
        blocker.len()
    );

    assert!(blocker.contains(ROUND158_HALFGCD_LIVE_PREFIX_PA_ROUTE_ENV));
    assert!(blocker.contains("tx is the live quantum denominator dx"));
    assert!(blocker.contains("dx=x1-x0=1"));
    assert!(blocker.contains("needs step0 q=p/dx with 256 bits"));
    assert!(blocker.contains("leaves u/v residuals, coeff_b/coeff_d, and q_tail live"));
    assert!(blocker.contains("wire/state blocker"));
}
