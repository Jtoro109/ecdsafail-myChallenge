//! Source-live D1 quotient/product transducer audit.
//!
//! This module is deliberately small and production-facing: it pins the exact
//! reversible surface that would unlock the Round100 PA rows, then makes the
//! currently-known cleanup obstruction executable in Rust.  The target
//! primitive is still one object:
//!
//!   forward:  (h, n, 0) -> (h, n / h, 0)
//!   backward: (h, n, 0) -> (h, h * n, 0)
//!
//! on the canonical nonzero field surface, with a fixed totalization outside
//! that surface.  The product direction is the quotient direction run backward.

use alloy_primitives::U256;
use std::collections::BTreeSet;

use crate::circuit::QubitId;

use super::{B, N, SECP256K1_P};

pub const LOCAL_PA_Q_TARGET: usize = 2_000;
pub const LOCAL_PA_T_TARGET: usize = 3_000_000;
pub const LOCAL_PA_QT_TARGET: usize = 9_000_000_000;

pub const ROUND100_CONDITIONAL_PA_Q: usize = 1_309;
pub const ROUND100_CONDITIONAL_PA_T: usize = 2_057_649;
pub const ROUND100_EXACT_EXCEPTION_PA_Q: usize = 1_315;
pub const ROUND100_EXACT_EXCEPTION_PA_T: usize = 2_068_377;

pub const D1_DIVISION_T_TARGET: usize = 1_607_479;
pub const ROUND101_SOURCE_LIVE_D1_ONE_WAY_TOFFOLI_TARGET: usize = 958_336;
pub const ROUND413_SOURCE_LIVE_D1_PA_QT: usize =
    ROUND100_EXACT_EXCEPTION_PA_Q * ROUND100_EXACT_EXCEPTION_PA_T;
pub const ROUND414_SOURCE_LIVE_D1_PRODUCTION_LOWERER_ENV: &str =
    "ROUND414_SOURCE_LIVE_D1_PRODUCTION_LOWERER";
pub const ROUND414_SOURCE_LIVE_D1_BACKEND_CONTRACT_CLASSIFICATION: &str =
    "ROUND414_SOURCE_LIVE_D1_BACKEND_CONTRACT_FAILS_CLOSED";
pub const ROUND414_SOURCE_LIVE_D1_MISSING_OBJECT: &str =
    "complete source-live D1 quotient/product PA KMX with exact stats and 9024 fuzz";
pub const ROUND414_SOURCE_LIVE_D1_BACKEND_REQUIREMENTS: [&str; 7] = [
    "complete_google_abi_pa",
    "source_parents_live_until_cleanup",
    "quotient_product_workspace_zero_exit",
    "exact_exception_totalized",
    "no_qtail_round217_alias",
    "same_artifact_stats",
    "deterministic_9024_google_pa_fuzz",
];
pub const ROUND415_D1_INPLACE_LOWERER_AUDIT_CLASSIFICATION: &str =
    "ROUND415_EXISTING_D1_INPLACE_LOWERER_REJECTED_FOR_SOURCE_LIVE_D1";
pub const ROUND415_D1_INPLACE_PRODUCT_QUBITS: usize = 2_475;
pub const ROUND415_D1_INPLACE_PRODUCT_TOFFOLI: usize = 1_919_786;
pub const ROUND415_D1_INPLACE_PRODUCT_OPS: usize = 14_234_801;
pub const ROUND415_D1_INPLACE_PRODUCT_BITS: usize = 1_141_762;
pub const ROUND415_D1_INPLACE_PRODUCT_PHASE_ROWS: usize = 6_055;
pub const ROUND415_D1_INPLACE_PRODUCT_HMR_OPS: usize = 1_141_762;
pub const ROUND415_D1_INPLACE_PRODUCT_R_OPS: usize = 1_356_913;
pub const ROUND415_D1_INPLACE_QUOTIENT_QUBITS: usize = 2_475;
pub const ROUND415_D1_INPLACE_QUOTIENT_TOFFOLI: usize = 1_919_786;
pub const ROUND415_D1_INPLACE_QUOTIENT_OPS: usize = 10_594_364;
pub const ROUND415_D1_INPLACE_QUOTIENT_BITS: usize = 0;
pub const ROUND415_D1_INPLACE_QUOTIENT_PHASE_ROWS: usize = 2;
pub const ROUND415_D1_INPLACE_PROJECTED_PA_QUBITS: usize = 2_475;
pub const ROUND415_D1_INPLACE_PROJECTED_PA_TOFFOLI: usize = 3_029_827;
pub const ROUND415_D1_INPLACE_PROJECTED_PA_QT: usize = 7_498_821_825;

pub const ROUND217_SOURCE_LIVE_TRANSPORT_BLOCK_BITS: usize = 5;
pub const ROUND217_SOURCE_LIVE_TRANSPORT_HIDDEN_QUBITS: usize = 241;
pub const ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX: usize = 1_023_289;
pub const ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_P99: usize = 1_004_134;
pub const ROUND217_SOURCE_LIVE_TRANSPORT_PARSER_TOFFOLI: usize = 77_290;
pub const ROUND217_SOURCE_LIVE_TRANSPORT_MAX_APPLICATION_TOFFOLI: usize = 945_999;
pub const ROUND217_SOURCE_LIVE_TRANSPORT_SAMPLES: usize = 9_024;
pub const ROUND217_SOURCE_LIVE_TRANSPORT_CLASSIFICATION: &str =
    "ROUND217_SOURCE_LIVE_TRANSPORT_LOWERER_FITS_401Q_1050KT";

pub const ROUND210_DIRECT_QUOTIENT_PA_QUBITS: usize = 2_737;
pub const ROUND210_DIRECT_QUOTIENT_PA_TOFFOLI: usize = 3_969_406;
pub const ROUND210_DIRECT_QUOTIENT_PA_OPS: usize = 29_285_631;
pub const ROUND210_PAIR1_DIRECT_QUOTIENT_TOFFOLI: usize = 1_920_735;
pub const ROUND210_PAIR2_D1_PRODUCT_TOFFOLI: usize = 1_891_898;
pub const ROUND210_OLD_D1_CORE_TOFFOLI: usize =
    ROUND210_PAIR1_DIRECT_QUOTIENT_TOFFOLI + ROUND210_PAIR2_D1_PRODUCT_TOFFOLI;
pub const ROUND218_B5_SOURCE_LIVE_REPLACEMENT_TOFFOLI: usize =
    2 * ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX;
pub const ROUND218_B5_SOURCE_LIVE_PA_TOFFOLI: usize = ROUND210_DIRECT_QUOTIENT_PA_TOFFOLI
    - ROUND210_OLD_D1_CORE_TOFFOLI
    + ROUND218_B5_SOURCE_LIVE_REPLACEMENT_TOFFOLI;
pub const ROUND218_B5_SOURCE_LIVE_PA_OUTSIDE_D1_QUBITS: usize = 1_562;
pub const ROUND218_B5_SOURCE_LIVE_PA_REPLACEMENT_QUBITS: usize =
    2 * 256 + ROUND217_SOURCE_LIVE_TRANSPORT_HIDDEN_QUBITS;
pub const ROUND218_B5_SOURCE_LIVE_PA_QUBITS: usize = if ROUND218_B5_SOURCE_LIVE_PA_OUTSIDE_D1_QUBITS
    > ROUND218_B5_SOURCE_LIVE_PA_REPLACEMENT_QUBITS
{
    ROUND218_B5_SOURCE_LIVE_PA_OUTSIDE_D1_QUBITS
} else {
    ROUND218_B5_SOURCE_LIVE_PA_REPLACEMENT_QUBITS
};
pub const ROUND218_B5_SOURCE_LIVE_PA_QT: usize =
    ROUND218_B5_SOURCE_LIVE_PA_QUBITS * ROUND218_B5_SOURCE_LIVE_PA_TOFFOLI;
pub const ROUND218_B5_SOURCE_LIVE_PA_CLASSIFICATION: &str =
    "ROUND218_B5_SOURCE_LIVE_TRANSPORT_PA_ASSEMBLED_RESOURCE_ROW";
pub const ROUND218_B5_COMPACT_HISTORY_RESOURCE_GATE_ENV: &str =
    "ROUND218_B5_COMPACT_HISTORY_RESOURCE_GATE";
pub const ROUND218_B5_COMPACT_HISTORY_CLASSIFICATION: &str =
    "ROUND218_B5_COMPACT_HISTORY_RESOURCE_DEAD";
pub const ROUND218_B5_HISTORY_STREAM_PA_QUBITS: usize = 3_912;
pub const ROUND218_B5_HISTORY_STREAM_PA_TOFFOLI: usize = 10_628_797;
pub const ROUND218_B5_RAW_CONTROL_HISTORY_BITS: usize = 2 * 590;
pub const ROUND218_B5_SEPARATOR_LOWER_BOUND_BITS_PER_BLOCK: usize = 7;
pub const ROUND218_B5_ZETA_OLD_SEPARATOR_BITS_PER_BLOCK: usize = 8;
pub const ROUND218_B5_BLOCK_COUNT: usize = 118;
pub const ROUND218_B5_SEPARATOR_LOWER_BOUND_HISTORY_BITS: usize =
    ROUND218_B5_SEPARATOR_LOWER_BOUND_BITS_PER_BLOCK * ROUND218_B5_BLOCK_COUNT;
pub const ROUND218_B5_ZETA_OLD_SEPARATOR_HISTORY_BITS: usize =
    ROUND218_B5_ZETA_OLD_SEPARATOR_BITS_PER_BLOCK * ROUND218_B5_BLOCK_COUNT;
pub const ROUND218_B5_OPTIMAL_SEPARATOR_PA_QUBITS: usize = ROUND218_B5_HISTORY_STREAM_PA_QUBITS
    - (ROUND218_B5_RAW_CONTROL_HISTORY_BITS - ROUND218_B5_SEPARATOR_LOWER_BOUND_HISTORY_BITS);
pub const ROUND218_B5_ZETA_OLD_SEPARATOR_PA_QUBITS: usize = ROUND218_B5_HISTORY_STREAM_PA_QUBITS
    - (ROUND218_B5_RAW_CONTROL_HISTORY_BITS - ROUND218_B5_ZETA_OLD_SEPARATOR_HISTORY_BITS);
pub const ROUND251_QTAIL_SECOND_INVERSE_BUDGET_GATE_ENV: &str =
    "ROUND251_QTAIL_SECOND_INVERSE_BUDGET_GATE";
pub const ROUND251_QTAIL_SECOND_INVERSE_CLASSIFICATION: &str =
    "ROUND251_QTAIL_SECOND_INVERSE_ARITHMETIC_DEAD";
pub const ROUND251_LOWQ_D1_PA_QUBITS: usize = 2_454;
pub const ROUND251_LOWQ_D1_PA_TOFFOLI: usize = 4_025_998;
pub const ROUND251_LOWQ_D1_PA_QT: usize = ROUND251_LOWQ_D1_PA_QUBITS * ROUND251_LOWQ_D1_PA_TOFFOLI;
pub const ROUND251_PAIR2_D1_PRODUCT_TOFFOLI: usize = 1_919_786;
pub const ROUND251_D1_CLEANUP_ARITHMETIC_SAVING_CEILING: usize = 252_145;
pub const ROUND251_D1_CLEANUP_ARITHMETIC_FLOOR_TOFFOLI: usize =
    ROUND251_PAIR2_D1_PRODUCT_TOFFOLI - ROUND251_D1_CLEANUP_ARITHMETIC_SAVING_CEILING;
pub const ROUND257_SOURCE_LIVE_CUBIC_PHASE_BUDGET_GATE_ENV: &str =
    "ROUND257_SOURCE_LIVE_CUBIC_PHASE_BUDGET_GATE";
pub const ROUND258_SOURCE_LIVE_PRODUCT_REPLACEMENT_BUDGET_GATE_ENV: &str =
    "ROUND258_SOURCE_LIVE_PRODUCT_REPLACEMENT_BUDGET_GATE";
pub const ROUND259_SOURCE_LIVE_HMR_OVERWRITE_GATE_ENV: &str =
    "ROUND259_SOURCE_LIVE_HMR_OVERWRITE_GATE";
pub const ROUND398_QTAIL_ROUND217_PRODUCT_BUDGET_GATE_ENV: &str =
    "ROUND398_QTAIL_ROUND217_PRODUCT_BUDGET";
pub const ROUND257_SOURCE_LIVE_CUBIC_PHASE_CLASSIFICATION: &str =
    "ROUND257_SOURCE_LIVE_CUBIC_PHASE_REPAIR_BUDGET_DEAD";
pub const ROUND258_SOURCE_LIVE_PRODUCT_REPLACEMENT_CLASSIFICATION: &str =
    "ROUND258_SOURCE_LIVE_PRODUCT_REPLACEMENT_BUDGET_PINNED";
pub const ROUND259_SOURCE_LIVE_HMR_OVERWRITE_CLASSIFICATION: &str =
    "ROUND259_SOURCE_LIVE_HMR_OVERWRITE_PHASE_BLOCKED";
pub const ROUND398_QTAIL_ROUND217_PRODUCT_BUDGET_CLASSIFICATION: &str =
    "ROUND398_QTAIL_ROUND217_PRODUCT_SPLICE_BUDGET_PINNED";
