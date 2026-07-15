//! Round162 source-live half-GCD PA splice route.
//!
//! This is a build route, not a resource note.  It is selected by
//! `ROUND162_HALFGCD_LIVE_PA=1` from `build_standard_ec_add`, after the
//! Google PA inputs have already been transformed to
//! `tx = dx = target_x - offset_x` and `ty = dy = target_y - offset_y`.
//!
//! The current source-live splice is deliberately fail-closed: Round202 removed
//! the stale shifted-denominator fit objection and the initial q-clean width
//! waste for a single full-width quotient step, but the route still has no total
//! fixed-depth prefix/tail schedule that restores the Google ABI registers and
//! erases the quotient/coefficient state.

use super::{B, N, SECP256K1_P};
use crate::circuit::{BitId, QubitId};
use alloy_primitives::U256;
use std::fmt::Write as _;

pub(super) const ROUND162_HALFGCD_LIVE_PA_ENV: &str = "ROUND162_HALFGCD_LIVE_PA";
pub const ROUND162_HALFGCD_LIVE_PA_RESOURCE_GATE_ENV: &str =
    "ROUND162_HALFGCD_LIVE_PA_RESOURCE_GATE";
pub const ROUND256_HALFGCD_ROUND145_FIRST_SLOT_GATE_ENV: &str =
    "ROUND256_HALFGCD_ROUND145_FIRST_SLOT_GATE";

const GOOGLE_RELAXED_Q_TARGET: usize = 2_100;
const GOOGLE_RELAXED_T_TARGET: usize = 3_100_000;
const GOOGLE_MILESTONE_Q_TARGET: usize = 2_000;
const GOOGLE_MILESTONE_T_TARGET: usize = 3_000_000;
const GOOGLE_MILESTONE_QT_TARGET: usize = 9_000_000_000;
const FIRST_STEP_LIVE_DX_MAX_Q_BITS: usize = 256;

pub const ROUND199_SEMANTIC_FULL_GCD_PREFIX_QUBITS: usize = 1_627;
pub const ROUND199_SEMANTIC_FULL_GCD_PREFIX_TOFFOLI: usize = 875_788;
pub const ROUND200_SEMANTIC_FULL_GCD_PAIR1_CHECKPOINT_QUBITS: usize = 2_654;
pub const ROUND200_SEMANTIC_FULL_GCD_PAIR1_CHECKPOINT_TOFFOLI: usize = 2_564_547;
pub const ROUND200_FULL_GCD_PAIR1_PA_QUBITS: usize = 2_654;
pub const ROUND200_FULL_GCD_PAIR1_PA_TOFFOLI: usize = 4_612_455;
pub const ROUND200_FULL_GCD_PAIR1_PA_QT: usize =
    ROUND200_FULL_GCD_PAIR1_PA_QUBITS * ROUND200_FULL_GCD_PAIR1_PA_TOFFOLI;
pub const ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_QUBITS: usize = 2_476;
pub const ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_TOFFOLI: usize = 5_142_140;
pub const ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_QT: usize =
    ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_QUBITS * ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_TOFFOLI;
pub const ROUND162_HALFGCD_RESOURCE_GATE_CLASSIFICATION: &str =
    "ROUND162_HALFGCD_FULL_GCD_PAIR1_RESOURCE_DEAD";
pub const ROUND256_HALFGCD_ROUND145_FIRST_SLOT_CLASSIFICATION: &str =
    "ROUND256_ROUND145_UNIVERSAL_FIRST_SLOT_DEAD";
pub const ROUND145_SHIFTED_SLICE_PROFILE_QUBITS: usize = 1_539;
pub const ROUND145_SHIFTED_SLICE_PROFILE_TOFFOLI: usize = 2_974_404;
pub const ROUND145_SHIFTED_SLICE_PROFILE_QT: usize =
    ROUND145_SHIFTED_SLICE_PROFILE_QUBITS * ROUND145_SHIFTED_SLICE_PROFILE_TOFFOLI;