pub const ROUND257_DIRTY_CUBIC_QUBITS: usize = 2_713;
pub const ROUND257_DIRTY_CUBIC_TOFFOLI: usize = 3_203_496;
pub const ROUND257_DIRTY_CUBIC_QT: usize =
    ROUND257_DIRTY_CUBIC_QUBITS * ROUND257_DIRTY_CUBIC_TOFFOLI;
pub const ROUND257_CLEAN_PRODUCT_TAIL_QUBITS: usize = 2_713;
pub const ROUND257_CLEAN_PRODUCT_TAIL_TOFFOLI: usize = 4_662_307;
pub const ROUND257_CLEAN_PRODUCT_TAIL_QT: usize =
    ROUND257_CLEAN_PRODUCT_TAIL_QUBITS * ROUND257_CLEAN_PRODUCT_TAIL_TOFFOLI;
pub const ROUND257_CLEAN_LAMBDA_TAIL_QUBITS: usize = 2_713;
pub const ROUND257_CLEAN_LAMBDA_TAIL_TOFFOLI: usize = 5_016_865;
pub const ROUND257_CLEAN_LAMBDA_TAIL_QT: usize =
    ROUND257_CLEAN_LAMBDA_TAIL_QUBITS * ROUND257_CLEAN_LAMBDA_TAIL_TOFFOLI;
pub const ROUND258_CURRENT_INPLACE_PRODUCT_TOFFOLI: usize = ROUND251_PAIR2_D1_PRODUCT_TOFFOLI;
pub const ROUND258_CLEAN_PRODUCT_NON_PRODUCT_TOFFOLI: usize =
    ROUND257_CLEAN_PRODUCT_TAIL_TOFFOLI - ROUND258_CURRENT_INPLACE_PRODUCT_TOFFOLI;
pub const ROUND259_HMR_OVERWRITE_QUBITS: usize = 2_713;
pub const ROUND259_HMR_OVERWRITE_TOFFOLI: usize = 2_892_410;
pub const ROUND259_HMR_OVERWRITE_QT: usize =
    ROUND259_HMR_OVERWRITE_QUBITS * ROUND259_HMR_OVERWRITE_TOFFOLI;
pub const ROUND259_HMR_OVERWRITE_MEASURED_LAM_BITS: usize = 256;
pub const ROUND259_DIRECT_CASE_PHASE: usize = 1;
pub const ROUND259_TOY_QUOTIENT_PHASE_DEGREE: usize = 15;
pub const ROUND259_TOY_QUOTIENT_PHASE_DENSITY: usize = 32_518;
pub const ROUND259_TOY_QUOTIENT_PHASE_TABLE: usize = 65_536;
pub const ROUND398_QTAIL_PRODUCT_QUBITS: usize = 2_453;
pub const ROUND398_PRODUCT_M1_QT_TARGET: usize = 9_000_000_000;
pub const ROUND398_PRODUCT_M2_QT_TARGET: usize = 8_000_000_000;
pub const ROUND398_PRODUCT_M3_QT_TARGET: usize = 7_000_000_000;
pub const ROUND398_PRODUCT_M4_QT_TARGET: usize = 6_000_000_000;
pub const ROUND398_PRODUCT_M5_QT_TARGET: usize = 5_000_000_000;
pub const ROUND398_FORBIDDEN_PROBE_QUBITS: usize = 3_912;
pub const ROUND398_FORBIDDEN_PROBE_TOFFOLI: usize = 9_843_898;
pub const ROUND398_FORBIDDEN_PROBE_QT: usize =
    ROUND398_FORBIDDEN_PROBE_QUBITS * ROUND398_FORBIDDEN_PROBE_TOFFOLI;
pub const ROUND398_FORBIDDEN_NAMED_PRODUCT_PHASE_TOFFOLI: usize = 5_824_302;
pub const ROUND398_FORBIDDEN_DOMINANT_INVERSE_TRANSPORT_TOFFOLI: usize = 5_085_562;
pub const ROUND398_FORBIDDEN_ENDPOINT_REPLAY_TOFFOLI: usize = 588_290;
pub const ROUND398_FORBIDDEN_PRODUCT_SCALE_TOFFOLI: usize = 150_450;
pub const ROUND398_FORBIDDEN_FULL_SOURCE_HISTORY_BITS: usize = 1_180;

pub type State = (i128, i128, i128, i128, i128);
pub type Control = (u8, u8);
pub type Vector2 = (i128, i128);
pub type Matrix2 = ((i128, i128), (i128, i128));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaRow {
    pub name: &'static str,
    pub qubits: usize,
    pub toffoli: usize,
}

impl PaRow {
    pub fn qubit_slack(self) -> isize {
        LOCAL_PA_Q_TARGET as isize - self.qubits as isize
    }

    pub fn toffoli_slack(self) -> isize {
        LOCAL_PA_T_TARGET as isize - self.toffoli as isize
    }
}

pub const ROUND100_CONDITIONAL_ROW: PaRow = PaRow {
    name: "round100_conditional_pa_row",
    qubits: ROUND100_CONDITIONAL_PA_Q,
    toffoli: ROUND100_CONDITIONAL_PA_T,
};

pub const ROUND100_EXACT_EXCEPTION_ROW: PaRow = PaRow {
    name: "round100_exact_exception_pa_row",
    qubits: ROUND100_EXACT_EXCEPTION_PA_Q,
    toffoli: ROUND100_EXACT_EXCEPTION_PA_T,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductQuotientEquivalence {
    pub prime: i128,
    pub canonical_rows_checked: usize,
    pub product_is_inverse_quotient: bool,
    pub totalized_fixed_points_checked: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D1ControlCandidate {
    pub label: &'static str,
    pub predecessor: State,
    pub retained_control: Control,
    pub output: State,
    pub determinant_quotient: i128,
    pub implied_h: i128,
    pub gcd_fg: i128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservableSearchReport {
    pub classification: &'static str,
    pub prime: i128,
    pub candidate_count: usize,
    pub distinct_outputs: usize,
    pub distinct_controls: usize,
    pub admissible_probe_count: usize,
    pub separating_admissible_probe_count: usize,
    pub forbidden_separator_count: usize,
    pub combined_admissible_separates: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotientRecord {
    pub raw: Vector2,
    pub canonical: Vector2,
    pub quotient: Vector2,
    pub low_key: Vector2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotientCollision {
    pub classification: &'static str,
    pub prime: i128,
    pub block_bits: usize,
    pub matrix: Matrix2,
    pub first_input: Vector2,
    pub second_input: Vector2,
    pub first: QuotientRecord,
    pub second: QuotientRecord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CleanupRangeFloor {
    pub classification: &'static str,
    pub block_bits: usize,
    pub d1_division_target: usize,
    pub blocks: usize,
    pub csd_slots_per_coordinate_pair: usize,
    pub floor_toffoli: usize,
    pub round82_d1_margin: isize,
    pub over_margin: isize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLiveD1Report {
    pub classification: &'static str,
    pub conditional_row: PaRow,
    pub exact_exception_row: PaRow,
    pub quotient_product_equivalence_pass: bool,
    pub output_observable_obstruction: bool,
    pub lowbit_quotient_obstruction: bool,
    pub range_cleanup_b8: CleanupRangeFloor,
    pub range_cleanup_b9: CleanupRangeFloor,
    pub makes_under_local_pa_target: bool,
    pub required_next_object: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLiveTransportPaAssembly {
    pub classification: &'static str,
    pub materialized_kmx: bool,
    pub block_bits: usize,
    pub round210_qubits: usize,
    pub round210_toffoli: usize,
    pub round210_ops: usize,
    pub state_a_old_toffoli: usize,
    pub state_b_old_toffoli: usize,
    pub replacement_one_way_toffoli: usize,
    pub replacement_total_toffoli: usize,
    pub outside_d1_qubits: usize,
    pub replacement_qubits: usize,
    pub assembled_qubits: usize,
    pub assembled_toffoli: usize,
    pub assembled_qt: usize,
    pub toffoli_delta: isize,
    pub qubit_delta: isize,
    pub splice_points: [&'static str; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Round218B5CompactHistoryGate {
    pub classification: &'static str,
    pub full_history_qubits: usize,
    pub full_history_toffoli: usize,
    pub full_history_qt: usize,
    pub raw_control_history_bits: usize,
    pub separator_lower_bound_bits_per_block: usize,
    pub separator_lower_bound_history_bits: usize,
    pub zeta_old_separator_bits_per_block: usize,
    pub zeta_old_separator_history_bits: usize,
    pub optimal_separator_qubits: usize,
    pub optimal_separator_toffoli: usize,
    pub optimal_separator_qt: usize,
    pub zeta_old_separator_qubits: usize,
    pub zeta_old_separator_toffoli: usize,
    pub zeta_old_separator_qt: usize,
    pub optimal_qubit_saving: usize,
    pub zeta_old_qubit_saving: usize,
    pub optimal_product_toffoli_limit: usize,
    pub zeta_old_product_toffoli_limit: usize,
    pub optimal_toffoli_over_product_limit: isize,
    pub zeta_old_toffoli_over_product_limit: isize,
    pub makes_first_milestone: bool,
    pub next_required_object: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Round251QtailSecondInverseBudgetGate {
    pub classification: &'static str,
    pub lowq_pa_qubits: usize,
    pub lowq_pa_toffoli: usize,
    pub lowq_pa_qt: usize,
    pub current_second_inverse_toffoli: usize,
    pub product_toffoli_limit_at_lowq: usize,
    pub required_toffoli_saving_for_product: usize,
    pub max_second_inverse_replacement_toffoli_for_product: usize,
    pub strict_t_toffoli_limit: usize,
    pub required_toffoli_saving_for_strict_t: usize,
    pub max_second_inverse_replacement_toffoli_for_strict_t: usize,
    pub arithmetic_only_saving_ceiling: usize,
    pub arithmetic_only_second_inverse_floor: usize,
    pub arithmetic_only_pa_toffoli: usize,
    pub arithmetic_only_pa_qt: usize,
    pub arithmetic_only_toffoli_over_product_limit: isize,
    pub arithmetic_only_makes_first_milestone: bool,
    pub next_required_object: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Round257SourceLiveCubicPhaseBudgetGate {
    pub classification: &'static str,
    pub dirty_qubits: usize,
    pub dirty_toffoli: usize,
    pub dirty_qt: usize,
    pub product_toffoli_limit_at_dirty_q: usize,
    pub dirty_product_slack: isize,
    pub dirty_toffoli_over_strict_t: isize,
    pub max_phase_repair_net_toffoli_for_first_milestone: isize,
    pub clean_product_qubits: usize,
    pub clean_product_toffoli: usize,
    pub clean_product_qt: usize,
    pub clean_product_delta_toffoli: isize,
    pub clean_product_toffoli_over_strict_t: isize,
    pub clean_product_toffoli_over_product_limit: isize,
    pub clean_product_qt_slack: isize,
    pub clean_lambda_qubits: usize,
    pub clean_lambda_toffoli: usize,
    pub clean_lambda_qt: usize,
    pub clean_lambda_delta_toffoli: isize,
    pub clean_lambda_toffoli_over_strict_t: isize,
    pub clean_lambda_toffoli_over_product_limit: isize,
    pub clean_lambda_qt_slack: isize,
    pub dirty_row_makes_first_milestone: bool,
    pub clean_product_makes_first_milestone: bool,
    pub clean_lambda_makes_first_milestone: bool,
    pub next_required_object: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Round258SourceLiveProductReplacementBudgetGate {
    pub classification: &'static str,
    pub clean_product_qubits: usize,
    pub clean_product_toffoli: usize,
    pub clean_product_qt: usize,
    pub current_inplace_product_toffoli: usize,
    pub non_product_tail_toffoli: usize,
    pub strict_t_toffoli_limit: usize,
    pub product_toffoli_limit_at_clean_product_q: usize,
    pub max_replacement_toffoli_for_strict_t: usize,
    pub max_replacement_toffoli_for_product: usize,
    pub required_current_product_saving_for_strict_t: usize,
    pub required_current_product_saving_for_product: usize,
    pub ideal_zero_product_toffoli: usize,
    pub ideal_zero_product_qt: usize,
    pub ideal_zero_product_makes_first_milestone: bool,
    pub round217_one_way_transport_toffoli: usize,
    pub round217_replacement_toffoli_over_strict_t_budget: isize,
    pub round217_replacement_toffoli_over_product_budget: isize,
    pub d1_product_toffoli_over_strict_t_budget: isize,
    pub d1_product_toffoli_over_product_budget: isize,
    pub current_clean_product_makes_first_milestone: bool,
    pub next_required_object: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round259SourceLiveHmrOverwriteGate {
    pub classification: &'static str,
    pub hmr_overwrite_qubits: usize,
    pub hmr_overwrite_toffoli: usize,
    pub hmr_overwrite_qt: usize,
    pub strict_t_slack: isize,
    pub qt_slack: isize,
    pub measured_lam_bits: usize,
    pub resource_makes_first_milestone: bool,
    pub direct_case_phase: usize,
    pub phase_clean: bool,
    pub toy_quotient_phase_degree: usize,
    pub toy_quotient_phase_density: usize,
    pub toy_quotient_phase_table: usize,
    pub next_required_object: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round398QtailRound217ProductBudgetGate {
    pub classification: &'static str,
    pub qtail_qubits: usize,
    pub qtail_current_toffoli: usize,
    pub current_product_toffoli: usize,
    pub non_product_toffoli: usize,
    pub m1_pa_toffoli_limit: usize,
    pub m2_pa_toffoli_limit: usize,
    pub m3_pa_toffoli_limit: usize,
    pub m4_pa_toffoli_limit: usize,
    pub m5_pa_toffoli_limit: usize,
    pub m1_replacement_toffoli_limit: isize,
    pub m2_replacement_toffoli_limit: isize,
    pub m3_replacement_toffoli_limit: isize,
    pub m4_replacement_toffoli_limit: isize,
    pub m5_replacement_toffoli_limit: isize,
    pub round217_one_way_toffoli: usize,
    pub round217_projected_pa_toffoli: usize,
    pub round217_projected_pa_qt: usize,
    pub round217_clears_m1: bool,
    pub round217_clears_m2: bool,
    pub round217_clears_m3: bool,
    pub round217_m2_replacement_slack: isize,
    pub round217_m3_replacement_over: isize,
    pub forbidden_probe_qubits: usize,
    pub forbidden_probe_toffoli: usize,
    pub forbidden_probe_qt: usize,
    pub forbidden_named_product_phase_toffoli: usize,
    pub forbidden_dominant_inverse_transport_toffoli: usize,
    pub forbidden_endpoint_replay_toffoli: usize,
    pub forbidden_product_scale_toffoli: usize,
    pub forbidden_full_source_history_bits: usize,
    pub forbidden_named_product_phase_over_m2_replacement_limit: isize,
    pub source_live_product_alias_must_fail_closed: bool,
    pub next_required_object: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLiveD1PhaseBlockContract {
    pub ir_node: &'static str,
    pub phase: &'static str,
    pub field_semantics: &'static str,
    pub register_lifecycle: &'static str,
    pub valid_branch: &'static str,
    pub phase_debt: &'static str,
    pub toffoli_budget: usize,
    pub backend_primitive: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLiveD1BackendContract {
    pub classification: &'static str,
    pub env: &'static str,
    pub target_row: &'static str,
    pub target_qubits: usize,
    pub target_toffoli: usize,
    pub target_qt: usize,
    pub one_way_toffoli_target: usize,
    pub requirements: [&'static str; 7],
    pub missing_object: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLiveD1BodyPlan {
    pub classification: &'static str,
    pub selected_target: &'static str,
    pub target_qubits: usize,
    pub target_toffoli: usize,
    pub target_qt: usize,
    pub one_way_toffoli_target: usize,
    pub phase_blocks: &'static [SourceLiveD1PhaseBlockContract],
    pub body_emits_gates: bool,
    pub codegen_allowed_now: bool,
    pub missing_object: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Round415D1InplaceLowererAudit {
    pub classification: &'static str,
    pub product_qubits: usize,
    pub product_toffoli: usize,
    pub product_ops: usize,
    pub product_bits: usize,
    pub product_phase_rows: usize,
    pub product_hmr_ops: usize,
    pub product_r_ops: usize,
    pub quotient_qubits: usize,
    pub quotient_toffoli: usize,
    pub quotient_ops: usize,
    pub quotient_bits: usize,
    pub quotient_phase_rows: usize,
    pub target_qubits: usize,
    pub target_toffoli: usize,
    pub one_way_toffoli_target: usize,
    pub one_way_toffoli_over_target: isize,
    pub qubits_over_target: isize,
    pub projected_pa_qubits: usize,
    pub projected_pa_toffoli: usize,
    pub projected_pa_qt: usize,
    pub projected_pa_toffoli_over_local_target: isize,
    pub projected_pa_product_m1: bool,
    pub projected_pa_product_m2: bool,
    pub projected_pa_product_slack_m1: isize,
    pub projected_pa_product_slack_m2: isize,
    pub product_has_measurement_debt: bool,
    pub contract_compatible: bool,
}

pub const ROUND414_SOURCE_LIVE_D1_PHASE_BLOCKS: [SourceLiveD1PhaseBlockContract; 6] = [
    SourceLiveD1PhaseBlockContract {
        ir_node: "round414_source_live_d1_route_guard",
        phase: "round414_source_live_d1_quotient_product_lowerer_fail_closed",
        field_semantics: "(h,n) remain borrowed/live; no reversible arithmetic emitted",
        register_lifecycle: "h:borrowed,n:live,workspace:zero,controls:zero",
        valid_branch: "all Google-ABI PA calls entering the Round392 source-live D1 target",
        phase_debt: "none; fail-closed before gates",
        toffoli_budget: 0,
        backend_primitive: "round414_source_live_d1_body_plan",
    },
    SourceLiveD1PhaseBlockContract {
        ir_node: "round414_source_parent_totalization",
        phase: "round414_source_live_d1_parent_totalization",
        field_semantics: "totalize h=0/exact-exception branches before quotient/product evaluation",
        register_lifecycle: "h:borrowed,n:borrowed,branch:borrowed,workspace:zero",
        valid_branch: "complete secp256k1 Google-ABI PA domain",
        phase_debt: "branch controls must be uncomputed before parent mutation",
        toffoli_budget: 0,
        backend_primitive: "Round101 exact-exception totalizer",
    },
    SourceLiveD1PhaseBlockContract {
        ir_node: "round414_source_live_d1_quotient_product_core",
        phase: "round414_source_live_d1_quotient_product_core",
        field_semantics: "compute q=n/h or product h*n on the canonical nonzero source surface",
        register_lifecycle: "h:borrowed,n:live,workspace:live,scratch:borrowed",
        valid_branch: "h in 1..p and n in 0..p",
        phase_debt: "quotient/product workspace must be uncomputed by the reverse direction",
        toffoli_budget: ROUND101_SOURCE_LIVE_D1_ONE_WAY_TOFFOLI_TARGET,
        backend_primitive: "source-live D1 quotient/product lowerer",
    },
    SourceLiveD1PhaseBlockContract {
        ir_node: "round414_exact_exception_output_map",
        phase: "round414_source_live_d1_exact_exception_output_map",
        field_semantics:
            "map exact-exception source-live theorem outputs without output-local D1 separator",
        register_lifecycle: "h:borrowed,n:borrowed,output:live,workspace:borrowed",
        valid_branch: "Round101 exact-exception theorem branch",
        phase_debt: "no output-local quotient or hidden separator transcript may remain",
        toffoli_budget: 0,
        backend_primitive: "Round101 source-live D1 phase certificate",
    },
    SourceLiveD1PhaseBlockContract {
        ir_node: "round414_workspace_uncompute",
        phase: "round414_source_live_d1_workspace_uncompute",
        field_semantics: "restore quotient/product workspace to zero before parent release",
        register_lifecycle: "h:borrowed,n:live,workspace:zero,scratch:zero",
        valid_branch: "same source-live D1 branch used for the forward core",
        phase_debt: "all source-live D1 phase debt cleared before parent mutation",
        toffoli_budget: 0,
        backend_primitive: "reverse source-live D1 quotient/product lowerer",
    },
    SourceLiveD1PhaseBlockContract {
        ir_node: "round414_same_artifact_verifier_barrier",
        phase: "round414_source_live_d1_same_artifact_verifier_barrier",
        field_semantics: "complete PA artifact must own exact Q/T/ops/SHA and fuzz evidence",
        register_lifecycle: "h:zero,n:zero,workspace:zero,measurements:zero",
        valid_branch: "complete secp256k1 Google-ABI PA",
        phase_debt: "promotion forbidden until deterministic verifier passes",
        toffoli_budget: 0,
        backend_primitive: "stats plus google_pa_exec_stats exact fuzz harness",
    },
];

pub fn round218_b5_source_live_transport_pa_assembly() -> SourceLiveTransportPaAssembly {
    SourceLiveTransportPaAssembly {
        classification: ROUND218_B5_SOURCE_LIVE_PA_CLASSIFICATION,
        materialized_kmx: false,
        block_bits: ROUND217_SOURCE_LIVE_TRANSPORT_BLOCK_BITS,
        round210_qubits: ROUND210_DIRECT_QUOTIENT_PA_QUBITS,
        round210_toffoli: ROUND210_DIRECT_QUOTIENT_PA_TOFFOLI,
        round210_ops: ROUND210_DIRECT_QUOTIENT_PA_OPS,
        state_a_old_toffoli: ROUND210_PAIR1_DIRECT_QUOTIENT_TOFFOLI,
        state_b_old_toffoli: ROUND210_PAIR2_D1_PRODUCT_TOFFOLI,
        replacement_one_way_toffoli: ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX,
        replacement_total_toffoli: ROUND218_B5_SOURCE_LIVE_REPLACEMENT_TOFFOLI,
        outside_d1_qubits: ROUND218_B5_SOURCE_LIVE_PA_OUTSIDE_D1_QUBITS,
        replacement_qubits: ROUND218_B5_SOURCE_LIVE_PA_REPLACEMENT_QUBITS,
        assembled_qubits: ROUND218_B5_SOURCE_LIVE_PA_QUBITS,
        assembled_toffoli: ROUND218_B5_SOURCE_LIVE_PA_TOFFOLI,
        assembled_qt: ROUND218_B5_SOURCE_LIVE_PA_QT,
        toffoli_delta: ROUND218_B5_SOURCE_LIVE_PA_TOFFOLI as isize
            - ROUND210_DIRECT_QUOTIENT_PA_TOFFOLI as isize,
        qubit_delta: ROUND218_B5_SOURCE_LIVE_PA_QUBITS as isize
            - ROUND210_DIRECT_QUOTIENT_PA_QUBITS as isize,
        splice_points: [
            "round181_d1_state_a_quotient -> round8_fallback_xtail_square",
            "d1_direct_quotient_free_displaced_target deleted by source-live terminal cleanup",
            "round8_fallback_c_ox_minus_rx -> round8_fallback_y_output",
            "d1_product_free_zero_displaced_target deleted by source-live reverse terminal cleanup",
        ],
    }
}

pub fn round218_b5_compact_history_resource_gate() -> Round218B5CompactHistoryGate {
    let full_history_qt =
        ROUND218_B5_HISTORY_STREAM_PA_QUBITS * ROUND218_B5_HISTORY_STREAM_PA_TOFFOLI;
    let optimal_separator_qt =
        ROUND218_B5_OPTIMAL_SEPARATOR_PA_QUBITS * ROUND218_B5_HISTORY_STREAM_PA_TOFFOLI;
    let zeta_old_separator_qt =
        ROUND218_B5_ZETA_OLD_SEPARATOR_PA_QUBITS * ROUND218_B5_HISTORY_STREAM_PA_TOFFOLI;
    let optimal_product_toffoli_limit =
        (LOCAL_PA_QT_TARGET - 1) / ROUND218_B5_OPTIMAL_SEPARATOR_PA_QUBITS;
    let zeta_old_product_toffoli_limit =
        (LOCAL_PA_QT_TARGET - 1) / ROUND218_B5_ZETA_OLD_SEPARATOR_PA_QUBITS;

    Round218B5CompactHistoryGate {
        classification: ROUND218_B5_COMPACT_HISTORY_CLASSIFICATION,
        full_history_qubits: ROUND218_B5_HISTORY_STREAM_PA_QUBITS,
        full_history_toffoli: ROUND218_B5_HISTORY_STREAM_PA_TOFFOLI,
        full_history_qt,
        raw_control_history_bits: ROUND218_B5_RAW_CONTROL_HISTORY_BITS,
        separator_lower_bound_bits_per_block: ROUND218_B5_SEPARATOR_LOWER_BOUND_BITS_PER_BLOCK,
        separator_lower_bound_history_bits: ROUND218_B5_SEPARATOR_LOWER_BOUND_HISTORY_BITS,
        zeta_old_separator_bits_per_block: ROUND218_B5_ZETA_OLD_SEPARATOR_BITS_PER_BLOCK,
        zeta_old_separator_history_bits: ROUND218_B5_ZETA_OLD_SEPARATOR_HISTORY_BITS,
        optimal_separator_qubits: ROUND218_B5_OPTIMAL_SEPARATOR_PA_QUBITS,
        optimal_separator_toffoli: ROUND218_B5_HISTORY_STREAM_PA_TOFFOLI,
        optimal_separator_qt,
        zeta_old_separator_qubits: ROUND218_B5_ZETA_OLD_SEPARATOR_PA_QUBITS,
        zeta_old_separator_toffoli: ROUND218_B5_HISTORY_STREAM_PA_TOFFOLI,
        zeta_old_separator_qt,
        optimal_qubit_saving: ROUND218_B5_RAW_CONTROL_HISTORY_BITS
            - ROUND218_B5_SEPARATOR_LOWER_BOUND_HISTORY_BITS,
        zeta_old_qubit_saving: ROUND218_B5_RAW_CONTROL_HISTORY_BITS
            - ROUND218_B5_ZETA_OLD_SEPARATOR_HISTORY_BITS,
        optimal_product_toffoli_limit,
        zeta_old_product_toffoli_limit,
        optimal_toffoli_over_product_limit: ROUND218_B5_HISTORY_STREAM_PA_TOFFOLI as isize
            - optimal_product_toffoli_limit as isize,
        zeta_old_toffoli_over_product_limit: ROUND218_B5_HISTORY_STREAM_PA_TOFFOLI as isize
            - zeta_old_product_toffoli_limit as isize,
        makes_first_milestone: optimal_separator_qt < LOCAL_PA_QT_TARGET,
        next_required_object:
            "history-erasing B5 source transducer or a different source-live invariant",
    }
}

pub fn round251_qtail_second_inverse_budget_gate() -> Round251QtailSecondInverseBudgetGate {
    let product_toffoli_limit_at_lowq = (LOCAL_PA_QT_TARGET - 1) / ROUND251_LOWQ_D1_PA_QUBITS;
    let required_toffoli_saving_for_product =
        ROUND251_LOWQ_D1_PA_TOFFOLI - product_toffoli_limit_at_lowq;
    let max_second_inverse_replacement_toffoli_for_product =
        ROUND251_PAIR2_D1_PRODUCT_TOFFOLI - required_toffoli_saving_for_product;
    let strict_t_toffoli_limit = LOCAL_PA_T_TARGET - 1;
    let required_toffoli_saving_for_strict_t = ROUND251_LOWQ_D1_PA_TOFFOLI - strict_t_toffoli_limit;
    let max_second_inverse_replacement_toffoli_for_strict_t =
        ROUND251_PAIR2_D1_PRODUCT_TOFFOLI - required_toffoli_saving_for_strict_t;
    let arithmetic_only_pa_toffoli =
        ROUND251_LOWQ_D1_PA_TOFFOLI - ROUND251_D1_CLEANUP_ARITHMETIC_SAVING_CEILING;
    let arithmetic_only_pa_qt = ROUND251_LOWQ_D1_PA_QUBITS * arithmetic_only_pa_toffoli;

    Round251QtailSecondInverseBudgetGate {
        classification: ROUND251_QTAIL_SECOND_INVERSE_CLASSIFICATION,
        lowq_pa_qubits: ROUND251_LOWQ_D1_PA_QUBITS,
        lowq_pa_toffoli: ROUND251_LOWQ_D1_PA_TOFFOLI,
        lowq_pa_qt: ROUND251_LOWQ_D1_PA_QT,
        current_second_inverse_toffoli: ROUND251_PAIR2_D1_PRODUCT_TOFFOLI,
        product_toffoli_limit_at_lowq,
        required_toffoli_saving_for_product,
        max_second_inverse_replacement_toffoli_for_product,
        strict_t_toffoli_limit,
        required_toffoli_saving_for_strict_t,
        max_second_inverse_replacement_toffoli_for_strict_t,
        arithmetic_only_saving_ceiling: ROUND251_D1_CLEANUP_ARITHMETIC_SAVING_CEILING,
        arithmetic_only_second_inverse_floor: ROUND251_D1_CLEANUP_ARITHMETIC_FLOOR_TOFFOLI,
        arithmetic_only_pa_toffoli,
        arithmetic_only_pa_qt,
        arithmetic_only_toffoli_over_product_limit: arithmetic_only_pa_toffoli as isize
            - product_toffoli_limit_at_lowq as isize,
        arithmetic_only_makes_first_milestone: arithmetic_only_pa_qt < LOCAL_PA_QT_TARGET,
        next_required_object:
            "new second-inverse lowerer below 1.56M T, not post-swap cleanup arithmetic",
    }
}

pub fn round257_source_live_cubic_phase_budget_gate() -> Round257SourceLiveCubicPhaseBudgetGate {
    let product_toffoli_limit_at_dirty_q = (LOCAL_PA_QT_TARGET - 1) / ROUND257_DIRTY_CUBIC_QUBITS;
    let strict_t_limit = LOCAL_PA_T_TARGET - 1;

    Round257SourceLiveCubicPhaseBudgetGate {
        classification: ROUND257_SOURCE_LIVE_CUBIC_PHASE_CLASSIFICATION,
        dirty_qubits: ROUND257_DIRTY_CUBIC_QUBITS,
        dirty_toffoli: ROUND257_DIRTY_CUBIC_TOFFOLI,
        dirty_qt: ROUND257_DIRTY_CUBIC_QT,
        product_toffoli_limit_at_dirty_q,
        dirty_product_slack: product_toffoli_limit_at_dirty_q as isize
            - ROUND257_DIRTY_CUBIC_TOFFOLI as isize,
        dirty_toffoli_over_strict_t: ROUND257_DIRTY_CUBIC_TOFFOLI as isize
            - strict_t_limit as isize,
        max_phase_repair_net_toffoli_for_first_milestone: product_toffoli_limit_at_dirty_q as isize
            - ROUND257_DIRTY_CUBIC_TOFFOLI as isize,
        clean_product_qubits: ROUND257_CLEAN_PRODUCT_TAIL_QUBITS,
        clean_product_toffoli: ROUND257_CLEAN_PRODUCT_TAIL_TOFFOLI,
        clean_product_qt: ROUND257_CLEAN_PRODUCT_TAIL_QT,
        clean_product_delta_toffoli: ROUND257_CLEAN_PRODUCT_TAIL_TOFFOLI as isize
            - ROUND257_DIRTY_CUBIC_TOFFOLI as isize,
        clean_product_toffoli_over_strict_t: ROUND257_CLEAN_PRODUCT_TAIL_TOFFOLI as isize
            - strict_t_limit as isize,
        clean_product_toffoli_over_product_limit: ROUND257_CLEAN_PRODUCT_TAIL_TOFFOLI as isize
            - product_toffoli_limit_at_dirty_q as isize,
        clean_product_qt_slack: LOCAL_PA_QT_TARGET as isize
            - ROUND257_CLEAN_PRODUCT_TAIL_QT as isize,
        clean_lambda_qubits: ROUND257_CLEAN_LAMBDA_TAIL_QUBITS,
        clean_lambda_toffoli: ROUND257_CLEAN_LAMBDA_TAIL_TOFFOLI,
        clean_lambda_qt: ROUND257_CLEAN_LAMBDA_TAIL_QT,
        clean_lambda_delta_toffoli: ROUND257_CLEAN_LAMBDA_TAIL_TOFFOLI as isize
            - ROUND257_DIRTY_CUBIC_TOFFOLI as isize,
        clean_lambda_toffoli_over_strict_t: ROUND257_CLEAN_LAMBDA_TAIL_TOFFOLI as isize
            - strict_t_limit as isize,
        clean_lambda_toffoli_over_product_limit: ROUND257_CLEAN_LAMBDA_TAIL_TOFFOLI as isize
            - product_toffoli_limit_at_dirty_q as isize,
        clean_lambda_qt_slack: LOCAL_PA_QT_TARGET as isize
            - ROUND257_CLEAN_LAMBDA_TAIL_QT as isize,
        dirty_row_makes_first_milestone: ROUND257_DIRTY_CUBIC_QT < LOCAL_PA_QT_TARGET,
        clean_product_makes_first_milestone: ROUND257_CLEAN_PRODUCT_TAIL_QT < LOCAL_PA_QT_TARGET,
        clean_lambda_makes_first_milestone: ROUND257_CLEAN_LAMBDA_TAIL_QT < LOCAL_PA_QT_TARGET,
        next_required_object:
            "phase repair costing <=113864T at Q=2713, or a source-live overwrite that avoids the lambda HMR phase debt",
    }
}

pub fn round258_source_live_product_replacement_budget_gate(
) -> Round258SourceLiveProductReplacementBudgetGate {
    let strict_t_toffoli_limit = LOCAL_PA_T_TARGET - 1;
    let product_toffoli_limit_at_clean_product_q =
        (LOCAL_PA_QT_TARGET - 1) / ROUND257_CLEAN_PRODUCT_TAIL_QUBITS;
    let max_replacement_toffoli_for_strict_t =
        strict_t_toffoli_limit.saturating_sub(ROUND258_CLEAN_PRODUCT_NON_PRODUCT_TOFFOLI);
    let max_replacement_toffoli_for_product = product_toffoli_limit_at_clean_product_q
        .saturating_sub(ROUND258_CLEAN_PRODUCT_NON_PRODUCT_TOFFOLI);
    let ideal_zero_product_qt =
        ROUND257_CLEAN_PRODUCT_TAIL_QUBITS * ROUND258_CLEAN_PRODUCT_NON_PRODUCT_TOFFOLI;

    Round258SourceLiveProductReplacementBudgetGate {
        classification: ROUND258_SOURCE_LIVE_PRODUCT_REPLACEMENT_CLASSIFICATION,
        clean_product_qubits: ROUND257_CLEAN_PRODUCT_TAIL_QUBITS,
        clean_product_toffoli: ROUND257_CLEAN_PRODUCT_TAIL_TOFFOLI,
        clean_product_qt: ROUND257_CLEAN_PRODUCT_TAIL_QT,
        current_inplace_product_toffoli: ROUND258_CURRENT_INPLACE_PRODUCT_TOFFOLI,
        non_product_tail_toffoli: ROUND258_CLEAN_PRODUCT_NON_PRODUCT_TOFFOLI,
        strict_t_toffoli_limit,
        product_toffoli_limit_at_clean_product_q,
        max_replacement_toffoli_for_strict_t,
        max_replacement_toffoli_for_product,
        required_current_product_saving_for_strict_t: ROUND258_CURRENT_INPLACE_PRODUCT_TOFFOLI
            - max_replacement_toffoli_for_strict_t,
        required_current_product_saving_for_product: ROUND258_CURRENT_INPLACE_PRODUCT_TOFFOLI
            - max_replacement_toffoli_for_product,
        ideal_zero_product_toffoli: ROUND258_CLEAN_PRODUCT_NON_PRODUCT_TOFFOLI,
        ideal_zero_product_qt,
        ideal_zero_product_makes_first_milestone: ideal_zero_product_qt < LOCAL_PA_QT_TARGET,
        round217_one_way_transport_toffoli: ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX,
        round217_replacement_toffoli_over_strict_t_budget:
            ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX as isize
                - max_replacement_toffoli_for_strict_t as isize,
        round217_replacement_toffoli_over_product_budget:
            ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX as isize
                - max_replacement_toffoli_for_product as isize,
        d1_product_toffoli_over_strict_t_budget: ROUND258_CURRENT_INPLACE_PRODUCT_TOFFOLI as isize
            - max_replacement_toffoli_for_strict_t as isize,
        d1_product_toffoli_over_product_budget: ROUND258_CURRENT_INPLACE_PRODUCT_TOFFOLI as isize
            - max_replacement_toffoli_for_product as isize,
        current_clean_product_makes_first_milestone:
            ROUND257_CLEAN_PRODUCT_TAIL_QT < LOCAL_PA_QT_TARGET,
        next_required_object:
            "an in-place source-live product or lambda-overwrite lowerer at <=574839T under product-only scoring",
    }
}

pub fn round259_source_live_hmr_overwrite_gate() -> Round259SourceLiveHmrOverwriteGate {
    Round259SourceLiveHmrOverwriteGate {
        classification: ROUND259_SOURCE_LIVE_HMR_OVERWRITE_CLASSIFICATION,
        hmr_overwrite_qubits: ROUND259_HMR_OVERWRITE_QUBITS,
        hmr_overwrite_toffoli: ROUND259_HMR_OVERWRITE_TOFFOLI,
        hmr_overwrite_qt: ROUND259_HMR_OVERWRITE_QT,
        strict_t_slack: LOCAL_PA_T_TARGET as isize - ROUND259_HMR_OVERWRITE_TOFFOLI as isize,
        qt_slack: LOCAL_PA_QT_TARGET as isize - ROUND259_HMR_OVERWRITE_QT as isize,
        measured_lam_bits: ROUND259_HMR_OVERWRITE_MEASURED_LAM_BITS,
        resource_makes_first_milestone: ROUND259_HMR_OVERWRITE_QT < LOCAL_PA_QT_TARGET,
        direct_case_phase: ROUND259_DIRECT_CASE_PHASE,
        phase_clean: ROUND259_DIRECT_CASE_PHASE == 0,
        toy_quotient_phase_degree: ROUND259_TOY_QUOTIENT_PHASE_DEGREE,
        toy_quotient_phase_density: ROUND259_TOY_QUOTIENT_PHASE_DENSITY,
        toy_quotient_phase_table: ROUND259_TOY_QUOTIENT_PHASE_TABLE,
        next_required_object:
            "phase-clean quotient-phase correction for lam=yprod/(Rx-Qx), or a non-HMR overwrite",
    }
}

fn round398_pa_toffoli_limit(q: usize, product_limit: usize) -> usize {
    (product_limit - 1) / q
}

pub fn round398_qtail_round217_product_budget_gate() -> Round398QtailRound217ProductBudgetGate {
    let non_product_toffoli = ROUND251_LOWQ_D1_PA_TOFFOLI - ROUND251_PAIR2_D1_PRODUCT_TOFFOLI;
    let m1_pa_toffoli_limit =
        round398_pa_toffoli_limit(ROUND398_QTAIL_PRODUCT_QUBITS, ROUND398_PRODUCT_M1_QT_TARGET);
    let m2_pa_toffoli_limit =
        round398_pa_toffoli_limit(ROUND398_QTAIL_PRODUCT_QUBITS, ROUND398_PRODUCT_M2_QT_TARGET);
    let m3_pa_toffoli_limit =
        round398_pa_toffoli_limit(ROUND398_QTAIL_PRODUCT_QUBITS, ROUND398_PRODUCT_M3_QT_TARGET);
    let m4_pa_toffoli_limit =
        round398_pa_toffoli_limit(ROUND398_QTAIL_PRODUCT_QUBITS, ROUND398_PRODUCT_M4_QT_TARGET);
    let m5_pa_toffoli_limit =
        round398_pa_toffoli_limit(ROUND398_QTAIL_PRODUCT_QUBITS, ROUND398_PRODUCT_M5_QT_TARGET);
    let replacement_limit =
        |pa_limit: usize| -> isize { pa_limit as isize - non_product_toffoli as isize };
    let round217_projected_pa_toffoli =
        non_product_toffoli + ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX;
    let round217_projected_pa_qt = ROUND398_QTAIL_PRODUCT_QUBITS * round217_projected_pa_toffoli;

    Round398QtailRound217ProductBudgetGate {
        classification: ROUND398_QTAIL_ROUND217_PRODUCT_BUDGET_CLASSIFICATION,
        qtail_qubits: ROUND398_QTAIL_PRODUCT_QUBITS,
        qtail_current_toffoli: ROUND251_LOWQ_D1_PA_TOFFOLI,
        current_product_toffoli: ROUND251_PAIR2_D1_PRODUCT_TOFFOLI,
        non_product_toffoli,
        m1_pa_toffoli_limit,
        m2_pa_toffoli_limit,
        m3_pa_toffoli_limit,
        m4_pa_toffoli_limit,
        m5_pa_toffoli_limit,
        m1_replacement_toffoli_limit: replacement_limit(m1_pa_toffoli_limit),
        m2_replacement_toffoli_limit: replacement_limit(m2_pa_toffoli_limit),
        m3_replacement_toffoli_limit: replacement_limit(m3_pa_toffoli_limit),
        m4_replacement_toffoli_limit: replacement_limit(m4_pa_toffoli_limit),
        m5_replacement_toffoli_limit: replacement_limit(m5_pa_toffoli_limit),
        round217_one_way_toffoli: ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX,
        round217_projected_pa_toffoli,
        round217_projected_pa_qt,
        round217_clears_m1: round217_projected_pa_qt < ROUND398_PRODUCT_M1_QT_TARGET,
        round217_clears_m2: round217_projected_pa_qt < ROUND398_PRODUCT_M2_QT_TARGET,
        round217_clears_m3: round217_projected_pa_qt < ROUND398_PRODUCT_M3_QT_TARGET,
        round217_m2_replacement_slack: replacement_limit(m2_pa_toffoli_limit)
            - ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX as isize,
        round217_m3_replacement_over: ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX as isize
            - replacement_limit(m3_pa_toffoli_limit),
        forbidden_probe_qubits: ROUND398_FORBIDDEN_PROBE_QUBITS,
        forbidden_probe_toffoli: ROUND398_FORBIDDEN_PROBE_TOFFOLI,
        forbidden_probe_qt: ROUND398_FORBIDDEN_PROBE_QT,
        forbidden_named_product_phase_toffoli: ROUND398_FORBIDDEN_NAMED_PRODUCT_PHASE_TOFFOLI,
        forbidden_dominant_inverse_transport_toffoli:
            ROUND398_FORBIDDEN_DOMINANT_INVERSE_TRANSPORT_TOFFOLI,
        forbidden_endpoint_replay_toffoli: ROUND398_FORBIDDEN_ENDPOINT_REPLAY_TOFFOLI,
        forbidden_product_scale_toffoli: ROUND398_FORBIDDEN_PRODUCT_SCALE_TOFFOLI,
        forbidden_full_source_history_bits: ROUND398_FORBIDDEN_FULL_SOURCE_HISTORY_BITS,
        forbidden_named_product_phase_over_m2_replacement_limit:
            ROUND398_FORBIDDEN_NAMED_PRODUCT_PHASE_TOFFOLI as isize
                - replacement_limit(m2_pa_toffoli_limit),
        source_live_product_alias_must_fail_closed: true,
        next_required_object:
            "source-live qtail/Round217 product splice at <=1155100T for M2 or <=747436T for M3; no full-source history alias",
    }
}

pub fn round414_source_live_d1_backend_contract() -> SourceLiveD1BackendContract {
    SourceLiveD1BackendContract {
        classification: ROUND414_SOURCE_LIVE_D1_BACKEND_CONTRACT_CLASSIFICATION,
        env: ROUND414_SOURCE_LIVE_D1_PRODUCTION_LOWERER_ENV,
        target_row: "round392_source_live_d1_branch_split_gate",
        target_qubits: ROUND100_EXACT_EXCEPTION_PA_Q,
        target_toffoli: ROUND100_EXACT_EXCEPTION_PA_T,
        target_qt: ROUND413_SOURCE_LIVE_D1_PA_QT,
        one_way_toffoli_target: ROUND101_SOURCE_LIVE_D1_ONE_WAY_TOFFOLI_TARGET,
        requirements: ROUND414_SOURCE_LIVE_D1_BACKEND_REQUIREMENTS,
        missing_object: ROUND414_SOURCE_LIVE_D1_MISSING_OBJECT,
    }
}

pub fn round414_source_live_d1_body_plan() -> SourceLiveD1BodyPlan {
    let contract = round414_source_live_d1_backend_contract();
    SourceLiveD1BodyPlan {
        classification: contract.classification,
        selected_target: contract.target_row,
        target_qubits: contract.target_qubits,
        target_toffoli: contract.target_toffoli,
        target_qt: contract.target_qt,
        one_way_toffoli_target: contract.one_way_toffoli_target,
        phase_blocks: &ROUND414_SOURCE_LIVE_D1_PHASE_BLOCKS,
        body_emits_gates: false,
        codegen_allowed_now: false,
        missing_object: contract.missing_object,
    }
}

pub fn round415_d1_inplace_lowerer_audit() -> Round415D1InplaceLowererAudit {
    Round415D1InplaceLowererAudit {
        classification: ROUND415_D1_INPLACE_LOWERER_AUDIT_CLASSIFICATION,
        product_qubits: ROUND415_D1_INPLACE_PRODUCT_QUBITS,
        product_toffoli: ROUND415_D1_INPLACE_PRODUCT_TOFFOLI,
        product_ops: ROUND415_D1_INPLACE_PRODUCT_OPS,
        product_bits: ROUND415_D1_INPLACE_PRODUCT_BITS,
        product_phase_rows: ROUND415_D1_INPLACE_PRODUCT_PHASE_ROWS,
        product_hmr_ops: ROUND415_D1_INPLACE_PRODUCT_HMR_OPS,
        product_r_ops: ROUND415_D1_INPLACE_PRODUCT_R_OPS,
        quotient_qubits: ROUND415_D1_INPLACE_QUOTIENT_QUBITS,
        quotient_toffoli: ROUND415_D1_INPLACE_QUOTIENT_TOFFOLI,
        quotient_ops: ROUND415_D1_INPLACE_QUOTIENT_OPS,
        quotient_bits: ROUND415_D1_INPLACE_QUOTIENT_BITS,
        quotient_phase_rows: ROUND415_D1_INPLACE_QUOTIENT_PHASE_ROWS,
        target_qubits: ROUND100_EXACT_EXCEPTION_PA_Q,
        target_toffoli: ROUND100_EXACT_EXCEPTION_PA_T,
        one_way_toffoli_target: ROUND101_SOURCE_LIVE_D1_ONE_WAY_TOFFOLI_TARGET,
        one_way_toffoli_over_target: ROUND415_D1_INPLACE_PRODUCT_TOFFOLI as isize
            - ROUND101_SOURCE_LIVE_D1_ONE_WAY_TOFFOLI_TARGET as isize,
        qubits_over_target: ROUND415_D1_INPLACE_PRODUCT_QUBITS as isize
            - ROUND100_EXACT_EXCEPTION_PA_Q as isize,
        projected_pa_qubits: ROUND415_D1_INPLACE_PROJECTED_PA_QUBITS,
        projected_pa_toffoli: ROUND415_D1_INPLACE_PROJECTED_PA_TOFFOLI,
        projected_pa_qt: ROUND415_D1_INPLACE_PROJECTED_PA_QT,
        projected_pa_toffoli_over_local_target: ROUND415_D1_INPLACE_PROJECTED_PA_TOFFOLI as isize
            - LOCAL_PA_T_TARGET as isize,
        projected_pa_product_m1: ROUND415_D1_INPLACE_PROJECTED_PA_QT
            < ROUND398_PRODUCT_M1_QT_TARGET,
        projected_pa_product_m2: ROUND415_D1_INPLACE_PROJECTED_PA_QT
            < ROUND398_PRODUCT_M2_QT_TARGET,
        projected_pa_product_slack_m1: ROUND398_PRODUCT_M1_QT_TARGET as isize
            - ROUND415_D1_INPLACE_PROJECTED_PA_QT as isize,
        projected_pa_product_slack_m2: ROUND398_PRODUCT_M2_QT_TARGET as isize
            - ROUND415_D1_INPLACE_PROJECTED_PA_QT as isize,
        product_has_measurement_debt: ROUND415_D1_INPLACE_PRODUCT_BITS != 0
            || ROUND415_D1_INPLACE_PRODUCT_HMR_OPS != 0,
        contract_compatible: false,
    }
}

pub(super) fn emit_round414_source_live_d1_quotient_product_lowerer(
    b: &mut B,
    h: &[QubitId],
    n: &[QubitId],
    p: U256,
) {
    assert_eq!(p, SECP256K1_P, "Round414 source-live D1 is secp256k1-only");
    assert_eq!(
        h.len(),
        N,
        "source-live D1 denominator parent must be 256 qubits"
    );
    assert_eq!(
        n.len(),
        N,
        "source-live D1 numerator/target must be 256 qubits"
    );
    let plan = round414_source_live_d1_body_plan();
    debug_assert!(!plan.body_emits_gates);
    debug_assert!(!plan.codegen_allowed_now);
    b.set_phase("round414_source_live_d1_quotient_product_lowerer_fail_closed");
    panic!(
        "emit_round414_source_live_d1_quotient_product_lowerer is intentionally fail-closed: \
         classification={classification}; target={target}; target_Q={q}; target_T={t}; \
         target_QT={qt}; one_way_target={one_way}; phase_blocks={blocks}; \
         missing_object={missing}.  Implement the complete source-live D1 quotient/product \
         PA KMX with live source parents, zero-exit quotient/product workspace, exact-exception \
         totalization, same-artifact stats, and deterministic 9024 Google PA fuzz before \
         enabling {env}=1.",
        classification = plan.classification,
        target = plan.selected_target,
        q = plan.target_qubits,
        t = plan.target_toffoli,
        qt = plan.target_qt,
        one_way = plan.one_way_toffoli_target,
        blocks = plan.phase_blocks.len(),
        missing = plan.missing_object,
        env = ROUND414_SOURCE_LIVE_D1_PRODUCTION_LOWERER_ENV,
    );
}

fn modp(x: i128, p: i128) -> i128 {
    x.rem_euclid(p)
}

fn egcd(a: i128, b: i128) -> (i128, i128, i128) {
    if b == 0 {
        (a.abs(), a.signum(), 0)
    } else {
        let (g, x, y) = egcd(b, a.rem_euclid(b));
        (g, y, x - (a.div_euclid(b)) * y)
    }
}

fn inv_mod(a: i128, p: i128) -> i128 {
    let (g, x, _) = egcd(modp(a, p), p);
    assert_eq!(g, 1, "value is not invertible modulo p");
    modp(x, p)
}

fn gcd_i128(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

fn totalized_divide_value(h: i128, n: i128, p: i128) -> i128 {
    if (1..p).contains(&h) && (0..p).contains(&n) {
        modp(n * inv_mod(h, p), p)
    } else {
        n
    }
}

fn totalized_product_value(h: i128, n: i128, p: i128) -> i128 {
    if (1..p).contains(&h) && (0..p).contains(&n) {
        modp(h * n, p)
    } else {
        n
    }
}

pub fn prove_product_quotient_equivalence(prime: i128) -> ProductQuotientEquivalence {
    assert!(prime > 2);
    let mut canonical_rows_checked = 0usize;
    let mut fixed_points_checked = 0usize;
    let mut ok = true;

    for h in 0..prime + 3 {
        for n in 0..prime + 3 {
            let q = totalized_divide_value(h, n, prime);
            let product_after_q = totalized_product_value(h, q, prime);
            let product = totalized_product_value(h, n, prime);
            let quotient_after_product = totalized_divide_value(h, product, prime);
            if (1..prime).contains(&h) && (0..prime).contains(&n) {
                canonical_rows_checked += 1;
                ok &= product_after_q == n;
                ok &= quotient_after_product == n;
            } else {
                fixed_points_checked += 1;
                ok &= q == n;
                ok &= product == n;
            }
        }
    }

    ProductQuotientEquivalence {
        prime,
        canonical_rows_checked,
        product_is_inverse_quotient: ok,
        totalized_fixed_points_checked: fixed_points_checked,
    }
}

pub fn half_delta_step_mod(state: State, p: i128) -> (State, u8, u8) {
    let (zeta, f, g, v, r) = state;
    let old_g0 = modp(g, 2) as u8;
    if zeta < 0 && old_g0 != 0 {
        (
            (-zeta - 2, g, (g - f) / 2, modp(2 * r, p), modp(r - v, p)),
            1,
            old_g0,
        )
    } else {
        (
            (
                zeta - 1,
                f,
                (g + old_g0 as i128 * f) / 2,
                modp(2 * v, p),
                modp(r + old_g0 as i128 * v, p),
            ),
            0,
            old_g0,
        )
    }
}

fn determinant_quotient(state: State, p: i128) -> i128 {
    let (_, f, g, v, r) = state;
    let det = f * r - g * v;
    assert_eq!(det.rem_euclid(p), 0, "determinant not divisible by p");
    det / p
}

fn implied_h(state: State, p: i128) -> i128 {
    let (_, f, g, v, r) = state;
    if modp(r, p) != 0 {
        modp(g * inv_mod(r, p), p)
    } else {
        assert_ne!(modp(v, p), 0, "state has no nonzero coefficient");
        modp(f * inv_mod(v, p), p)
    }
}

pub fn local_inverse_candidates_mod(output: State, p: i128) -> Vec<(&'static str, State)> {
    let (zeta_out, f_out, g_out, v_out, r_out) = output;
    let inv2 = inv_mod(2, p);
    let v_half = modp(v_out * inv2, p);
    let mut out = Vec::new();

    let positive = (
        -zeta_out - 2,
        f_out - 2 * g_out,
        f_out,
        modp(v_half - r_out, p),
        v_half,
    );
    if positive.0 < 0 && modp(positive.2, 2) != 0 {
        out.push(("positive_branch_old_g0_1", positive));
    }

    out.push((
        "nonbranch_old_g0_0",
        (zeta_out + 1, f_out, 2 * g_out, v_half, r_out),
    ));

    let odd = (
        zeta_out + 1,
        f_out,
        2 * g_out - f_out,
        v_half,
        modp(r_out - v_half, p),
    );
    if odd.0 >= 0 && modp(odd.2, 2) != 0 {
        out.push(("nonbranch_old_g0_1", odd));
    }

    out
}

pub fn seed_collision_candidates(p: i128) -> Vec<D1ControlCandidate> {
    let seed = (-1, p, 1, 0, 1);
    let (output, branch, old_g0) = half_delta_step_mod(seed, p);
    assert_eq!((branch, old_g0), (1, 1));

    local_inverse_candidates_mod(output, p)
        .into_iter()
        .map(|(label, predecessor)| {
            let (got_output, retained_branch, retained_old_g0) =
                half_delta_step_mod(predecessor, p);
            assert_eq!(got_output, output);
            D1ControlCandidate {
                label,
                predecessor,
                retained_control: (retained_branch, retained_old_g0),
                output: got_output,
                determinant_quotient: determinant_quotient(predecessor, p),
                implied_h: implied_h(predecessor, p),
                gcd_fg: gcd_i128(predecessor.1, predecessor.2),
            }
        })
        .collect()
}

fn state_key(state: State) -> Vec<i128> {
    vec![state.0, state.1, state.2, state.3, state.4]
}

fn state_low_key(state: State, bits: usize) -> Vec<i128> {
    let modulus = 1i128 << bits;
    vec![
        state.0.rem_euclid(modulus),
        state.1.rem_euclid(modulus),
        state.2.rem_euclid(modulus),
        state.3.rem_euclid(modulus),
        state.4.rem_euclid(modulus),
    ]
}

fn state_parity_key(state: State) -> Vec<i128> {
    state_low_key(state, 1)
}

fn delayed_trace_key(mut state: State, p: i128, depth: usize) -> Vec<i128> {
    let mut out = Vec::with_capacity(2 * depth);
    for _ in 0..depth {
        let (next, branch, old_g0) = half_delta_step_mod(state, p);
        out.push(branch as i128);
        out.push(old_g0 as i128);
        state = next;
    }
    out
}

fn distinct_count(keys: &[Vec<i128>]) -> usize {
    keys.iter().cloned().collect::<BTreeSet<_>>().len()
}

fn probe_separates(candidates: &[D1ControlCandidate], keys: &[Vec<i128>]) -> bool {
    let distinct_controls = candidates
        .iter()
        .map(|candidate| candidate.retained_control)
        .collect::<BTreeSet<_>>()
        .len();
    distinct_count(keys) >= distinct_controls
        && keys
            .iter()
            .zip(candidates.iter())
            .map(|(key, candidate)| (key.clone(), candidate.retained_control))
            .collect::<BTreeSet<_>>()
            .len()
            == distinct_controls
}

pub fn observable_search_report(p: i128) -> ObservableSearchReport {
    let candidates = seed_collision_candidates(p);
    let mut probes: Vec<Vec<Vec<i128>>> = vec![
        candidates
            .iter()
            .map(|candidate| state_key(candidate.output))
            .collect(),
        candidates
            .iter()
            .map(|candidate| vec![candidate.output.0])
            .collect(),
        candidates
            .iter()
            .map(|candidate| vec![candidate.output.0.rem_euclid(2)])
            .collect(),
        candidates
            .iter()
            .map(|candidate| state_parity_key(candidate.output))
            .collect(),
        candidates
            .iter()
            .map(|candidate| vec![candidate.determinant_quotient])
            .collect(),
        candidates
            .iter()
            .map(|candidate| vec![candidate.implied_h])
            .collect(),
        candidates
            .iter()
            .map(|candidate| vec![candidate.gcd_fg])
            .collect(),
    ];
    for bits in [1usize, 2, 4, 8, 16] {
        probes.push(
            candidates
                .iter()
                .map(|candidate| state_low_key(candidate.output, bits))
                .collect(),
        );
        probes.push(
            candidates
                .iter()
                .map(|candidate| vec![candidate.determinant_quotient.rem_euclid(1i128 << bits)])
                .collect(),
        );
        probes.push(
            candidates
                .iter()
                .map(|candidate| vec![candidate.implied_h.rem_euclid(1i128 << bits)])
                .collect(),
        );
    }
    for depth in [1usize, 2, 4, 8] {
        probes.push(
            candidates
                .iter()
                .map(|candidate| delayed_trace_key(candidate.output, p, depth))
                .collect(),
        );
    }

    let separating_admissible_probe_count = probes
        .iter()
        .filter(|probe| probe_separates(&candidates, probe))
        .count();

    let combined_keys: Vec<Vec<i128>> = (0..candidates.len())
        .map(|idx| {
            let mut key = Vec::new();
            for probe in &probes {
                key.extend_from_slice(&probe[idx]);
                key.push(i128::MIN);
            }
            key
        })
        .collect();

    let forbidden_probes = [
        candidates
            .iter()
            .map(|candidate| state_key(candidate.predecessor))
            .collect::<Vec<_>>(),
        candidates
            .iter()
            .map(|candidate| {
                vec![
                    candidate.retained_control.0 as i128,
                    candidate.retained_control.1 as i128,
                ]
            })
            .collect::<Vec<_>>(),
    ];
    let forbidden_separator_count = forbidden_probes
        .iter()
        .filter(|probe| probe_separates(&candidates, probe))
        .count();

    let distinct_outputs = candidates
        .iter()
        .map(|candidate| candidate.output)
        .collect::<BTreeSet<_>>()
        .len();
    let distinct_controls = candidates
        .iter()
        .map(|candidate| candidate.retained_control)
        .collect::<BTreeSet<_>>()
        .len();
    let combined_admissible_separates = distinct_count(&combined_keys) >= distinct_controls;

    ObservableSearchReport {
        classification: if separating_admissible_probe_count == 0
            && !combined_admissible_separates
            && forbidden_separator_count > 0
        {
            "SOURCE_LIVE_D1_OUTPUT_OBSERVABLE_HARD_OBSTRUCTION"
        } else {
            "SOURCE_LIVE_D1_OUTPUT_OBSERVABLE_NEEDS_REVIEW"
        },
        prime: p,
        candidate_count: candidates.len(),
        distinct_outputs,
        distinct_controls,
        admissible_probe_count: probes.len(),
        separating_admissible_probe_count,
        forbidden_separator_count,
        combined_admissible_separates,
    }
}

pub fn triangular_stream_matrix(block_bits: usize) -> Matrix2 {
    assert!(block_bits > 0);
    let scale = 1i128 << block_bits;
    ((scale, 0), (scale - 1, 1))
}

pub fn quotient_record(
    matrix: Matrix2,
    vector: Vector2,
    prime: i128,
    low_bits: usize,
) -> QuotientRecord {
    let raw = (
        matrix.0 .0 * vector.0 + matrix.0 .1 * vector.1,
        matrix.1 .0 * vector.0 + matrix.1 .1 * vector.1,
    );
    let canonical = (modp(raw.0, prime), modp(raw.1, prime));
    let quotient = ((raw.0 - canonical.0) / prime, (raw.1 - canonical.1) / prime);
    let mask = (1i128 << low_bits) - 1;
    QuotientRecord {
        raw,
        canonical,
        quotient,
        low_key: (canonical.0 & mask, canonical.1 & mask),
    }
}

pub fn triangular_lowbit_quotient_collision(prime: i128, block_bits: usize) -> QuotientCollision {
    assert!(prime > 2);
    let scale = 1i128 << block_bits;
    assert!(prime > 2 * scale);
    let matrix = triangular_stream_matrix(block_bits);
    let second_b = prime - (scale - 1);
    let mut first_b = second_b - prime.rem_euclid(scale);
    if first_b == second_b {
        first_b -= scale;
    }
    let first_input = (1, first_b);
    let second_input = (1, second_b);
    let first = quotient_record(matrix, first_input, prime, block_bits);
    let second = quotient_record(matrix, second_input, prime, block_bits);
    QuotientCollision {
        classification: if first.low_key == second.low_key && first.quotient != second.quotient {
            "LOW_BITS_DO_NOT_DETERMINE_FOLD_QUOTIENT"
        } else {
            "LOWBIT_COLLISION_CONSTRUCTION_FAILED"
        },
        prime,
        block_bits,
        matrix,
        first_input,
        second_input,
        first,
        second,
    }
}

pub fn cleanup_range_floor(block_bits: usize) -> CleanupRangeFloor {
    let (blocks, csd_slots_per_coordinate_pair, round82_d1_margin) = match block_bits {
        8 => (65usize, 14usize, 288_239isize),
        9 => (57usize, 16usize, 276_244isize),
        _ => panic!("only the Round82 B=8/B=9 cleanup floors are pinned"),
    };
    let floor_toffoli = 2 * csd_slots_per_coordinate_pair * 256 * blocks;
    let over_margin = floor_toffoli as isize - round82_d1_margin;
    CleanupRangeFloor {
        classification: if over_margin > 0 {
            "COUNTED_RANGE_CLEANUP_OVER_ROUND82_MARGIN"
        } else {
            "RANGE_CLEANUP_FLOOR_FITS_ROUND82_MARGIN"
        },
        block_bits,
        d1_division_target: D1_DIVISION_T_TARGET,
        blocks,
        csd_slots_per_coordinate_pair,
        floor_toffoli,
        round82_d1_margin,
        over_margin,
    }
}

pub fn production_report() -> SourceLiveD1Report {
    let equivalence = prove_product_quotient_equivalence(251);
    let observable = observable_search_report(251);
    let lowbit = triangular_lowbit_quotient_collision(101, 4);
    let range_cleanup_b8 = cleanup_range_floor(8);
    let range_cleanup_b9 = cleanup_range_floor(9);

    let quotient_product_equivalence_pass = equivalence.product_is_inverse_quotient;
    let output_observable_obstruction =
        observable.classification == "SOURCE_LIVE_D1_OUTPUT_OBSERVABLE_HARD_OBSTRUCTION";
    let lowbit_quotient_obstruction =
        lowbit.classification == "LOW_BITS_DO_NOT_DETERMINE_FOLD_QUOTIENT";
    let range_cleanup_over = range_cleanup_b8.over_margin > 0 && range_cleanup_b9.over_margin > 0;

    SourceLiveD1Report {
        classification: if quotient_product_equivalence_pass
            && output_observable_obstruction
            && lowbit_quotient_obstruction
            && range_cleanup_over
        {
            "BREAK_NO_PRODUCTION_SOURCE_LIVE_D1_TRANSDUCER"
        } else {
            "REVIEW_SOURCE_LIVE_D1_TRANSDUCER"
        },
        conditional_row: ROUND100_CONDITIONAL_ROW,
        exact_exception_row: ROUND100_EXACT_EXCEPTION_ROW,
        quotient_product_equivalence_pass,
        output_observable_obstruction,
        lowbit_quotient_obstruction,
        range_cleanup_b8,
        range_cleanup_b9,
        makes_under_local_pa_target: false,
        required_next_object: "a counted source-live range separator that clears branch, old_g0, quotient, carry, canonical, and high bits without predecessor/history tape",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d1_product_is_reversed_quotient_on_totalized_toy_fields() {
        for prime in [5i128, 7, 13, 251] {
            let proof = prove_product_quotient_equivalence(prime);
            println!(
                "METRIC source_live_d1_product_quotient_prime_{prime}_canonical_rows={}",
                proof.canonical_rows_checked
            );
            println!(
                "METRIC source_live_d1_product_quotient_prime_{prime}_fixed_rows={}",
                proof.totalized_fixed_points_checked
            );
            assert!(proof.product_is_inverse_quotient);
        }
    }

    #[test]
    fn output_local_observables_do_not_clean_source_live_controls() {
        let report = observable_search_report(251);
        println!(
            "METRIC source_live_d1_observable_candidates={}",
            report.candidate_count
        );
        println!(
            "METRIC source_live_d1_observable_distinct_outputs={}",
            report.distinct_outputs
        );
        println!(
            "METRIC source_live_d1_observable_distinct_controls={}",
            report.distinct_controls
        );
        println!(
            "METRIC source_live_d1_observable_admissible_probes={}",
            report.admissible_probe_count
        );
        println!(
            "METRIC source_live_d1_observable_forbidden_separators={}",
            report.forbidden_separator_count
        );
        assert_eq!(
            report.classification,
            "SOURCE_LIVE_D1_OUTPUT_OBSERVABLE_HARD_OBSTRUCTION"
        );
        assert_eq!(report.candidate_count, 3);
        assert_eq!(report.distinct_outputs, 1);
        assert_eq!(report.distinct_controls, 3);
        assert_eq!(report.separating_admissible_probe_count, 0);
        assert!(!report.combined_admissible_separates);
        assert!(report.forbidden_separator_count > 0);
    }

    #[test]
    fn low_output_bits_do_not_determine_fold_quotient() {
        let collision = triangular_lowbit_quotient_collision(101, 4);
        println!(
            "METRIC source_live_d1_lowbit_collision_prime={}",
            collision.prime
        );
        println!(
            "METRIC source_live_d1_lowbit_collision_block_bits={}",
            collision.block_bits
        );
        assert_eq!(
            collision.classification,
            "LOW_BITS_DO_NOT_DETERMINE_FOLD_QUOTIENT"
        );
        assert_eq!(collision.first.low_key, collision.second.low_key);
        assert_ne!(collision.first.quotient, collision.second.quotient);
    }

    #[test]
    fn source_live_d1_resource_gate_breaks_current_pa_path() {
        let report = production_report();
        println!(
            "METRIC source_live_d1_conditional_pa_q={}",
            report.conditional_row.qubits
        );
        println!(
            "METRIC source_live_d1_conditional_pa_t={}",
            report.conditional_row.toffoli
        );
        println!(
            "METRIC source_live_d1_conditional_pa_q_slack={}",
            report.conditional_row.qubit_slack()
        );
        println!(
            "METRIC source_live_d1_conditional_pa_t_slack={}",
            report.conditional_row.toffoli_slack()
        );
        println!(
            "METRIC source_live_d1_exact_exception_pa_q={}",
            report.exact_exception_row.qubits
        );
        println!(
            "METRIC source_live_d1_exact_exception_pa_t={}",
            report.exact_exception_row.toffoli
        );
        println!(
            "METRIC source_live_d1_exact_exception_pa_q_slack={}",
            report.exact_exception_row.qubit_slack()
        );
        println!(
            "METRIC source_live_d1_exact_exception_pa_t_slack={}",
            report.exact_exception_row.toffoli_slack()
        );
        println!(
            "METRIC source_live_d1_b8_range_cleanup_floor={}",
            report.range_cleanup_b8.floor_toffoli
        );
        println!(
            "METRIC source_live_d1_b8_range_cleanup_over_margin={}",
            report.range_cleanup_b8.over_margin
        );
        println!(
            "METRIC source_live_d1_b9_range_cleanup_floor={}",
            report.range_cleanup_b9.floor_toffoli
        );
        println!(
            "METRIC source_live_d1_b9_range_cleanup_over_margin={}",
            report.range_cleanup_b9.over_margin
        );
        assert_eq!(
            report.classification,
            "BREAK_NO_PRODUCTION_SOURCE_LIVE_D1_TRANSDUCER"
        );
        assert_eq!(report.conditional_row.qubit_slack(), 691);
        assert_eq!(report.conditional_row.toffoli_slack(), 942_351);
        assert_eq!(report.exact_exception_row.qubit_slack(), 685);
        assert_eq!(report.exact_exception_row.toffoli_slack(), 931_623);
        assert!(report.quotient_product_equivalence_pass);
        assert!(report.output_observable_obstruction);
        assert!(report.lowbit_quotient_obstruction);
        assert_eq!(
            report.range_cleanup_b8.d1_division_target,
            D1_DIVISION_T_TARGET
        );
        assert!(report.range_cleanup_b8.over_margin > 0);
        assert!(report.range_cleanup_b9.over_margin > 0);
        assert!(!report.makes_under_local_pa_target);
    }

    #[test]
    fn round414_source_live_d1_backend_contract_maps_round392_target() {
        let contract = round414_source_live_d1_backend_contract();
        assert_eq!(
            contract.classification,
            ROUND414_SOURCE_LIVE_D1_BACKEND_CONTRACT_CLASSIFICATION
        );
        assert_eq!(contract.env, ROUND414_SOURCE_LIVE_D1_PRODUCTION_LOWERER_ENV);
        assert_eq!(
            contract.target_row,
            "round392_source_live_d1_branch_split_gate"
        );
        assert_eq!(contract.target_qubits, 1_315);
        assert_eq!(contract.target_toffoli, 2_068_377);
        assert_eq!(contract.target_qt, 2_719_915_755);
        assert_eq!(contract.one_way_toffoli_target, 958_336);
        assert!(contract.requirements.contains(&"complete_google_abi_pa"));
        assert!(contract
            .requirements
            .contains(&"quotient_product_workspace_zero_exit"));
        assert!(contract
            .requirements
            .contains(&"deterministic_9024_google_pa_fuzz"));
        assert_eq!(
            contract.missing_object,
            ROUND414_SOURCE_LIVE_D1_MISSING_OBJECT
        );
    }

    #[test]
    fn round414_source_live_d1_body_plan_fails_closed_before_gates() {
        let plan = round414_source_live_d1_body_plan();
        assert_eq!(
            plan.classification,
            ROUND414_SOURCE_LIVE_D1_BACKEND_CONTRACT_CLASSIFICATION
        );
        assert_eq!(
            plan.selected_target,
            "round392_source_live_d1_branch_split_gate"
        );
        assert_eq!(plan.target_qubits, 1_315);
        assert_eq!(plan.target_toffoli, 2_068_377);
        assert_eq!(plan.target_qt, 2_719_915_755);
        assert_eq!(plan.one_way_toffoli_target, 958_336);
        assert_eq!(plan.phase_blocks.len(), 6);
        assert!(!plan.body_emits_gates);
        assert!(!plan.codegen_allowed_now);
        let budget_sum: usize = plan
            .phase_blocks
            .iter()
            .map(|block| block.toffoli_budget)
            .sum();
        assert_eq!(budget_sum, plan.one_way_toffoli_target);
        assert!(plan
            .phase_blocks
            .iter()
            .any(|block| block.ir_node == "round414_workspace_uncompute"));
        assert!(plan
            .phase_blocks
            .iter()
            .any(|block| block.phase_debt.contains("promotion forbidden")));
        assert!(plan
            .phase_blocks
            .iter()
            .all(|block| !block.backend_primitive.contains("qtail/Round217")));
    }

    #[test]
    fn round414_source_live_d1_lowerer_refuses_unowned_emission() {
        let mut b = B::new();
        let h = b.alloc_qubits(N);
        let n = b.alloc_qubits(N);
        let before_ops = b.ops.len();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            emit_round414_source_live_d1_quotient_product_lowerer(&mut b, &h, &n, SECP256K1_P);
        }))
        .expect_err("Round414 source-live D1 lowerer must fail closed");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .expect("panic has message");
        assert!(message.contains("intentionally fail-closed"));
        assert!(message.contains("target_Q=1315"));
        assert!(message.contains("target_T=2068377"));
        assert!(message.contains("9024 Google"));
        assert_eq!(
            b.phase,
            "round414_source_live_d1_quotient_product_lowerer_fail_closed"
        );
        assert_eq!(b.ops.len(), before_ops);
    }

    #[test]
    fn round415_existing_d1_inplace_lowerer_is_rejected_for_source_live_contract() {
        let audit = round415_d1_inplace_lowerer_audit();
        assert_eq!(
            audit.classification,
            ROUND415_D1_INPLACE_LOWERER_AUDIT_CLASSIFICATION
        );
        assert_eq!(audit.product_qubits, 2_475);
        assert_eq!(audit.product_toffoli, 1_919_786);
        assert_eq!(audit.product_ops, 14_234_801);
        assert_eq!(audit.product_bits, 1_141_762);
        assert_eq!(audit.product_phase_rows, 6_055);
        assert_eq!(audit.quotient_qubits, 2_475);
        assert_eq!(audit.quotient_toffoli, 1_919_786);
        assert_eq!(audit.quotient_ops, 10_594_364);
        assert_eq!(audit.quotient_bits, 0);
        assert_eq!(audit.one_way_toffoli_target, 958_336);
        assert_eq!(audit.one_way_toffoli_over_target, 961_450);
        assert_eq!(audit.qubits_over_target, 1_160);
        assert_eq!(audit.projected_pa_qubits, 2_475);
        assert_eq!(audit.projected_pa_toffoli, 3_029_827);
        assert_eq!(audit.projected_pa_qt, 7_498_821_825);
        assert_eq!(audit.projected_pa_toffoli_over_local_target, 29_827);
        assert!(audit.projected_pa_product_m1);
        assert!(audit.projected_pa_product_m2);
        assert_eq!(audit.projected_pa_product_slack_m1, 1_501_178_175);
        assert_eq!(audit.projected_pa_product_slack_m2, 501_178_175);
        assert!(audit.product_has_measurement_debt);
        assert!(!audit.contract_compatible);
    }

    #[test]
    fn round218_b5_source_live_pa_assembly_is_pinned() {
        let row = round218_b5_source_live_transport_pa_assembly();
        println!(
            "METRIC round218_b5_source_live_pa_q={}",
            row.assembled_qubits
        );
        println!(
            "METRIC round218_b5_source_live_pa_t={}",
            row.assembled_toffoli
        );
        println!("METRIC round218_b5_source_live_pa_qt={}", row.assembled_qt);
        println!(
            "METRIC round218_b5_source_live_pa_delta_t={}",
            row.toffoli_delta
        );
        println!(
            "METRIC round218_b5_source_live_pa_delta_q={}",
            row.qubit_delta
        );
        assert_eq!(
            row.classification,
            ROUND218_B5_SOURCE_LIVE_PA_CLASSIFICATION
        );
        assert!(!row.materialized_kmx);
        assert_eq!(row.block_bits, 5);
        assert_eq!(row.state_a_old_toffoli + row.state_b_old_toffoli, 3_812_633);
        assert_eq!(row.replacement_total_toffoli, 2_046_578);
        assert_eq!(row.assembled_qubits, 1_562);
        assert_eq!(row.assembled_toffoli, 2_203_351);
        assert_eq!(row.assembled_qt, 3_441_634_262);
        assert_eq!(row.toffoli_delta, -1_766_055);
        assert_eq!(row.qubit_delta, -1_175);
        assert!(row.assembled_qubits < LOCAL_PA_Q_TARGET);
        assert!(row.assembled_toffoli < LOCAL_PA_T_TARGET);
    }

    #[test]
    fn round218_b5_compact_history_resource_gate_is_dead() {
        let report = round218_b5_compact_history_resource_gate();
        println!(
            "METRIC round218_b5_compact_history_full_q={}",
            report.full_history_qubits
        );
        println!(
            "METRIC round218_b5_compact_history_full_t={}",
            report.full_history_toffoli
        );
        println!(
            "METRIC round218_b5_compact_history_full_qt={}",
            report.full_history_qt
        );
        println!(
            "METRIC round218_b5_compact_history_optimal_q={}",
            report.optimal_separator_qubits
        );
        println!(
            "METRIC round218_b5_compact_history_optimal_t={}",
            report.optimal_separator_toffoli
        );
        println!(
            "METRIC round218_b5_compact_history_optimal_qt={}",
            report.optimal_separator_qt
        );
        println!(
            "METRIC round218_b5_compact_history_zeta_old_q={}",
            report.zeta_old_separator_qubits
        );
        println!(
            "METRIC round218_b5_compact_history_zeta_old_t={}",
            report.zeta_old_separator_toffoli
        );
        println!(
            "METRIC round218_b5_compact_history_zeta_old_qt={}",
            report.zeta_old_separator_qt
        );
        assert_eq!(
            report.classification,
            ROUND218_B5_COMPACT_HISTORY_CLASSIFICATION
        );
        assert_eq!(report.full_history_qubits, 3_912);
        assert_eq!(report.full_history_toffoli, 10_628_797);
        assert_eq!(report.full_history_qt, 41_579_853_864);
        assert_eq!(report.raw_control_history_bits, 1_180);
        assert_eq!(report.separator_lower_bound_history_bits, 826);
        assert_eq!(report.zeta_old_separator_history_bits, 944);
        assert_eq!(report.optimal_qubit_saving, 354);
        assert_eq!(report.zeta_old_qubit_saving, 236);
        assert_eq!(report.optimal_separator_qubits, 3_558);
        assert_eq!(report.optimal_separator_qt, 37_817_259_726);
        assert_eq!(report.zeta_old_separator_qubits, 3_676);
        assert_eq!(report.zeta_old_separator_qt, 39_071_457_772);
        assert!(report.optimal_toffoli_over_product_limit > 8_000_000);
        assert!(report.zeta_old_toffoli_over_product_limit > 8_000_000);
        assert!(!report.makes_first_milestone);
    }

    #[test]
    fn round251_qtail_second_inverse_budget_gate_is_dead() {
        let report = round251_qtail_second_inverse_budget_gate();
        println!("METRIC round251_qtail_lowq_pa_q={}", report.lowq_pa_qubits);
        println!("METRIC round251_qtail_lowq_pa_t={}", report.lowq_pa_toffoli);
        println!("METRIC round251_qtail_lowq_pa_qt={}", report.lowq_pa_qt);
        println!(
            "METRIC round251_qtail_product_t_limit_at_lowq={}",
            report.product_toffoli_limit_at_lowq
        );
        println!(
            "METRIC round251_qtail_required_t_saving_for_product={}",
            report.required_toffoli_saving_for_product
        );
        println!(
            "METRIC round251_qtail_max_replacement_t_for_product={}",
            report.max_second_inverse_replacement_toffoli_for_product
        );
        println!(
            "METRIC round251_qtail_arithmetic_only_floor_t={}",
            report.arithmetic_only_second_inverse_floor
        );
        println!(
            "METRIC round251_qtail_arithmetic_only_pa_qt={}",
            report.arithmetic_only_pa_qt
        );
        assert_eq!(
            report.classification,
            ROUND251_QTAIL_SECOND_INVERSE_CLASSIFICATION
        );
        assert_eq!(report.lowq_pa_qubits, 2_454);
        assert_eq!(report.lowq_pa_toffoli, 4_025_998);
        assert_eq!(report.lowq_pa_qt, 9_879_799_092);
        assert_eq!(report.current_second_inverse_toffoli, 1_919_786);
        assert_eq!(report.product_toffoli_limit_at_lowq, 3_667_481);
        assert_eq!(report.required_toffoli_saving_for_product, 358_517);
        assert_eq!(
            report.max_second_inverse_replacement_toffoli_for_product,
            1_561_269
        );
        assert_eq!(report.strict_t_toffoli_limit, 2_999_999);
        assert_eq!(report.required_toffoli_saving_for_strict_t, 1_025_999);
        assert_eq!(
            report.max_second_inverse_replacement_toffoli_for_strict_t,
            893_787
        );
        assert_eq!(report.arithmetic_only_saving_ceiling, 252_145);
        assert_eq!(report.arithmetic_only_second_inverse_floor, 1_667_641);
        assert_eq!(report.arithmetic_only_pa_toffoli, 3_773_853);
        assert_eq!(report.arithmetic_only_pa_qt, 9_261_035_262);
        assert_eq!(report.arithmetic_only_toffoli_over_product_limit, 106_372);
        assert!(!report.arithmetic_only_makes_first_milestone);
    }

    #[test]
    fn round257_source_live_cubic_phase_budget_gate_is_dead() {
        let report = round257_source_live_cubic_phase_budget_gate();

        println!(
            "METRIC round257_source_live_cubic_phase_classification={}",
            report.classification
        );
        println!("METRIC round257_dirty_cubic_q={}", report.dirty_qubits);
        println!("METRIC round257_dirty_cubic_t={}", report.dirty_toffoli);
        println!("METRIC round257_dirty_cubic_qt={}", report.dirty_qt);
        println!(
            "METRIC round257_product_t_limit_at_dirty_q={}",
            report.product_toffoli_limit_at_dirty_q
        );
        println!(
            "METRIC round257_dirty_product_slack={}",
            report.dirty_product_slack
        );
        println!(
            "METRIC round257_dirty_t_over_strict_t={}",
            report.dirty_toffoli_over_strict_t
        );
        println!(
            "METRIC round257_max_product_phase_repair_net_t={}",
            report.max_phase_repair_net_toffoli_for_first_milestone
        );
        println!(
            "METRIC round257_clean_product_t={}",
            report.clean_product_toffoli
        );
        println!(
            "METRIC round257_clean_product_delta_t={}",
            report.clean_product_delta_toffoli
        );
        println!(
            "METRIC round257_clean_lambda_t={}",
            report.clean_lambda_toffoli
        );
        println!(
            "METRIC round257_clean_lambda_delta_t={}",
            report.clean_lambda_delta_toffoli
        );

        assert_eq!(
            report.classification,
            ROUND257_SOURCE_LIVE_CUBIC_PHASE_CLASSIFICATION
        );
        assert_eq!(report.dirty_qubits, 2_713);
        assert_eq!(report.dirty_toffoli, 3_203_496);
        assert_eq!(report.dirty_qt, 8_691_084_648);
        assert_eq!(report.product_toffoli_limit_at_dirty_q, 3_317_360);
        assert_eq!(report.dirty_product_slack, 113_864);
        assert_eq!(report.dirty_toffoli_over_strict_t, 203_497);
        assert_eq!(
            report.max_phase_repair_net_toffoli_for_first_milestone,
            113_864
        );
        assert_eq!(report.clean_product_qubits, 2_713);
        assert_eq!(report.clean_product_toffoli, 4_662_307);
        assert_eq!(report.clean_product_qt, 12_648_838_891);
        assert_eq!(report.clean_product_delta_toffoli, 1_458_811);
        assert_eq!(report.clean_product_toffoli_over_strict_t, 1_662_308);
        assert_eq!(report.clean_product_toffoli_over_product_limit, 1_344_947);
        assert_eq!(report.clean_product_qt_slack, -3_648_838_891);
        assert_eq!(report.clean_lambda_qubits, 2_713);
        assert_eq!(report.clean_lambda_toffoli, 5_016_865);
        assert_eq!(report.clean_lambda_qt, 13_610_754_745);
        assert_eq!(report.clean_lambda_delta_toffoli, 1_813_369);
        assert_eq!(report.clean_lambda_toffoli_over_strict_t, 2_016_866);
        assert_eq!(report.clean_lambda_toffoli_over_product_limit, 1_699_505);
        assert_eq!(report.clean_lambda_qt_slack, -4_610_754_745);
        assert!(report.dirty_row_makes_first_milestone);
        assert!(!report.clean_product_makes_first_milestone);
        assert!(!report.clean_lambda_makes_first_milestone);
    }

    #[test]
    fn round258_source_live_product_replacement_budget_is_pinned() {
        let report = round258_source_live_product_replacement_budget_gate();

        println!(
            "METRIC round258_source_live_product_replacement_classification={}",
            report.classification
        );
        println!(
            "METRIC round258_clean_product_non_product_t={}",
            report.non_product_tail_toffoli
        );
        println!(
            "METRIC round258_current_inplace_product_t={}",
            report.current_inplace_product_toffoli
        );
        println!(
            "METRIC round258_max_replacement_t_for_strict_t={}",
            report.max_replacement_toffoli_for_strict_t
        );
        println!(
            "METRIC round258_max_replacement_t_for_product={}",
            report.max_replacement_toffoli_for_product
        );
        println!(
            "METRIC round258_required_current_product_saving_for_strict_t={}",
            report.required_current_product_saving_for_strict_t
        );
        println!(
            "METRIC round258_ideal_zero_product_t={}",
            report.ideal_zero_product_toffoli
        );
        println!(
            "METRIC round258_ideal_zero_product_qt={}",
            report.ideal_zero_product_qt
        );

        assert_eq!(
            report.classification,
            ROUND258_SOURCE_LIVE_PRODUCT_REPLACEMENT_CLASSIFICATION
        );
        assert_eq!(report.clean_product_qubits, 2_713);
        assert_eq!(report.clean_product_toffoli, 4_662_307);
        assert_eq!(report.clean_product_qt, 12_648_838_891);
        assert_eq!(report.current_inplace_product_toffoli, 1_919_786);
        assert_eq!(report.non_product_tail_toffoli, 2_742_521);
        assert_eq!(report.strict_t_toffoli_limit, 2_999_999);
        assert_eq!(report.product_toffoli_limit_at_clean_product_q, 3_317_360);
        assert_eq!(report.max_replacement_toffoli_for_strict_t, 257_478);
        assert_eq!(report.max_replacement_toffoli_for_product, 574_839);
        assert_eq!(
            report.required_current_product_saving_for_strict_t,
            1_662_308
        );
        assert_eq!(
            report.required_current_product_saving_for_product,
            1_344_947
        );
        assert_eq!(report.ideal_zero_product_toffoli, 2_742_521);
        assert_eq!(report.ideal_zero_product_qt, 7_440_459_473);
        assert!(report.ideal_zero_product_makes_first_milestone);
        assert_eq!(report.round217_one_way_transport_toffoli, 1_023_289);
        assert_eq!(
            report.round217_replacement_toffoli_over_strict_t_budget,
            765_811
        );
        assert_eq!(
            report.round217_replacement_toffoli_over_product_budget,
            448_450
        );
        assert_eq!(report.d1_product_toffoli_over_strict_t_budget, 1_662_308);
        assert_eq!(report.d1_product_toffoli_over_product_budget, 1_344_947);
        assert!(!report.current_clean_product_makes_first_milestone);
    }

    #[test]
    fn round398_qtail_round217_product_budget_is_pinned() {
        let report = round398_qtail_round217_product_budget_gate();

        println!(
            "METRIC round398_qtail_round217_product_budget_classification={}",
            report.classification
        );
        println!(
            "METRIC round398_qtail_non_product_t={}",
            report.non_product_toffoli
        );
        println!(
            "METRIC round398_m2_replacement_limit={}",
            report.m2_replacement_toffoli_limit
        );
        println!(
            "METRIC round398_m3_replacement_limit={}",
            report.m3_replacement_toffoli_limit
        );
        println!(
            "METRIC round398_round217_projected_pa_t={}",
            report.round217_projected_pa_toffoli
        );
        println!(
            "METRIC round398_round217_projected_pa_qt={}",
            report.round217_projected_pa_qt
        );
        println!(
            "METRIC round398_forbidden_named_product_phase_t={}",
            report.forbidden_named_product_phase_toffoli
        );

        assert_eq!(
            report.classification,
            ROUND398_QTAIL_ROUND217_PRODUCT_BUDGET_CLASSIFICATION
        );
        assert_eq!(report.qtail_qubits, 2_453);
        assert_eq!(report.qtail_current_toffoli, 4_025_998);
        assert_eq!(report.current_product_toffoli, 1_919_786);
        assert_eq!(report.non_product_toffoli, 2_106_212);
        assert_eq!(report.m1_pa_toffoli_limit, 3_668_976);
        assert_eq!(report.m2_pa_toffoli_limit, 3_261_312);
        assert_eq!(report.m3_pa_toffoli_limit, 2_853_648);
        assert_eq!(report.m4_pa_toffoli_limit, 2_445_984);
        assert_eq!(report.m5_pa_toffoli_limit, 2_038_320);
        assert_eq!(report.m1_replacement_toffoli_limit, 1_562_764);
        assert_eq!(report.m2_replacement_toffoli_limit, 1_155_100);
        assert_eq!(report.m3_replacement_toffoli_limit, 747_436);
        assert_eq!(report.m4_replacement_toffoli_limit, 339_772);
        assert_eq!(report.m5_replacement_toffoli_limit, -67_892);
        assert_eq!(report.round217_one_way_toffoli, 1_023_289);
        assert_eq!(report.round217_projected_pa_toffoli, 3_129_501);
        assert_eq!(report.round217_projected_pa_qt, 7_676_665_953);
        assert!(report.round217_clears_m1);
        assert!(report.round217_clears_m2);
        assert!(!report.round217_clears_m3);
        assert_eq!(report.round217_m2_replacement_slack, 131_811);
        assert_eq!(report.round217_m3_replacement_over, 275_853);
        assert_eq!(report.forbidden_probe_qubits, 3_912);
        assert_eq!(report.forbidden_probe_toffoli, 9_843_898);
        assert_eq!(report.forbidden_probe_qt, 38_509_328_976);
        assert_eq!(report.forbidden_named_product_phase_toffoli, 5_824_302);
        assert_eq!(
            report.forbidden_dominant_inverse_transport_toffoli,
            5_085_562
        );
        assert_eq!(report.forbidden_endpoint_replay_toffoli, 588_290);
        assert_eq!(report.forbidden_product_scale_toffoli, 150_450);
        assert_eq!(report.forbidden_full_source_history_bits, 1_180);
        assert_eq!(
            report.forbidden_named_product_phase_over_m2_replacement_limit,
            4_669_202
        );
        assert!(report.source_live_product_alias_must_fail_closed);
    }

    #[test]
    fn round259_hmr_overwrite_is_resource_passing_but_phase_blocked() {
        let report = round259_source_live_hmr_overwrite_gate();

        println!(
            "METRIC round259_source_live_hmr_overwrite_classification={}",
            report.classification
        );
        println!(
            "METRIC round259_hmr_overwrite_t={}",
            report.hmr_overwrite_toffoli
        );
        println!(
            "METRIC round259_hmr_overwrite_qt={}",
            report.hmr_overwrite_qt
        );
        println!("METRIC round259_strict_t_slack={}", report.strict_t_slack);
        println!("METRIC round259_qt_slack={}", report.qt_slack);
        println!(
            "METRIC round259_direct_case_phase={}",
            report.direct_case_phase
        );
        println!(
            "METRIC round259_toy_quotient_phase_degree={}",
            report.toy_quotient_phase_degree
        );
        println!(
            "METRIC round259_toy_quotient_phase_density={}",
            report.toy_quotient_phase_density
        );

        assert_eq!(
            report.classification,
            ROUND259_SOURCE_LIVE_HMR_OVERWRITE_CLASSIFICATION
        );
        assert_eq!(report.hmr_overwrite_qubits, 2_713);
        assert_eq!(report.hmr_overwrite_toffoli, 2_892_410);
        assert_eq!(report.hmr_overwrite_qt, 7_847_108_330);
        assert_eq!(report.strict_t_slack, 107_590);
        assert_eq!(report.qt_slack, 1_152_891_670);
        assert_eq!(report.measured_lam_bits, 256);
        assert!(report.resource_makes_first_milestone);
        assert_eq!(report.direct_case_phase, 1);
        assert!(!report.phase_clean);
        assert_eq!(report.toy_quotient_phase_degree, 15);
        assert_eq!(report.toy_quotient_phase_density, 32_518);
        assert_eq!(report.toy_quotient_phase_table, 65_536);
    }
}