pub const ROUND145_PROFILE_MAX_Q_BITS: usize = 26;
pub const ROUND179_UNIVERSAL_FIRST_SLOT_COMPONENT_QUBITS: usize = 1_283;
pub const ROUND179_UNIVERSAL_FIRST_SLOT_COMPONENT_TOFFOLI: usize = 458_241;
pub const ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QUBITS: usize = 2_307;
pub const ROUND179_UNIVERSAL_FIRST_SLOT_NOREUSE_QUBITS: usize = 2_822;
pub const ROUND179_UNIVERSAL_FIRST_SLOT_TOFFOLI: usize = 4_524_688;
pub const ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QT: usize =
    ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QUBITS * ROUND179_UNIVERSAL_FIRST_SLOT_TOFFOLI;
pub const ROUND179_UNIVERSAL_FIRST_SLOT_NOREUSE_QT: usize =
    ROUND179_UNIVERSAL_FIRST_SLOT_NOREUSE_QUBITS * ROUND179_UNIVERSAL_FIRST_SLOT_TOFFOLI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Round162HalfgcdResourceGate {
    pub classification: &'static str,
    pub prefix_qubits: usize,
    pub prefix_toffoli: usize,
    pub state_a_checkpoint_qubits: usize,
    pub state_a_checkpoint_toffoli: usize,
    pub full_pa_qubits: usize,
    pub full_pa_toffoli: usize,
    pub full_pa_qt: usize,
    pub full_pa_qubit_slack: isize,
    pub full_pa_toffoli_slack: isize,
    pub full_pa_qt_slack: isize,
    pub state_a_checkpoint_tail_toffoli: usize,
    pub tail_toffoli_budget_to_3m: usize,
    pub tail_toffoli_over_3m_budget: isize,
    pub product_toffoli_limit_at_full_pa_q: usize,
    pub full_pa_toffoli_over_product_limit: isize,
    pub lowq_cmod_qubits: usize,
    pub lowq_cmod_toffoli: usize,
    pub lowq_cmod_qt: usize,
    pub lowq_cmod_delta_qubits: isize,
    pub lowq_cmod_delta_toffoli: isize,
    pub lowq_cmod_delta_qt: isize,
    pub makes_first_milestone: bool,
    pub next_required_object: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Round256HalfgcdRound145FirstSlotGate {
    pub classification: &'static str,
    pub round145_profile_qubits: usize,
    pub round145_profile_toffoli: usize,
    pub round145_profile_qt: usize,
    pub round145_profile_qubit_slack: isize,
    pub round145_profile_toffoli_slack: isize,
    pub round145_profile_qt_slack: isize,
    pub round145_profile_max_q_bits: usize,
    pub live_dx_first_step_q_bits: usize,
    pub missing_q_bits: usize,
    pub universal_slot_component_qubits: usize,
    pub universal_slot_component_toffoli: usize,
    pub universal_reuse_qubits: usize,
    pub universal_noreuse_qubits: usize,
    pub universal_toffoli: usize,
    pub universal_reuse_qt: usize,
    pub universal_noreuse_qt: usize,
    pub universal_reuse_qubit_slack: isize,
    pub universal_toffoli_slack: isize,
    pub universal_reuse_qt_slack: isize,
    pub universal_noreuse_qubit_slack: isize,
    pub universal_noreuse_qt_slack: isize,
    pub universal_delta_qubits_vs_profile: isize,
    pub universal_delta_toffoli_vs_profile: isize,
    pub universal_delta_qt_vs_profile: isize,
    pub universal_toffoli_over_profile_margin: isize,
    pub profiled_row_makes_first_milestone: bool,
    pub universal_row_makes_first_milestone: bool,
    pub next_required_object: &'static str,
}

pub fn round162_halfgcd_live_pa_resource_gate() -> Round162HalfgcdResourceGate {
    let state_a_checkpoint_tail_toffoli =
        ROUND200_FULL_GCD_PAIR1_PA_TOFFOLI - ROUND200_SEMANTIC_FULL_GCD_PAIR1_CHECKPOINT_TOFFOLI;
    let tail_toffoli_budget_to_3m = GOOGLE_MILESTONE_T_TARGET
        .saturating_sub(ROUND200_SEMANTIC_FULL_GCD_PAIR1_CHECKPOINT_TOFFOLI);
    let product_toffoli_limit_at_full_pa_q =
        (GOOGLE_MILESTONE_QT_TARGET - 1) / ROUND200_FULL_GCD_PAIR1_PA_QUBITS;

    Round162HalfgcdResourceGate {
        classification: ROUND162_HALFGCD_RESOURCE_GATE_CLASSIFICATION,
        prefix_qubits: ROUND199_SEMANTIC_FULL_GCD_PREFIX_QUBITS,
        prefix_toffoli: ROUND199_SEMANTIC_FULL_GCD_PREFIX_TOFFOLI,
        state_a_checkpoint_qubits: ROUND200_SEMANTIC_FULL_GCD_PAIR1_CHECKPOINT_QUBITS,
        state_a_checkpoint_toffoli: ROUND200_SEMANTIC_FULL_GCD_PAIR1_CHECKPOINT_TOFFOLI,
        full_pa_qubits: ROUND200_FULL_GCD_PAIR1_PA_QUBITS,
        full_pa_toffoli: ROUND200_FULL_GCD_PAIR1_PA_TOFFOLI,
        full_pa_qt: ROUND200_FULL_GCD_PAIR1_PA_QT,
        full_pa_qubit_slack: GOOGLE_MILESTONE_Q_TARGET as isize
            - ROUND200_FULL_GCD_PAIR1_PA_QUBITS as isize,
        full_pa_toffoli_slack: GOOGLE_MILESTONE_T_TARGET as isize
            - ROUND200_FULL_GCD_PAIR1_PA_TOFFOLI as isize,
        full_pa_qt_slack: GOOGLE_MILESTONE_QT_TARGET as isize
            - ROUND200_FULL_GCD_PAIR1_PA_QT as isize,
        state_a_checkpoint_tail_toffoli,
        tail_toffoli_budget_to_3m,
        tail_toffoli_over_3m_budget: state_a_checkpoint_tail_toffoli as isize
            - tail_toffoli_budget_to_3m as isize,
        product_toffoli_limit_at_full_pa_q,
        full_pa_toffoli_over_product_limit: ROUND200_FULL_GCD_PAIR1_PA_TOFFOLI as isize
            - product_toffoli_limit_at_full_pa_q as isize,
        lowq_cmod_qubits: ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_QUBITS,
        lowq_cmod_toffoli: ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_TOFFOLI,
        lowq_cmod_qt: ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_QT,
        lowq_cmod_delta_qubits: ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_QUBITS as isize
            - ROUND200_FULL_GCD_PAIR1_PA_QUBITS as isize,
        lowq_cmod_delta_toffoli: ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_TOFFOLI as isize
            - ROUND200_FULL_GCD_PAIR1_PA_TOFFOLI as isize,
        lowq_cmod_delta_qt: ROUND200_FULL_GCD_PAIR1_LOWQ_CMOD_PA_QT as isize
            - ROUND200_FULL_GCD_PAIR1_PA_QT as isize,
        makes_first_milestone: ROUND200_FULL_GCD_PAIR1_PA_QT < GOOGLE_MILESTONE_QT_TARGET,
        next_required_object:
            "delete the 257-bit full-GCD coefficient lane or replace the state_b cleanup wall",
    }
}

pub fn round256_halfgcd_round145_first_slot_gate() -> Round256HalfgcdRound145FirstSlotGate {
    let round145_profile_qubit_slack =
        GOOGLE_MILESTONE_Q_TARGET as isize - ROUND145_SHIFTED_SLICE_PROFILE_QUBITS as isize;
    let round145_profile_toffoli_slack =
        GOOGLE_MILESTONE_T_TARGET as isize - ROUND145_SHIFTED_SLICE_PROFILE_TOFFOLI as isize;
    let round145_profile_qt_slack =
        GOOGLE_MILESTONE_QT_TARGET as isize - ROUND145_SHIFTED_SLICE_PROFILE_QT as isize;
    let universal_reuse_qubit_slack =
        GOOGLE_MILESTONE_Q_TARGET as isize - ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QUBITS as isize;
    let universal_toffoli_slack =
        GOOGLE_MILESTONE_T_TARGET as isize - ROUND179_UNIVERSAL_FIRST_SLOT_TOFFOLI as isize;
    let universal_reuse_qt_slack =
        GOOGLE_MILESTONE_QT_TARGET as isize - ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QT as isize;
    let universal_noreuse_qubit_slack =
        GOOGLE_MILESTONE_Q_TARGET as isize - ROUND179_UNIVERSAL_FIRST_SLOT_NOREUSE_QUBITS as isize;
    let universal_noreuse_qt_slack =
        GOOGLE_MILESTONE_QT_TARGET as isize - ROUND179_UNIVERSAL_FIRST_SLOT_NOREUSE_QT as isize;
    let universal_delta_qubits_vs_profile = ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QUBITS as isize
        - ROUND145_SHIFTED_SLICE_PROFILE_QUBITS as isize;
    let universal_delta_toffoli_vs_profile = ROUND179_UNIVERSAL_FIRST_SLOT_TOFFOLI as isize
        - ROUND145_SHIFTED_SLICE_PROFILE_TOFFOLI as isize;
    let universal_delta_qt_vs_profile = ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QT as isize
        - ROUND145_SHIFTED_SLICE_PROFILE_QT as isize;

    Round256HalfgcdRound145FirstSlotGate {
        classification: ROUND256_HALFGCD_ROUND145_FIRST_SLOT_CLASSIFICATION,
        round145_profile_qubits: ROUND145_SHIFTED_SLICE_PROFILE_QUBITS,
        round145_profile_toffoli: ROUND145_SHIFTED_SLICE_PROFILE_TOFFOLI,
        round145_profile_qt: ROUND145_SHIFTED_SLICE_PROFILE_QT,
        round145_profile_qubit_slack,
        round145_profile_toffoli_slack,
        round145_profile_qt_slack,
        round145_profile_max_q_bits: ROUND145_PROFILE_MAX_Q_BITS,
        live_dx_first_step_q_bits: FIRST_STEP_LIVE_DX_MAX_Q_BITS,
        missing_q_bits: FIRST_STEP_LIVE_DX_MAX_Q_BITS - ROUND145_PROFILE_MAX_Q_BITS,
        universal_slot_component_qubits: ROUND179_UNIVERSAL_FIRST_SLOT_COMPONENT_QUBITS,
        universal_slot_component_toffoli: ROUND179_UNIVERSAL_FIRST_SLOT_COMPONENT_TOFFOLI,
        universal_reuse_qubits: ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QUBITS,
        universal_noreuse_qubits: ROUND179_UNIVERSAL_FIRST_SLOT_NOREUSE_QUBITS,
        universal_toffoli: ROUND179_UNIVERSAL_FIRST_SLOT_TOFFOLI,
        universal_reuse_qt: ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QT,
        universal_noreuse_qt: ROUND179_UNIVERSAL_FIRST_SLOT_NOREUSE_QT,
        universal_reuse_qubit_slack,
        universal_toffoli_slack,
        universal_reuse_qt_slack,
        universal_noreuse_qubit_slack,
        universal_noreuse_qt_slack,
        universal_delta_qubits_vs_profile,
        universal_delta_toffoli_vs_profile,
        universal_delta_qt_vs_profile,
        universal_toffoli_over_profile_margin: universal_delta_toffoli_vs_profile
            - round145_profile_toffoli_slack,
        profiled_row_makes_first_milestone: ROUND145_SHIFTED_SLICE_PROFILE_QT
            < GOOGLE_MILESTONE_QT_TARGET,
        universal_row_makes_first_milestone: ROUND179_UNIVERSAL_FIRST_SLOT_REUSE_QT
            < GOOGLE_MILESTONE_QT_TARGET,
        next_required_object:
            "a total prefix/tail schedule that handles q0_bits=256 without the universal slot row",
    }
}

pub(super) fn round162_halfgcd_live_pa_enabled() -> bool {
    std::env::var(ROUND162_HALFGCD_LIVE_PA_ENV).ok().as_deref() == Some("1")
}

pub(super) fn emit_round162_halfgcd_live_pa_or_fail(
    b: &mut B,
    tx: &[QubitId],
    ty: &[QubitId],
    ox: &[BitId],
    oy: &[BitId],
    p: U256,
) -> ! {
    b.set_phase("round162_halfgcd_live_pa_fail_closed");
    panic!(
        "{}",
        round162_halfgcd_live_pa_missing_wire_invariant(tx, ty, ox, oy, p)
    );
}

fn u256_bit_len(x: U256) -> usize {
    if x.is_zero() {
        0
    } else {
        256 - x.leading_zeros() as usize
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
    panic!("round162 no secp x-coordinate pair with dx=1 below {limit}");
}

pub(super) fn round162_halfgcd_live_pa_missing_wire_invariant(
    tx: &[QubitId],
    ty: &[QubitId],
    ox: &[BitId],
    oy: &[BitId],
    p: U256,
) -> String {
    assert_eq!(tx.len(), N, "round162 tx must be 256 qubits");
    assert_eq!(ty.len(), N, "round162 ty must be 256 qubits");
    assert_eq!(ox.len(), N, "round162 ox must be 256 classical bits");
    assert_eq!(oy.len(), N, "round162 oy must be 256 classical bits");

    let (x0, y0, x1, y1) = first_secp_x_pair_with_delta_one(512);
    let dx = x1 - x0;
    let q0 = p / dx;
    let q0_bits = u256_bit_len(q0).max(1);
    assert_eq!(dx, U256::from(1u64));
    assert_eq!(q0_bits, FIRST_STEP_LIVE_DX_MAX_Q_BITS);

    let mut msg = String::new();
    let _ = write!(
        msg,
        "{env}=1 fail-closed before KMX emission: missing wire/state invariant \
         round162.total_prefix_tail_cleanup. ",
        env = ROUND162_HALFGCD_LIVE_PA_ENV
    );
    let _ = write!(
        msg,
        "At the splice point the Google ABI wires are tx[{}]=dx quantum, \
         ty[{}]=dy quantum, ox[{}]/oy[{}]=public addend bits. ",
        tx.len(),
        ty.len(),
        ox.len(),
        oy.len()
    );
    let _ = write!(
        msg,
        "There is a valid secp256k1 PA input with x0={}, y0={}, x1={}, y1={}, \
         so dx=x1-x0=1 and the first half-GCD quotient is floor(p/dx), requiring \
         q0_bits={}. ",
        x0, y0, x1, y1, q0_bits
    );
    let _ = write!(
        msg,
        "The Round145 observed profile decoder has max q_bits<=26, while a \
         total Google-ABI PA operator must also handle this valid first step \
         with q_bits={}.  Round202's overflow-aware direct-slice decoder and \
         initial coefficient cleanup can make a single full-width quotient step \
         reversible, but only by charging a full-width q slot and keeping the \
         resulting prefix state live. ",
        q0_bits
    );
    let _ = write!(
        msg,
        "The missing object is now the total schedule that consumes dy, emits \
         tx=Rx and ty=Ry, and erases u, v, coeff_b, coeff_d, and q_tail back to \
         the four Google ABI registers.  Emitting the old splice would still \
         return with non-ABI scratch live. Targets are Q<{}, T<{}.",
        GOOGLE_RELAXED_Q_TARGET, GOOGLE_RELAXED_T_TARGET
    );
    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round162_live_pa_route_reports_exact_missing_wire_invariant() {
        let mut b = B::new();
        let tx = b.alloc_qubits(N);
        let ty = b.alloc_qubits(N);
        let ox = b.alloc_bits(N);
        let oy = b.alloc_bits(N);

        let msg = round162_halfgcd_live_pa_missing_wire_invariant(&tx, &ty, &ox, &oy, SECP256K1_P);

        println!("METRIC round162_halfgcd_live_pa_fail_closed=1");
        println!(
            "METRIC round162_halfgcd_live_pa_missing_wire_invariant=total_prefix_tail_cleanup"
        );
        println!("METRIC round162_halfgcd_live_pa_msg_len={}", msg.len());

        assert!(msg.contains(ROUND162_HALFGCD_LIVE_PA_ENV));
        assert!(msg.contains("round162.total_prefix_tail_cleanup"));
        assert!(msg.contains("Round202"));
        assert!(msg.contains("q0_bits=256"));
        assert!(msg.contains("tx=Rx"));
        assert!(msg.contains("ty=Ry"));
        assert!(!msg.contains("component"));
        assert!(!msg.contains("certificate"));
    }

    #[test]
    fn round162_full_gcd_state_a_replacement_resource_gate_is_dead() {
        let report = round162_halfgcd_live_pa_resource_gate();

        println!(
            "METRIC round162_halfgcd_resource_classification={}",
            report.classification
        );
        println!("METRIC round162_halfgcd_prefix_q={}", report.prefix_qubits);
        println!("METRIC round162_halfgcd_prefix_t={}", report.prefix_toffoli);
        println!(
            "METRIC round162_halfgcd_state_a_checkpoint_q={}",
            report.state_a_checkpoint_qubits
        );
        println!(
            "METRIC round162_halfgcd_state_a_checkpoint_t={}",
            report.state_a_checkpoint_toffoli
        );
        println!(
            "METRIC round162_halfgcd_full_pa_q={}",
            report.full_pa_qubits
        );
        println!(
            "METRIC round162_halfgcd_full_pa_t={}",
            report.full_pa_toffoli
        );
        println!("METRIC round162_halfgcd_full_pa_qt={}", report.full_pa_qt);
        println!(
            "METRIC round162_halfgcd_full_pa_q_slack={}",
            report.full_pa_qubit_slack
        );
        println!(
            "METRIC round162_halfgcd_full_pa_t_slack={}",
            report.full_pa_toffoli_slack
        );
        println!(
            "METRIC round162_halfgcd_full_pa_qt_slack={}",
            report.full_pa_qt_slack
        );
        println!(
            "METRIC round162_halfgcd_tail_t_after_checkpoint={}",
            report.state_a_checkpoint_tail_toffoli
        );
        println!(
            "METRIC round162_halfgcd_tail_t_budget_to_3m={}",
            report.tail_toffoli_budget_to_3m
        );
        println!(
            "METRIC round162_halfgcd_tail_t_over_3m_budget={}",
            report.tail_toffoli_over_3m_budget
        );
        println!(
            "METRIC round162_halfgcd_product_t_limit_at_q={}",
            report.product_toffoli_limit_at_full_pa_q
        );
        println!(
            "METRIC round162_halfgcd_t_over_product_limit={}",
            report.full_pa_toffoli_over_product_limit
        );
        println!(
            "METRIC round162_halfgcd_lowq_cmod_q={}",
            report.lowq_cmod_qubits
        );
        println!(
            "METRIC round162_halfgcd_lowq_cmod_t={}",
            report.lowq_cmod_toffoli
        );
        println!(
            "METRIC round162_halfgcd_lowq_cmod_qt={}",
            report.lowq_cmod_qt
        );

        assert_eq!(
            report.classification,
            ROUND162_HALFGCD_RESOURCE_GATE_CLASSIFICATION
        );
        assert_eq!(report.prefix_qubits, 1_627);
        assert_eq!(report.prefix_toffoli, 875_788);
        assert_eq!(report.state_a_checkpoint_qubits, 2_654);
        assert_eq!(report.state_a_checkpoint_toffoli, 2_564_547);
        assert_eq!(report.full_pa_qubits, 2_654);
        assert_eq!(report.full_pa_toffoli, 4_612_455);
        assert_eq!(report.full_pa_qt, 12_241_455_570);
        assert_eq!(report.full_pa_qubit_slack, -654);
        assert_eq!(report.full_pa_toffoli_slack, -1_612_455);
        assert_eq!(report.full_pa_qt_slack, -3_241_455_570);
        assert_eq!(report.state_a_checkpoint_tail_toffoli, 2_047_908);
        assert_eq!(report.tail_toffoli_budget_to_3m, 435_453);
        assert_eq!(report.tail_toffoli_over_3m_budget, 1_612_455);
        assert_eq!(report.product_toffoli_limit_at_full_pa_q, 3_391_107);
        assert_eq!(report.full_pa_toffoli_over_product_limit, 1_221_348);
        assert_eq!(report.lowq_cmod_qubits, 2_476);
        assert_eq!(report.lowq_cmod_toffoli, 5_142_140);
        assert_eq!(report.lowq_cmod_qt, 12_731_938_640);
        assert_eq!(report.lowq_cmod_delta_qubits, -178);
        assert_eq!(report.lowq_cmod_delta_toffoli, 529_685);
        assert_eq!(report.lowq_cmod_delta_qt, 490_483_070);
        assert!(!report.makes_first_milestone);
    }

    #[test]
    fn round256_round145_universal_first_slot_gate_is_dead() {
        let report = round256_halfgcd_round145_first_slot_gate();

        println!(
            "METRIC round256_halfgcd_round145_first_slot_classification={}",
            report.classification
        );
        println!(
            "METRIC round256_round145_profile_q={}",
            report.round145_profile_qubits
        );
        println!(
            "METRIC round256_round145_profile_t={}",
            report.round145_profile_toffoli
        );
        println!(
            "METRIC round256_round145_profile_qt={}",
            report.round145_profile_qt
        );
        println!(
            "METRIC round256_round145_profile_t_slack={}",
            report.round145_profile_toffoli_slack
        );
        println!(
            "METRIC round256_round145_profile_max_q_bits={}",
            report.round145_profile_max_q_bits
        );
        println!(
            "METRIC round256_live_dx_first_step_q_bits={}",
            report.live_dx_first_step_q_bits
        );
        println!("METRIC round256_missing_q_bits={}", report.missing_q_bits);
        println!(
            "METRIC round256_universal_slot_component_q={}",
            report.universal_slot_component_qubits
        );
        println!(
            "METRIC round256_universal_slot_component_t={}",
            report.universal_slot_component_toffoli
        );
        println!(
            "METRIC round256_universal_reuse_q={}",
            report.universal_reuse_qubits
        );
        println!("METRIC round256_universal_t={}", report.universal_toffoli);
        println!(
            "METRIC round256_universal_reuse_qt={}",
            report.universal_reuse_qt
        );
        println!(
            "METRIC round256_universal_t_over_profile_margin={}",
            report.universal_toffoli_over_profile_margin
        );

        assert_eq!(
            report.classification,
            ROUND256_HALFGCD_ROUND145_FIRST_SLOT_CLASSIFICATION
        );
        assert_eq!(report.round145_profile_qubits, 1_539);
        assert_eq!(report.round145_profile_toffoli, 2_974_404);
        assert_eq!(report.round145_profile_qt, 4_577_607_756);
        assert_eq!(report.round145_profile_qubit_slack, 461);
        assert_eq!(report.round145_profile_toffoli_slack, 25_596);
        assert_eq!(report.round145_profile_qt_slack, 4_422_392_244);
        assert_eq!(report.round145_profile_max_q_bits, 26);
        assert_eq!(report.live_dx_first_step_q_bits, 256);
        assert_eq!(report.missing_q_bits, 230);
        assert_eq!(report.universal_slot_component_qubits, 1_283);
        assert_eq!(report.universal_slot_component_toffoli, 458_241);
        assert_eq!(report.universal_reuse_qubits, 2_307);
        assert_eq!(report.universal_noreuse_qubits, 2_822);
        assert_eq!(report.universal_toffoli, 4_524_688);
        assert_eq!(report.universal_reuse_qt, 10_438_455_216);
        assert_eq!(report.universal_noreuse_qt, 12_768_669_536);
        assert_eq!(report.universal_reuse_qubit_slack, -307);
        assert_eq!(report.universal_toffoli_slack, -1_524_688);
        assert_eq!(report.universal_reuse_qt_slack, -1_438_455_216);
        assert_eq!(report.universal_noreuse_qubit_slack, -822);
        assert_eq!(report.universal_noreuse_qt_slack, -3_768_669_536);
        assert_eq!(report.universal_delta_qubits_vs_profile, 768);
        assert_eq!(report.universal_delta_toffoli_vs_profile, 1_550_284);
        assert_eq!(report.universal_delta_qt_vs_profile, 5_860_847_460);
        assert_eq!(report.universal_toffoli_over_profile_margin, 1_524_688);
        assert!(report.profiled_row_makes_first_milestone);
        assert!(!report.universal_row_makes_first_milestone);
    }
}
