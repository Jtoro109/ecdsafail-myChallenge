//! Round218 B=5 source-live transport PA emission gate.
//!
//! The resource row is pinned in `round218_b5_program.rs`; this module is the
//! PA hook that refuses to emit a fake KMX until the source-live parser,
//! coefficient transport, and cleanup are all lowered as gates.

use alloy_primitives::U256;
use std::collections::HashSet;
use std::sync::OnceLock;

use crate::circuit::{BitId, Op, OperationType, QubitId, NO_BIT, NO_QUBIT};

use super::{
    mod_add_double_qb, mod_add_qb, mod_double_inplace_fast_with_dirty, mod_mul_add_qq,
    mod_mul_sub_qq, mod_neg_inplace_fast, mod_sub_qb, phase_b5_execution, phase_b5_state_selector,
    quantum_d1_data_source, B, N, SECP256K1_P,
};

pub const ROUND218_B5_SOURCE_LIVE_TRANSPORT_PA_ENV: &str = "ROUND218_B5_SOURCE_LIVE_TRANSPORT_PA";
pub const ROUND218_B5_FIXED_BLOCK_LOWERER_ENV: &str = "ROUND218_B5_FIXED_BLOCK_LOWERER";
pub const ROUND218_B5_LAZY_BLOCK_TRANSPORT_ENV: &str = "ROUND218_B5_LAZY_BLOCK_TRANSPORT";
pub const ROUND218_B5_SMALL_DIV32_ENV: &str = "ROUND218_B5_SMALL_DIV32";
pub const ROUND218_B5_SELECTED_ONE_DIV32_ENV: &str = "ROUND218_B5_SELECTED_ONE_DIV32";
pub const ROUND218_B5_FIXED_ZETA_ONE_DIV32_ENV: &str = "ROUND218_B5_FIXED_ZETA_ONE_DIV32";
pub const ROUND218_B5_NONCANONICAL_BRANCH_NEG_ENV: &str = "ROUND218_B5_NONCANONICAL_BRANCH_NEG";
pub const ROUND218_B5_PROJECTIVE_NONCANONICAL_BRANCH_NEG_DIAG_ENV: &str =
    "ROUND218_B5_PROJECTIVE_NONCANONICAL_BRANCH_NEG_DIAG";
pub const ROUND218_B5_TRANSPORT_CLASSIFICATION: &str =
    "ROUND218_B5_SOURCE_LIVE_TRANSPORT_PA_ASSEMBLED_RESOURCE_ROW";
pub const ROUND218_B5_TRANSPORT_PRIVACY_BLOCKER: &str =
    "materialized branch history and endpoint/product cleanup are forbidden for this gate";
pub const ROUND218_B5_TRANSPORT_MISSING_OBJECT: &str =
    "phase-clean parent source advance/cleanup without branch history, endpoint cleanup, hidden replay, or product tape";

pub const ROUND218_B5_EMISSION_REQUIREMENTS: [&str; 6] = [
    "source_live_projective_scalar_transport",
    "local_control_uncompute",
    "phase_clean_source_advance",
    "phase_clean_source_cleanup",
    "no_endpoint_or_product_cleanup",
    "google_pa_abi_fuzz_pass",
];

pub const ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_CLASSIFICATION: &str =
    "ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_FAILS_CLOSED";
pub const ROUND218_B5_ALLOW_HASH_HISTORY_PRODUCT_PROBE_ENV: &str =
    "ROUND218_B5_ALLOW_HASH_HISTORY_PRODUCT_PROBE";
pub const ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_MISSING_OBJECT: &str =
    "promotable no-history qtail/Round217 product splice body";
pub const ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_REQUIREMENTS: [&str; 6] = [
    "no_full_source_history_tape",
    "no_endpoint_replay",
    "no_hidden_product_tape",
    "zero_phase_and_scratch",
    "same_artifact_stats",
    "deterministic_9024_google_pa_fuzz",
];
pub const ROUND404_QTAIL_ROUND217_HPREFIX_ONE_WAY_TOFFOLI_BOUND: usize = 1_313_539;
pub const ROUND404_QTAIL_ROUND217_HPREFIX_PARSER_TOFFOLI_BOUND: usize = 77_290;
pub const ROUND404_QTAIL_ROUND217_HPREFIX_APPLICATION_TOFFOLI_BOUND: usize =
    ROUND404_QTAIL_ROUND217_HPREFIX_ONE_WAY_TOFFOLI_BOUND
        - ROUND404_QTAIL_ROUND217_HPREFIX_PARSER_TOFFOLI_BOUND;
pub const ROUND406_QTAIL_ROUND217_PRODUCT_M2_ONE_WAY_TOFFOLI_BOUND: usize =
    quantum_d1_data_source::ROUND217_SOURCE_LIVE_TRANSPORT_ONE_WAY_TOFFOLI_MAX;
pub const ROUND406_QTAIL_ROUND217_PRODUCT_M2_PARSER_TOFFOLI_BOUND: usize =
    quantum_d1_data_source::ROUND217_SOURCE_LIVE_TRANSPORT_PARSER_TOFFOLI;
pub const ROUND406_QTAIL_ROUND217_PRODUCT_M2_APPLICATION_TOFFOLI_BOUND: usize =
    quantum_d1_data_source::ROUND217_SOURCE_LIVE_TRANSPORT_MAX_APPLICATION_TOFFOLI;
pub const ROUND406_B5_SOURCE_LIVE_PRODUCT_LOWERER_BODY_PLAN_CLASSIFICATION: &str =
    "ROUND442_SOURCE_LIVE_PRODUCT_LOWERER_HASH_HISTORY_BODY_EMITS_GATES";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct B5TransportContract {
    pub classification: &'static str,
    pub target_classification: &'static str,
    pub target_qubits: usize,
    pub target_toffoli: usize,
    pub block_bits: usize,
    pub blocks: usize,
    pub requirements: [&'static str; 6],
    pub privacy_blocker: &'static str,
    pub missing_object: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct B5SourceLiveProductLowererContract {
    pub classification: &'static str,
    pub qtail_qubits: usize,
    pub non_product_toffoli: usize,
    pub current_product_toffoli: usize,
    pub m1_replacement_toffoli_limit: isize,
    pub m2_replacement_toffoli_limit: isize,
    pub m3_replacement_toffoli_limit: isize,
    pub proof_carrying_m1_one_way_toffoli: usize,
    pub proof_carrying_m1_projected_pa_toffoli: usize,
    pub proof_carrying_m1_projected_pa_qt: usize,
    pub sampled_m2_one_way_toffoli: usize,
    pub sampled_m2_projected_pa_toffoli: usize,
    pub sampled_m2_projected_pa_qt: usize,
    pub requirements: [&'static str; 6],
    pub missing_object: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct B5SourceLiveProductLowererPhaseBlockContract {
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
pub struct B5SourceLiveProductLowererBodyPlan {
    pub classification: &'static str,
    pub selected_route: &'static str,
    pub qtail_qubits: usize,
    pub one_way_toffoli_bound: usize,
    pub projected_pa_toffoli: usize,
    pub projected_pa_qt: usize,
    pub phase_blocks: &'static [B5SourceLiveProductLowererPhaseBlockContract],
    pub body_emits_gates: bool,
    pub codegen_allowed_now: bool,
    pub missing_object: &'static str,
}

pub const ROUND406_B5_SOURCE_LIVE_PRODUCT_LOWERER_BODY_PHASE_BLOCKS:
    [B5SourceLiveProductLowererPhaseBlockContract; 6] = [
    B5SourceLiveProductLowererPhaseBlockContract {
        ir_node: "round406_route_guard",
        phase: "round218_b5_source_live_stream_product_hash_history_enter",
        field_semantics: "(h,n) remain live; hash-history reversible arithmetic emitted",
        register_lifecycle: "h:live,n:live,hash_history:internal,scratch:zero_exit",
        valid_branch: "all Google-ABI qtail product calls",
        phase_debt: "not milestone-promotable until same-artifact stats and PA fuzz pass",
        toffoli_budget: 0,
        backend_primitive: "round218_b5_source_live_stream_product_hash_history_body",
    },
    B5SourceLiveProductLowererPhaseBlockContract {
        ir_node: "round406_source_parser",
        phase: "round406_source_live_product_parse_source_controls",
        field_semantics:
            "derive B=5 branch/old-g0 controls from live h without storing full history",
        register_lifecycle: "h:borrowed,n:live,source:live,controls:borrowed,scratch:zero",
        valid_branch: "h != 0; totalized PA inputs use fixed outside-surface branch",
        phase_debt: "controls must be uncomputed in the same block window",
        toffoli_budget: ROUND406_QTAIL_ROUND217_PRODUCT_M2_PARSER_TOFFOLI_BOUND,
        backend_primitive: "emit_round218_b5_source_stream_forward_block",
    },
    B5SourceLiveProductLowererPhaseBlockContract {
        ir_node: "round406_hprefix_selected_transport",
        phase: "round406_source_live_product_hprefix_selected_transport",
        field_semantics: "apply the Round217 sampled source-live product transport to n",
        register_lifecycle: "h:borrowed,n:live,controls:borrowed,scratch:borrowed",
        valid_branch: "9024 sampled qtail/Round217 source-live product path",
        phase_debt: "no hidden product tape; transport scratch must return zero",
        toffoli_budget: ROUND406_QTAIL_ROUND217_PRODUCT_M2_APPLICATION_TOFFOLI_BOUND,
        backend_primitive: "Round217 sampled source-live transport lowerer",
    },
    B5SourceLiveProductLowererPhaseBlockContract {
        ir_node: "round406_swap_product_into_target",
        phase: "round406_source_live_product_swap_product_into_target",
        field_semantics: "target register contains h*n after product transport",
        register_lifecycle: "h:borrowed,n:live,product:zero",
        valid_branch: "all source-live product calls after transport",
        phase_debt: "none; swaps are phase-neutral",
        toffoli_budget: 0,
        backend_primitive: "swap product into target",
    },
    B5SourceLiveProductLowererPhaseBlockContract {
        ir_node: "round406_clean_source_controls",
        phase: "round406_source_live_product_clean_controls_and_scratch",
        field_semantics: "erase branch/old-g0 controls without endpoint replay",
        register_lifecycle: "h:live,n:live,controls:zero,scratch:zero",
        valid_branch: "same h-prefix block window used for parse",
        phase_debt: "all parser/control phase debt cleared",
        toffoli_budget: 0,
        backend_primitive: "window-local parser inverse, not full-source history",
    },
    B5SourceLiveProductLowererPhaseBlockContract {
        ir_node: "round406_same_artifact_verifier_barrier",
        phase: "round406_source_live_product_same_artifact_verifier_barrier",
        field_semantics: "complete PA artifact must report exact Q/T/ops/SHA and 9024 fuzz",
        register_lifecycle: "h:live,n:live,source:zero,scratch:zero,measurements:zero",
        valid_branch: "complete secp256k1 Google-ABI PA",
        phase_debt: "promotion forbidden until deterministic verifier passes",
        toffoli_budget: 0,
        backend_primitive: "stats plus google_pa_exec_stats exact fuzz harness",
    },
];

pub fn round218_b5_transport_contract() -> B5TransportContract {
    let row = phase_b5_execution::ROUND218_B5_RESOURCE_ROW;
    B5TransportContract {
        classification: ROUND218_B5_TRANSPORT_CLASSIFICATION,
        target_classification: row.classification,
        target_qubits: row.qubits,
        target_toffoli: row.toffoli,
        block_bits: row.block_bits,
        blocks: row.blocks,
        requirements: ROUND218_B5_EMISSION_REQUIREMENTS,
        privacy_blocker: ROUND218_B5_TRANSPORT_PRIVACY_BLOCKER,
        missing_object: ROUND218_B5_TRANSPORT_MISSING_OBJECT,
    }
}

pub fn round218_b5_source_live_product_lowerer_contract() -> B5SourceLiveProductLowererContract {
    let budget = quantum_d1_data_source::round398_qtail_round217_product_budget_gate();
    let proof_projected_pa_toffoli =
        budget.non_product_toffoli + ROUND404_QTAIL_ROUND217_HPREFIX_ONE_WAY_TOFFOLI_BOUND;
    B5SourceLiveProductLowererContract {
        classification: ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_CLASSIFICATION,
        qtail_qubits: budget.qtail_qubits,
        non_product_toffoli: budget.non_product_toffoli,
        current_product_toffoli: budget.current_product_toffoli,
        m1_replacement_toffoli_limit: budget.m1_replacement_toffoli_limit,
        m2_replacement_toffoli_limit: budget.m2_replacement_toffoli_limit,
        m3_replacement_toffoli_limit: budget.m3_replacement_toffoli_limit,
        proof_carrying_m1_one_way_toffoli: ROUND404_QTAIL_ROUND217_HPREFIX_ONE_WAY_TOFFOLI_BOUND,
        proof_carrying_m1_projected_pa_toffoli: proof_projected_pa_toffoli,
        proof_carrying_m1_projected_pa_qt: budget.qtail_qubits * proof_projected_pa_toffoli,
        sampled_m2_one_way_toffoli: budget.round217_one_way_toffoli,
        sampled_m2_projected_pa_toffoli: budget.round217_projected_pa_toffoli,
        sampled_m2_projected_pa_qt: budget.round217_projected_pa_qt,
        requirements: ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_REQUIREMENTS,
        missing_object: ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_MISSING_OBJECT,
    }
}

pub fn round218_b5_source_live_product_lowerer_body_plan() -> B5SourceLiveProductLowererBodyPlan {
    let contract = round218_b5_source_live_product_lowerer_contract();
    B5SourceLiveProductLowererBodyPlan {
        classification: ROUND406_B5_SOURCE_LIVE_PRODUCT_LOWERER_BODY_PLAN_CLASSIFICATION,
        selected_route: "round217_sampled_product_m2_contract_path",
        qtail_qubits: contract.qtail_qubits,
        one_way_toffoli_bound: contract.sampled_m2_one_way_toffoli,
        projected_pa_toffoli: contract.sampled_m2_projected_pa_toffoli,
        projected_pa_qt: contract.sampled_m2_projected_pa_qt,
        phase_blocks: &ROUND406_B5_SOURCE_LIVE_PRODUCT_LOWERER_BODY_PHASE_BLOCKS,
        body_emits_gates: false,
        codegen_allowed_now: false,
        missing_object: ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_MISSING_OBJECT,
    }
}

fn round218_b5_hash_history_product_probe_enabled() -> bool {
    std::env::var(ROUND218_B5_ALLOW_HASH_HISTORY_PRODUCT_PROBE_ENV)
        .ok()
        .as_deref()
        == Some("1")
}

fn round695_forbidden_state_a_scalep_diag_enabled() -> bool {
    std::env::var("ROUND695_B5_FORBIDDEN_PAIR1_SCALEP_DIAG")
        .ok()
        .as_deref()
        == Some("1")
}

pub(super) fn emit_round218_b5_source_live_transport_pa_or_fail(
    b: &mut B,
    tx: &[QubitId],
    ty: &[QubitId],
    ox: &[BitId],
    oy: &[BitId],
    p: U256,
) {
    assert_eq!(p, SECP256K1_P, "Round218 B=5 route is secp256k1-only");
    assert_eq!(tx.len(), N, "round218 tx must be 256 qubits");
    assert_eq!(ty.len(), N, "round218 ty must be 256 qubits");
    assert_eq!(ox.len(), N, "round218 ox must be 256 bits");
    assert_eq!(oy.len(), N, "round218 oy must be 256 bits");
    if std::env::var("ROUND218_B5_ALLOW_FORBIDDEN_MATERIALIZED_PA")
        .ok()
        .as_deref()
        != Some("1")
    {
        panic!("{}", round218_b5_transport_blocker_message(tx, ty, ox, oy));
    }

    let source_steps = phase_b5_execution::ROUND218_B5_STEPS;
    let mut zeta = b.alloc_qubits(11);
    let mut f = b.alloc_qubits(source_steps);
    let mut g = b.alloc_qubits(source_steps);
    let mut branch_hist = b.alloc_qubits(source_steps);
    let mut old_g0_hist = b.alloc_qubits(source_steps);

    emit_round218_b5_source_low_state_parser(
        b,
        tx,
        &mut f,
        &mut g,
        &mut zeta,
        &mut branch_hist,
        &mut old_g0_hist,
        p,
    );

    b.set_phase("round218_b5_source_live_transport_state_a_quotient");
    let scaled_quotient = b.alloc_qubits(N);
    for block in 0..phase_b5_execution::ROUND218_B5_BLOCKS {
        let lo = block * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        let hi = lo + phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        emit_round218_b5_source_live_transport_block(
            b,
            &f[lo..hi],
            &g[lo..hi],
            &scaled_quotient,
            ty,
            -1,
            &branch_hist[lo..hi],
            &old_g0_hist[lo..hi],
            p,
        );
    }
    b.set_phase("round218_b5_source_live_transport_state_a_normalize");
    let final_f_sign = round218_b5_final_f_sign(&f);
    emit_round218_cmod_neg_canonical(b, &scaled_quotient, final_f_sign, p);
    b.set_phase("round218_b5_source_live_transport_state_a_unscale");
    for _ in 0..phase_b5_execution::ROUND218_B5_STEPS {
        super::mod_halve_inplace_fast(b, &scaled_quotient, p);
    }
    b.set_phase("round218_b5_source_live_transport_state_a_quotient_swap");
    for i in 0..N {
        b.swap(ty[i], scaled_quotient[i]);
    }
    emit_round218_b5_forward_quotient_endpoint_cleanup(b, tx, &scaled_quotient, ty, p);
    b.free_vec(&scaled_quotient);

    if round695_forbidden_state_a_scalep_diag_enabled() {
        b.set_phase("round695_b5_diag_source_cleanup_after_state_a");
        emit_round218_b5_source_low_state_parser_cleanup(
            b,
            tx,
            &f,
            &g,
            &zeta,
            &branch_hist,
            &old_g0_hist,
            p,
        );

        b.free_vec(&old_g0_hist);
        b.free_vec(&branch_hist);
        b.free_vec(&g);
        b.free_vec(&f);
        b.free_vec(&zeta);

        b.set_phase("round695_b5_diag_xtail");
        super::round84_emit_fused_square_xtail(b, tx, ty, ox, p);

        if std::env::var("ROUND691_SKIP_EQ_DERIVATIVE").ok().as_deref() == Some("1") {
            b.set_phase("round695_b5_diag_noeq_compute_d");
            let d = super::load_bits(b, ox);
            super::mod_sub_qq_fast(b, &d, tx, p);

            super::round691_emit_scale_p(b, &d, ty, p);

            b.set_phase("round695_b5_diag_noeq_y_sub_offset_y");
            mod_sub_qb(b, ty, oy, p);

            b.set_phase("round695_b5_diag_noeq_uncompute_d");
            super::mod_add_qq_fast(b, &d, tx, p);
            super::unload_bits(b, &d, ox);
            return;
        }

        b.set_phase("round695_b5_diag_eq_diff");
        let eq_diff = super::load_bits(b, ox);
        super::mod_sub_qq_fast(b, &eq_diff, tx, p);
        let eq = b.alloc_qubit();
        super::toggle_eq_zero_flag_fast(b, &eq_diff, eq);

        b.set_phase("round695_b5_diag_compute_polarized_d");
        let d = super::round564_compute_polarized_d(b, tx, ox, oy, eq, p);

        super::round691_emit_scale_p(b, &d, ty, p);

        b.set_phase("round695_b5_diag_y_sub_offset_y");
        mod_sub_qb(b, ty, oy, p);

        b.set_phase("round695_b5_diag_compute_derivative_square");
        let ox_q = super::load_bits(b, ox);
        let derivative = b.alloc_qubits(N);
        super::round564_square_add_selected(b, &derivative, &ox_q, p);
        b.set_phase("round695_b5_diag_sub_eq_derivative_x3");
        for _ in 0..3 {
            super::cmod_sub_qq(b, ty, &derivative, eq, p);
        }
        b.set_phase("round695_b5_diag_uncompute_derivative_square");
        super::round564_square_sub_selected(b, &derivative, &ox_q, p);
        b.free_vec(&derivative);
        super::unload_bits(b, &ox_q, ox);

        b.set_phase("round695_b5_diag_uncompute_polarized_d");
        super::round564_uncompute_polarized_d(b, &d, tx, ox, oy, eq, p);

        b.set_phase("round695_b5_diag_uncompute_eq");
        super::toggle_eq_zero_flag_fast(b, &eq_diff, eq);
        b.free(eq);
        super::mod_add_qq_fast(b, &eq_diff, tx, p);
        super::unload_bits(b, &eq_diff, ox);
        return;
    }

    b.set_phase("round218_b5_source_live_transport_state_b_product");
    let product = b.alloc_qubits(N);
    for _ in 0..source_steps {
        super::mod_double_inplace_fast(b, ty, p);
    }
    for block in 0..phase_b5_execution::ROUND218_B5_BLOCKS {
        let lo = block * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        let hi = lo + phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        emit_round218_b5_source_live_transport_block(
            b,
            &f[lo..hi],
            &g[lo..hi],
            ty,
            &product,
            -1,
            &branch_hist[lo..hi],
            &old_g0_hist[lo..hi],
            p,
        );
    }
    b.set_phase("round218_b5_source_live_transport_state_b_normalize");
    emit_round218_cmod_neg_canonical(b, ty, final_f_sign, p);
    b.set_phase("round218_b5_source_live_transport_state_b_product_swap");
    for i in 0..N {
        b.swap(ty[i], product[i]);
    }
    emit_round218_b5_reverse_product_endpoint_cleanup(b, tx, &product, ty, p);
    b.free_vec(&product);

    emit_round218_b5_source_low_state_parser_cleanup(
        b,
        tx,
        &f,
        &g,
        &zeta,
        &branch_hist,
        &old_g0_hist,
        p,
    );

    b.free_vec(&old_g0_hist);
    b.free_vec(&branch_hist);
    b.free_vec(&g);
    b.free_vec(&f);
    b.free_vec(&zeta);

    b.set_phase("round8_fallback_xtail_square");
    mod_mul_sub_qq(b, tx, ty, ty, p);
    b.set_phase("round8_fallback_xtail_add_2ox");
    mod_add_double_qb(b, tx, ox, p);
    b.set_phase("round8_fallback_xtail_to_rx");
    mod_neg_inplace_fast(b, tx, p);

    b.set_phase("round8_fallback_c_ox_minus_rx");
    mod_sub_qb(b, tx, ox, p);
    mod_neg_inplace_fast(b, tx, p);

    b.set_phase("round8_fallback_y_output");
    mod_sub_qb(b, ty, oy, p);
    b.set_phase("round8_fallback_x_restore");
    mod_neg_inplace_fast(b, tx, p);
    mod_add_qb(b, tx, ox, p);
}

pub(super) fn round218_b5_transport_blocker_message(
    tx: &[QubitId],
    ty: &[QubitId],
    ox: &[BitId],
    oy: &[BitId],
) -> String {
    let contract = round218_b5_transport_contract();
    format!(
        "{env}=1 refuses KMX emission: classification={class}; \
         target={target}; target_Q={q}; target_T={t}; wires tx={}, ty={}, ox={}, oy={}; \
         block_bits={}, blocks={}; required=[{}]; privacy_blocker={privacy}; \
         missing_object={missing}.",
        tx.len(),
        ty.len(),
        ox.len(),
        oy.len(),
        contract.block_bits,
        contract.blocks,
        contract.requirements.join(","),
        env = ROUND218_B5_SOURCE_LIVE_TRANSPORT_PA_ENV,
        class = contract.classification,
        target = contract.target_classification,
        q = contract.target_qubits,
        t = contract.target_toffoli,
        privacy = contract.privacy_blocker,
        missing = contract.missing_object,
    )
}

fn emit_round218_b5_quantum_zeta_update(b: &mut B, zeta: &[QubitId], branch: QubitId) {
    phase_b5_state_selector::emit_round218_b5_twos_zeta_update_step(b, zeta, branch);
}

fn emit_round218_b5_quantum_zeta_update_inverse(b: &mut B, zeta: &[QubitId], branch: QubitId) {
    phase_b5_state_selector::emit_round218_b5_twos_zeta_update_step_inverse(b, zeta, branch);
}

fn emit_round218_b5_controlled_scaled_coefficient_step(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    p: U256,
) {
    emit_round218_scaled_coeff_b5_block_selected(b, v, r, branch_word, old_g0_word, p);
}

fn emit_round218_b5_forward_quotient_endpoint_cleanup(
    b: &mut B,
    h: &[QubitId],
    displaced_target: &[QubitId],
    final_quotient: &[QubitId],
    p: U256,
) {
    assert_eq!(
        h.len(),
        N,
        "forward quotient endpoint cleanup denominator width must be 256"
    );
    assert_eq!(
        displaced_target.len(),
        N,
        "forward quotient endpoint cleanup numerator width must be 256"
    );
    assert_eq!(
        final_quotient.len(),
        N,
        "forward quotient endpoint cleanup quotient width must be 256"
    );
    assert_eq!(
        p, SECP256K1_P,
        "forward quotient endpoint cleanup is secp256k1-only"
    );
    b.set_phase("round218_b5_forward_quotient_endpoint_cleanup");
    b.set_phase("round218_b5_forward_quotient_endpoint_cleanup_mul_sub");
    // The full source-live quotient cleanup is intentionally the exact inverse of
    // the final swap: given target = denominator^{-1}·numerator and
    // displaced_target = numerator, this rewrites displaced_target to 0.
    mod_mul_sub_qq(b, displaced_target, h, final_quotient, p);
}

fn emit_round218_b5_reverse_product_endpoint_cleanup(
    b: &mut B,
    h: &[QubitId],
    displaced_target: &[QubitId],
    final_product: &[QubitId],
    p: U256,
) {
    assert_eq!(
        h.len(),
        N,
        "reverse product endpoint cleanup denominator width must be 256"
    );
    assert_eq!(
        displaced_target.len(),
        N,
        "reverse product endpoint cleanup numerator width must be 256"
    );
    assert_eq!(
        final_product.len(),
        N,
        "reverse product endpoint cleanup product width must be 256"
    );
    assert_eq!(
        p, SECP256K1_P,
        "reverse product endpoint cleanup is secp256k1-only"
    );
    b.set_phase("round218_b5_reverse_product_endpoint_cleanup");
    b.set_phase("round218_b5_reverse_product_endpoint_cleanup_inverse");
    let inverse_iters =
        super::eea_effective_iters(h.len(), phase_b5_execution::ROUND218_B5_STEPS);
    let mut dirty: Vec<QubitId> = h.to_vec();
    dirty.extend_from_slice(final_product);
    super::with_kal_inv_raw_borrowing_v(b, h, p, inverse_iters, |b, inv_raw| {
        // The full source-live product cleanup uses the inverse route to restore
        // the borrowed product output and zero out the displaced source slot.
        for _ in 0..inverse_iters {
            mod_double_inplace_fast_with_dirty(b, displaced_target, p, Some(&dirty));
        }
        mod_mul_add_qq(b, displaced_target, inv_raw, final_product, p);
    });
}

fn emit_round218_b5_source_low_state_parser(
    b: &mut B,
    dx: &[QubitId],
    f: &mut [QubitId],
    g: &mut [QubitId],
    zeta: &mut [QubitId],
    branch_hist: &mut [QubitId],
    old_g0_hist: &mut [QubitId],
    p: U256,
) {
    assert_eq!(
        dx.len(),
        N,
        "source-live parser denominator must be 256 qubits"
    );
    assert_eq!(
        f.len(),
        phase_b5_execution::ROUND218_B5_STEPS,
        "source-live parser work register width"
    );
    assert_eq!(g.len(), f.len(), "source-live parser state register width");
    assert_eq!(zeta.len(), 11, "source-live parser zeta width is 11");
    assert_eq!(p, SECP256K1_P, "source-live parser is secp256k1-only");

    let expected_hist =
        phase_b5_execution::ROUND218_B5_BLOCK_BITS * phase_b5_execution::ROUND218_B5_BLOCKS;
    assert_eq!(
        branch_hist.len(),
        expected_hist,
        "source-live parser must retain all forward-branch bits"
    );
    assert_eq!(
        old_g0_hist.len(),
        expected_hist,
        "source-live parser must retain all forward old-g0 bits"
    );

    for &q in zeta.iter() {
        b.x(q);
    }
    for i in 0..N {
        if p.bit(i) {
            b.x(f[i]);
        }
        b.cx(dx[i], g[i]);
    }

    for block in 0..phase_b5_execution::ROUND218_B5_BLOCKS {
        let lo = block * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        let hi = lo + phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        b.set_phase("round218_b5_source_live_transport_source_block_parse");
        for j in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
            let step = lo + j;
            let width = f.len() - step;
            b.cx(g[0], old_g0_hist[step]);
            let sign = zeta[zeta.len() - 1];
            b.ccx(sign, g[0], branch_hist[step]);
            phase_b5_state_selector::emit_round218_b5_low_window_apply_step(
                b,
                f,
                g,
                width,
                branch_hist[step],
                old_g0_hist[step],
            );
            emit_round218_b5_quantum_zeta_update(b, zeta, branch_hist[step]);
        }
    }
}

fn emit_round218_b5_source_low_state_parser_cleanup(
    b: &mut B,
    dx: &[QubitId],
    f: &[QubitId],
    g: &[QubitId],
    zeta: &[QubitId],
    branch_hist: &[QubitId],
    old_g0_hist: &[QubitId],
    p: U256,
) {
    assert_eq!(
        dx.len(),
        N,
        "source-live parser cleanup denominator must be 256 qubits"
    );
    assert_eq!(
        f.len(),
        phase_b5_execution::ROUND218_B5_STEPS,
        "source-live parser cleanup width"
    );
    assert_eq!(g.len(), f.len(), "source-live parser cleanup state width");
    assert_eq!(zeta.len(), 11, "source-live parser zeta width is 11");
    assert_eq!(
        p, SECP256K1_P,
        "source-live parser cleanup is secp256k1-only"
    );

    let expected_hist =
        phase_b5_execution::ROUND218_B5_BLOCK_BITS * phase_b5_execution::ROUND218_B5_BLOCKS;
    assert_eq!(
        branch_hist.len(),
        expected_hist,
        "source-live parser must retain all forward-branch bits"
    );
    assert_eq!(
        old_g0_hist.len(),
        expected_hist,
        "source-live parser must retain all forward old-g0 bits"
    );

    for block in (0..phase_b5_execution::ROUND218_B5_BLOCKS).rev() {
        let lo = block * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        let hi = lo + phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        b.set_phase("round218_b5_source_live_transport_source_block_unparse");
        for j in (0..phase_b5_execution::ROUND218_B5_BLOCK_BITS).rev() {
            let step = lo + j;
            let width = f.len() - step;
            emit_round218_b5_quantum_zeta_update_inverse(b, zeta, branch_hist[step]);
            phase_b5_state_selector::emit_round218_b5_low_window_unapply_step(
                b,
                f,
                g,
                width,
                branch_hist[step],
                old_g0_hist[step],
            );
            let sign = zeta[zeta.len() - 1];
            b.ccx(sign, g[0], branch_hist[step]);
            b.cx(g[0], old_g0_hist[step]);
        }
    }

    for i in (0..N).rev() {
        b.cx(dx[i], g[i]);
        if p.bit(i) {
            b.x(f[i]);
        }
    }
    for &q in zeta.iter().rev() {
        b.x(q);
    }
}

pub(super) fn emit_round218_b5_source_live_transport_block(
    b: &mut B,
    f_low: &[QubitId],
    g_low: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    zeta_start: i128,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 source-live block is secp256k1-only"
    );
    assert_eq!(
        f_low.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "source-live f_low must be a B=5 window"
    );
    assert_eq!(
        g_low.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "source-live g_low must be a B=5 window"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "branch word must be a B=5 word"
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "old_g0 word must be a B=5 word"
    );

    b.set_phase("round218_b5_source_live_block_select");
    phase_b5_state_selector::emit_round218_b5_low_state_selector(
        b,
        f_low,
        g_low,
        zeta_start,
        branch_word,
        old_g0_word,
    );

    b.set_phase("round218_b5_source_live_block_transport");
    if round218_b5_fixed_zeta_one_div32_enabled() {
        emit_round218_scaled_coeff_b5_block_fixed_zeta_lulu_one_div32(
            b,
            v,
            r,
            old_g0_word,
            zeta_start,
            p,
        );
    } else {
        emit_round218_scaled_coeff_b5_block_selected(b, v, r, branch_word, old_g0_word, p);
    }
}

pub(super) fn emit_round218_b5_source_window_transport_block(
    b: &mut B,
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    zeta_start: i128,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    next_f_low: &[QubitId],
    next_g_low: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 source-window block is secp256k1-only"
    );
    assert_eq!(
        f_window.len(),
        phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS,
        "source-window f must be a 2B=10-bit window"
    );
    assert_eq!(
        g_window.len(),
        phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS,
        "source-window g must be a 2B=10-bit window"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "branch word must be a B=5 word"
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "old_g0 word must be a B=5 word"
    );
    assert_eq!(
        next_f_low.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "next f low word must be a B=5 word"
    );
    assert_eq!(
        next_g_low.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "next g low word must be a B=5 word"
    );

    b.set_phase("round218_b5_source_window_block_parse");
    phase_b5_state_selector::emit_round218_b5_low_window_parser(
        b,
        f_window,
        g_window,
        zeta_start,
        branch_word,
        old_g0_word,
        next_f_low,
        next_g_low,
    );

    b.set_phase("round218_b5_source_window_block_transport");
    if round218_b5_fixed_zeta_one_div32_enabled() {
        emit_round218_scaled_coeff_b5_block_fixed_zeta_lulu_one_div32(
            b,
            v,
            r,
            old_g0_word,
            zeta_start,
            p,
        );
    } else {
        emit_round218_scaled_coeff_b5_block_selected(b, v, r, branch_word, old_g0_word, p);
    }
}

pub(super) fn emit_round218_b5_dynamic_source_window_transport_block(
    b: &mut B,
    spec: phase_b5_state_selector::Round218B5DynamicZetaTransducerSpec,
    zeta_start: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    end_zeta: &[QubitId],
    next_f: &[QubitId],
    next_g: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 dynamic source-window block is secp256k1-only"
    );
    assert_eq!(zeta_start.len(), spec.start_zeta_bits());
    assert!(
        f_window.len() >= phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "dynamic source-window block needs at least B=5 source bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "branch word must be a B=5 word"
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "old_g0 word must be a B=5 word"
    );
    assert_eq!(end_zeta.len(), spec.end_zeta_bits());
    assert_eq!(
        next_f.len(),
        f_window.len() - phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "next f word must retain window_bits-B bits"
    );
    assert_eq!(next_g.len(), next_f.len());

    b.set_phase("round218_b5_dynamic_source_window_block_parse");
    phase_b5_state_selector::emit_round218_b5_dynamic_window_parser(
        b,
        spec,
        zeta_start,
        f_window,
        g_window,
        branch_word,
        old_g0_word,
        end_zeta,
        next_f,
        next_g,
    );

    b.set_phase("round218_b5_dynamic_source_window_block_transport");
    emit_round218_scaled_coeff_b5_block_selected(b, v, r, branch_word, old_g0_word, p);
}

pub(super) fn emit_round218_b5_twos_zeta_source_window_transport_block(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    next_f: &[QubitId],
    next_g: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 two's-complement source-window block is secp256k1-only"
    );
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta register needs at least 3 signed bits"
    );
    assert!(
        f_window.len() >= phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "two's-complement source-window block needs at least B=5 source bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "branch word must be a B=5 word"
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "old_g0 word must be a B=5 word"
    );
    assert_eq!(
        next_f.len(),
        f_window.len() - phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "next f word must retain window_bits-B bits"
    );
    assert_eq!(next_g.len(), next_f.len());

    b.set_phase("round218_b5_twos_zeta_source_window_block_parse");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_window_parser(
        b,
        zeta,
        f_window,
        g_window,
        branch_word,
        old_g0_word,
        next_f,
        next_g,
    );

    b.set_phase("round218_b5_twos_zeta_source_window_block_transport");
    emit_round218_scaled_coeff_b5_block_selected_with_branch_neg(
        b,
        v,
        r,
        branch_word,
        old_g0_word,
        p,
        false,
    );
}

const ROUND314_B5_HASH_BITS: usize = 8;
const ROUND314_B5_CONTROL_BITS: usize = 2 * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
const ROUND314_B5_HASH_MASKS: [u16; ROUND314_B5_HASH_BITS] = [513, 258, 128, 64, 32, 16, 8, 7];
const ROUND314_B5_KERNEL_A: u16 = 262;
const ROUND314_B5_KERNEL_B: u16 = 517;
const ROUND314_B5_KERNELS_BY_SELECTOR_MASK: [u16; 4] = [
    0,
    ROUND314_B5_KERNEL_A,
    ROUND314_B5_KERNEL_B,
    ROUND314_B5_KERNEL_A ^ ROUND314_B5_KERNEL_B,
];

#[derive(Clone, Copy, Debug)]
struct Round326B5RankTerm {
    region: usize,
    output: usize,
    mask: u16,
    fixed: u16,
}

const ROUND326_B5_LIVE_L_REGIONS: [(i128, i128); 17] = [
    (-584, -584),
    (-583, -583),
    (-582, -582),
    (-581, -581),
    (-580, -580),
    (-579, -579),
    (-578, -578),
    (-577, -577),
    (-576, -5),
    (-4, -4),
    (-3, -3),
    (-2, -2),
    (-1, -1),
    (0, 0),
    (1, 1),
    (2, 2),
    (3, 3),
];

const ROUND326_B5_BRANCH_REGIONS: [(i128, i128); 14] = [
    (-5, -5),
    (-4, -4),
    (-3, -3),
    (-2, -2),
    (-1, -1),
    (0, 0),
    (1, 1),
    (2, 2),
    (3, 578),
    (579, 580),
    (581, 582),
    (583, 584),
    (585, 586),
    (587, 588),
];

include!("round326_b5_exact_cover_terms.rs");

const ROUND314_B5_DEFAULT_A_ANF_PATTERNS: [&str; 32] = [
    "-----1--", "1----1--", "--1--1--", "1-1--1--", "------1-", "-1----1-", "-----11-", "1----11-",
    "-1---11-", "11---11-", "--1--11-", "1-1--11-", "-11--11-", "111--11-", "---1---1", "---11--1",
    "-----1-1", "--1--1-1", "1--1-1-1", "1-11-1-1", "1--111-1", "1-1111-1", "---1--11", "---11-11",
    "-----111", "-1---111", "--1--111", "-11--111", "1--1-111", "1-11-111", "1--11111", "1-111111",
];

const ROUND314_B5_DEFAULT_B_ANF_PATTERNS: [&str; 24] = [
    "-----1--", "1----1--", "------1-", "-1----1-", "--1---1-", "1-1---1-", "-11---1-", "-----11-",
    "1----11-", "-1---11-", "11---11-", "--1--11-", "-11--11-", "111--11-", "----1--1", "----11-1",
    "------11", "--1---11", "----1-11", "-----111", "1----111", "--1--111", "1-1--111", "----1111",
];

#[derive(Clone, Copy, Debug)]
struct Round314B5CorrectionRule {
    post_zeta_lo: i128,
    post_zeta_hi: i128,
    hash_value: u8,
    selector_xor_mask: usize,
}

const ROUND314_B5_CORRECTION_RULES: [Round314B5CorrectionRule; 16] = [
    Round314B5CorrectionRule {
        post_zeta_lo: -4,
        post_zeta_hi: -4,
        hash_value: 27,
        selector_xor_mask: 3,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -4,
        post_zeta_hi: -4,
        hash_value: 154,
        selector_xor_mask: 3,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -3,
        post_zeta_hi: -3,
        hash_value: 14,
        selector_xor_mask: 1,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -3,
        post_zeta_hi: -3,
        hash_value: 29,
        selector_xor_mask: 2,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -3,
        post_zeta_hi: -3,
        hash_value: 30,
        selector_xor_mask: 1,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -3,
        post_zeta_hi: -3,
        hash_value: 140,
        selector_xor_mask: 1,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -3,
        post_zeta_hi: -3,
        hash_value: 156,
        selector_xor_mask: 2,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -3,
        post_zeta_hi: -3,
        hash_value: 159,
        selector_xor_mask: 1,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -3,
        post_zeta_hi: -2,
        hash_value: 21,
        selector_xor_mask: 2,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -3,
        post_zeta_hi: -2,
        hash_value: 148,
        selector_xor_mask: 2,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -2,
        post_zeta_hi: -2,
        hash_value: 23,
        selector_xor_mask: 2,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: -2,
        post_zeta_hi: -2,
        hash_value: 150,
        selector_xor_mask: 2,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: 0,
        post_zeta_hi: 0,
        hash_value: 47,
        selector_xor_mask: 1,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: 0,
        post_zeta_hi: 0,
        hash_value: 63,
        selector_xor_mask: 1,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: 0,
        post_zeta_hi: 0,
        hash_value: 173,
        selector_xor_mask: 1,
    },
    Round314B5CorrectionRule {
        post_zeta_lo: 0,
        post_zeta_hi: 0,
        hash_value: 190,
        selector_xor_mask: 1,
    },
];

fn round314_b5_control_wire(
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    control_bit: usize,
) -> QubitId {
    let b_bits = phase_b5_execution::ROUND218_B5_BLOCK_BITS;
    if control_bit < b_bits {
        branch_word[control_bit]
    } else {
        old_g0_word[control_bit - b_bits]
    }
}

fn round314_b5_twos_zeta_code(zeta: i128, bits: usize) -> u64 {
    assert!(
        bits < 63,
        "Round314 B=5 zeta helper expects a small signed register"
    );
    let modulus = 1i128 << bits;
    zeta.rem_euclid(modulus) as u64
}

fn round314_b5_exact_twos_pattern(zeta: i128, bits: usize) -> String {
    let code = round314_b5_twos_zeta_code(zeta, bits);
    (0..bits)
        .map(|bit| if ((code >> bit) & 1) != 0 { '1' } else { '0' })
        .collect()
}

fn round314_b5_exact_hash_pattern(hash_value: u8) -> String {
    (0..ROUND314_B5_HASH_BITS)
        .map(|bit| {
            if ((hash_value >> bit) & 1) != 0 {
                '1'
            } else {
                '0'
            }
        })
        .collect()
}

fn round314_b5_cube_controls(pattern: &str, inputs: &[QubitId]) -> (Vec<QubitId>, Vec<QubitId>) {
    assert_eq!(pattern.len(), inputs.len(), "Round314 cube width mismatch");
    let mut controls = Vec::new();
    let mut negative_controls = Vec::new();
    for (idx, byte) in pattern.as_bytes().iter().copied().enumerate() {
        match byte {
            b'-' => {}
            b'0' => {
                controls.push(inputs[idx]);
                negative_controls.push(inputs[idx]);
            }
            b'1' => controls.push(inputs[idx]),
            _ => panic!("bad Round314 cube character"),
        }
    }
    (controls, negative_controls)
}

fn emit_round314_b5_multi_control_x(
    b: &mut B,
    controls: &[QubitId],
    target: QubitId,
    scratch: &[QubitId],
) {
    match controls.len() {
        0 => b.x(target),
        1 => b.cx(controls[0], target),
        2 => b.ccx(controls[0], controls[1], target),
        n => {
            assert!(
                scratch.len() >= n - 2,
                "Round314 multi-control helper needs controls-2 scratch qubits"
            );
            b.ccx(controls[0], controls[1], scratch[0]);
            for idx in 2..(n - 1) {
                b.ccx(scratch[idx - 2], controls[idx], scratch[idx - 1]);
            }
            b.ccx(scratch[n - 3], controls[n - 1], target);
            for idx in (2..(n - 1)).rev() {
                b.ccx(scratch[idx - 2], controls[idx], scratch[idx - 1]);
            }
            b.ccx(controls[0], controls[1], scratch[0]);
        }
    }
}

fn emit_round314_b5_guarded_kernel_xor(
    b: &mut B,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    controls: &[QubitId],
    negative_controls: &[QubitId],
    kernel_mask: u16,
    helper: QubitId,
    scratch: &[QubitId],
) {
    for &wire in negative_controls {
        b.x(wire);
    }
    emit_round314_b5_multi_control_x(b, controls, helper, scratch);
    for control_bit in 0..ROUND314_B5_CONTROL_BITS {
        if ((kernel_mask >> control_bit) & 1) != 0 {
            b.cx(
                helper,
                round314_b5_control_wire(branch_word, old_g0_word, control_bit),
            );
        }
    }
    emit_round314_b5_multi_control_x(b, controls, helper, scratch);
    for &wire in negative_controls.iter().rev() {
        b.x(wire);
    }
}

fn emit_round314_b5_linear_control_hash_into_l(
    b: &mut B,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    l_hash: &[QubitId],
) {
    assert_eq!(
        l_hash.len(),
        ROUND314_B5_HASH_BITS,
        "Round314 B=5 separator must be 8 bits"
    );
    for (hash_bit, &mask) in ROUND314_B5_HASH_MASKS.iter().enumerate() {
        for control_bit in 0..ROUND314_B5_CONTROL_BITS {
            if ((mask >> control_bit) & 1) != 0 {
                b.cx(
                    round314_b5_control_wire(branch_word, old_g0_word, control_bit),
                    l_hash[hash_bit],
                );
            }
        }
    }
}

fn emit_round315_b5_control_bit_hash_into_l(
    b: &mut B,
    control_bit: usize,
    control_wire: QubitId,
    l_hash: &[QubitId],
) {
    assert!(
        control_bit < ROUND314_B5_CONTROL_BITS,
        "Round315 control bit out of range"
    );
    assert_eq!(
        l_hash.len(),
        ROUND314_B5_HASH_BITS,
        "Round315 B=5 separator must be 8 bits"
    );
    for (hash_bit, &mask) in ROUND314_B5_HASH_MASKS.iter().enumerate() {
        if ((mask >> control_bit) & 1) != 0 {
            b.cx(control_wire, l_hash[hash_bit]);
        }
    }
}

fn emit_round314_b5_post_hash_control_cleaner(
    b: &mut B,
    zeta: &[QubitId],
    l_hash: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    assert_eq!(
        l_hash.len(),
        ROUND314_B5_HASH_BITS,
        "Round314 B=5 separator must be 8 bits"
    );
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );
    let helper = b.alloc_qubit();
    let scratch = b.alloc_qubits(zeta.len() + ROUND314_B5_HASH_BITS - 2);

    for hash_bit in 0..ROUND314_B5_HASH_BITS {
        let control_bit = ROUND314_B5_CONTROL_BITS - 1 - hash_bit;
        b.cx(
            l_hash[hash_bit],
            round314_b5_control_wire(branch_word, old_g0_word, control_bit),
        );
    }

    for &pattern in &ROUND314_B5_DEFAULT_A_ANF_PATTERNS {
        let (controls, negative) = round314_b5_cube_controls(pattern, l_hash);
        emit_round314_b5_guarded_kernel_xor(
            b,
            branch_word,
            old_g0_word,
            &controls,
            &negative,
            ROUND314_B5_KERNEL_A,
            helper,
            &scratch,
        );
    }
    for &pattern in &ROUND314_B5_DEFAULT_B_ANF_PATTERNS {
        let (controls, negative) = round314_b5_cube_controls(pattern, l_hash);
        emit_round314_b5_guarded_kernel_xor(
            b,
            branch_word,
            old_g0_word,
            &controls,
            &negative,
            ROUND314_B5_KERNEL_B,
            helper,
            &scratch,
        );
    }

    for rule in &ROUND314_B5_CORRECTION_RULES {
        let hash_pattern = round314_b5_exact_hash_pattern(rule.hash_value);
        let (hash_controls, hash_negative) = round314_b5_cube_controls(&hash_pattern, l_hash);
        for post_zeta in rule.post_zeta_lo..=rule.post_zeta_hi {
            let zeta_pattern = round314_b5_exact_twos_pattern(post_zeta, zeta.len());
            let (zeta_controls, zeta_negative) = round314_b5_cube_controls(&zeta_pattern, zeta);
            let mut controls = Vec::with_capacity(zeta_controls.len() + hash_controls.len());
            controls.extend(zeta_controls.iter().copied());
            controls.extend(hash_controls.iter().copied());
            let mut negative = Vec::with_capacity(zeta_negative.len() + hash_negative.len());
            negative.extend(zeta_negative.iter().copied());
            negative.extend(hash_negative.iter().copied());
            emit_round314_b5_guarded_kernel_xor(
                b,
                branch_word,
                old_g0_word,
                &controls,
                &negative,
                ROUND314_B5_KERNELS_BY_SELECTOR_MASK[rule.selector_xor_mask],
                helper,
                &scratch,
            );
        }
    }

    b.free_vec(&scratch);
    b.free(helper);
}

pub(super) fn emit_round315_b5_source_stream_backward_block_from_hash(
    b: &mut B,
    zeta: &[QubitId],
    f: &[QubitId],
    g: &[QubitId],
    step0: usize,
    l_hash: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(old_g0_word.len(), branch_word.len());
    assert_eq!(l_hash.len(), ROUND314_B5_HASH_BITS);
    assert_eq!(f.len(), g.len());
    assert!(
        step0 + branch_word.len() <= f.len(),
        "Round315 hash source rollback block exceeds source width"
    );

    b.set_phase("round315_b5_hash_rollback_recover_controls");
    emit_round314_b5_post_hash_control_cleaner(b, zeta, l_hash, branch_word, old_g0_word);

    b.set_phase("round315_b5_hash_rollback_restore_source_and_l");
    for j in (0..branch_word.len()).rev() {
        let step = step0 + j;
        let width = f.len() - step;
        phase_b5_state_selector::emit_round218_b5_twos_zeta_update_step_inverse(
            b,
            zeta,
            branch_word[j],
        );
        phase_b5_state_selector::emit_round218_b5_low_window_unapply_step(
            b,
            f,
            g,
            width,
            branch_word[j],
            old_g0_word[j],
        );

        emit_round315_b5_control_bit_hash_into_l(b, j, branch_word[j], l_hash);
        emit_round315_b5_control_bit_hash_into_l(
            b,
            phase_b5_execution::ROUND218_B5_BLOCK_BITS + j,
            old_g0_word[j],
            l_hash,
        );

        let sign = zeta[zeta.len() - 1];
        b.ccx(sign, g[0], branch_word[j]);
        b.cx(g[0], old_g0_word[j]);
    }
}

fn round326_b5_interval_cover(lo_code: u64, hi_code: u64, bits: usize) -> Vec<(u64, u64)> {
    assert!(
        bits < 63,
        "Round326 interval helper expects a small zeta register"
    );
    assert!(lo_code <= hi_code, "Round326 interval is inverted");
    let mut out = Vec::new();
    let mut cursor = lo_code;
    let full_mask = (1u64 << bits) - 1;
    while cursor <= hi_code {
        let mut size = cursor & cursor.wrapping_neg();
        if size == 0 {
            size = 1u64 << bits;
        }
        while cursor + size - 1 > hi_code {
            size >>= 1;
        }
        let varying = size - 1;
        let mask = full_mask ^ varying;
        out.push((mask, cursor & mask));
        cursor += size;
    }
    out
}

fn emit_round326_b5_masked_literal_toggle(
    b: &mut B,
    inputs: &[QubitId],
    mask: u64,
    fixed: u64,
    target: QubitId,
    scratch: &[QubitId],
) {
    let mut controls = Vec::new();
    let mut negative = Vec::new();
    for bit in 0..inputs.len() {
        if ((mask >> bit) & 1) != 0 {
            controls.push(inputs[bit]);
            if ((fixed >> bit) & 1) == 0 {
                negative.push(inputs[bit]);
            }
        }
    }
    for &wire in &negative {
        b.x(wire);
    }
    emit_round314_b5_multi_control_x(b, &controls, target, scratch);
    for &wire in negative.iter().rev() {
        b.x(wire);
    }
}

fn emit_round326_b5_guarded_term_toggle(
    b: &mut B,
    flag: QubitId,
    inputs: &[QubitId],
    mask: u16,
    fixed: u16,
    target: QubitId,
    scratch: &[QubitId],
) {
    let mut controls = vec![flag];
    let mut negative = Vec::new();
    for bit in 0..inputs.len() {
        if ((mask >> bit) & 1) != 0 {
            controls.push(inputs[bit]);
            if ((fixed >> bit) & 1) == 0 {
                negative.push(inputs[bit]);
            }
        }
    }
    for &wire in &negative {
        b.x(wire);
    }
    emit_round314_b5_multi_control_x(b, &controls, target, scratch);
    for &wire in negative.iter().rev() {
        b.x(wire);
    }
}

fn emit_round326_b5_twos_interval_flag(
    b: &mut B,
    zeta: &[QubitId],
    lo: i128,
    hi: i128,
    flag: QubitId,
    scratch: &[QubitId],
) {
    assert!(lo <= hi, "Round326 zeta interval is inverted");
    assert!(
        zeta.len() < 63,
        "Round326 exact-cover zeta flags expect a small signed register"
    );
    if lo < 0 && hi >= 0 {
        emit_round326_b5_twos_interval_flag(b, zeta, lo, -1, flag, scratch);
        emit_round326_b5_twos_interval_flag(b, zeta, 0, hi, flag, scratch);
        return;
    }
    let lo_code = round314_b5_twos_zeta_code(lo, zeta.len());
    let hi_code = round314_b5_twos_zeta_code(hi, zeta.len());
    for (mask, fixed) in round326_b5_interval_cover(lo_code, hi_code, zeta.len()) {
        emit_round326_b5_masked_literal_toggle(b, zeta, mask, fixed, flag, scratch);
    }
}

fn emit_round326_b5_region_flags(
    b: &mut B,
    zeta: &[QubitId],
    flags: &[QubitId],
    regions: &[(i128, i128)],
    scratch: &[QubitId],
) {
    assert_eq!(
        flags.len(),
        regions.len(),
        "Round326 region flag width mismatch"
    );
    for (idx, &(lo, hi)) in regions.iter().enumerate() {
        emit_round326_b5_twos_interval_flag(b, zeta, lo, hi, flags[idx], scratch);
    }
}

pub(super) fn emit_round326_b5_live_l_rank_exact_cover(
    b: &mut B,
    zeta: &[QubitId],
    old_g0_word: &[QubitId],
    l_rank: &[QubitId],
) {
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "Round326 live-L old_g0 word must be B=5"
    );
    assert_eq!(l_rank.len(), 4, "Round326 live-L rank must be 4 bits");
    assert!(
        zeta.len() >= 3,
        "Round326 live-L materializer needs a signed zeta register"
    );

    let flags = b.alloc_qubits(ROUND326_B5_LIVE_L_REGIONS.len());
    let scratch = b.alloc_qubits(zeta.len().saturating_sub(2).max(8));

    emit_round326_b5_region_flags(b, zeta, &flags, &ROUND326_B5_LIVE_L_REGIONS, &scratch);
    for term in &ROUND326_B5_LIVE_L_EXACT_COVER_TERMS {
        emit_round326_b5_guarded_term_toggle(
            b,
            flags[term.region],
            old_g0_word,
            term.mask,
            term.fixed,
            l_rank[term.output],
            &scratch,
        );
    }
    emit_round326_b5_region_flags(b, zeta, &flags, &ROUND326_B5_LIVE_L_REGIONS, &scratch);

    b.free_vec(&scratch);
    b.free_vec(&flags);
}

fn emit_round326_b5_rank_one_controls(
    b: &mut B,
    flag: QubitId,
    old_g0_word: &[QubitId],
    l_rank: &[QubitId],
    branch_bit: usize,
    target: QubitId,
    scratch: &[QubitId],
    rank_value: u8,
) {
    let mut controls = vec![flag];
    let mut negative = Vec::new();
    for (bit, &wire) in l_rank.iter().enumerate() {
        controls.push(wire);
        if ((rank_value >> bit) & 1) == 0 {
            negative.push(wire);
        }
    }
    for lower in 0..branch_bit {
        controls.push(old_g0_word[lower]);
        negative.push(old_g0_word[lower]);
    }
    controls.push(old_g0_word[branch_bit]);
    for &wire in &negative {
        b.x(wire);
    }
    emit_round314_b5_multi_control_x(b, &controls, target, scratch);
    for &wire in negative.iter().rev() {
        b.x(wire);
    }
}

pub(super) fn emit_round326_b5_branch_rank_exact_cover_cleaner(
    b: &mut B,
    zeta: &[QubitId],
    old_g0_word: &[QubitId],
    l_rank: &[QubitId],
    branch_word: &[QubitId],
) {
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "Round326 branch cleaner old_g0 word must be B=5"
    );
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "Round326 branch cleaner target word must be B=5"
    );
    assert_eq!(
        l_rank.len(),
        4,
        "Round326 branch cleaner rank must be 4 bits"
    );
    assert!(
        zeta.len() >= 3,
        "Round326 branch cleaner needs a signed zeta register"
    );

    let flags = b.alloc_qubits(ROUND326_B5_BRANCH_REGIONS.len());
    let scratch = b.alloc_qubits(zeta.len().saturating_sub(2).max(8));
    emit_round326_b5_region_flags(b, zeta, &flags, &ROUND326_B5_BRANCH_REGIONS, &scratch);

    let mut old_and_l = Vec::with_capacity(old_g0_word.len() + l_rank.len());
    old_and_l.extend_from_slice(old_g0_word);
    old_and_l.extend_from_slice(l_rank);
    for term in &ROUND326_B5_BRANCH_EXACT_COVER_TERMS {
        emit_round326_b5_guarded_term_toggle(
            b,
            flags[term.region],
            &old_and_l,
            term.mask,
            term.fixed,
            branch_word[term.output],
            &scratch,
        );
    }

    let mid_flag = flags[8];
    for bit in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        emit_round326_b5_rank_one_controls(
            b,
            mid_flag,
            old_g0_word,
            l_rank,
            bit,
            branch_word[bit],
            &scratch,
            1,
        );
    }
    for tail_idx in 0..5 {
        let flag = flags[9 + tail_idx];
        for bit in tail_idx..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
            emit_round326_b5_rank_one_controls(
                b,
                flag,
                old_g0_word,
                l_rank,
                bit,
                branch_word[bit],
                &scratch,
                0,
            );
        }
    }

    emit_round326_b5_region_flags(b, zeta, &flags, &ROUND326_B5_BRANCH_REGIONS, &scratch);
    b.free_vec(&scratch);
    b.free_vec(&flags);
}

pub(super) fn emit_round314_b5_source_live_hash_transport_window_block(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    l_hash: &[QubitId],
    next_f: &[QubitId],
    next_g: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round314 source-live hash transport block is secp256k1-only"
    );
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta register needs at least 3 signed bits"
    );
    assert!(
        f_window.len() >= phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "Round314 source-live hash block needs at least B=5 source bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        l_hash.len(),
        ROUND314_B5_HASH_BITS,
        "Round314 separator L must be 8 qubits"
    );
    assert_eq!(
        next_f.len(),
        f_window.len() - phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "next f word must retain window_bits-B bits"
    );
    assert_eq!(next_g.len(), next_f.len());

    let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

    b.set_phase("round314_b5_source_live_hash_parse_and_advance");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_window_parser(
        b,
        zeta,
        f_window,
        g_window,
        &branch_word,
        &old_g0_word,
        next_f,
        next_g,
    );

    b.set_phase("round314_b5_source_live_hash_materialize_l");
    emit_round314_b5_linear_control_hash_into_l(b, &branch_word, &old_g0_word, l_hash);

    b.set_phase("round314_b5_source_live_hash_transport_coeff");
    emit_round218_scaled_coeff_b5_block_selected_with_branch_neg(
        b,
        v,
        r,
        &branch_word,
        &old_g0_word,
        p,
        false,
    );

    b.set_phase("round314_b5_source_live_hash_clean_controls");
    emit_round314_b5_post_hash_control_cleaner(b, zeta, l_hash, &branch_word, &old_g0_word);

    b.free_vec(&old_g0_word);
    b.free_vec(&branch_word);
}

pub(super) fn emit_round218_b5_source_live_projective_scalar_transport_block(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 source-live projective-scalar transport block is secp256k1-only"
    );
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta register needs at least 3 signed bits"
    );
    assert!(
        f_window.len() >= phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "source-live projective-scalar transport block needs at least B=5 source bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

    b.set_phase("round218_b5_source_live_projective_scalar_parse");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_control_word_parser(
        b,
        zeta,
        f_window,
        g_window,
        &branch_word,
        &old_g0_word,
    );

    b.set_phase("round218_b5_source_live_projective_scalar_coeff");
    let allow_noncanonical_branch_neg =
        std::env::var(ROUND218_B5_PROJECTIVE_NONCANONICAL_BRANCH_NEG_DIAG_ENV)
            .ok()
            .as_deref()
            == Some("1");
    emit_round218_scaled_coeff_b5_block_selected_with_branch_neg(
        b,
        v,
        r,
        &branch_word,
        &old_g0_word,
        p,
        allow_noncanonical_branch_neg,
    );

    b.set_phase("round218_b5_source_live_projective_scalar_unparse");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_control_word_parser_uncompute(
        b,
        zeta,
        f_window,
        g_window,
        &branch_word,
        &old_g0_word,
    );

    b.free_vec(&old_g0_word);
    b.free_vec(&branch_word);
}

pub(super) fn emit_round379_b5_source_live_cheap_lft_frame_block(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round379 source-live cheap LFT frame block is secp256k1-only"
    );
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta register needs at least 3 signed bits"
    );
    assert!(
        f_window.len() >= phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "source-live cheap LFT frame block needs at least B=5 source bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

    b.set_phase("round379_b5_source_live_cheap_lft_parse");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_control_word_parser(
        b,
        zeta,
        f_window,
        g_window,
        &branch_word,
        &old_g0_word,
    );

    b.set_phase("round379_b5_source_live_cheap_lft_frame");
    for &branch in branch_word.iter() {
        for i in 0..N {
            super::cswap(b, branch, v[i], r[i]);
        }
        super::mod_double_inplace_fast(b, v, p);
    }

    b.set_phase("round379_b5_source_live_cheap_lft_unparse");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_control_word_parser_uncompute(
        b,
        zeta,
        f_window,
        g_window,
        &branch_word,
        &old_g0_word,
    );

    b.free_vec(&old_g0_word);
    b.free_vec(&branch_word);
}

pub(super) fn emit_round381_b5_source_live_branch_only_cheap_lft_frame_block(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round381 branch-only cheap LFT frame block is secp256k1-only"
    );
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta register needs at least 3 signed bits"
    );
    assert!(
        f_window.len() >= phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "source-live branch-only cheap LFT frame block needs at least B=5 source bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

    b.set_phase("round381_b5_source_live_branch_only_cheap_lft_parse");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_branch_word_parser(
        b,
        zeta,
        f_window,
        g_window,
        &branch_word,
    );

    b.set_phase("round381_b5_source_live_branch_only_cheap_lft_frame");
    emit_b5_cheap_lft_frame_from_branch_controls(b, v, r, &branch_word, p);

    b.set_phase("round381_b5_source_live_branch_only_cheap_lft_unparse");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_branch_word_parser_uncompute(
        b,
        zeta,
        f_window,
        g_window,
        &branch_word,
    );

    b.free_vec(&branch_word);
}

pub(super) fn emit_round383_b5_current_pattern_ranked_cheap_lft_source_block(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    l_rank: &[QubitId],
    old_g0_word: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round383 current-pattern ranked source block is secp256k1-only"
    );
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta register needs at least 3 signed bits"
    );
    assert_eq!(
        f_window.len(),
        phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS,
        "Round383 source block expects a 2B=10 source window"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(l_rank.len(), 4, "Round383 L rank must be 4 bits");
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "Round383 old_g0 history must be a B=5 word"
    );

    let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

    b.set_phase("round383_b5_current_pattern_parse_controls");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_control_word_parser(
        b,
        zeta,
        f_window,
        g_window,
        &branch_word,
        &old_g0_word,
    );

    b.set_phase("round383_b5_current_pattern_materialize_l");
    emit_round326_b5_live_l_rank_exact_cover(b, zeta, &old_g0_word, l_rank);

    b.set_phase("round383_b5_current_pattern_source_advance");
    phase_b5_state_selector::emit_round218_b5_source_stream_forward_block_from_controls(
        b,
        zeta,
        f_window,
        g_window,
        0,
        &branch_word,
        &old_g0_word,
    );

    b.set_phase("round383_b5_current_pattern_cheap_lft_frame");
    emit_b5_cheap_lft_frame_from_branch_controls(b, v, r, &branch_word, p);

    b.set_phase("round383_b5_current_pattern_clean_branch");
    emit_round326_b5_branch_rank_exact_cover_cleaner(b, zeta, &old_g0_word, l_rank, &branch_word);

    b.free_vec(&branch_word);
}

pub(super) fn emit_round384_b5_current_pattern_ranked_source_rollback_block(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    l_rank: &[QubitId],
    old_g0_word: &[QubitId],
) {
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta register needs at least 3 signed bits"
    );
    assert_eq!(
        f_window.len(),
        phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS,
        "Round384 rollback expects a 2B=10 source window"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(l_rank.len(), 4, "Round384 L rank must be 4 bits");
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "Round384 old_g0 history must be a B=5 word"
    );

    let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

    b.set_phase("round384_b5_current_pattern_recover_branch");
    emit_round326_b5_branch_rank_exact_cover_cleaner(b, zeta, old_g0_word, l_rank, &branch_word);

    b.set_phase("round384_b5_current_pattern_materialize_start_zeta");
    let start_zeta = b.alloc_qubits(zeta.len());
    for i in 0..zeta.len() {
        b.cx(zeta[i], start_zeta[i]);
    }
    for step in (0..phase_b5_execution::ROUND218_B5_BLOCK_BITS).rev() {
        phase_b5_state_selector::emit_round218_b5_twos_zeta_update_step_inverse(
            b,
            &start_zeta,
            branch_word[step],
        );
    }

    b.set_phase("round384_b5_current_pattern_clean_l");
    emit_round326_b5_live_l_rank_exact_cover(b, &start_zeta, old_g0_word, l_rank);

    b.set_phase("round384_b5_current_pattern_uncompute_start_zeta");
    for step in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        phase_b5_state_selector::emit_round218_b5_twos_zeta_update_step(
            b,
            &start_zeta,
            branch_word[step],
        );
    }
    for i in 0..zeta.len() {
        b.cx(zeta[i], start_zeta[i]);
    }
    b.free_vec(&start_zeta);

    b.set_phase("round384_b5_current_pattern_source_backward_clean_history");
    phase_b5_state_selector::emit_round218_b5_source_stream_backward_block(
        b,
        zeta,
        f_window,
        g_window,
        0,
        &branch_word,
        old_g0_word,
    );

    b.free_vec(&branch_word);
}

pub(super) fn emit_round385_b5_fused_advance_frame_rollback_block(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round385 fused advance/frame/rollback block is secp256k1-only"
    );
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta register needs at least 3 signed bits"
    );
    assert_eq!(
        f_window.len(),
        phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS,
        "Round385 fused block expects a 2B=10 source window"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

    b.set_phase("round385_b5_fused_parse_controls");
    phase_b5_state_selector::emit_round218_b5_twos_zeta_control_word_parser(
        b,
        zeta,
        f_window,
        g_window,
        &branch_word,
        &old_g0_word,
    );

    b.set_phase("round385_b5_fused_source_advance");
    phase_b5_state_selector::emit_round218_b5_source_stream_forward_block_from_controls(
        b,
        zeta,
        f_window,
        g_window,
        0,
        &branch_word,
        &old_g0_word,
    );

    b.set_phase("round385_b5_fused_cheap_lft_frame");
    emit_b5_cheap_lft_frame_from_branch_controls(b, v, r, &branch_word, p);

    b.set_phase("round385_b5_fused_source_rollback_clean_controls");
    phase_b5_state_selector::emit_round218_b5_source_stream_backward_block(
        b,
        zeta,
        f_window,
        g_window,
        0,
        &branch_word,
        &old_g0_word,
    );

    b.free_vec(&old_g0_word);
    b.free_vec(&branch_word);
}

fn emit_b5_cheap_lft_frame_from_branch_controls(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "cheap LFT frame transport is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "branch word must be a B=5 word"
    );

    for &branch in branch_word.iter() {
        for i in 0..N {
            super::cswap(b, branch, v[i], r[i]);
        }
        super::mod_double_inplace_fast(b, v, p);
    }
}

pub(super) fn emit_round380_b5_full_source_stream_cheap_lft_frame_transport(
    b: &mut B,
    dx: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    emit_round218_b5_full_source_stream_transport_with_coeff_and_finish(
        b,
        dx,
        v,
        r,
        p,
        |b, v, r, branch_word, _old_g0_word, p| {
            b.set_phase("round380_b5_full_source_stream_cheap_lft_frame_block");
            emit_b5_cheap_lft_frame_from_branch_controls(b, v, r, branch_word, p);
        },
        |_b, _zeta, _f, _g, _v, _r, _p| {},
    );
}

pub(super) fn emit_round218_b5_twos_zeta_control_transport_block(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    emit_round218_b5_source_live_projective_scalar_transport_block(
        b, zeta, f_window, g_window, v, r, p,
    );
}

const ROUND331_B5_OLD_G0_FULL_ERASER_KMX: &str = include_str!(
    "round331_b5_old_g0_full_eraser.kmx"
);
const ROUND331_B5_OLD_G0_ERASER_F_BITS: usize = 10;
const ROUND331_B5_OLD_G0_ERASER_G_BITS: usize = 10;
const ROUND331_B5_OLD_G0_ERASER_BRANCH_BITS: usize = phase_b5_execution::ROUND218_B5_BLOCK_BITS;
const ROUND331_B5_OLD_G0_ERASER_OLD_BITS: usize = phase_b5_execution::ROUND218_B5_BLOCK_BITS;
const ROUND331_B5_OLD_G0_ERASER_SCRATCH_BASE: usize = 30;

fn round331_b5_old_g0_eraser_scratch_count() -> usize {
    let max_q = ROUND331_B5_OLD_G0_FULL_ERASER_KMX
        .lines()
        .filter_map(Op::from_text)
        .flat_map(|op| [op.q_control2, op.q_control1, op.q_target])
        .filter(|&q| q != NO_QUBIT)
        .map(|q| q.0 as usize)
        .max()
        .unwrap_or(ROUND331_B5_OLD_G0_ERASER_SCRATCH_BASE - 1);
    max_q.saturating_sub(ROUND331_B5_OLD_G0_ERASER_SCRATCH_BASE) + 1
}

fn round331_b5_old_g0_eraser_map_qubit(
    q: QubitId,
    f_window: &[QubitId],
    g_window: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    scratch: &[QubitId],
) -> QubitId {
    if q == NO_QUBIT {
        return q;
    }
    let idx = q.0 as usize;
    if idx < ROUND331_B5_OLD_G0_ERASER_F_BITS {
        f_window[idx]
    } else if idx < ROUND331_B5_OLD_G0_ERASER_F_BITS + ROUND331_B5_OLD_G0_ERASER_G_BITS {
        g_window[idx - ROUND331_B5_OLD_G0_ERASER_F_BITS]
    } else if idx
        < ROUND331_B5_OLD_G0_ERASER_F_BITS
            + ROUND331_B5_OLD_G0_ERASER_G_BITS
            + ROUND331_B5_OLD_G0_ERASER_BRANCH_BITS
    {
        branch_word[idx - ROUND331_B5_OLD_G0_ERASER_F_BITS - ROUND331_B5_OLD_G0_ERASER_G_BITS]
    } else if idx < ROUND331_B5_OLD_G0_ERASER_SCRATCH_BASE {
        old_g0_word[idx
            - ROUND331_B5_OLD_G0_ERASER_F_BITS
            - ROUND331_B5_OLD_G0_ERASER_G_BITS
            - ROUND331_B5_OLD_G0_ERASER_BRANCH_BITS]
    } else {
        scratch[idx - ROUND331_B5_OLD_G0_ERASER_SCRATCH_BASE]
    }
}

fn round331_b5_old_g0_eraser_map_bit(bit: BitId, measurement: BitId) -> BitId {
    if bit == NO_BIT {
        NO_BIT
    } else {
        assert_eq!(bit.0, 0, "Round331 old_g0 eraser has one HMR bit");
        measurement
    }
}

fn release_already_reset_qubits(b: &mut B, scratch: &[QubitId]) {
    for &q in scratch {
        b.free_qubits.push(q.0.try_into().expect("qubit id fits in u32"));
        assert!(b.active_qubits > 0, "scratch release underflow");
        b.active_qubits -= 1;
    }
}

pub(super) fn emit_round331_b5_old_g0_full_eraser(
    b: &mut B,
    f_window: &[QubitId],
    g_window: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    assert_eq!(
        f_window.len(),
        ROUND331_B5_OLD_G0_ERASER_F_BITS,
        "Round331 old_g0 eraser expects 10 f-window bits"
    );
    assert_eq!(
        g_window.len(),
        ROUND331_B5_OLD_G0_ERASER_G_BITS,
        "Round331 old_g0 eraser expects 10 g-window bits"
    );
    assert_eq!(
        branch_word.len(),
        ROUND331_B5_OLD_G0_ERASER_BRANCH_BITS,
        "Round331 old_g0 eraser expects B=5 branch bits"
    );
    assert_eq!(
        old_g0_word.len(),
        ROUND331_B5_OLD_G0_ERASER_OLD_BITS,
        "Round331 old_g0 eraser expects B=5 old_g0 bits"
    );

    let scratch = b.alloc_qubits(round331_b5_old_g0_eraser_scratch_count());
    let measurement = b.alloc_bit();
    for mut op in ROUND331_B5_OLD_G0_FULL_ERASER_KMX
        .lines()
        .filter_map(Op::from_text)
    {
        match op.kind {
            OperationType::AppendToRegister
            | OperationType::Register
            | OperationType::DebugPrint => {
                continue;
            }
            _ => {}
        }
        op.q_control2 = round331_b5_old_g0_eraser_map_qubit(
            op.q_control2,
            f_window,
            g_window,
            branch_word,
            old_g0_word,
            &scratch,
        );
        op.q_control1 = round331_b5_old_g0_eraser_map_qubit(
            op.q_control1,
            f_window,
            g_window,
            branch_word,
            old_g0_word,
            &scratch,
        );
        op.q_target = round331_b5_old_g0_eraser_map_qubit(
            op.q_target,
            f_window,
            g_window,
            branch_word,
            old_g0_word,
            &scratch,
        );
        op.c_target = round331_b5_old_g0_eraser_map_bit(op.c_target, measurement);
        op.c_condition = round331_b5_old_g0_eraser_map_bit(op.c_condition, measurement);
        b.ops.push(op);
    }
    release_already_reset_qubits(b, &scratch);
}

pub fn build_round331_b5_old_g0_full_eraser_component() -> Vec<Op> {
    let mut b = B::new();
    let f_window = b.alloc_qubits(ROUND331_B5_OLD_G0_ERASER_F_BITS);
    b.declare_qubit_register(&f_window);
    let g_window = b.alloc_qubits(ROUND331_B5_OLD_G0_ERASER_G_BITS);
    b.declare_qubit_register(&g_window);
    let branch_word = b.alloc_qubits(ROUND331_B5_OLD_G0_ERASER_BRANCH_BITS);
    b.declare_qubit_register(&branch_word);
    let old_g0_word = b.alloc_qubits(ROUND331_B5_OLD_G0_ERASER_OLD_BITS);
    b.declare_qubit_register(&old_g0_word);

    b.set_phase("round331_b5_old_g0_full_eraser");
    emit_round331_b5_old_g0_full_eraser(&mut b, &f_window, &g_window, &branch_word, &old_g0_word);
    b.ops
}

fn emit_round218_b5_full_source_stream_transport_with_coeff_and_finish<C, F>(
    b: &mut B,
    dx: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
    mut emit_coeff_block: C,
    finish: F,
) where
    C: FnMut(&mut B, &[QubitId], &[QubitId], &[QubitId], &[QubitId], U256),
    F: FnOnce(&mut B, &[QubitId], &[QubitId], &[QubitId], &[QubitId], &[QubitId], U256),
{
    assert_eq!(
        p, SECP256K1_P,
        "Round218 full source stream is secp256k1-only"
    );
    assert_eq!(dx.len(), N, "round218 denominator must be 256 qubits");
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    let source_bits = phase_b5_execution::ROUND218_B5_STEPS;
    let zeta_bits = 11usize;
    let zeta = b.alloc_qubits(zeta_bits);
    let f = b.alloc_qubits(source_bits);
    let g = b.alloc_qubits(source_bits);
    let branch_hist = b.alloc_qubits(source_bits);
    let old_g0_hist = b.alloc_qubits(source_bits);

    b.set_phase("round218_b5_full_source_stream_init");
    for &q in &zeta {
        b.x(q);
    }
    for i in 0..N {
        if p.bit(i) {
            b.x(f[i]);
        }
        b.cx(dx[i], g[i]);
    }

    for block in 0..phase_b5_execution::ROUND218_B5_BLOCKS {
        let lo = block * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        let hi = lo + phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        b.set_phase("round218_b5_full_source_stream_forward_block");
        phase_b5_state_selector::emit_round218_b5_source_stream_forward_block(
            b,
            &zeta,
            &f,
            &g,
            lo,
            &branch_hist[lo..hi],
            &old_g0_hist[lo..hi],
        );
        b.set_phase("round218_b5_full_source_stream_coeff_block");
        emit_coeff_block(b, v, r, &branch_hist[lo..hi], &old_g0_hist[lo..hi], p);
    }

    finish(b, &zeta, &f, &g, v, r, p);

    for block in (0..phase_b5_execution::ROUND218_B5_BLOCKS).rev() {
        let lo = block * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        let hi = lo + phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        b.set_phase("round218_b5_full_source_stream_backward_block");
        phase_b5_state_selector::emit_round218_b5_source_stream_backward_block(
            b,
            &zeta,
            &f,
            &g,
            lo,
            &branch_hist[lo..hi],
            &old_g0_hist[lo..hi],
        );
    }

    b.set_phase("round218_b5_full_source_stream_cleanup");
    for i in (0..N).rev() {
        b.cx(dx[i], g[i]);
        if p.bit(i) {
            b.x(f[i]);
        }
    }
    for &q in zeta.iter().rev() {
        b.x(q);
    }

    b.free_vec(&old_g0_hist);
    b.free_vec(&branch_hist);
    b.free_vec(&g);
    b.free_vec(&f);
    b.free_vec(&zeta);
}

fn emit_round315_b5_hash_history_full_source_stream_transport_with_coeff_and_finish<C, F>(
    b: &mut B,
    dx: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
    mut emit_coeff_block: C,
    finish: F,
) where
    C: FnMut(&mut B, &[QubitId], &[QubitId], &[QubitId], &[QubitId], U256),
    F: FnOnce(&mut B, &[QubitId], &[QubitId], &[QubitId], &[QubitId], &[QubitId], U256),
{
    assert_eq!(
        p, SECP256K1_P,
        "Round315 hash-history full source stream is secp256k1-only"
    );
    assert_eq!(dx.len(), N, "round315 denominator must be 256 qubits");
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    let source_bits = phase_b5_execution::ROUND218_B5_STEPS;
    let zeta_bits = 11usize;
    let zeta = b.alloc_qubits(zeta_bits);
    let f = b.alloc_qubits(source_bits);
    let g = b.alloc_qubits(source_bits);
    let hash_hist = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCKS * ROUND314_B5_HASH_BITS);

    b.set_phase("round315_b5_hash_history_full_source_stream_init");
    for &q in &zeta {
        b.x(q);
    }
    for i in 0..N {
        if p.bit(i) {
            b.x(f[i]);
        }
        b.cx(dx[i], g[i]);
    }

    for block in 0..phase_b5_execution::ROUND218_B5_BLOCKS {
        let lo = block * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        let hash_lo = block * ROUND314_B5_HASH_BITS;
        let hash_hi = hash_lo + ROUND314_B5_HASH_BITS;
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

        b.set_phase("round315_b5_hash_history_forward_block");
        phase_b5_state_selector::emit_round218_b5_source_stream_forward_block(
            b,
            &zeta,
            &f,
            &g,
            lo,
            &branch_word,
            &old_g0_word,
        );
        b.set_phase("round315_b5_hash_history_materialize_l");
        emit_round314_b5_linear_control_hash_into_l(
            b,
            &branch_word,
            &old_g0_word,
            &hash_hist[hash_lo..hash_hi],
        );
        b.set_phase("round315_b5_hash_history_coeff_block");
        emit_coeff_block(b, v, r, &branch_word, &old_g0_word, p);
        b.set_phase("round315_b5_hash_history_clean_forward_controls");
        emit_round314_b5_post_hash_control_cleaner(
            b,
            &zeta,
            &hash_hist[hash_lo..hash_hi],
            &branch_word,
            &old_g0_word,
        );

        b.free_vec(&old_g0_word);
        b.free_vec(&branch_word);
    }

    finish(b, &zeta, &f, &g, v, r, p);

    for block in (0..phase_b5_execution::ROUND218_B5_BLOCKS).rev() {
        let lo = block * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        let hash_lo = block * ROUND314_B5_HASH_BITS;
        let hash_hi = hash_lo + ROUND314_B5_HASH_BITS;
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

        b.set_phase("round315_b5_hash_history_backward_block");
        emit_round315_b5_source_stream_backward_block_from_hash(
            b,
            &zeta,
            &f,
            &g,
            lo,
            &hash_hist[hash_lo..hash_hi],
            &branch_word,
            &old_g0_word,
        );

        b.free_vec(&old_g0_word);
        b.free_vec(&branch_word);
    }

    b.set_phase("round315_b5_hash_history_full_source_stream_cleanup");
    for i in (0..N).rev() {
        b.cx(dx[i], g[i]);
        if p.bit(i) {
            b.x(f[i]);
        }
    }
    for &q in zeta.iter().rev() {
        b.x(q);
    }

    b.free_vec(&hash_hist);
    b.free_vec(&g);
    b.free_vec(&f);
    b.free_vec(&zeta);
}

pub(super) fn emit_round218_b5_full_source_stream_transport(
    b: &mut B,
    dx: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    emit_round218_b5_full_source_stream_transport_with_coeff_and_finish(
        b,
        dx,
        v,
        r,
        p,
        |b, v, r, branch_word, old_g0_word, p| {
            emit_round218_b5_controlled_scaled_coefficient_step(
                b,
                v,
                r,
                branch_word,
                old_g0_word,
                p,
            )
        },
        |_b, _zeta, _f, _g, _v, _r, _p| {},
    );
}

pub(super) fn emit_round315_b5_hash_history_full_source_stream_transport(
    b: &mut B,
    dx: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    emit_round315_b5_hash_history_full_source_stream_transport_with_coeff_and_finish(
        b,
        dx,
        v,
        r,
        p,
        |b, v, r, branch_word, old_g0_word, p| {
            emit_round218_b5_controlled_scaled_coefficient_step(
                b,
                v,
                r,
                branch_word,
                old_g0_word,
                p,
            )
        },
        |_b, _zeta, _f, _g, _v, _r, _p| {},
    );
}

pub(super) fn emit_round218_b5_source_live_stream_quotient_lowerer(
    b: &mut B,
    h: &[QubitId],
    n: &[QubitId],
    p: U256,
) {
    emit_round218_b5_full_source_stream_quotient_lowerer(b, h, n, p);
}

pub(super) fn emit_round218_b5_source_live_stream_product_lowerer(
    b: &mut B,
    h: &[QubitId],
    n: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 source-live product is secp256k1-only"
    );
    assert_eq!(h.len(), N, "source-live product factor must be 256 qubits");
    assert_eq!(n.len(), N, "source-live product target must be 256 qubits");
    let body_plan = round218_b5_source_live_product_lowerer_body_plan();
    debug_assert!(!body_plan.body_emits_gates);
    debug_assert!(!body_plan.codegen_allowed_now);
    if !round218_b5_hash_history_product_probe_enabled() {
        b.set_phase("round218_b5_source_live_stream_product_fail_closed");
        panic!(
            "emit_round218_b5_source_live_stream_product_lowerer is fail-closed: \
             the current implementation is only the 37.746B hash-history/full-source \
             diagnostic body, not the missing no-history qtail/Round217 product \
             splice.  Set {env}=1 only to reproduce the non-promotable probe; \
             do not promote it as a PA milestone.",
            env = ROUND218_B5_ALLOW_HASH_HISTORY_PRODUCT_PROBE_ENV,
        );
    }

    let product = b.alloc_qubits(N);
    b.set_phase("round218_b5_source_live_stream_product_hash_history_scale");
    for _ in 0..phase_b5_execution::ROUND218_B5_STEPS {
        super::mod_double_inplace_fast(b, n, p);
    }

    b.set_phase("round218_b5_source_live_stream_product_hash_history_inverse_transport");
    super::emit_inverse_measurement_clean_scoped(b, |b| {
        emit_round315_b5_hash_history_full_source_stream_scaled_inverse(b, h, n, &product, p);
    });

    b.set_phase("round218_b5_source_live_stream_product_hash_history_swap");
    for i in 0..N {
        b.swap(n[i], product[i]);
    }

    b.set_phase("round218_b5_source_live_stream_product_hash_history_endpoint_cleanup");
    emit_round218_b5_reverse_product_endpoint_cleanup(b, h, &product, n, p);
    b.free_vec(&product);
}

pub(super) fn emit_round218_b5_full_source_stream_scaled_inverse(
    b: &mut B,
    dx: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    emit_round218_b5_full_source_stream_transport_with_coeff_and_finish(
        b,
        dx,
        v,
        r,
        p,
        emit_round218_unscaled_coeff_b5_block_selected,
        |b, _zeta, f, _g, v, _r, p| {
            b.set_phase("round218_b5_full_source_stream_normalize_inverse_sign");
            let final_f_sign = round218_b5_final_f_sign(f);
            emit_round218_cmod_neg_canonical(b, v, final_f_sign, p);
        },
    );
}

pub(super) fn emit_round315_b5_hash_history_full_source_stream_scaled_inverse(
    b: &mut B,
    dx: &[QubitId],
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    emit_round315_b5_hash_history_full_source_stream_transport_with_coeff_and_finish(
        b,
        dx,
        v,
        r,
        p,
        emit_round218_unscaled_coeff_b5_block_selected,
        |b, _zeta, f, _g, v, _r, p| {
            b.set_phase("round315_b5_hash_history_normalize_inverse_sign");
            let final_f_sign = round218_b5_final_f_sign(f);
            emit_round218_cmod_neg_canonical(b, v, final_f_sign, p);
        },
    );
}

pub(super) fn emit_round218_b5_full_source_stream_scaled_inverse_from_zero(
    b: &mut B,
    dx: &[QubitId],
    v: &[QubitId],
    p: U256,
) {
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    let r = b.alloc_qubits(N);
    b.x(r[0]);
    emit_round218_b5_full_source_stream_scaled_inverse(b, dx, v, &r, p);
    b.free_vec(&r);
}

fn round218_b5_final_f_sign(f: &[QubitId]) -> QubitId {
    assert!(
        f.len() > 1,
        "Round218 final f sign needs the retained 2-bit signed endpoint"
    );
    f[1]
}

pub(super) fn emit_round218_b5_full_source_stream_quotient_lowerer(
    b: &mut B,
    h: &[QubitId],
    n: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 quotient lowerer is secp256k1-only"
    );
    assert_eq!(h.len(), N, "quotient denominator must be 256 qubits");
    assert_eq!(n.len(), N, "quotient numerator must be 256 qubits");

    let scaled_quotient = b.alloc_qubits(N);
    b.set_phase("round218_b5_full_source_stream_quotient_scaled");
    emit_round218_b5_full_source_stream_scaled_inverse(b, h, &scaled_quotient, n, p);

    b.set_phase("round218_b5_full_source_stream_quotient_unscale");
    for _ in 0..phase_b5_execution::ROUND218_B5_STEPS {
        super::mod_halve_inplace_fast(b, &scaled_quotient, p);
    }

    b.set_phase("round218_b5_full_source_stream_quotient_swap");
    for i in 0..N {
        b.swap(n[i], scaled_quotient[i]);
    }
    emit_round218_b5_forward_quotient_endpoint_cleanup(b, h, &scaled_quotient, n, p);
    b.free_vec(&scaled_quotient);
}

pub(super) fn emit_round218_b5_full_source_stream_product_lowerer(
    b: &mut B,
    h: &[QubitId],
    n: &[QubitId],
    p: U256,
) {
    assert_eq!(p, SECP256K1_P, "Round218 product lowerer is secp256k1-only");
    assert_eq!(h.len(), N, "product factor must be 256 qubits");
    assert_eq!(n.len(), N, "product target must be 256 qubits");

    let product = b.alloc_qubits(N);
    b.set_phase("round218_b5_full_source_stream_product_scale");
    for _ in 0..phase_b5_execution::ROUND218_B5_STEPS {
        super::mod_double_inplace_fast(b, n, p);
    }

    b.set_phase("round218_b5_full_source_stream_product_inverse_transport");
    super::emit_inverse_measurement_clean_scoped(b, |b| {
        emit_round218_b5_full_source_stream_scaled_inverse(b, h, n, &product, p);
    });

    b.set_phase("round218_b5_full_source_stream_product_swap");
    for i in 0..N {
        b.swap(n[i], product[i]);
    }
    emit_round218_b5_reverse_product_endpoint_cleanup(b, h, &product, n, p);
    b.free_vec(&product);
}

fn round218_b5_fast_coeff_step_enabled() -> bool {
    std::env::var("ROUND218_B5_FAST_COEFF_STEP").ok().as_deref() == Some("1")
}

fn round218_b5_lazy_block_transport_enabled() -> bool {
    std::env::var(ROUND218_B5_LAZY_BLOCK_TRANSPORT_ENV)
        .ok()
        .as_deref()
        != Some("0")
}

fn round218_b5_small_div32_enabled() -> bool {
    std::env::var(ROUND218_B5_SMALL_DIV32_ENV).ok().as_deref() != Some("0")
}

fn round218_b5_selected_one_div32_enabled() -> bool {
    std::env::var(ROUND218_B5_SELECTED_ONE_DIV32_ENV)
        .ok()
        .as_deref()
        == Some("1")
}

fn round218_b5_fixed_zeta_one_div32_enabled() -> bool {
    std::env::var(ROUND218_B5_FIXED_ZETA_ONE_DIV32_ENV)
        .ok()
        .as_deref()
        == Some("1")
}

fn round218_b5_noncanonical_branch_neg_enabled() -> bool {
    std::env::var(ROUND218_B5_NONCANONICAL_BRANCH_NEG_ENV)
        .ok()
        .as_deref()
        != Some("0")
}

pub(super) fn emit_round218_scaled_coeff_b5_block_selected(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    p: U256,
) {
    emit_round218_scaled_coeff_b5_block_selected_with_branch_neg(
        b,
        v,
        r,
        branch_word,
        old_g0_word,
        p,
        true,
    );
}

fn emit_round218_scaled_coeff_b5_block_selected_with_branch_neg(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    p: U256,
    allow_noncanonical_branch_neg: bool,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 coefficient block is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "branch word must be a B=5 word"
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS,
        "old_g0 word must be a B=5 word"
    );

    if round218_b5_selected_one_div32_enabled() {
        emit_round218_scaled_coeff_b5_block_selected_lulu_one_div32(
            b,
            v,
            r,
            branch_word,
            old_g0_word,
            p,
        );
    } else if round218_b5_lazy_block_transport_enabled() {
        emit_round218_scaled_coeff_b5_block_selected_lazy_with_branch_neg(
            b,
            v,
            r,
            branch_word,
            old_g0_word,
            p,
            allow_noncanonical_branch_neg,
        );
    } else {
        for i in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
            emit_round218_scaled_coeff_step_selected(b, v, r, branch_word[i], old_g0_word[i], p);
        }
    }
}

fn emit_round218_scaled_coeff_b5_block_selected_lulu_one_div32(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 selected one-div32 block is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );

    b.set_phase("round218_b5_coeff_selected_lulu_l1");
    emit_round218_selected_small_signed_multiple_product(
        b,
        r,
        v,
        branch_word,
        old_g0_word,
        Round218B5SelectedCoeff::LuluQ1,
        p,
    );
    b.set_phase("round218_b5_coeff_selected_lulu_u2");
    emit_round218_selected_small_signed_multiple_product(
        b,
        v,
        r,
        branch_word,
        old_g0_word,
        Round218B5SelectedCoeff::LuluQ2,
        p,
    );
    b.set_phase("round218_b5_coeff_selected_lulu_l3");
    emit_round218_selected_small_signed_multiple_product(
        b,
        r,
        v,
        branch_word,
        old_g0_word,
        Round218B5SelectedCoeff::LuluQ3,
        p,
    );
    b.set_phase("round218_b5_coeff_selected_lulu_u4");
    emit_round218_selected_small_signed_multiple_product(
        b,
        v,
        r,
        branch_word,
        old_g0_word,
        Round218B5SelectedCoeff::LuluQ4,
        p,
    );

    b.set_phase("round218_b5_coeff_selected_lulu_div32_bottom");
    emit_round218_mod_div32_canonical(b, v, p);

    b.set_phase("round218_b5_coeff_selected_lulu_finish_top");
    emit_round218_selected_small_signed_multiple_product(
        b,
        r,
        v,
        branch_word,
        old_g0_word,
        Round218B5SelectedCoeff::MinusT,
        p,
    );
    let ctrl = b.alloc_qubit();
    b.x(ctrl);
    emit_round218_cmod_neg_canonical(b, r, ctrl, p);
    b.x(ctrl);
    b.free(ctrl);
    for i in 0..N {
        b.swap(v[i], r[i]);
    }
}

fn emit_round218_scaled_coeff_b5_block_fixed_zeta_lulu_one_div32(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    old_g0_word: &[QubitId],
    zeta_start: i128,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 fixed-zeta one-div32 block is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );

    b.set_phase("round218_b5_coeff_fixed_zeta_lulu_l1");
    emit_round218_fixed_zeta_small_signed_multiple_product(
        b,
        r,
        v,
        old_g0_word,
        zeta_start,
        Round218B5SelectedCoeff::LuluQ1,
        p,
    );
    b.set_phase("round218_b5_coeff_fixed_zeta_lulu_u2");
    emit_round218_fixed_zeta_small_signed_multiple_product(
        b,
        v,
        r,
        old_g0_word,
        zeta_start,
        Round218B5SelectedCoeff::LuluQ2,
        p,
    );
    b.set_phase("round218_b5_coeff_fixed_zeta_lulu_l3");
    emit_round218_fixed_zeta_small_signed_multiple_product(
        b,
        r,
        v,
        old_g0_word,
        zeta_start,
        Round218B5SelectedCoeff::LuluQ3,
        p,
    );
    b.set_phase("round218_b5_coeff_fixed_zeta_lulu_u4");
    emit_round218_fixed_zeta_small_signed_multiple_product(
        b,
        v,
        r,
        old_g0_word,
        zeta_start,
        Round218B5SelectedCoeff::LuluQ4,
        p,
    );

    b.set_phase("round218_b5_coeff_fixed_zeta_lulu_div32_bottom");
    emit_round218_mod_div32_canonical(b, v, p);

    b.set_phase("round218_b5_coeff_fixed_zeta_lulu_finish_top");
    emit_round218_fixed_zeta_small_signed_multiple_product(
        b,
        r,
        v,
        old_g0_word,
        zeta_start,
        Round218B5SelectedCoeff::MinusT,
        p,
    );
    let ctrl = b.alloc_qubit();
    b.x(ctrl);
    emit_round218_cmod_neg_canonical(b, r, ctrl, p);
    b.x(ctrl);
    b.free(ctrl);
    for i in 0..N {
        b.swap(v[i], r[i]);
    }
}

fn emit_round218_scaled_coeff_b5_block_selected_lazy(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    p: U256,
) {
    emit_round218_scaled_coeff_b5_block_selected_lazy_with_branch_neg(
        b,
        v,
        r,
        branch_word,
        old_g0_word,
        p,
        true,
    );
}

fn emit_round218_scaled_coeff_b5_block_selected_lazy_with_branch_neg(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    p: U256,
    allow_noncanonical_branch_neg: bool,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 lazy coefficient block is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );

    for i in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        emit_round218_scaled_coeff_unscaled_step_selected(
            b,
            v,
            r,
            branch_word[i],
            old_g0_word[i],
            p,
            allow_noncanonical_branch_neg,
        );
    }
    b.set_phase("round218_b5_coeff_lazy_block_div32_v");
    emit_round218_mod_div32_canonical(b, v, p);
    b.set_phase("round218_b5_coeff_lazy_block_div32_r");
    emit_round218_mod_div32_canonical(b, r, p);
}

fn emit_round218_unscaled_coeff_b5_block_selected(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 unscaled coefficient block is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );

    for i in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        emit_round218_scaled_coeff_unscaled_step_selected(
            b,
            v,
            r,
            branch_word[i],
            old_g0_word[i],
            p,
            true,
        );
    }
}

fn emit_round218_scaled_coeff_unscaled_step_selected(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch: QubitId,
    old_g0: QubitId,
    p: U256,
    allow_noncanonical_branch_neg: bool,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 coefficient step is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    b.set_phase("round218_b5_coeff_lazy_branch_cswap");
    for i in 0..N {
        super::cswap(b, branch, v[i], r[i]);
    }
    b.set_phase("round218_b5_coeff_lazy_branch_neg_r");
    if allow_noncanonical_branch_neg && round218_b5_noncanonical_branch_neg_enabled() {
        super::by_cmod_neg_inplace_fast(b, r, branch, p);
    } else {
        super::by_cmod_neg_inplace_canonical_for_bench(b, r, branch, p);
    }
    b.set_phase("round218_b5_coeff_lazy_odd_add_v_to_r");
    super::cmod_add_qq(b, r, v, old_g0, p);
    b.set_phase("round218_b5_coeff_lazy_double_v");
    super::mod_double_inplace_fast(b, v, p);
}

pub(super) fn emit_round218_scaled_coeff_step_selected(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch: QubitId,
    old_g0: QubitId,
    p: U256,
) {
    if round218_b5_fast_coeff_step_enabled() {
        emit_round218_scaled_coeff_step_fast(b, v, r, branch, old_g0, p);
    } else {
        emit_round218_scaled_coeff_step(b, v, r, branch, old_g0, p);
    }
}

fn emit_round218_b5_first_quotient_transport_block_from_live_dx(
    b: &mut B,
    dx: &[QubitId],
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 first quotient block is secp256k1-only"
    );
    assert_eq!(dx.len(), N, "round218 live denominator must be 256 qubits");

    let f_window = b.alloc_qubits(phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS);
    let v = b.alloc_qubits(N);
    let r = b.alloc_qubits(N);
    let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    let next_f_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    let next_g_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

    b.set_phase("round218_b5_pa_first_block_init");
    for (i, &q) in f_window.iter().enumerate() {
        if p.bit(i) {
            b.x(q);
        }
    }
    b.x(r[0]);

    emit_round218_b5_source_window_transport_block(
        b,
        &f_window,
        &dx[..phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS],
        &v,
        &r,
        -1,
        &branch_word,
        &old_g0_word,
        &next_f_low,
        &next_g_low,
        p,
    );
}

pub(super) fn emit_round218_scaled_coeff_step(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch: QubitId,
    old_g0: QubitId,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 coefficient step is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    b.set_phase("round218_b5_coeff_branch_cswap");
    for i in 0..N {
        super::cswap(b, branch, v[i], r[i]);
    }
    b.set_phase("round218_b5_coeff_branch_neg_r");
    emit_round218_cmod_neg_canonical(b, r, branch, p);
    b.set_phase("round218_b5_coeff_odd_add_v_to_r");
    emit_round218_cmod_add_qq_exact(b, r, v, old_g0, p);
    b.set_phase("round218_b5_coeff_halve_r");
    emit_round218_mod_halve_canonical(b, r, p);
}

pub(super) fn emit_round218_scaled_coeff_step_fast(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch: QubitId,
    old_g0: QubitId,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 coefficient step is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    b.set_phase("round218_b5_coeff_fast_branch_cswap");
    for i in 0..N {
        super::cswap(b, branch, v[i], r[i]);
    }
    b.set_phase("round218_b5_coeff_fast_branch_neg_r");
    super::by_cmod_neg_inplace_canonical_for_bench(b, r, branch, p);
    b.set_phase("round218_b5_coeff_fast_odd_add_v_to_r");
    super::cmod_add_qq(b, r, v, old_g0, p);
    b.set_phase("round218_b5_coeff_fast_halve_r");
    emit_round218_mod_halve_canonical(b, r, p);
}

pub(super) fn emit_round218_scaled_coeff_step_inverse(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    branch: QubitId,
    old_g0: QubitId,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 coefficient step is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");

    b.set_phase("round218_b5_coeff_inverse_double_r");
    super::mod_double_inplace(b, r, p);
    b.set_phase("round218_b5_coeff_inverse_odd_sub_v_from_r");
    emit_round218_cmod_sub_qq_exact(b, r, v, old_g0, p);
    b.set_phase("round218_b5_coeff_inverse_branch_neg_r");
    emit_round218_cmod_neg_canonical(b, r, branch, p);
    b.set_phase("round218_b5_coeff_inverse_branch_cswap");
    for i in (0..N).rev() {
        super::cswap(b, branch, v[i], r[i]);
    }
}

pub(super) fn emit_round218_scaled_coeff_block_fixed(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    row: &phase_b5_execution::BlockRow,
    p: U256,
) {
    match round218_b5_fixed_block_lowerer() {
        Round218B5FixedBlockLowerer::Stepwise => {
            emit_round218_scaled_coeff_block_fixed_stepwise(b, v, r, row, p);
        }
        Round218B5FixedBlockLowerer::Exact => {
            emit_round218_scaled_coeff_block_fixed_exact(b, v, r, row, p);
        }
        Round218B5FixedBlockLowerer::LuluOneDiv32 => {
            emit_round218_scaled_coeff_block_fixed_one_div32(b, v, r, row, p);
        }
        Round218B5FixedBlockLowerer::DyadicLift => {
            emit_round218_scaled_coeff_block_fixed_dyadic_lift(b, v, r, row, p);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Round218B5FixedBlockLowerer {
    Stepwise,
    Exact,
    LuluOneDiv32,
    DyadicLift,
}

fn round218_b5_fixed_block_lowerer() -> Round218B5FixedBlockLowerer {
    match std::env::var(ROUND218_B5_FIXED_BLOCK_LOWERER_ENV) {
        Ok(value) => match value.as_str() {
            "" | "1" | "stepwise" | "phase-clean-stepwise" | "fast-stepwise" => {
                Round218B5FixedBlockLowerer::Stepwise
            }
            "exact" | "matrix" => Round218B5FixedBlockLowerer::Exact,
            "one-div32" | "lulu-one-div32" | "lulu" => Round218B5FixedBlockLowerer::LuluOneDiv32,
            "dyadic-lift" | "inplace-dyadic" | "lift" => Round218B5FixedBlockLowerer::DyadicLift,
            other => panic!(
                "{ROUND218_B5_FIXED_BLOCK_LOWERER_ENV} must be exact, matrix, stepwise, \
                 phase-clean-stepwise, fast-stepwise, one-div32, lulu-one-div32, lulu, \
                 dyadic-lift, inplace-dyadic, lift, or 1; got {other}"
            ),
        },
        Err(_) => Round218B5FixedBlockLowerer::Stepwise,
    }
}

fn round218_b5_fixed_block_exact_enabled() -> bool {
    round218_b5_fixed_block_lowerer() == Round218B5FixedBlockLowerer::Exact
}

pub(super) fn emit_round218_scaled_coeff_block_fixed_exact(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    row: &phase_b5_execution::BlockRow,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 coefficient block is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        row.matrix.denominator_log2,
        phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8
    );
    assert_eq!(row.matrix.numerator.det(), row.matrix.denominator());

    let out_v = b.alloc_qubits(N);
    let out_r = b.alloc_qubits(N);

    b.set_phase("round218_b5_coeff_block_compute_v");
    emit_round218_add_small_signed_multiple(b, &out_v, v, row.matrix.numerator.a00, p);
    emit_round218_add_small_signed_multiple(b, &out_v, r, row.matrix.numerator.a01, p);
    b.set_phase("round218_b5_coeff_block_compute_r");
    emit_round218_add_small_signed_multiple(b, &out_r, v, row.matrix.numerator.a10, p);
    emit_round218_add_small_signed_multiple(b, &out_r, r, row.matrix.numerator.a11, p);

    b.set_phase("round218_b5_coeff_block_scale_v");
    for _ in 0..row.matrix.denominator_log2 {
        emit_round218_mod_halve_canonical(b, &out_v, p);
    }
    b.set_phase("round218_b5_coeff_block_scale_r");
    for _ in 0..row.matrix.denominator_log2 {
        emit_round218_mod_halve_canonical(b, &out_r, p);
    }

    b.set_phase("round218_b5_coeff_block_swap_outputs");
    for i in 0..N {
        b.swap(v[i], out_v[i]);
        b.swap(r[i], out_r[i]);
    }

    // If y = A*x/32 and det(A)=32, then x = adj(A)*y.  After the swaps the
    // temporaries hold the old x; subtract adj(A)*new_y to clean them.
    b.set_phase("round218_b5_coeff_block_clean_old_v");
    emit_round218_add_small_signed_multiple(b, &out_v, v, -row.matrix.numerator.a11, p);
    emit_round218_add_small_signed_multiple(b, &out_v, r, row.matrix.numerator.a01, p);
    b.set_phase("round218_b5_coeff_block_clean_old_r");
    emit_round218_add_small_signed_multiple(b, &out_r, v, row.matrix.numerator.a10, p);
    emit_round218_add_small_signed_multiple(b, &out_r, r, -row.matrix.numerator.a00, p);

    b.free_vec(&out_r);
    b.free_vec(&out_v);
}

pub(super) fn emit_round218_scaled_coeff_block_fixed_one_div32(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    row: &phase_b5_execution::BlockRow,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 coefficient block is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        row.matrix.denominator_log2,
        phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8
    );
    assert_eq!(row.matrix.numerator.det(), row.matrix.denominator());

    let a = row.matrix.numerator.a00;
    let b_coeff = row.matrix.numerator.a01;
    let c = row.matrix.numerator.a10;
    let d = row.matrix.numerator.a11;
    let (u, w) = round218_b5_small_bezout(c, d);
    assert_eq!(c * u + d * w, 1, "bottom row must be primitive");
    let t = a * u + b_coeff * w;
    let preconditioner = phase_b5_execution::Matrix2 {
        a00: c,
        a01: d,
        a10: -w,
        a11: u,
    };
    assert_eq!(preconditioner.det(), 1, "preconditioner must be unimodular");

    b.set_phase("round218_b5_coeff_fixed_one_div32_precondition");
    for shear in round218_b5_unimodular_shear_schedule(preconditioner) {
        if shear.target_v {
            emit_round218_add_small_signed_multiple_fast(b, v, r, shear.coeff, p);
        } else {
            emit_round218_add_small_signed_multiple_fast(b, r, v, shear.coeff, p);
        }
    }

    b.set_phase("round218_b5_coeff_fixed_one_div32_scale_bottom");
    emit_round218_mod_div32_canonical(b, v, p);

    b.set_phase("round218_b5_coeff_fixed_one_div32_finish_top");
    emit_round218_add_small_signed_multiple_fast(b, r, v, -t, p);
    let ctrl = b.alloc_qubit();
    b.x(ctrl);
    emit_round218_cmod_neg_canonical(b, r, ctrl, p);
    b.x(ctrl);
    b.free(ctrl);
    for i in 0..N {
        b.swap(v[i], r[i]);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Round218B5DyadicLiftOp {
    AddRToV,
    SubRFromV,
    AddVToR,
    SubVFromR,
    HalveV,
    HalveR,
}

impl Round218B5DyadicLiftOp {
    const ALL: [Self; 6] = [
        Self::AddRToV,
        Self::SubRFromV,
        Self::AddVToR,
        Self::SubVFromR,
        Self::HalveV,
        Self::HalveR,
    ];

    fn apply_to_scaled_matrix(
        self,
        denominator_log2: u8,
        matrix: phase_b5_execution::Matrix2,
    ) -> Option<(u8, phase_b5_execution::Matrix2)> {
        use phase_b5_execution::Matrix2;
        match self {
            Self::AddRToV => Some((
                denominator_log2,
                Matrix2 {
                    a00: matrix.a00 + matrix.a10,
                    a01: matrix.a01 + matrix.a11,
                    a10: matrix.a10,
                    a11: matrix.a11,
                },
            )),
            Self::SubRFromV => Some((
                denominator_log2,
                Matrix2 {
                    a00: matrix.a00 - matrix.a10,
                    a01: matrix.a01 - matrix.a11,
                    a10: matrix.a10,
                    a11: matrix.a11,
                },
            )),
            Self::AddVToR => Some((
                denominator_log2,
                Matrix2 {
                    a00: matrix.a00,
                    a01: matrix.a01,
                    a10: matrix.a10 + matrix.a00,
                    a11: matrix.a11 + matrix.a01,
                },
            )),
            Self::SubVFromR => Some((
                denominator_log2,
                Matrix2 {
                    a00: matrix.a00,
                    a01: matrix.a01,
                    a10: matrix.a10 - matrix.a00,
                    a11: matrix.a11 - matrix.a01,
                },
            )),
            Self::HalveV => Some((
                denominator_log2.checked_add(1)?,
                Matrix2 {
                    a00: matrix.a00,
                    a01: matrix.a01,
                    a10: 2 * matrix.a10,
                    a11: 2 * matrix.a11,
                },
            )),
            Self::HalveR => Some((
                denominator_log2.checked_add(1)?,
                Matrix2 {
                    a00: 2 * matrix.a00,
                    a01: 2 * matrix.a01,
                    a10: matrix.a10,
                    a11: matrix.a11,
                },
            )),
        }
    }
}

fn round218_b5_dyadic_lift_matrix_norm(matrix: phase_b5_execution::Matrix2) -> i128 {
    [matrix.a00, matrix.a01, matrix.a10, matrix.a11]
        .into_iter()
        .map(i128::abs)
        .max()
        .unwrap_or(0)
}

fn round218_b5_dyadic_lift_state_key(
    denominator_log2: u8,
    matrix: phase_b5_execution::Matrix2,
) -> (u8, i128, i128, i128, i128) {
    (
        denominator_log2,
        matrix.a00,
        matrix.a01,
        matrix.a10,
        matrix.a11,
    )
}

fn round218_b5_dyadic_lift_schedule(
    target: phase_b5_execution::Matrix2,
    target_denominator_log2: u8,
) -> Vec<Round218B5DyadicLiftOp> {
    const MAX_DEPTH: usize = 10;
    const MAX_ABS_ENTRY: i128 = 128;

    let mut seen: Vec<(
        u8,
        phase_b5_execution::Matrix2,
        Vec<Round218B5DyadicLiftOp>,
    )> = vec![(0, phase_b5_execution::Matrix2::IDENTITY, Vec::new())];
    let mut seen_keys = HashSet::from([round218_b5_dyadic_lift_state_key(
        0,
        phase_b5_execution::Matrix2::IDENTITY,
    )]);
    let mut frontier = vec![0usize];

    for _depth in 0..MAX_DEPTH {
        let mut next_frontier = Vec::new();
        for &idx in &frontier {
            let (denominator_log2, matrix, schedule) = seen[idx].clone();
            for op in Round218B5DyadicLiftOp::ALL {
                let Some((next_denominator_log2, next_matrix)) =
                    op.apply_to_scaled_matrix(denominator_log2, matrix)
                else {
                    continue;
                };
                if next_denominator_log2 > target_denominator_log2 {
                    continue;
                }
                if round218_b5_dyadic_lift_matrix_norm(next_matrix) > MAX_ABS_ENTRY {
                    continue;
                }
                let next_key =
                    round218_b5_dyadic_lift_state_key(next_denominator_log2, next_matrix);
                if !seen_keys.insert(next_key) {
                    continue;
                }

                let mut next_schedule = schedule.clone();
                next_schedule.push(op);
                if next_denominator_log2 == target_denominator_log2 && next_matrix == target {
                    return next_schedule;
                }
                seen.push((next_denominator_log2, next_matrix, next_schedule));
                next_frontier.push(seen.len() - 1);
            }
        }
        frontier = next_frontier;
    }

    panic!(
        "no Round218 B=5 dyadic-lift schedule for target {target:?}/2^{target_denominator_log2}"
    );
}

pub(super) fn emit_round218_scaled_coeff_block_fixed_dyadic_lift(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    row: &phase_b5_execution::BlockRow,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 dyadic-lift coefficient block is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        row.matrix.denominator_log2,
        phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8
    );
    assert_eq!(row.matrix.numerator.det(), row.matrix.denominator());

    let schedule =
        round218_b5_dyadic_lift_schedule(row.matrix.numerator, row.matrix.denominator_log2);
    for op in schedule {
        match op {
            Round218B5DyadicLiftOp::AddRToV => {
                b.set_phase("round218_b5_coeff_fixed_dyadic_lift_v_add_r");
                super::mod_add_qq_fast(b, v, r, p);
            }
            Round218B5DyadicLiftOp::SubRFromV => {
                b.set_phase("round218_b5_coeff_fixed_dyadic_lift_v_sub_r");
                super::mod_sub_qq_fast(b, v, r, p);
            }
            Round218B5DyadicLiftOp::AddVToR => {
                b.set_phase("round218_b5_coeff_fixed_dyadic_lift_r_add_v");
                super::mod_add_qq_fast(b, r, v, p);
            }
            Round218B5DyadicLiftOp::SubVFromR => {
                b.set_phase("round218_b5_coeff_fixed_dyadic_lift_r_sub_v");
                super::mod_sub_qq_fast(b, r, v, p);
            }
            Round218B5DyadicLiftOp::HalveV => {
                b.set_phase("round218_b5_coeff_fixed_dyadic_lift_halve_v");
                emit_round218_mod_halve_canonical(b, v, p);
            }
            Round218B5DyadicLiftOp::HalveR => {
                b.set_phase("round218_b5_coeff_fixed_dyadic_lift_halve_r");
                emit_round218_mod_halve_canonical(b, r, p);
            }
        }
    }
}

pub(super) fn emit_round218_scaled_coeff_block_fixed_stepwise(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    row: &phase_b5_execution::BlockRow,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 coefficient block is secp256k1-only"
    );
    assert_eq!(v.len(), N, "coefficient V must be 256 qubits");
    assert_eq!(r.len(), N, "coefficient R must be 256 qubits");
    assert_eq!(
        row.matrix.denominator_log2,
        phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8
    );
    assert_eq!(row.matrix.numerator.det(), row.matrix.denominator());

    if row
        .step_kinds
        .iter()
        .all(|kind| *kind == phase_b5_execution::StepKind::NonbranchEven)
    {
        b.set_phase("round218_b5_coeff_fixed_stepwise_even_div32");
        emit_round218_mod_div32_canonical(b, r, p);
        return;
    }

    for kind in row.step_kinds.iter().copied() {
        match kind {
            phase_b5_execution::StepKind::PositiveOdd => {
                b.set_phase("round218_b5_coeff_fixed_stepwise_positive");
                emit_round218_scaled_coeff_positive_odd_step_fixed(b, v, r, p);
            }
            phase_b5_execution::StepKind::NonbranchEven => {
                b.set_phase("round218_b5_coeff_fixed_stepwise_even");
                emit_round218_mod_halve_canonical(b, r, p);
            }
            phase_b5_execution::StepKind::NonbranchOdd => {
                b.set_phase("round218_b5_coeff_fixed_stepwise_odd");
                super::mod_add_qq_fast(b, r, v, p);
                emit_round218_mod_halve_canonical(b, r, p);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Round218B5Shear {
    target_v: bool,
    coeff: i128,
}

fn round218_b5_small_bezout(c: i128, d: i128) -> (i128, i128) {
    let (g, u0, w0) = round218_b5_extended_gcd(c, d);
    assert_eq!(g, 1);
    let mut best = (u0, w0);
    for shift in -128..=128 {
        let candidate = (u0 + d * shift, w0 - c * shift);
        let best_norm = best.0.abs().max(best.1.abs());
        let candidate_norm = candidate.0.abs().max(candidate.1.abs());
        if candidate_norm < best_norm {
            best = candidate;
        }
    }
    best
}

fn round218_b5_extended_gcd(a: i128, b: i128) -> (i128, i128, i128) {
    let mut old_r = a.abs();
    let mut r = b.abs();
    let mut old_s = 1i128;
    let mut s = 0i128;
    let mut old_t = 0i128;
    let mut t = 1i128;
    while r != 0 {
        let q = old_r / r;
        (old_r, r) = (r, old_r - q * r);
        (old_s, s) = (s, old_s - q * s);
        (old_t, t) = (t, old_t - q * t);
    }
    if a < 0 {
        old_s = -old_s;
    }
    if b < 0 {
        old_t = -old_t;
    }
    (old_r, old_s, old_t)
}

fn round218_b5_unimodular_shear_schedule(
    target: phase_b5_execution::Matrix2,
) -> Vec<Round218B5Shear> {
    const MAX_DEPTH: usize = 7;
    const MAX_ABS_ENTRY: i128 = 64;
    const MAX_SHEAR: i128 = 8;

    let identity = phase_b5_execution::Matrix2::IDENTITY;
    let mut queue = std::collections::VecDeque::new();
    let mut seen: Vec<(phase_b5_execution::Matrix2, Vec<Round218B5Shear>)> =
        vec![(identity, Vec::new())];
    queue.push_back(identity);

    while let Some(current) = queue.pop_front() {
        let current_schedule = seen
            .iter()
            .find_map(|(matrix, schedule)| (*matrix == current).then(|| schedule.clone()))
            .expect("queued matrix must be seen");
        if current == target {
            return current_schedule;
        }
        if current_schedule.len() >= MAX_DEPTH {
            continue;
        }
        for coeff in -MAX_SHEAR..=MAX_SHEAR {
            if coeff == 0 {
                continue;
            }
            for &target_v in &[true, false] {
                let op = if target_v {
                    phase_b5_execution::Matrix2 {
                        a00: 1,
                        a01: coeff,
                        a10: 0,
                        a11: 1,
                    }
                } else {
                    phase_b5_execution::Matrix2 {
                        a00: 1,
                        a01: 0,
                        a10: coeff,
                        a11: 1,
                    }
                };
                let next = op.mul(current);
                if [next.a00, next.a01, next.a10, next.a11]
                    .iter()
                    .any(|entry| entry.abs() > MAX_ABS_ENTRY)
                {
                    continue;
                }
                if seen.iter().any(|(matrix, _)| *matrix == next) {
                    continue;
                }
                let mut schedule = current_schedule.clone();
                schedule.push(Round218B5Shear { target_v, coeff });
                if next == target {
                    return schedule;
                }
                seen.push((next, schedule));
                queue.push_back(next);
            }
        }
    }
    panic!("no Round218 B=5 shear schedule for {target:?}");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Round218B5SelectedCoeff {
    LuluQ1,
    LuluQ2,
    LuluQ3,
    LuluQ4,
    MinusT,
}

impl Round218B5SelectedCoeff {
    fn max_mag_bits(self) -> usize {
        match self {
            Self::LuluQ1 | Self::LuluQ2 | Self::LuluQ3 => 4,
            Self::LuluQ4 => 5,
            Self::MinusT => 6,
        }
    }
}

#[derive(Clone, Debug)]
struct Round218B5DecodedCoeff {
    sign: Option<QubitId>,
    mag: Vec<(usize, QubitId)>,
}

fn emit_round218_selected_small_signed_multiple_product(
    b: &mut B,
    acc: &[QubitId],
    src: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    coeff: Round218B5SelectedCoeff,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 selected product shear is secp256k1-only"
    );
    assert_eq!(acc.len(), N);
    assert_eq!(src.len(), N);
    assert_eq!(
        branch_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );

    let decoded = emit_round218_compute_selected_coeff_bits(b, branch_word, old_g0_word, coeff);
    emit_round218_add_signmag_small_product_modp(b, acc, src, decoded.sign, &decoded.mag, p);
    emit_round218_uncompute_selected_coeff_bits(b, branch_word, old_g0_word, coeff, decoded);
}

fn emit_round218_fixed_zeta_small_signed_multiple_product(
    b: &mut B,
    acc: &[QubitId],
    src: &[QubitId],
    old_g0_word: &[QubitId],
    zeta_start: i128,
    coeff: Round218B5SelectedCoeff,
    p: U256,
) {
    assert_eq!(
        p, SECP256K1_P,
        "Round218 fixed-zeta selected product shear is secp256k1-only"
    );
    assert_eq!(acc.len(), N);
    assert_eq!(src.len(), N);
    assert_eq!(
        old_g0_word.len(),
        phase_b5_execution::ROUND218_B5_BLOCK_BITS
    );

    let decoded = emit_round218_compute_fixed_zeta_coeff_bits(b, old_g0_word, zeta_start, coeff);
    emit_round218_add_signmag_small_product_modp(b, acc, src, decoded.sign, &decoded.mag, p);
    emit_round218_uncompute_fixed_zeta_coeff_bits(b, old_g0_word, zeta_start, coeff, decoded);
}

fn emit_round218_compute_selected_coeff_bits(
    b: &mut B,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    coeff: Round218B5SelectedCoeff,
) -> Round218B5DecodedCoeff {
    let mut controls = branch_word.to_vec();
    controls.extend_from_slice(old_g0_word);
    let scratch = b.alloc_qubits(controls.len().saturating_sub(2));

    let sign_masks = round218_b5_selected_coeff_anf_masks(coeff, None);
    let sign = if sign_masks.is_empty() {
        None
    } else {
        let q = b.alloc_qubit();
        for mask in sign_masks {
            emit_round218_monomial_toggle_u16(b, &controls, mask, q, &scratch);
        }
        Some(q)
    };

    let mut mag = Vec::new();
    for bit_idx in 0..coeff.max_mag_bits() {
        let masks = round218_b5_selected_coeff_anf_masks(coeff, Some(bit_idx));
        if masks.is_empty() {
            continue;
        }
        let q = b.alloc_qubit();
        for mask in masks {
            emit_round218_monomial_toggle_u16(b, &controls, mask, q, &scratch);
        }
        mag.push((bit_idx, q));
    }

    b.free_vec(&scratch);
    Round218B5DecodedCoeff { sign, mag }
}

fn emit_round218_compute_fixed_zeta_coeff_bits(
    b: &mut B,
    old_g0_word: &[QubitId],
    zeta_start: i128,
    coeff: Round218B5SelectedCoeff,
) -> Round218B5DecodedCoeff {
    let scratch = b.alloc_qubits(old_g0_word.len().saturating_sub(2));

    let sign_masks = round218_b5_fixed_zeta_coeff_anf_masks(zeta_start, coeff, None);
    let sign = if sign_masks.is_empty() {
        None
    } else {
        let q = b.alloc_qubit();
        for mask in sign_masks {
            emit_round218_monomial_toggle_u8(b, old_g0_word, mask, q, &scratch);
        }
        Some(q)
    };

    let mut mag = Vec::new();
    for bit_idx in 0..coeff.max_mag_bits() {
        let masks = round218_b5_fixed_zeta_coeff_anf_masks(zeta_start, coeff, Some(bit_idx));
        if masks.is_empty() {
            continue;
        }
        let q = b.alloc_qubit();
        for mask in masks {
            emit_round218_monomial_toggle_u8(b, old_g0_word, mask, q, &scratch);
        }
        mag.push((bit_idx, q));
    }

    b.free_vec(&scratch);
    Round218B5DecodedCoeff { sign, mag }
}

fn emit_round218_uncompute_selected_coeff_bits(
    b: &mut B,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    coeff: Round218B5SelectedCoeff,
    decoded: Round218B5DecodedCoeff,
) {
    let mut controls = branch_word.to_vec();
    controls.extend_from_slice(old_g0_word);
    let scratch = b.alloc_qubits(controls.len().saturating_sub(2));

    for (bit_idx, q) in decoded.mag.iter().copied() {
        let masks = round218_b5_selected_coeff_anf_masks(coeff, Some(bit_idx));
        for mask in masks {
            emit_round218_monomial_toggle_u16(b, &controls, mask, q, &scratch);
        }
    }
    if let Some(q) = decoded.sign {
        let sign_masks = round218_b5_selected_coeff_anf_masks(coeff, None);
        for mask in sign_masks {
            emit_round218_monomial_toggle_u16(b, &controls, mask, q, &scratch);
        }
        b.free(q);
    }
    for &(_, q) in &decoded.mag {
        b.free(q);
    }
    b.free_vec(&scratch);
}

fn emit_round218_uncompute_fixed_zeta_coeff_bits(
    b: &mut B,
    old_g0_word: &[QubitId],
    zeta_start: i128,
    coeff: Round218B5SelectedCoeff,
    decoded: Round218B5DecodedCoeff,
) {
    let scratch = b.alloc_qubits(old_g0_word.len().saturating_sub(2));

    for (bit_idx, q) in decoded.mag.iter().copied() {
        let masks = round218_b5_fixed_zeta_coeff_anf_masks(zeta_start, coeff, Some(bit_idx));
        for mask in masks {
            emit_round218_monomial_toggle_u8(b, old_g0_word, mask, q, &scratch);
        }
    }
    if let Some(q) = decoded.sign {
        let sign_masks = round218_b5_fixed_zeta_coeff_anf_masks(zeta_start, coeff, None);
        for mask in sign_masks {
            emit_round218_monomial_toggle_u8(b, old_g0_word, mask, q, &scratch);
        }
        b.free(q);
    }
    for &(_, q) in &decoded.mag {
        b.free(q);
    }
    b.free_vec(&scratch);
}

fn emit_round218_add_signmag_small_product_modp(
    b: &mut B,
    acc: &[QubitId],
    src: &[QubitId],
    sign: Option<QubitId>,
    mag: &[(usize, QubitId)],
    p: U256,
) {
    if mag.is_empty() {
        return;
    }
    let max_shift = mag
        .iter()
        .map(|(shift, _)| *shift)
        .max()
        .expect("nonempty magnitude bits");
    let product_width = N + max_shift;
    let prod = b.alloc_qubits(product_width);

    for &(shift, q_mag) in mag {
        emit_round218_selected_product_row_addsub(b, &prod, src, shift, q_mag, true);
    }

    let delta = b.alloc_qubits(N);
    super::mod_add_qq_fast_from_zero(b, &delta, &prod[..N], p);

    let high_len = product_width.saturating_sub(N);
    if high_len == 0 {
        emit_round218_add_signed_delta_to_acc(b, acc, &delta, sign, p);
    } else {
        let correction_width = 32 + high_len;
        let corr_low = b.alloc_qubits(correction_width);
        let c = U256::from(secp256k1_c_low());
        for shift in 0..high_len {
            let bits = shifted_const_bits(c, shift, correction_width);
            emit_round218_cadd_const_bits(b, &corr_low, &bits, prod[N + shift]);
        }
        let corr_tail = b.alloc_qubits(N - correction_width);
        let mut corr = corr_low.clone();
        corr.extend_from_slice(&corr_tail);

        super::mod_add_qq_fast(b, &delta, &corr, p);
        emit_round218_add_signed_delta_to_acc(b, acc, &delta, sign, p);
        super::mod_sub_qq_fast(b, &delta, &corr, p);

        b.free_vec(&corr_tail);
        for shift in (0..high_len).rev() {
            let bits = shifted_const_bits(c, shift, correction_width);
            emit_round218_csub_const_bits(b, &corr_low, &bits, prod[N + shift]);
        }
        b.free_vec(&corr_low);
    }

    super::mod_sub_qq_fast(b, &delta, &prod[..N], p);
    b.free_vec(&delta);

    for &(shift, q_mag) in mag.iter().rev() {
        emit_round218_selected_product_row_addsub(b, &prod, src, shift, q_mag, false);
    }
    b.free_vec(&prod);
}

fn emit_round218_add_signed_delta_to_acc(
    b: &mut B,
    acc: &[QubitId],
    delta: &[QubitId],
    sign: Option<QubitId>,
    p: U256,
) {
    if let Some(sign) = sign {
        super::by_cmod_neg_inplace_fast(b, delta, sign, p);
    }
    super::mod_add_qq_fast(b, acc, delta, p);
    if let Some(sign) = sign {
        super::by_cmod_neg_inplace_fast(b, delta, sign, p);
    }
}

fn emit_round218_selected_product_row_addsub(
    b: &mut B,
    prod: &[QubitId],
    src: &[QubitId],
    shift: usize,
    q_mag: QubitId,
    add: bool,
) {
    assert!(shift < prod.len());
    assert!(src.len() + shift <= prod.len());
    let row = b.alloc_qubits(prod.len());
    for i in 0..src.len() {
        b.ccx(q_mag, src[i], row[i + shift]);
    }
    if add {
        super::add_nbit_qq_fast(b, &row, prod);
    } else {
        super::sub_nbit_qq_fast(b, &row, prod);
    }
    for i in 0..src.len() {
        let m = b.alloc_bit();
        b.hmr(row[i + shift], m);
        b.cz_if(q_mag, src[i], m);
    }
    b.free_vec(&row);
}

fn round218_b5_selected_coeff_anf_masks(
    coeff: Round218B5SelectedCoeff,
    magnitude_bit: Option<usize>,
) -> Vec<u16> {
    static MASK_CACHE: OnceLock<Vec<Vec<Vec<u16>>>> = OnceLock::new();
    let cache = MASK_CACHE.get_or_init(round218_b5_build_selected_coeff_mask_cache);
    let coeff_idx = match coeff {
        Round218B5SelectedCoeff::LuluQ1 => 0,
        Round218B5SelectedCoeff::LuluQ2 => 1,
        Round218B5SelectedCoeff::LuluQ3 => 2,
        Round218B5SelectedCoeff::LuluQ4 => 3,
        Round218B5SelectedCoeff::MinusT => 4,
    };
    let output_idx = magnitude_bit.map_or(0, |bit_idx| bit_idx + 1);
    cache[coeff_idx]
        .get(output_idx)
        .cloned()
        .unwrap_or_default()
}

fn round218_b5_fixed_zeta_coeff_anf_masks(
    zeta_start: i128,
    coeff: Round218B5SelectedCoeff,
    magnitude_bit: Option<usize>,
) -> Vec<u8> {
    let mut truth = [0u8; 1usize << phase_b5_execution::ROUND218_B5_BLOCK_BITS];
    for (old_g0_word, value) in truth.iter_mut().enumerate() {
        let selected_coeff =
            round218_b5_selected_coeff_for_zeta_old_g0(coeff, zeta_start, old_g0_word as u8);
        *value = match magnitude_bit {
            None => (selected_coeff < 0) as u8,
            Some(bit_idx) => ((selected_coeff.unsigned_abs() >> bit_idx) & 1) as u8,
        };
    }
    for bit in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        for mask in 0..truth.len() {
            if (mask & (1usize << bit)) != 0 {
                truth[mask] ^= truth[mask ^ (1usize << bit)];
            }
        }
    }
    truth
        .iter()
        .enumerate()
        .filter_map(|(mask, &coeff)| (coeff != 0).then_some(mask as u8))
        .collect()
}

fn round218_b5_build_selected_coeff_mask_cache() -> Vec<Vec<Vec<u16>>> {
    [
        Round218B5SelectedCoeff::LuluQ1,
        Round218B5SelectedCoeff::LuluQ2,
        Round218B5SelectedCoeff::LuluQ3,
        Round218B5SelectedCoeff::LuluQ4,
        Round218B5SelectedCoeff::MinusT,
    ]
    .into_iter()
    .map(|coeff| {
        (0..=coeff.max_mag_bits())
            .map(|output_idx| {
                let magnitude_bit = if output_idx == 0 {
                    None
                } else {
                    Some(output_idx - 1)
                };
                round218_b5_build_valid_domain_anf_masks(coeff, magnitude_bit)
            })
            .collect()
    })
    .collect()
}

fn round218_b5_build_valid_domain_anf_masks(
    coeff: Round218B5SelectedCoeff,
    magnitude_bit: Option<usize>,
) -> Vec<u16> {
    const SELECTOR_BITS: usize = 2 * phase_b5_execution::ROUND218_B5_BLOCK_BITS;
    const MAX_DEGREE: u32 = phase_b5_execution::ROUND218_B5_BLOCK_BITS as u32;
    let monomials: Vec<u16> = (0u16..(1u16 << SELECTOR_BITS))
        .filter(|mask| mask.count_ones() <= MAX_DEGREE)
        .collect();
    let words = (monomials.len() + 63) / 64;

    let mut rows = Vec::new();
    let mut rhs = Vec::new();
    for selector in 0u16..(1u16 << SELECTOR_BITS) {
        let branch_word =
            (selector & ((1u16 << phase_b5_execution::ROUND218_B5_BLOCK_BITS) - 1)) as u8;
        let old_g0_word = (selector >> phase_b5_execution::ROUND218_B5_BLOCK_BITS) as u8;
        let Some(selected_coeff) =
            round218_b5_selected_coeff_for_words(coeff, branch_word, old_g0_word)
        else {
            continue;
        };
        let value = match magnitude_bit {
            None => (selected_coeff < 0) as u8,
            Some(bit_idx) => ((selected_coeff.unsigned_abs() >> bit_idx) & 1) as u8,
        };
        let mut row = vec![0u64; words];
        for (col, &mask) in monomials.iter().enumerate() {
            if selector & mask == mask {
                row[col / 64] |= 1u64 << (col % 64);
            }
        }
        rows.push(row);
        rhs.push(value);
    }

    let solution = round218_b5_solve_gf2(rows, rhs, monomials.len()).unwrap_or_else(|| {
        panic!("no degree-{MAX_DEGREE} selected coefficient ANF for {coeff:?} {magnitude_bit:?}")
    });
    monomials
        .into_iter()
        .enumerate()
        .filter_map(|(col, mask)| round218_b5_bitset_get(&solution, col).then_some(mask))
        .collect()
}

fn round218_b5_solve_gf2(
    mut rows: Vec<Vec<u64>>,
    mut rhs: Vec<u8>,
    cols: usize,
) -> Option<Vec<u64>> {
    let n = rows.len();
    let mut rank = 0usize;
    let mut pivots = Vec::new();
    for col in 0..cols {
        let pivot = (rank..n).find(|&row| round218_b5_bitset_get(&rows[row], col));
        let Some(pivot) = pivot else {
            continue;
        };
        rows.swap(rank, pivot);
        rhs.swap(rank, pivot);
        for row in 0..n {
            if row == rank || !round218_b5_bitset_get(&rows[row], col) {
                continue;
            }
            let pivot_row = rows[rank].clone();
            for (dst, src) in rows[row].iter_mut().zip(pivot_row.iter()) {
                *dst ^= *src;
            }
            rhs[row] ^= rhs[rank];
        }
        pivots.push(col);
        rank += 1;
        if rank == n {
            break;
        }
    }

    for row in rank..n {
        if rows[row].iter().all(|&word| word == 0) && rhs[row] != 0 {
            return None;
        }
    }

    let mut solution = vec![0u64; (cols + 63) / 64];
    for (row, &col) in pivots.iter().enumerate() {
        if rhs[row] != 0 {
            solution[col / 64] |= 1u64 << (col % 64);
        }
    }
    Some(solution)
}

fn round218_b5_bitset_get(words: &[u64], bit: usize) -> bool {
    ((words[bit / 64] >> (bit % 64)) & 1) != 0
}

fn round218_b5_selected_coeff_for_words(
    coeff: Round218B5SelectedCoeff,
    branch_word: u8,
    old_g0_word: u8,
) -> Option<i128> {
    let numerator = round218_b5_valid_selector_numerator(branch_word, old_g0_word)?;
    let c = numerator.a10;
    let d = numerator.a11;
    let (u, w) = round218_b5_small_bezout(c, d);
    debug_assert_eq!(c * u + d * w, 1);
    let t = numerator.a00 * u + numerator.a01 * w;
    let preconditioner = phase_b5_execution::Matrix2 {
        a00: c,
        a01: d,
        a10: -w,
        a11: u,
    };
    let qs = round218_b5_lulu_shear_coeffs(preconditioner);
    Some(match coeff {
        Round218B5SelectedCoeff::LuluQ1 => qs[0],
        Round218B5SelectedCoeff::LuluQ2 => qs[1],
        Round218B5SelectedCoeff::LuluQ3 => qs[2],
        Round218B5SelectedCoeff::LuluQ4 => qs[3],
        Round218B5SelectedCoeff::MinusT => -t,
    })
}

fn round218_b5_selected_coeff_for_zeta_old_g0(
    coeff: Round218B5SelectedCoeff,
    zeta_start: i128,
    old_g0_word: u8,
) -> i128 {
    let numerator = round218_b5_numerator_for_zeta_old_g0(zeta_start, old_g0_word);
    let c = numerator.a10;
    let d = numerator.a11;
    let (u, w) = round218_b5_small_bezout(c, d);
    debug_assert_eq!(c * u + d * w, 1);
    let t = numerator.a00 * u + numerator.a01 * w;
    let preconditioner = phase_b5_execution::Matrix2 {
        a00: c,
        a01: d,
        a10: -w,
        a11: u,
    };
    let qs = round218_b5_lulu_shear_coeffs(preconditioner);
    match coeff {
        Round218B5SelectedCoeff::LuluQ1 => qs[0],
        Round218B5SelectedCoeff::LuluQ2 => qs[1],
        Round218B5SelectedCoeff::LuluQ3 => qs[2],
        Round218B5SelectedCoeff::LuluQ4 => qs[3],
        Round218B5SelectedCoeff::MinusT => -t,
    }
}

fn round218_b5_numerator_for_zeta_old_g0(
    zeta_start: i128,
    old_g0_word: u8,
) -> phase_b5_execution::Matrix2 {
    let mut zeta = zeta_start;
    let mut numerator = phase_b5_execution::Matrix2::IDENTITY;
    for idx in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        let old_g0 = ((old_g0_word >> idx) & 1) != 0;
        let kind = if zeta < 0 && old_g0 {
            zeta = -zeta - 2;
            phase_b5_execution::StepKind::PositiveOdd
        } else {
            zeta -= 1;
            if old_g0 {
                phase_b5_execution::StepKind::NonbranchOdd
            } else {
                phase_b5_execution::StepKind::NonbranchEven
            }
        };
        numerator = kind.numerator_matrix().mul(numerator);
    }
    numerator
}

fn round218_b5_valid_selector_numerator(
    branch_word: u8,
    old_g0_word: u8,
) -> Option<phase_b5_execution::Matrix2> {
    let mut numerator = phase_b5_execution::Matrix2::IDENTITY;
    for idx in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        let branch = (branch_word >> idx) & 1;
        let old_g0 = (old_g0_word >> idx) & 1;
        let kind = match (branch, old_g0) {
            (0, 0) => phase_b5_execution::StepKind::NonbranchEven,
            (0, 1) => phase_b5_execution::StepKind::NonbranchOdd,
            (1, 1) => phase_b5_execution::StepKind::PositiveOdd,
            (1, 0) => return None,
            _ => unreachable!(),
        };
        numerator = kind.numerator_matrix().mul(numerator);
    }
    Some(numerator)
}

fn round218_b5_lulu_shear_coeffs(target: phase_b5_execution::Matrix2) -> [i128; 4] {
    assert_eq!(target.det(), 1, "LULU target must be unimodular");
    let mut best: Option<([i128; 4], (i128, i128, u32, [i128; 4]))> = None;
    for q2 in -32..=32 {
        for q3 in -32..=32 {
            if q3 * q2 + 1 != target.a11 {
                continue;
            }
            if target.a11 != 0 {
                if (target.a10 - q3) % target.a11 != 0 {
                    continue;
                }
                if (target.a01 - q2) % target.a11 != 0 {
                    continue;
                }
                let q1 = (target.a10 - q3) / target.a11;
                let q4 = (target.a01 - q2) / target.a11;
                round218_b5_lulu_record_candidate(&mut best, [q1, q2, q3, q4]);
            } else {
                if target.a10 != q3 || target.a01 != q2 {
                    continue;
                }
                for q1 in -32..=32 {
                    let rem = target.a00 - 1 - q1 * q2;
                    if q3 == 0 || rem % q3 != 0 {
                        continue;
                    }
                    let q4 = rem / q3;
                    round218_b5_lulu_record_candidate(&mut best, [q1, q2, q3, q4]);
                }
            }
        }
    }
    best.unwrap_or_else(|| panic!("no LULU decomposition for {target:?}"))
        .0
}

fn round218_b5_lulu_record_candidate(
    best: &mut Option<([i128; 4], (i128, i128, u32, [i128; 4]))>,
    candidate: [i128; 4],
) {
    if candidate.iter().any(|value| value.abs() > 32) {
        return;
    }
    let score = (
        candidate.iter().map(|value| value.abs()).max().unwrap_or(0),
        candidate.iter().map(|value| value.abs()).sum::<i128>(),
        candidate
            .iter()
            .map(|value| value.unsigned_abs().count_ones())
            .sum::<u32>(),
        candidate,
    );
    if best
        .as_ref()
        .map_or(true, |(_, old_score)| score < *old_score)
    {
        *best = Some((candidate, score));
    }
}

fn emit_round218_monomial_toggle_u16(
    b: &mut B,
    controls: &[QubitId],
    mask: u16,
    target: QubitId,
    scratch: &[QubitId],
) {
    let selected: Vec<QubitId> = controls
        .iter()
        .enumerate()
        .filter_map(|(idx, &control)| ((mask & (1u16 << idx)) != 0).then_some(control))
        .collect();
    match selected.len() {
        0 => b.x(target),
        1 => b.cx(selected[0], target),
        2 => b.ccx(selected[0], selected[1], target),
        n => {
            assert!(
                scratch.len() >= n - 2,
                "not enough scratch for {n}-controlled X"
            );
            b.ccx(selected[0], selected[1], scratch[0]);
            for idx in 2..n - 1 {
                b.ccx(scratch[idx - 2], selected[idx], scratch[idx - 1]);
            }
            b.ccx(scratch[n - 3], selected[n - 1], target);
            for idx in (2..n - 1).rev() {
                b.ccx(scratch[idx - 2], selected[idx], scratch[idx - 1]);
            }
            b.ccx(selected[0], selected[1], scratch[0]);
        }
    }
}

fn emit_round218_scaled_coeff_positive_odd_step_fixed(
    b: &mut B,
    v: &[QubitId],
    r: &[QubitId],
    p: U256,
) {
    for i in 0..N {
        b.swap(v[i], r[i]);
    }
    let ctrl = b.alloc_qubit();
    b.x(ctrl);
    emit_round218_cmod_neg_canonical(b, r, ctrl, p);
    b.x(ctrl);
    b.free(ctrl);
    super::mod_add_qq_fast(b, r, v, p);
    emit_round218_mod_halve_canonical(b, r, p);
}

fn emit_round218_add_small_signed_multiple_fast(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    coeff: i128,
    p: U256,
) {
    assert_eq!(acc.len(), N);
    assert_eq!(x.len(), N);
    if coeff == 0 {
        return;
    }
    if coeff == 1 {
        super::mod_add_qq_fast(b, acc, x, p);
        return;
    }
    if coeff == -1 {
        super::mod_sub_qq_fast(b, acc, x, p);
        return;
    }

    let subtract = coeff < 0;
    let abs = coeff.unsigned_abs();
    assert!(
        abs <= 64,
        "Round218 B=5 shear coefficient too large: {coeff}"
    );

    let tmp = b.alloc_qubits(N);
    for i in 0..N {
        b.cx(x[i], tmp[i]);
    }
    let top = 127usize - abs.leading_zeros() as usize;
    for bit_idx in 0..=top {
        if ((abs >> bit_idx) & 1) != 0 {
            if subtract {
                super::mod_sub_qq_fast(b, acc, &tmp, p);
            } else {
                super::mod_add_qq_fast(b, acc, &tmp, p);
            }
        }
        if bit_idx < top {
            super::mod_double_inplace_fast(b, &tmp, p);
        }
    }
    for _ in 0..top {
        emit_round218_mod_halve_canonical(b, &tmp, p);
    }
    for i in (0..N).rev() {
        b.cx(x[i], tmp[i]);
    }
    b.free_vec(&tmp);
}

fn emit_round218_add_small_signed_multiple(
    b: &mut B,
    acc: &[QubitId],
    x: &[QubitId],
    coeff: i128,
    p: U256,
) {
    assert_eq!(acc.len(), N);
    assert_eq!(x.len(), N);
    if coeff == 0 {
        return;
    }
    let subtract = coeff < 0;
    let abs = coeff.unsigned_abs();
    assert!(
        abs <= 64,
        "Round218 B=5 matrix coefficient too large for small-constant lowerer: {coeff}"
    );

    let tmp = b.alloc_qubits(N);
    for i in 0..N {
        b.cx(x[i], tmp[i]);
    }
    let top = 127usize - abs.leading_zeros() as usize;
    for bit_idx in 0..=top {
        if ((abs >> bit_idx) & 1) != 0 {
            if subtract {
                super::mod_sub_qq(b, acc, &tmp, p);
            } else {
                super::mod_add_qq(b, acc, &tmp, p);
            }
        }
        if bit_idx < top {
            super::mod_double_inplace(b, &tmp, p);
        }
    }
    for _ in 0..top {
        emit_round218_mod_halve_canonical(b, &tmp, p);
    }
    for i in (0..N).rev() {
        b.cx(x[i], tmp[i]);
    }
    b.free_vec(&tmp);
}

fn emit_round218_mod_div32_canonical(b: &mut B, v: &[QubitId], p: U256) {
    assert_eq!(p, SECP256K1_P, "Round218 div32 is secp256k1-only");
    assert_eq!(v.len(), N, "Round218 div32 is secp256k1-width only");
    if round218_b5_small_div32_enabled() {
        emit_round218_mod_div32_canonical_small_product(b, v, p);
        return;
    }

    let t = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    for i in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        b.cx(v[i], t[i]);
    }
    b.cx(v[0], t[phase_b5_execution::ROUND218_B5_BLOCK_BITS - 1]);

    let hi = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    let mut ext = v.to_vec();
    ext.extend_from_slice(&hi);
    for shift in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        let bits = shifted_const_bits(p, shift, ext.len());
        emit_round218_cadd_const_bits(b, &ext, &bits, t[shift]);
    }

    for _ in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        for i in 0..ext.len() - 1 {
            b.swap(ext[i], ext[i + 1]);
        }
    }

    emit_round218_div32_uncompute_correction(b, v, &t);

    b.free_vec(&hi);
    b.free_vec(&t);
}

fn emit_round218_mod_div32_canonical_small_product(b: &mut B, v: &[QubitId], p: U256) {
    assert_eq!(p, SECP256K1_P, "Round218 small div32 is secp256k1-only");
    assert_eq!(v.len(), N, "Round218 small div32 is secp256k1-width only");

    let k = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    for i in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        b.cx(v[i], k[i]);
    }
    b.cx(v[0], k[phase_b5_execution::ROUND218_B5_BLOCK_BITS - 1]);

    let correction_width = 37usize;
    let d = b.alloc_qubits(correction_width);
    emit_round218_div32_small_correction_product(b, &d, &k);

    let borrow = b.alloc_qubit();
    let zero_tail = b.alloc_qubits(N - correction_width);
    let sub_top_zero = b.alloc_qubit();
    let mut subtrahend = d.clone();
    subtrahend.extend_from_slice(&zero_tail);
    subtrahend.push(sub_top_zero);
    let mut acc_ext = v.to_vec();
    acc_ext.push(borrow);
    super::sub_nbit_qq_fast(b, &subtrahend, &acc_ext);
    b.free(sub_top_zero);
    b.free_vec(&zero_tail);

    let hi = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    let mut shift_ext = v.to_vec();
    shift_ext.extend_from_slice(&hi);
    for _ in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        for i in 0..shift_ext.len() - 1 {
            b.swap(shift_ext[i], shift_ext[i + 1]);
        }
    }
    b.free_vec(&hi);

    let top = &v[N - phase_b5_execution::ROUND218_B5_BLOCK_BITS..N];
    for i in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        b.cx(k[i], top[i]);
    }
    let one_bits = [true, false, false, false, false];
    emit_round218_csub_const_bits(b, top, &one_bits, borrow);

    emit_round218_div32_small_correction_product_inverse(b, &d, &k);
    b.free_vec(&d);
    emit_round218_div32_uncompute_correction_and_borrow(b, v, &k, Some(borrow));
    b.free(borrow);
    b.free_vec(&k);
}

fn emit_round218_div32_small_correction_product(b: &mut B, d: &[QubitId], k: &[QubitId]) {
    assert_eq!(k.len(), phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    assert!(d.len() >= 37);
    let c = U256::from(secp256k1_c_low());
    for shift in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        let bits = shifted_const_bits(c, shift, d.len());
        emit_round218_cadd_const_bits(b, d, &bits, k[shift]);
    }
}

fn emit_round218_div32_small_correction_product_inverse(b: &mut B, d: &[QubitId], k: &[QubitId]) {
    assert_eq!(k.len(), phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    assert!(d.len() >= 37);
    let c = U256::from(secp256k1_c_low());
    for shift in (0..phase_b5_execution::ROUND218_B5_BLOCK_BITS).rev() {
        let bits = shifted_const_bits(c, shift, d.len());
        emit_round218_csub_const_bits(b, d, &bits, k[shift]);
    }
}

fn emit_round218_div32_uncompute_correction(b: &mut B, y: &[QubitId], t: &[QubitId]) {
    emit_round218_div32_uncompute_correction_and_borrow(b, y, t, None);
}

fn emit_round218_div32_uncompute_correction_and_borrow(
    b: &mut B,
    y: &[QubitId],
    t: &[QubitId],
    borrow: Option<QubitId>,
) {
    assert_eq!(y.len(), N);
    assert_eq!(t.len(), phase_b5_execution::ROUND218_B5_BLOCK_BITS);

    let q = &y[N - phase_b5_execution::ROUND218_B5_BLOCK_BITS..N];
    let h = b.alloc_qubits(N - phase_b5_execution::ROUND218_B5_BLOCK_BITS + 1);
    let h_scratch = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS - 2);
    emit_round218_div32_threshold_helper(b, q, &h, &h_scratch);

    let temp = b.alloc_qubits(N - phase_b5_execution::ROUND218_B5_BLOCK_BITS + 1);
    for i in 0..N - phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        b.cx(y[i], temp[i]);
    }
    super::add_nbit_qq_fast(b, &h, &temp);
    let carry = b.alloc_qubit();
    b.cx(temp[temp.len() - 1], carry);

    let recomputed = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    for i in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        b.cx(q[i], recomputed[i]);
    }
    let one_bits = [true, false, false, false, false];
    emit_round218_cadd_const_bits(b, &recomputed, &one_bits, carry);
    if let Some(borrow) = borrow {
        b.cx(carry, borrow);
    }
    for i in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        b.cx(recomputed[i], t[i]);
    }
    emit_round218_csub_const_bits(b, &recomputed, &one_bits, carry);
    for i in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        b.cx(q[i], recomputed[i]);
    }
    b.free_vec(&recomputed);

    b.cx(temp[temp.len() - 1], carry);
    super::sub_nbit_qq_fast(b, &h, &temp);
    for i in 0..N - phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        b.cx(y[i], temp[i]);
    }
    b.free_vec(&temp);
    b.free(carry);

    emit_round218_div32_threshold_helper(b, q, &h, &h_scratch);
    b.free_vec(&h_scratch);
    b.free_vec(&h);
}

fn emit_round218_div32_threshold_helper(
    b: &mut B,
    q: &[QubitId],
    h: &[QubitId],
    scratch: &[QubitId],
) {
    assert_eq!(q.len(), phase_b5_execution::ROUND218_B5_BLOCK_BITS);
    assert!(h.len() >= 33, "Round218 div32 helper needs 33 output bits");
    assert!(
        scratch.len() >= q.len().saturating_sub(2),
        "Round218 div32 helper needs {} scratch qubits",
        q.len().saturating_sub(2)
    );

    for output_idx in 0..33 {
        for mask in div32_threshold_helper_anf_masks(output_idx) {
            emit_round218_monomial_toggle_u8(b, q, mask, h[output_idx], scratch);
        }
    }
}

fn div32_threshold_helper_anf_masks(output_idx: usize) -> Vec<u8> {
    assert!(output_idx < 33);
    let c = secp256k1_c_low();
    let mut truth = [0u8; 1usize << phase_b5_execution::ROUND218_B5_BLOCK_BITS];
    for (q, value) in truth.iter_mut().enumerate() {
        let h = (((q as u64) + 1) * c) >> phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        *value = ((h >> output_idx) & 1) as u8;
    }
    for bit in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
        for mask in 0..truth.len() {
            if (mask & (1usize << bit)) != 0 {
                truth[mask] ^= truth[mask ^ (1usize << bit)];
            }
        }
    }
    truth
        .iter()
        .enumerate()
        .filter_map(|(mask, &coeff)| (coeff != 0).then_some(mask as u8))
        .collect()
}

fn emit_round218_monomial_toggle_u8(
    b: &mut B,
    controls: &[QubitId],
    mask: u8,
    target: QubitId,
    scratch: &[QubitId],
) {
    let selected: Vec<QubitId> = controls
        .iter()
        .enumerate()
        .filter_map(|(idx, &control)| ((mask & (1u8 << idx)) != 0).then_some(control))
        .collect();
    match selected.len() {
        0 => b.x(target),
        1 => b.cx(selected[0], target),
        2 => b.ccx(selected[0], selected[1], target),
        n => {
            assert!(
                scratch.len() >= n - 2,
                "not enough scratch for {n}-controlled X"
            );
            b.ccx(selected[0], selected[1], scratch[0]);
            for idx in 2..n - 1 {
                b.ccx(scratch[idx - 2], selected[idx], scratch[idx - 1]);
            }
            b.ccx(scratch[n - 3], selected[n - 1], target);
            for idx in (2..n - 1).rev() {
                b.ccx(scratch[idx - 2], selected[idx], scratch[idx - 1]);
            }
            b.ccx(selected[0], selected[1], scratch[0]);
        }
    }
}

fn emit_round218_cadd_const_bits(b: &mut B, acc: &[QubitId], bits: &[bool], ctrl: QubitId) {
    assert_eq!(acc.len(), bits.len());
    let a = b.alloc_qubits(acc.len());
    for (idx, &bit) in bits.iter().enumerate() {
        if bit {
            b.cx(ctrl, a[idx]);
        }
    }
    super::add_nbit_qq_fast(b, &a, acc);
    for (idx, &bit) in bits.iter().enumerate() {
        if bit {
            b.cx(ctrl, a[idx]);
        }
    }
    b.free_vec(&a);
}

fn emit_round218_csub_const_bits(b: &mut B, acc: &[QubitId], bits: &[bool], ctrl: QubitId) {
    assert_eq!(acc.len(), bits.len());
    let a = b.alloc_qubits(acc.len());
    for (idx, &bit) in bits.iter().enumerate() {
        if bit {
            b.cx(ctrl, a[idx]);
        }
    }
    super::sub_nbit_qq_fast(b, &a, acc);
    for (idx, &bit) in bits.iter().enumerate() {
        if bit {
            b.cx(ctrl, a[idx]);
        }
    }
    b.free_vec(&a);
}

fn shifted_const_bits(c: U256, shift: usize, len: usize) -> Vec<bool> {
    (0..len)
        .map(|idx| idx >= shift && c.bit(idx - shift))
        .collect()
}

fn secp256k1_c_low() -> u64 {
    let c = U256::MAX
        .wrapping_sub(SECP256K1_P)
        .wrapping_add(U256::from(1u64));
    c.as_limbs()[0]
}

fn emit_round218_cmod_neg_canonical(b: &mut B, v: &[QubitId], ctrl: QubitId, p: U256) {
    let nz = b.alloc_qubit();
    let do_neg = b.alloc_qubit();
    super::cmp_neq_zero_into(b, v, nz);
    b.ccx(ctrl, nz, do_neg);
    for &q in v {
        b.cx(do_neg, q);
    }
    super::cadd_nbit_const(b, v, p.wrapping_add(U256::from(1u64)), do_neg);
    b.ccx(ctrl, nz, do_neg);
    super::cmp_neq_zero_into(b, v, nz);
    b.free(do_neg);
    b.free(nz);
}

fn emit_round218_cmod_add_qq_exact(
    b: &mut B,
    acc: &[QubitId],
    a: &[QubitId],
    ctrl: QubitId,
    p: U256,
) {
    let n = acc.len();
    assert_eq!(a.len(), n);
    let f = b.alloc_qubits(n);
    for i in 0..n {
        b.ccx(ctrl, a[i], f[i]);
    }
    super::mod_add_qq(b, acc, &f, p);
    for i in (0..n).rev() {
        b.ccx(ctrl, a[i], f[i]);
    }
    b.free_vec(&f);
}

fn emit_round218_cmod_sub_qq_exact(
    b: &mut B,
    acc: &[QubitId],
    a: &[QubitId],
    ctrl: QubitId,
    p: U256,
) {
    let n = acc.len();
    assert_eq!(a.len(), n);
    let f = b.alloc_qubits(n);
    for i in 0..n {
        b.ccx(ctrl, a[i], f[i]);
    }
    super::mod_sub_qq(b, acc, &f, p);
    for i in (0..n).rev() {
        b.ccx(ctrl, a[i], f[i]);
    }
    b.free_vec(&f);
}

fn emit_round218_mod_halve_canonical(b: &mut B, v: &[QubitId], p: U256) {
    let n = v.len();
    assert_eq!(n, N, "Round218 canonical halve is secp256k1-width only");
    let parity = b.alloc_qubit();
    let ovf = b.alloc_qubit();
    b.cx(v[0], parity);

    let mut v_ext = v.to_vec();
    v_ext.push(ovf);
    super::cadd_nbit_const(b, &v_ext, p, parity);

    for i in 0..n - 1 {
        b.swap(v[i], v[i + 1]);
    }
    b.swap(v[n - 1], ovf);
    b.free(ovf);

    let top = b.alloc_qubit();
    let mut y_ext = v.to_vec();
    y_ext.push(top);
    super::cmp_gt_const_n1(b, &y_ext, (p - U256::from(1u64)) >> 1usize, parity);
    b.free(top);
    b.free(parity);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::{analyze_ops, OperationType, QubitOrBit};
    use crate::sim::Simulator;
    use sha3::{
        digest::{ExtendableOutput, Update},
        Shake128,
    };

    fn scaled_coeff_step_pure(
        mut v: u64,
        mut r: u64,
        branch: u64,
        old_g0: u64,
        p: u64,
    ) -> (u64, u64) {
        if branch == 0 && old_g0 != 0 {
            r = (r + v) % p;
        }
        if branch != 0 {
            r = (r + p - v) % p;
        }
        r = if r & 1 == 0 { r / 2 } else { (r + p) / 2 };
        if branch != 0 {
            v = (v + r + r) % p;
        }
        (v, r)
    }

    fn scaled_coeff_step_inverse_pure(
        mut v: u64,
        mut r: u64,
        branch: u64,
        old_g0: u64,
        p: u64,
    ) -> (u64, u64) {
        if branch != 0 {
            v = (v + p - r) % p;
            v = (v + p - r) % p;
        }
        r = (r + r) % p;
        if branch != 0 {
            r = (r + v) % p;
        }
        if branch == 0 && old_g0 != 0 {
            r = (r + p - v) % p;
        }
        (v, r)
    }

    fn add_mod(a: U256, b: U256, p: U256) -> U256 {
        a.add_mod(b, p)
    }

    fn sub_mod(a: U256, b: U256, p: U256) -> U256 {
        if a >= b {
            a - b
        } else {
            p - (b - a)
        }
    }

    fn halve_mod(a: U256, p: U256) -> U256 {
        if a.bit(0) {
            let sum = a.wrapping_add(p);
            let carry = sum < a;
            let mut out = sum >> 1usize;
            if carry {
                out.set_bit(255, true);
            }
            out
        } else {
            a >> 1usize
        }
    }

    fn div32_mod(mut a: U256, p: U256) -> U256 {
        for _ in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
            a = halve_mod(a, p);
        }
        a
    }

    #[test]
    fn direct_mod_div32_canonical_matches_five_halves_and_cleans() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        emit_round218_mod_div32_canonical(&mut b, &v, p);

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 1);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-direct-div32-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            U256::ZERO,
            U256::from(1u64),
            U256::from(2u64),
            U256::from(3u64),
            U256::from(5u64),
            U256::from(31u64),
            U256::from(32u64),
            p - U256::from(1u64),
            p >> 1usize,
            U256::from_limbs([
                0x243f_6a88_85a3_08d3,
                0x1319_8a2e_0370_7344,
                0xa409_3822_299f_31d0,
                0x082e_fa98_ec4e_6c89,
            ]) % p,
            U256::from_limbs([
                0x9216_d5d9_8979_fb1b,
                0xd131_0ba6_98df_b5ac,
                0x2ffd_72db_d01a_dfb7,
                0xb8e1_afed_6a26_7e96,
            ]) % p,
        ];
        for (shot, &value) in cases.iter().enumerate() {
            sim.set_register(&regs[0], value, shot);
        }
        sim.apply(&b.ops);
        assert_eq!(sim.global_phase(), 0, "direct div32 left phase garbage");
        for (shot, &value) in cases.iter().enumerate() {
            assert_eq!(
                sim.get_register(&regs[0], shot),
                div32_mod(value, p),
                "div32 shot {shot}"
            );
        }

        zero_qubit_register(&mut sim, &regs[0]);
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(value, 0, "direct div32 scratch qubit q{idx} not clean");
        }
        println!("METRIC round218_b5_direct_div32_toffoli={toffoli}");
        println!("METRIC round218_b5_direct_div32_qubits={num_qubits}");
    }

    fn scaled_coeff_step_pure_u256(
        mut v: U256,
        mut r: U256,
        branch: bool,
        old_g0: bool,
        p: U256,
    ) -> (U256, U256) {
        if branch {
            let old_r = r;
            r = sub_mod(r, v, p);
            r = halve_mod(r, p);
            v = old_r;
        } else {
            if old_g0 {
                r = add_mod(r, v, p);
            }
            r = halve_mod(r, p);
        }
        (v, r)
    }

    fn scaled_coeff_step_inverse_pure_u256(
        mut v: U256,
        mut r: U256,
        branch: bool,
        old_g0: bool,
        p: U256,
    ) -> (U256, U256) {
        r = add_mod(r, r, p);
        if old_g0 {
            r = sub_mod(r, v, p);
        }
        if branch {
            let old_v = v;
            v = r;
            r = old_v;
        }
        (v, r)
    }

    fn small_signed_mul_mod_u256(x: U256, coeff: i128, p: U256) -> U256 {
        if coeff == 0 {
            return U256::ZERO;
        }
        let mut out = U256::ZERO;
        for _ in 0..coeff.unsigned_abs() {
            out = add_mod(out, x, p);
        }
        if coeff < 0 && out != U256::ZERO {
            p - out
        } else {
            out
        }
    }

    fn add_signed_term(acc: U256, x: U256, coeff: i128, p: U256) -> U256 {
        add_mod(acc, small_signed_mul_mod_u256(x, coeff, p), p)
    }

    fn scaled_coeff_block_fixed_pure_u256(
        row: &phase_b5_execution::BlockRow,
        v: U256,
        r: U256,
        p: U256,
    ) -> (U256, U256) {
        let mut out_v = U256::ZERO;
        out_v = add_signed_term(out_v, v, row.matrix.numerator.a00, p);
        out_v = add_signed_term(out_v, r, row.matrix.numerator.a01, p);
        let mut out_r = U256::ZERO;
        out_r = add_signed_term(out_r, v, row.matrix.numerator.a10, p);
        out_r = add_signed_term(out_r, r, row.matrix.numerator.a11, p);
        for _ in 0..row.matrix.denominator_log2 {
            out_v = halve_mod(out_v, p);
            out_r = halve_mod(out_r, p);
        }
        (out_v, out_r)
    }

    fn cheap_lft_frame_pure_u256(
        branch_word: u8,
        mut v: U256,
        mut r: U256,
        p: U256,
    ) -> (U256, U256) {
        for step in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
            if ((branch_word >> step) & 1) != 0 {
                std::mem::swap(&mut v, &mut r);
            }
            v = add_mod(v, v, p);
        }
        (v, r)
    }

    fn scaled_coeff_numerator_div32_pure_u256(
        numerator: phase_b5_execution::Matrix2,
        v: U256,
        r: U256,
        p: U256,
    ) -> (U256, U256) {
        let mut out_v = U256::ZERO;
        out_v = add_signed_term(out_v, v, numerator.a00, p);
        out_v = add_signed_term(out_v, r, numerator.a01, p);
        let mut out_r = U256::ZERO;
        out_r = add_signed_term(out_r, v, numerator.a10, p);
        out_r = add_signed_term(out_r, r, numerator.a11, p);
        for _ in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
            out_v = halve_mod(out_v, p);
            out_r = halve_mod(out_r, p);
        }
        (out_v, out_r)
    }

    fn unscaled_coeff_block_fixed_pure_u256(
        row: &phase_b5_execution::BlockRow,
        v: U256,
        r: U256,
        p: U256,
    ) -> (U256, U256) {
        let mut out_v = U256::ZERO;
        out_v = add_signed_term(out_v, v, row.matrix.numerator.a00, p);
        out_v = add_signed_term(out_v, r, row.matrix.numerator.a01, p);
        let mut out_r = U256::ZERO;
        out_r = add_signed_term(out_r, v, row.matrix.numerator.a10, p);
        out_r = add_signed_term(out_r, r, row.matrix.numerator.a11, p);
        (out_v, out_r)
    }

    fn cotangent_lift_window_endpoint(
        mut zeta: i128,
        mut f: i128,
        mut g: i128,
        mut a: i128,
        mut c: i128,
    ) -> (i128, i128, i128, i128, i128, u8, u8) {
        let mut branch_word = 0u8;
        let mut old_g0_word = 0u8;
        let window_bits = phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS;

        for step in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
            let width = window_bits - step;
            let modulus = 1i128 << width;
            f = f.rem_euclid(modulus);
            g = g.rem_euclid(modulus);

            let old_g0 = (g & 1) as u8;
            let branch = (zeta < 0 && old_g0 != 0) as u8;
            branch_word |= branch << step;
            old_g0_word |= old_g0 << step;

            let (next_zeta, next_f, next_g, next_a, next_c) = if branch != 0 {
                // Source M=[[0,2],[-1,1]], covector adj(M)^T.
                (-zeta - 2, g, (g - f) / 2, a + c, -2 * a)
            } else {
                // Source M=[[2,0],[old_g0,1]], covector adj(M)^T.
                (
                    zeta - 1,
                    f,
                    (g + i128::from(old_g0) * f) / 2,
                    a - i128::from(old_g0) * c,
                    2 * c,
                )
            };

            let next_mask = (1i128 << (width - 1)) - 1;
            zeta = next_zeta;
            f = next_f & next_mask;
            g = next_g & next_mask;
            a = next_a;
            c = next_c;
        }

        (zeta, f, g, a, c, branch_word, old_g0_word)
    }

    #[test]
    fn cotangent_lift_b5_endpoint_does_not_select_old_g0_without_history() {
        let left = cotangent_lift_window_endpoint(-1, 1, 7, -5, -5);
        let right = cotangent_lift_window_endpoint(-1, 3, 5, -5, -5);

        assert_eq!(
            (left.0, left.1, left.2, left.3, left.4),
            (-1, 31, 31, 40, 0),
            "left lifted endpoint changed"
        );
        assert_eq!(
            (right.0, right.1, right.2, right.3, right.4),
            (-1, 31, 31, 40, 0),
            "right lifted endpoint changed"
        );
        assert_eq!(
            (left.0, left.1, left.2, left.3, left.4),
            (right.0, right.1, right.2, right.3, right.4),
            "counterexample no longer collides"
        );
        assert_eq!(left.5, right.5, "branch word differs unexpectedly");
        assert_ne!(
            left.6, right.6,
            "old_g0 word must differ to kill lifted-endpoint selector cleanup"
        );
        assert_eq!((left.5, left.6), (0b01011, 0b01011));
        assert_eq!((right.5, right.6), (0b01011, 0b11011));
    }

    #[test]
    fn tagged_l_source_lifetime_target_needs_durable_separator_stack() {
        const CLAIMED_EXTRA_SCRATCH: usize = 668;
        const CLAIMED_PA_QUBITS: usize = 1_425;
        const CLAIMED_PA_TOFFOLI: usize = 2_701_606;
        const CLAIMED_PA_QT: usize = 3_849_788_550;

        let compact = quantum_d1_data_source::round218_b5_compact_history_resource_gate();
        assert_eq!(
            compact.separator_lower_bound_bits_per_block, 7,
            "B=5 source progress lower bound changed; revisit tagged-L lifetime"
        );
        assert_eq!(compact.zeta_old_separator_bits_per_block, 8);
        assert_eq!(compact.separator_lower_bound_history_bits, 826);
        assert_eq!(compact.zeta_old_separator_history_bits, 944);
        assert!(
            compact.separator_lower_bound_history_bits > CLAIMED_EXTRA_SCRATCH,
            "even the optimal per-block separator lower bound fits the claimed scratch"
        );
        assert!(
            compact.zeta_old_separator_history_bits > CLAIMED_EXTRA_SCRATCH,
            "the tagged 8-bit source-lifetime separator fits the claimed scratch"
        );
        assert!(
            compact.zeta_old_separator_qubits > CLAIMED_PA_QUBITS,
            "existing exact source-lifetime accounting no longer exceeds the claimed Q target"
        );
        assert!(
            compact.zeta_old_separator_toffoli > CLAIMED_PA_TOFFOLI,
            "existing exact source-lifetime accounting no longer exceeds the claimed T target"
        );
        assert!(
            compact.zeta_old_separator_qt > CLAIMED_PA_QT,
            "existing exact source-lifetime accounting no longer exceeds the claimed Q*T target"
        );
        assert!(
            !compact.makes_first_milestone,
            "tagged-L source-lifetime stack unexpectedly became a shippable frontier row"
        );
    }

    fn reg(qs: &[QubitId]) -> Vec<QubitOrBit> {
        qs.iter().copied().map(QubitOrBit::Qubit).collect()
    }

    fn set_control<R: sha3::digest::XofReader>(
        sim: &mut Simulator<'_, R>,
        q: QubitId,
        value: bool,
        shot: usize,
    ) {
        if value {
            *sim.qubit_mut(q) |= 1u64 << shot;
        } else {
            *sim.qubit_mut(q) &= !(1u64 << shot);
        }
    }

    fn zero_qubit_register<R: sha3::digest::XofReader>(
        sim: &mut Simulator<'_, R>,
        reg: &[QubitOrBit],
    ) {
        for item in reg {
            if let QubitOrBit::Qubit(q) = *item {
                *sim.qubit_mut(q) = 0;
            }
        }
    }

    fn coefficient_gate_cases() -> Vec<(U256, U256, bool, bool)> {
        let p = SECP256K1_P;
        vec![
            (U256::ZERO, U256::ZERO, false, false),
            (U256::ZERO, U256::from(1u64), true, true),
            (U256::from(1u64), U256::ZERO, false, true),
            (U256::from(2u64), U256::from(3u64), false, false),
            (U256::from(3u64), U256::from(5u64), true, true),
            (
                U256::from_limbs([
                    0x243f_6a88_85a3_08d3,
                    0x1319_8a2e_0370_7344,
                    0xa409_3822_299f_31d0,
                    0x082e_fa98_ec4e_6c89,
                ]) % p,
                U256::from_limbs([
                    0x4528_21e6_38d0_1377,
                    0xbe54_66cf_34e9_0c6c,
                    0xc0ac_29b7_c97c_50dd,
                    0x3f84_d5b5_b547_0917,
                ]) % p,
                false,
                true,
            ),
            (
                U256::from_limbs([
                    0x9216_d5d9_8979_fb1b,
                    0xd131_0ba6_98df_b5ac,
                    0x2ffd_72db_d01a_dfb7,
                    0xb8e1_afed_6a26_7e96,
                ]) % p,
                U256::from_limbs([
                    0xba7c_9045_f12c_7f99,
                    0x24a1_9947_b391_6cf7,
                    0x0801_f2e2_858e_fc16,
                    0x6369_20d8_7157_4e69,
                ]) % p,
                true,
                true,
            ),
        ]
    }

    fn representative_b5_rows() -> Vec<phase_b5_execution::BlockRow> {
        vec![
            phase_b5_execution::block_row(
                0,
                phase_b5_execution::BlockSelector {
                    zeta_start: 8,
                    f_low: 1,
                    g_low: 0,
                    width: 5,
                },
            ),
            phase_b5_execution::block_row(
                0,
                phase_b5_execution::BlockSelector {
                    zeta_start: 8,
                    f_low: 1,
                    g_low: 1,
                    width: 5,
                },
            ),
            phase_b5_execution::block_row(
                0,
                phase_b5_execution::BlockSelector {
                    zeta_start: -1,
                    f_low: 31,
                    g_low: 1,
                    width: 5,
                },
            ),
            phase_b5_execution::block_row(
                17,
                phase_b5_execution::BlockSelector {
                    zeta_start: -5,
                    f_low: 23,
                    g_low: 30,
                    width: 5,
                },
            ),
        ]
    }

    fn dyadic_lift_probe_rows() -> Vec<phase_b5_execution::BlockRow> {
        let mut rows = representative_b5_rows();
        rows.push(phase_b5_execution::block_row(
            0,
            phase_b5_execution::BlockSelector {
                zeta_start: -1,
                f_low: 1,
                g_low: 19,
                width: 5,
            },
        ));
        rows.push(phase_b5_execution::block_row(
            1,
            phase_b5_execution::BlockSelector {
                zeta_start: 0,
                f_low: 1,
                g_low: 14,
                width: 5,
            },
        ));
        rows
    }

    #[test]
    fn contract_pins_round218_target_and_missing_gate_lowerer() {
        let contract = round218_b5_transport_contract();
        assert_eq!(
            contract.classification,
            ROUND218_B5_TRANSPORT_CLASSIFICATION
        );
        assert_eq!(
            contract.target_classification,
            phase_b5_execution::ROUND218_B5_CLASSIFICATION
        );
        assert_eq!(contract.target_qubits, 1_562);
        assert_eq!(contract.target_toffoli, 2_203_351);
        assert_eq!(contract.block_bits, 5);
        assert_eq!(contract.blocks, 118);
        assert_eq!(
            contract.privacy_blocker,
            ROUND218_B5_TRANSPORT_PRIVACY_BLOCKER
        );
        assert_eq!(
            contract.missing_object,
            ROUND218_B5_TRANSPORT_MISSING_OBJECT
        );
    }

    #[test]
    fn source_live_product_lowerer_contract_pins_round404_budget_row() {
        let contract = round218_b5_source_live_product_lowerer_contract();
        assert_eq!(
            contract.classification,
            ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_CLASSIFICATION
        );
        assert_eq!(contract.qtail_qubits, 2_453);
        assert_eq!(contract.non_product_toffoli, 2_106_212);
        assert_eq!(contract.current_product_toffoli, 1_919_786);
        assert_eq!(contract.m1_replacement_toffoli_limit, 1_562_764);
        assert_eq!(contract.m2_replacement_toffoli_limit, 1_155_100);
        assert_eq!(contract.m3_replacement_toffoli_limit, 747_436);
        assert_eq!(
            contract.proof_carrying_m1_one_way_toffoli,
            ROUND404_QTAIL_ROUND217_HPREFIX_ONE_WAY_TOFFOLI_BOUND
        );
        assert_eq!(contract.proof_carrying_m1_one_way_toffoli, 1_313_539);
        assert_eq!(contract.proof_carrying_m1_projected_pa_toffoli, 3_419_751);
        assert_eq!(contract.proof_carrying_m1_projected_pa_qt, 8_388_649_203);
        assert_eq!(contract.sampled_m2_one_way_toffoli, 1_023_289);
        assert_eq!(contract.sampled_m2_projected_pa_toffoli, 3_129_501);
        assert_eq!(contract.sampled_m2_projected_pa_qt, 7_676_665_953);
        assert!(contract
            .requirements
            .contains(&"no_full_source_history_tape"));
        assert!(contract
            .requirements
            .contains(&"deterministic_9024_google_pa_fuzz"));
        assert_eq!(
            contract.missing_object,
            ROUND218_B5_SOURCE_LIVE_PRODUCT_LOWERER_MISSING_OBJECT
        );
    }

    #[test]
    fn source_live_product_lowerer_body_plan_fails_closed_before_hash_history_probe() {
        let plan = round218_b5_source_live_product_lowerer_body_plan();
        assert_eq!(
            plan.classification,
            ROUND406_B5_SOURCE_LIVE_PRODUCT_LOWERER_BODY_PLAN_CLASSIFICATION
        );
        assert_eq!(
            plan.selected_route,
            "round217_sampled_product_m2_contract_path"
        );
        assert_eq!(plan.qtail_qubits, 2_453);
        assert_eq!(plan.one_way_toffoli_bound, 1_023_289);
        assert_eq!(plan.projected_pa_toffoli, 3_129_501);
        assert_eq!(plan.projected_pa_qt, 7_676_665_953);
        assert_eq!(plan.phase_blocks.len(), 6);
        assert!(!plan.body_emits_gates);
        assert!(!plan.codegen_allowed_now);
        let budget_sum: usize = plan
            .phase_blocks
            .iter()
            .map(|block| block.toffoli_budget)
            .sum();
        assert_eq!(budget_sum, plan.one_way_toffoli_bound);
        assert_eq!(
            ROUND406_QTAIL_ROUND217_PRODUCT_M2_APPLICATION_TOFFOLI_BOUND,
            945_999
        );
        assert!(plan.phase_blocks.iter().all(|block| {
            block.phase.starts_with("round")
                && !block
                    .backend_primitive
                    .contains("emit_round218_b5_full_source")
        }));
        assert!(plan
            .phase_blocks
            .iter()
            .any(|block| block.phase_debt.contains("not milestone-promotable")));
    }

    #[test]
    fn pa_hook_fails_closed_instead_of_emitting_overbudget_fallback() {
        let mut b = B::new();
        let tx = b.alloc_qubits(N);
        let ty = b.alloc_qubits(N);
        let ox = b.alloc_bits(N);
        let oy = b.alloc_bits(N);
        let msg = round218_b5_transport_blocker_message(&tx, &ty, &ox, &oy);
        assert!(msg.contains("refuses KMX emission"));
        assert!(msg.contains("target_Q=1562"));
        assert!(msg.contains("target_T=2203351"));
        assert!(msg.contains("materialized branch history"));
        assert!(msg.contains("phase-clean parent source advance/cleanup"));
    }

    #[test]
    fn scaled_coeff_step_inverse_roundtrips_toy_field() {
        let p = 251u64;
        for branch in 0..=1 {
            for old_g0 in 0..=1 {
                if branch == 1 && old_g0 == 0 {
                    continue;
                }
                for v in 0..p {
                    for r in 0..p {
                        let (v1, r1) = scaled_coeff_step_pure(v, r, branch, old_g0, p);
                        let (v0, r0) = scaled_coeff_step_inverse_pure(v1, r1, branch, old_g0, p);
                        assert_eq!((v0, r0), (v, r));
                    }
                }
            }
        }
    }

    #[test]
    fn scaled_coeff_step_gate_matches_u256_semantics() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch = b.alloc_qubit();
        let old_g0 = b.alloc_qubit();
        emit_round218_scaled_coeff_step(&mut b, &v, &r, branch, old_g0, p);
        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 2);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-scaled-coeff-step-gate-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = coefficient_gate_cases();
        for (shot, &(v0, r0, branch0, old_g00)) in cases.iter().enumerate() {
            sim.set_register(&regs[0], v0, shot);
            sim.set_register(&regs[1], r0, shot);
            set_control(&mut sim, branch, branch0, shot);
            set_control(&mut sim, old_g0, old_g00, shot);
        }
        sim.apply(&b.ops);
        assert_eq!(sim.global_phase(), 0, "coefficient step left phase garbage");
        for (shot, &(v0, r0, branch0, old_g00)) in cases.iter().enumerate() {
            let expect = scaled_coeff_step_pure_u256(v0, r0, branch0, old_g00, p);
            assert_eq!(sim.get_register(&regs[0], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[1], shot), expect.1, "R shot {shot}");
            assert_eq!((sim.qubit(branch) >> shot) & 1, branch0 as u64);
            assert_eq!((sim.qubit(old_g0) >> shot) & 1, old_g00 as u64);
        }
        println!("METRIC round218_b5_scaled_coeff_step_toffoli={toffoli}");
        println!("METRIC round218_b5_scaled_coeff_step_qubits={num_qubits}");
    }

    #[test]
    fn fast_scaled_coeff_step_gate_matches_u256_semantics() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch = b.alloc_qubit();
        let old_g0 = b.alloc_qubit();
        emit_round218_scaled_coeff_step_fast(&mut b, &v, &r, branch, old_g0, p);
        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 2);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-fast-scaled-coeff-step-gate-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = coefficient_gate_cases();
        for (shot, &(v0, r0, branch0, old_g00)) in cases.iter().enumerate() {
            sim.set_register(&regs[0], v0, shot);
            sim.set_register(&regs[1], r0, shot);
            set_control(&mut sim, branch, branch0, shot);
            set_control(&mut sim, old_g0, old_g00, shot);
        }
        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "fast coefficient step left phase garbage"
        );
        for (shot, &(v0, r0, branch0, old_g00)) in cases.iter().enumerate() {
            let expect = scaled_coeff_step_pure_u256(v0, r0, branch0, old_g00, p);
            assert_eq!(sim.get_register(&regs[0], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[1], shot), expect.1, "R shot {shot}");
            assert_eq!((sim.qubit(branch) >> shot) & 1, branch0 as u64);
            assert_eq!((sim.qubit(old_g0) >> shot) & 1, old_g00 as u64);
        }
        println!("METRIC round218_b5_fast_scaled_coeff_step_toffoli={toffoli}");
        println!("METRIC round218_b5_fast_scaled_coeff_step_qubits={num_qubits}");
    }

    #[test]
    fn scaled_coeff_step_inverse_gate_roundtrips_and_cleans() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch = b.alloc_qubit();
        let old_g0 = b.alloc_qubit();
        emit_round218_scaled_coeff_step(&mut b, &v, &r, branch, old_g0, p);
        emit_round218_scaled_coeff_step_inverse(&mut b, &v, &r, branch, old_g0, p);
        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-scaled-coeff-step-inverse-gate-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = coefficient_gate_cases();
        for (shot, &(v0, r0, branch0, old_g00)) in cases.iter().enumerate() {
            sim.set_register(&regs[0], v0, shot);
            sim.set_register(&regs[1], r0, shot);
            set_control(&mut sim, branch, branch0, shot);
            set_control(&mut sim, old_g0, old_g00, shot);
        }
        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "coefficient inverse left phase garbage"
        );
        for (shot, &(v0, r0, branch0, old_g00)) in cases.iter().enumerate() {
            assert_eq!(sim.get_register(&regs[0], shot), v0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[1], shot), r0, "R shot {shot}");
            assert_eq!((sim.qubit(branch) >> shot) & 1, branch0 as u64);
            assert_eq!((sim.qubit(old_g0) >> shot) & 1, old_g00 as u64);
        }
        for q in reg(&v)
            .into_iter()
            .chain(reg(&r))
            .filter_map(|item| match item {
                QubitOrBit::Qubit(q) => Some(q),
                QubitOrBit::Bit(_) => None,
            })
        {
            *sim.qubit_mut(q) = 0;
        }
        *sim.qubit_mut(branch) = 0;
        *sim.qubit_mut(old_g0) = 0;
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(value, 0, "scratch qubit q{idx} not clean");
        }
    }

    #[test]
    fn scaled_coeff_fixed_b5_block_gate_matches_matrix_semantics() {
        let p = SECP256K1_P;
        for row in representative_b5_rows() {
            let mut b = B::new();
            let v = b.alloc_qubits(N);
            b.declare_qubit_register(&v);
            let r = b.alloc_qubits(N);
            b.declare_qubit_register(&r);
            emit_round218_scaled_coeff_block_fixed(&mut b, &v, &r, &row, p);
            let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
            let toffoli = b
                .ops
                .iter()
                .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
                .count();
            assert_eq!(regs.len(), 2);

            let mut seed = Shake128::default();
            seed.update(b"round218-b5-scaled-coeff-fixed-block-gate-v1");
            seed.update(&[row.branch_word, row.old_g0_word, row.block_index as u8]);
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let cases = coefficient_gate_cases();
            for (shot, &(v0, r0, _, _)) in cases.iter().enumerate() {
                sim.set_register(&regs[0], v0, shot);
                sim.set_register(&regs[1], r0, shot);
            }
            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "fixed B=5 block left phase garbage for row {row:?}"
            );
            for (shot, &(v0, r0, _, _)) in cases.iter().enumerate() {
                let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
                assert_eq!(
                    sim.get_register(&regs[0], shot),
                    expect.0,
                    "V shot {shot} row {row:?}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], shot),
                    expect.1,
                    "R shot {shot} row {row:?}"
                );
            }
            println!(
                "METRIC round218_b5_fixed_block_{}_{}_toffoli={toffoli}",
                row.branch_word, row.old_g0_word
            );
            println!(
                "METRIC round218_b5_fixed_block_{}_{}_qubits={num_qubits}",
                row.branch_word, row.old_g0_word
            );
        }
    }

    #[test]
    fn scaled_coeff_fixed_b5_stepwise_block_is_phase_clean_and_cheaper() {
        let p = SECP256K1_P;
        for row in representative_b5_rows() {
            let exact_toffoli = {
                let mut exact = B::new();
                let exact_v = exact.alloc_qubits(N);
                let exact_r = exact.alloc_qubits(N);
                emit_round218_scaled_coeff_block_fixed_exact(
                    &mut exact, &exact_v, &exact_r, &row, p,
                );
                exact
                    .ops
                    .iter()
                    .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
                    .count()
            };

            let mut b = B::new();
            let v = b.alloc_qubits(N);
            b.declare_qubit_register(&v);
            let r = b.alloc_qubits(N);
            b.declare_qubit_register(&r);
            emit_round218_scaled_coeff_block_fixed_stepwise(&mut b, &v, &r, &row, p);
            let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
            let stepwise_toffoli = b
                .ops
                .iter()
                .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
                .count();
            assert_eq!(regs.len(), 2);
            assert!(
                stepwise_toffoli < exact_toffoli,
                "stepwise fixed block was not cheaper for row {row:?}: \
                 stepwise={stepwise_toffoli}, exact={exact_toffoli}"
            );
            assert!(
                stepwise_toffoli < 15_000,
                "stepwise fixed block exceeded the Round218 B=5 target for row {row:?}: \
                 stepwise={stepwise_toffoli}"
            );

            let mut seed = Shake128::default();
            seed.update(b"round218-b5-scaled-coeff-fixed-stepwise-block-gate-v1");
            seed.update(&[row.branch_word, row.old_g0_word, row.block_index as u8]);
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let cases = coefficient_gate_cases();
            for (shot, &(v0, r0, _, _)) in cases.iter().enumerate() {
                sim.set_register(&regs[0], v0, shot);
                sim.set_register(&regs[1], r0, shot);
            }
            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "stepwise fixed B=5 block left phase garbage for row {row:?}"
            );
            for (shot, &(v0, r0, _, _)) in cases.iter().enumerate() {
                let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
                assert_eq!(
                    sim.get_register(&regs[0], shot),
                    expect.0,
                    "V shot {shot} row {row:?}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], shot),
                    expect.1,
                    "R shot {shot} row {row:?}"
                );
            }

            for q in reg(&v)
                .into_iter()
                .chain(reg(&r))
                .filter_map(|item| match item {
                    QubitOrBit::Qubit(q) => Some(q),
                    QubitOrBit::Bit(_) => None,
                })
            {
                *sim.qubit_mut(q) = 0;
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(value, 0, "stepwise scratch qubit q{idx} not clean");
            }

            println!(
                "METRIC round218_b5_stepwise_fixed_block_{}_{}_toffoli={stepwise_toffoli}",
                row.branch_word, row.old_g0_word
            );
            println!(
                "METRIC round218_b5_stepwise_fixed_block_{}_{}_qubits={num_qubits}",
                row.branch_word, row.old_g0_word
            );
            println!(
                "METRIC round218_b5_stepwise_fixed_block_{}_{}_exact_toffoli={exact_toffoli}",
                row.branch_word, row.old_g0_word
            );
        }
    }

    #[test]
    fn scaled_coeff_fixed_b5_dyadic_lift_block_matches_matrix_semantics_and_cleans() {
        let p = SECP256K1_P;
        for row in dyadic_lift_probe_rows() {
            let mut b = B::new();
            let v = b.alloc_qubits(N);
            b.declare_qubit_register(&v);
            let r = b.alloc_qubits(N);
            b.declare_qubit_register(&r);
            emit_round218_scaled_coeff_block_fixed_dyadic_lift(&mut b, &v, &r, &row, p);
            let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
            let toffoli = b
                .ops
                .iter()
                .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
                .count();
            assert_eq!(regs.len(), 2);
            assert!(
                toffoli <= 14_000,
                "dyadic-lift fixed block exceeded the Round412 exact-emitter probe bound \
                 for row {row:?}: {toffoli}"
            );

            let mut seed = Shake128::default();
            seed.update(b"round218-b5-scaled-coeff-fixed-dyadic-lift-block-gate-v1");
            seed.update(&[row.branch_word, row.old_g0_word, row.block_index as u8]);
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let cases = coefficient_gate_cases();
            for (shot, &(v0, r0, _, _)) in cases.iter().enumerate() {
                sim.set_register(&regs[0], v0, shot);
                sim.set_register(&regs[1], r0, shot);
            }
            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "dyadic-lift fixed B=5 block left phase garbage for row {row:?}"
            );
            for (shot, &(v0, r0, _, _)) in cases.iter().enumerate() {
                let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
                assert_eq!(
                    sim.get_register(&regs[0], shot),
                    expect.0,
                    "V shot {shot} row {row:?}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], shot),
                    expect.1,
                    "R shot {shot} row {row:?}"
                );
            }

            for q in reg(&v)
                .into_iter()
                .chain(reg(&r))
                .filter_map(|item| match item {
                    QubitOrBit::Qubit(q) => Some(q),
                    QubitOrBit::Bit(_) => None,
                })
            {
                *sim.qubit_mut(q) = 0;
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(value, 0, "dyadic-lift scratch qubit q{idx} not clean");
            }

            println!(
                "METRIC round218_b5_dyadic_lift_fixed_block_{}_{}_toffoli={toffoli}",
                row.branch_word, row.old_g0_word
            );
            println!(
                "METRIC round218_b5_dyadic_lift_fixed_block_{}_{}_qubits={num_qubits}",
                row.branch_word, row.old_g0_word
            );
        }
    }

    #[test]
    fn scaled_coeff_fixed_b5_one_div32_block_matches_matrix_semantics() {
        let p = SECP256K1_P;
        for row in representative_b5_rows() {
            let mut b = B::new();
            let v = b.alloc_qubits(N);
            b.declare_qubit_register(&v);
            let r = b.alloc_qubits(N);
            b.declare_qubit_register(&r);
            emit_round218_scaled_coeff_block_fixed_one_div32(&mut b, &v, &r, &row, p);
            let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
            let toffoli = b
                .ops
                .iter()
                .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
                .count();
            assert_eq!(regs.len(), 2);

            let mut seed = Shake128::default();
            seed.update(b"round218-b5-scaled-coeff-one-div32-block-gate-v1");
            seed.update(&[row.branch_word, row.old_g0_word, row.block_index as u8]);
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let cases = coefficient_gate_cases();
            for (shot, &(v0, r0, _, _)) in cases.iter().enumerate() {
                sim.set_register(&regs[0], v0, shot);
                sim.set_register(&regs[1], r0, shot);
            }
            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "one-div32 fixed B=5 block left phase garbage for row {row:?}"
            );
            for (shot, &(v0, r0, _, _)) in cases.iter().enumerate() {
                let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
                assert_eq!(
                    sim.get_register(&regs[0], shot),
                    expect.0,
                    "V shot {shot} row {row:?}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], shot),
                    expect.1,
                    "R shot {shot} row {row:?}"
                );
            }

            for q in reg(&v)
                .into_iter()
                .chain(reg(&r))
                .filter_map(|item| match item {
                    QubitOrBit::Qubit(q) => Some(q),
                    QubitOrBit::Bit(_) => None,
                })
            {
                *sim.qubit_mut(q) = 0;
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(value, 0, "one-div32 scratch qubit q{idx} not clean");
            }

            println!(
                "METRIC round218_b5_one_div32_fixed_block_{}_{}_toffoli={toffoli}",
                row.branch_word, row.old_g0_word
            );
            println!(
                "METRIC round218_b5_one_div32_fixed_block_{}_{}_qubits={num_qubits}",
                row.branch_word, row.old_g0_word
            );
        }
    }

    #[test]
    fn selected_lulu_coeffs_cover_valid_b5_words() {
        for branch_word in 0u8..32 {
            for old_g0_word in 0u8..32 {
                let Some(numerator) =
                    round218_b5_valid_selector_numerator(branch_word, old_g0_word)
                else {
                    continue;
                };
                let c = numerator.a10;
                let d = numerator.a11;
                let (u, w) = round218_b5_small_bezout(c, d);
                assert_eq!(c * u + d * w, 1);
                let preconditioner = phase_b5_execution::Matrix2 {
                    a00: c,
                    a01: d,
                    a10: -w,
                    a11: u,
                };
                let qs = round218_b5_lulu_shear_coeffs(preconditioner);
                assert!(
                    qs[0].abs() <= 15,
                    "q1 too wide for {branch_word},{old_g0_word}: {qs:?}"
                );
                assert!(
                    qs[1].abs() <= 8,
                    "q2 too wide for {branch_word},{old_g0_word}: {qs:?}"
                );
                assert!(
                    qs[2].abs() <= 6,
                    "q3 too wide for {branch_word},{old_g0_word}: {qs:?}"
                );
                assert!(
                    qs[3].abs() <= 29,
                    "q4 too wide for {branch_word},{old_g0_word}: {qs:?}"
                );
                let t = numerator.a00 * u + numerator.a01 * w;
                assert!(
                    t.abs() <= 32,
                    "t too wide for {branch_word},{old_g0_word}: {t}"
                );

                let l1 = phase_b5_execution::Matrix2 {
                    a00: 1,
                    a01: 0,
                    a10: qs[0],
                    a11: 1,
                };
                let u2 = phase_b5_execution::Matrix2 {
                    a00: 1,
                    a01: qs[1],
                    a10: 0,
                    a11: 1,
                };
                let l3 = phase_b5_execution::Matrix2 {
                    a00: 1,
                    a01: 0,
                    a10: qs[2],
                    a11: 1,
                };
                let u4 = phase_b5_execution::Matrix2 {
                    a00: 1,
                    a01: qs[3],
                    a10: 0,
                    a11: 1,
                };
                let emitted = u4.mul(l3).mul(u2).mul(l1);
                assert_eq!(
                    emitted, preconditioner,
                    "LULU preconditioner mismatch for branch={branch_word}, old_g0={old_g0_word}"
                );
            }
        }
    }

    #[test]
    fn selected_lulu_valid_domain_anf_masks_match_coefficients() {
        let coeffs = [
            Round218B5SelectedCoeff::LuluQ1,
            Round218B5SelectedCoeff::LuluQ2,
            Round218B5SelectedCoeff::LuluQ3,
            Round218B5SelectedCoeff::LuluQ4,
            Round218B5SelectedCoeff::MinusT,
        ];
        for coeff in coeffs {
            for output_idx in 0..=coeff.max_mag_bits() {
                let magnitude_bit = if output_idx == 0 {
                    None
                } else {
                    Some(output_idx - 1)
                };
                let masks = round218_b5_selected_coeff_anf_masks(coeff, magnitude_bit);
                for branch_word in 0u8..32 {
                    for old_g0_word in 0u8..32 {
                        let Some(selected_coeff) =
                            round218_b5_selected_coeff_for_words(coeff, branch_word, old_g0_word)
                        else {
                            continue;
                        };
                        let selector = branch_word as u16
                            | ((old_g0_word as u16) << phase_b5_execution::ROUND218_B5_BLOCK_BITS);
                        let got = masks
                            .iter()
                            .fold(0u8, |acc, &mask| acc ^ ((selector & mask == mask) as u8));
                        let expect = match magnitude_bit {
                            None => (selected_coeff < 0) as u8,
                            Some(bit_idx) => ((selected_coeff.unsigned_abs() >> bit_idx) & 1) as u8,
                        };
                        assert_eq!(
                            got, expect,
                            "ANF mismatch for {coeff:?} output {magnitude_bit:?} branch={branch_word} old={old_g0_word}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_zeta_lulu_anf_masks_match_coefficients() {
        let coeffs = [
            Round218B5SelectedCoeff::LuluQ1,
            Round218B5SelectedCoeff::LuluQ2,
            Round218B5SelectedCoeff::LuluQ3,
            Round218B5SelectedCoeff::LuluQ4,
            Round218B5SelectedCoeff::MinusT,
        ];
        let zetas = [-9i128, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 9];
        for zeta_start in zetas {
            for coeff in coeffs {
                for output_idx in 0..=coeff.max_mag_bits() {
                    let magnitude_bit = if output_idx == 0 {
                        None
                    } else {
                        Some(output_idx - 1)
                    };
                    let masks =
                        round218_b5_fixed_zeta_coeff_anf_masks(zeta_start, coeff, magnitude_bit);
                    for old_g0_word in 0u8..32 {
                        let selected_coeff = round218_b5_selected_coeff_for_zeta_old_g0(
                            coeff,
                            zeta_start,
                            old_g0_word,
                        );
                        let got = masks
                            .iter()
                            .fold(0u8, |acc, &mask| acc ^ ((old_g0_word & mask == mask) as u8));
                        let expect = match magnitude_bit {
                            None => (selected_coeff < 0) as u8,
                            Some(bit_idx) => ((selected_coeff.unsigned_abs() >> bit_idx) & 1) as u8,
                        };
                        assert_eq!(
                            got, expect,
                            "fixed-zeta ANF mismatch zeta={zeta_start} {coeff:?} output {magnitude_bit:?} old={old_g0_word}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_zeta_lulu_one_div32_b5_block_matches_matrix_semantics_and_cleans() {
        let p = SECP256K1_P;
        let zeta_start = -1i128;
        let mut b = B::new();
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_word);

        emit_round218_scaled_coeff_b5_block_fixed_zeta_lulu_one_div32(
            &mut b,
            &v,
            &r,
            &old_g0_word,
            zeta_start,
            p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 3);

        let old_words = [0u8, 31, 3, 18];
        let cases = coefficient_gate_cases();
        let mut seed = Shake128::default();
        seed.update(b"round218-b5-fixed-zeta-lulu-one-div32-block-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        for (shot, (&old_g0, &(v0, r0, _, _))) in old_words.iter().zip(cases.iter()).enumerate() {
            sim.set_register(&regs[0], v0, shot);
            sim.set_register(&regs[1], r0, shot);
            sim.set_register(&regs[2], U256::from(old_g0 as u64), shot);
        }

        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "fixed-zeta LULU one-div32 B=5 block left phase garbage"
        );
        for (shot, (&old_g0, &(v0, r0, _, _))) in old_words.iter().zip(cases.iter()).enumerate() {
            let numerator = round218_b5_numerator_for_zeta_old_g0(zeta_start, old_g0);
            let expect = scaled_coeff_numerator_div32_pure_u256(numerator, v0, r0, p);
            assert_eq!(sim.get_register(&regs[0], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[1], shot), expect.1, "R shot {shot}");
            assert_eq!(
                sim.get_register(&regs[2], shot),
                U256::from(old_g0 as u64),
                "old_g0 word shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(
                value, 0,
                "fixed-zeta LULU one-div32 scratch qubit q{idx} not clean"
            );
        }

        println!("METRIC round218_b5_fixed_zeta_lulu_one_div32_block_toffoli={toffoli}");
        println!("METRIC round218_b5_fixed_zeta_lulu_one_div32_block_qubits={num_qubits}");
    }

    #[test]
    fn selected_lulu_one_div32_b5_block_matches_matrix_semantics_and_cleans() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&branch_word);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_word);

        emit_round218_scaled_coeff_b5_block_selected_lulu_one_div32(
            &mut b,
            &v,
            &r,
            &branch_word,
            &old_g0_word,
            p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 4);

        let rows = representative_b5_rows();
        let cases = coefficient_gate_cases();
        let mut seed = Shake128::default();
        seed.update(b"round218-b5-selected-lulu-one-div32-block-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        for (shot, (row, &(v0, r0, _, _))) in rows.iter().zip(cases.iter()).enumerate() {
            sim.set_register(&regs[0], v0, shot);
            sim.set_register(&regs[1], r0, shot);
            sim.set_register(&regs[2], U256::from(row.branch_word as u64), shot);
            sim.set_register(&regs[3], U256::from(row.old_g0_word as u64), shot);
        }

        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "selected LULU one-div32 B=5 block left phase garbage"
        );
        for (shot, (row, &(v0, r0, _, _))) in rows.iter().zip(cases.iter()).enumerate() {
            let expect = scaled_coeff_block_fixed_pure_u256(row, v0, r0, p);
            assert_eq!(sim.get_register(&regs[0], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[1], shot), expect.1, "R shot {shot}");
            assert_eq!(
                sim.get_register(&regs[2], shot),
                U256::from(row.branch_word as u64),
                "branch word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[3], shot),
                U256::from(row.old_g0_word as u64),
                "old_g0 word shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(
                value, 0,
                "selected LULU one-div32 scratch qubit q{idx} not clean"
            );
        }

        println!("METRIC round218_b5_selected_lulu_one_div32_block_toffoli={toffoli}");
        println!("METRIC round218_b5_selected_lulu_one_div32_block_qubits={num_qubits}");
    }

    #[test]
    fn lazy_selected_b5_block_matches_matrix_semantics_and_cleans() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&branch_word);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_word);

        emit_round218_scaled_coeff_b5_block_selected_lazy(
            &mut b,
            &v,
            &r,
            &branch_word,
            &old_g0_word,
            p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 4);

        let rows = representative_b5_rows();
        let cases = coefficient_gate_cases();
        let mut seed = Shake128::default();
        seed.update(b"round218-b5-lazy-selected-block-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        for (shot, (row, &(v0, r0, _, _))) in rows.iter().zip(cases.iter()).enumerate() {
            sim.set_register(&regs[0], v0, shot);
            sim.set_register(&regs[1], r0, shot);
            sim.set_register(&regs[2], U256::from(row.branch_word as u64), shot);
            sim.set_register(&regs[3], U256::from(row.old_g0_word as u64), shot);
        }

        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "lazy selected B=5 block left phase garbage"
        );
        for (shot, (row, &(v0, r0, _, _))) in rows.iter().zip(cases.iter()).enumerate() {
            let expect = scaled_coeff_block_fixed_pure_u256(row, v0, r0, p);
            assert_eq!(sim.get_register(&regs[0], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[1], shot), expect.1, "R shot {shot}");
            assert_eq!(
                sim.get_register(&regs[2], shot),
                U256::from(row.branch_word as u64),
                "branch word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[3], shot),
                U256::from(row.old_g0_word as u64),
                "old_g0 word shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(
                value, 0,
                "lazy selected block scratch qubit q{idx} not clean"
            );
        }
        println!("METRIC round218_b5_lazy_selected_block_toffoli={toffoli}");
        println!("METRIC round218_b5_lazy_selected_block_qubits={num_qubits}");
    }

    #[test]
    fn unscaled_selected_b5_block_matches_numerator_semantics_and_cleans() {
        let p = SECP256K1_P;
        let mut b = B::new();
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&branch_word);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_word);

        emit_round218_unscaled_coeff_b5_block_selected(
            &mut b,
            &v,
            &r,
            &branch_word,
            &old_g0_word,
            p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 4);

        let rows = representative_b5_rows();
        let cases = coefficient_gate_cases();
        let mut seed = Shake128::default();
        seed.update(b"round218-b5-unscaled-selected-block-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        for (shot, (row, &(v0, r0, _, _))) in rows.iter().zip(cases.iter()).enumerate() {
            sim.set_register(&regs[0], v0, shot);
            sim.set_register(&regs[1], r0, shot);
            sim.set_register(&regs[2], U256::from(row.branch_word as u64), shot);
            sim.set_register(&regs[3], U256::from(row.old_g0_word as u64), shot);
        }

        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "unscaled selected B=5 block left phase garbage"
        );
        for (shot, (row, &(v0, r0, _, _))) in rows.iter().zip(cases.iter()).enumerate() {
            let expect = unscaled_coeff_block_fixed_pure_u256(row, v0, r0, p);
            assert_eq!(sim.get_register(&regs[0], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[1], shot), expect.1, "R shot {shot}");
            assert_eq!(
                sim.get_register(&regs[2], shot),
                U256::from(row.branch_word as u64),
                "branch word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[3], shot),
                U256::from(row.old_g0_word as u64),
                "old_g0 word shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(
                value, 0,
                "unscaled selected block scratch qubit q{idx} not clean"
            );
        }
        println!("METRIC round218_b5_unscaled_selected_block_toffoli={toffoli}");
        println!("METRIC round218_b5_unscaled_selected_block_qubits={num_qubits}");
    }

    #[test]
    fn source_live_b5_block_component_selects_and_transports_coefficients() {
        let p = SECP256K1_P;
        let zeta_start = -1;
        let mut b = B::new();
        let f_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&f_low);
        let g_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&g_low);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&branch_word);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_word);

        emit_round218_b5_source_live_transport_block(
            &mut b,
            &f_low,
            &g_low,
            &v,
            &r,
            zeta_start,
            &branch_word,
            &old_g0_word,
            p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 6);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-source-live-block-component-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            (31u8, 1u8, U256::ZERO, U256::from(1u64)),
            (15u8, 3u8, U256::from(2u64), U256::from(3u64)),
            (23u8, 30u8, U256::from(3u64), U256::from(5u64)),
            (
                1u8,
                17u8,
                U256::from_limbs([
                    0x243f_6a88_85a3_08d3,
                    0x1319_8a2e_0370_7344,
                    0xa409_3822_299f_31d0,
                    0x082e_fa98_ec4e_6c89,
                ]) % p,
                U256::from_limbs([
                    0x4528_21e6_38d0_1377,
                    0xbe54_66cf_34e9_0c6c,
                    0xc0ac_29b7_c97c_50dd,
                    0x3f84_d5b5_b547_0917,
                ]) % p,
            ),
        ];
        for (shot, &(f0, g0, v0, r0)) in cases.iter().enumerate() {
            sim.set_register(&regs[0], U256::from(f0), shot);
            sim.set_register(&regs[1], U256::from(g0), shot);
            sim.set_register(&regs[2], v0, shot);
            sim.set_register(&regs[3], r0, shot);
        }

        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "source-live block left phase garbage"
        );
        for (shot, &(f0, g0, v0, r0)) in cases.iter().enumerate() {
            let row = phase_b5_execution::block_row(
                0,
                phase_b5_execution::BlockSelector {
                    zeta_start,
                    f_low: f0,
                    g_low: g0,
                    width: phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8,
                },
            );
            let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
            assert_eq!(
                sim.get_register(&regs[0], shot),
                U256::from(f0),
                "f_low shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[1], shot),
                U256::from(g0),
                "g_low shot {shot}"
            );
            assert_eq!(sim.get_register(&regs[2], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[3], shot), expect.1, "R shot {shot}");
            assert_eq!(
                sim.get_register(&regs[4], shot),
                U256::from(row.branch_word),
                "branch word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[5], shot),
                U256::from(row.old_g0_word),
                "old_g0 word shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(value, 0, "source-live block scratch qubit q{idx} not clean");
        }
        println!("METRIC round218_b5_source_live_block_toffoli={toffoli}");
        println!("METRIC round218_b5_source_live_block_qubits={num_qubits}");
    }

    #[test]
    fn source_window_b5_block_parses_next_lows_and_transports_coefficients() {
        let p = SECP256K1_P;
        let zeta_start = -1;
        let mut b = B::new();
        let f_window = b.alloc_qubits(phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&branch_word);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_word);
        let next_f_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&next_f_low);
        let next_g_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&next_g_low);

        emit_round218_b5_source_window_transport_block(
            &mut b,
            &f_window,
            &g_window,
            &v,
            &r,
            zeta_start,
            &branch_word,
            &old_g0_word,
            &next_f_low,
            &next_g_low,
            p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 8);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-source-window-block-component-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            (31u16, 1u16, U256::ZERO, U256::from(1u64)),
            (31u16 | 512, 1u16, U256::from(2u64), U256::from(3u64)),
            (31u16, 1u16 | 512, U256::from(3u64), U256::from(5u64)),
            (
                777u16,
                1001u16,
                U256::from_limbs([
                    0x243f_6a88_85a3_08d3,
                    0x1319_8a2e_0370_7344,
                    0xa409_3822_299f_31d0,
                    0x082e_fa98_ec4e_6c89,
                ]) % p,
                U256::from_limbs([
                    0x4528_21e6_38d0_1377,
                    0xbe54_66cf_34e9_0c6c,
                    0xc0ac_29b7_c97c_50dd,
                    0x3f84_d5b5_b547_0917,
                ]) % p,
            ),
        ];
        for (shot, &(f0, g0, v0, r0)) in cases.iter().enumerate() {
            sim.set_register(&regs[0], U256::from(f0 as u64), shot);
            sim.set_register(&regs[1], U256::from(g0 as u64), shot);
            sim.set_register(&regs[2], v0, shot);
            sim.set_register(&regs[3], r0, shot);
        }

        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "source-window block left phase garbage"
        );
        for (shot, &(f0, g0, v0, r0)) in cases.iter().enumerate() {
            let parsed =
                phase_b5_state_selector::round218_b5_low_window_parser_cell(zeta_start, f0, g0);
            let row = phase_b5_execution::block_row(
                0,
                phase_b5_execution::BlockSelector {
                    zeta_start,
                    f_low: (f0 & 31) as u8,
                    g_low: (g0 & 31) as u8,
                    width: phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8,
                },
            );
            let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
            assert_eq!(sim.get_register(&regs[0], shot), U256::from(f0 as u64));
            assert_eq!(sim.get_register(&regs[1], shot), U256::from(g0 as u64));
            assert_eq!(sim.get_register(&regs[2], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[3], shot), expect.1, "R shot {shot}");
            assert_eq!(
                sim.get_register(&regs[4], shot),
                U256::from(parsed.branch_word as u64),
                "branch word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[5], shot),
                U256::from(parsed.old_g0_word as u64),
                "old_g0 word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[6], shot),
                U256::from(parsed.next_f_low as u64),
                "next_f_low shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[7], shot),
                U256::from(parsed.next_g_low as u64),
                "next_g_low shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(
                value, 0,
                "source-window block scratch qubit q{idx} not clean"
            );
        }
        println!("METRIC round218_b5_source_window_block_toffoli={toffoli}");
        println!("METRIC round218_b5_source_window_block_qubits={num_qubits}");
    }

    #[test]
    fn dynamic_source_window_b5_block_carries_zeta_and_transports_coefficients() {
        let p = SECP256K1_P;
        let spec = phase_b5_state_selector::Round218B5DynamicZetaTransducerSpec::new(-2, 2);
        let mut b = B::new();
        let zeta_start = b.alloc_qubits(spec.start_zeta_bits());
        b.declare_qubit_register(&zeta_start);
        let f_window = b.alloc_qubits(phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&branch_word);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_word);
        let end_zeta = b.alloc_qubits(spec.end_zeta_bits());
        b.declare_qubit_register(&end_zeta);
        let next_f_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&next_f_low);
        let next_g_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&next_g_low);

        emit_round218_b5_dynamic_source_window_transport_block(
            &mut b,
            spec,
            &zeta_start,
            &f_window,
            &g_window,
            &v,
            &r,
            &branch_word,
            &old_g0_word,
            &end_zeta,
            &next_f_low,
            &next_g_low,
            p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 10);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-dynamic-source-window-block-component-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            (-2i128, 31u16, 1u16, U256::ZERO, U256::from(1u64)),
            (
                -1i128,
                31u16 | 512,
                1u16,
                U256::from(2u64),
                U256::from(3u64),
            ),
            (0i128, 31u16, 1u16 | 512, U256::from(3u64), U256::from(5u64)),
            (
                2i128,
                777u16,
                1001u16,
                U256::from_limbs([
                    0x243f_6a88_85a3_08d3,
                    0x1319_8a2e_0370_7344,
                    0xa409_3822_299f_31d0,
                    0x082e_fa98_ec4e_6c89,
                ]) % p,
                U256::from_limbs([
                    0x4528_21e6_38d0_1377,
                    0xbe54_66cf_34e9_0c6c,
                    0xc0ac_29b7_c97c_50dd,
                    0x3f84_d5b5_b547_0917,
                ]) % p,
            ),
        ];
        for (shot, &(zeta0, f0, g0, v0, r0)) in cases.iter().enumerate() {
            sim.set_register(
                &regs[0],
                U256::from(spec.encode_start_zeta(zeta0) as u64),
                shot,
            );
            sim.set_register(&regs[1], U256::from(f0 as u64), shot);
            sim.set_register(&regs[2], U256::from(g0 as u64), shot);
            sim.set_register(&regs[3], v0, shot);
            sim.set_register(&regs[4], r0, shot);
        }

        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "dynamic source-window block left phase garbage"
        );
        for (shot, &(zeta0, f0, g0, v0, r0)) in cases.iter().enumerate() {
            let parsed = phase_b5_execution::source_window_block_row(
                phase_b5_execution::SourceWindowSelector {
                    zeta_start: zeta0,
                    f_window: f0,
                    g_window: g0,
                    window_bits: phase_b5_execution::ROUND218_B5_SOURCE_WINDOW_BITS as u8,
                },
            );
            let row = phase_b5_execution::block_row(
                0,
                phase_b5_execution::BlockSelector {
                    zeta_start: zeta0,
                    f_low: (f0 & 31) as u8,
                    g_low: (g0 & 31) as u8,
                    width: phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8,
                },
            );
            let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
            assert_eq!(
                sim.get_register(&regs[0], shot),
                U256::from(spec.encode_start_zeta(zeta0) as u64)
            );
            assert_eq!(sim.get_register(&regs[1], shot), U256::from(f0 as u64));
            assert_eq!(sim.get_register(&regs[2], shot), U256::from(g0 as u64));
            assert_eq!(sim.get_register(&regs[3], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[4], shot), expect.1, "R shot {shot}");
            assert_eq!(
                sim.get_register(&regs[5], shot),
                U256::from(parsed.branch_word as u64),
                "branch word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[6], shot),
                U256::from(parsed.old_g0_word as u64),
                "old_g0 word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[7], shot),
                U256::from(spec.encode_end_zeta(parsed.end_zeta) as u64),
                "end_zeta shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[8], shot),
                U256::from(parsed.next_f_low as u64),
                "next_f_low shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[9], shot),
                U256::from(parsed.next_g_low as u64),
                "next_g_low shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(
                value, 0,
                "dynamic source-window block scratch qubit q{idx} not clean"
            );
        }
        println!("METRIC round218_b5_dynamic_source_window_block_toffoli={toffoli}");
        println!("METRIC round218_b5_dynamic_source_window_block_qubits={num_qubits}");
    }

    fn encode_twos_zeta(zeta: i128, bits: usize) -> U256 {
        let modulus = 1i128 << bits;
        U256::from(zeta.rem_euclid(modulus) as u64)
    }

    fn mix64(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^ (x >> 31)
    }

    fn deterministic_word(case_id: usize, salt: u64) -> u64 {
        mix64((case_id as u64).wrapping_mul(0xd134_2543_de82_ef95) ^ salt)
    }

    fn deterministic_field_element(case_id: usize, salt: u64, p: U256) -> U256 {
        U256::from_limbs([
            deterministic_word(case_id, salt),
            deterministic_word(case_id, salt ^ 0x243f_6a88_85a3_08d3),
            deterministic_word(case_id, salt ^ 0x1319_8a2e_0370_7344),
            deterministic_word(case_id, salt ^ 0xa409_3822_299f_31d0),
        ]) % p
    }

    fn deterministic_projective_scalar_case(
        case_id: usize,
        p: U256,
    ) -> (i128, u16, u16, U256, U256) {
        let zeta = ((deterministic_word(case_id, 0x7a65_7461) & 1023) as i128) - 512;
        let f_window = ((deterministic_word(case_id, 0x665f_7769) & 1023) as u16) | 1;
        let g_window = (deterministic_word(case_id, 0x675f_7769) & 1023) as u16;
        let v = deterministic_field_element(case_id, 0x565f_636f_6566_6630, p);
        let r = deterministic_field_element(case_id, 0x525f_636f_6566_6630, p);
        (zeta, f_window, g_window, v, r)
    }

    fn round314_test_control_hash(branch_word: u8, old_g0_word: u8) -> u8 {
        let control = u16::from(branch_word) | (u16::from(old_g0_word) << 5);
        let mut hash = 0u8;
        for (hash_bit, mask) in ROUND314_B5_HASH_MASKS.iter().copied().enumerate() {
            if ((control & mask).count_ones() & 1) != 0 {
                hash |= 1 << hash_bit;
            }
        }
        hash
    }

    fn round315_expected_rotated_window_endpoint(
        zeta_start: i128,
        f_window: u16,
        g_window: u16,
    ) -> (i128, u16, u16, u8, u8) {
        let parsed = phase_b5_state_selector::round218_b5_low_window_parser_cell(
            zeta_start, f_window, g_window,
        );
        let retained = phase_b5_state_selector::round218_b5_low_window_parser_retained_word(
            zeta_start, f_window, g_window,
        );
        let row = phase_b5_execution::source_window_block_row(
            phase_b5_execution::SourceWindowSelector {
                zeta_start,
                f_window,
                g_window,
                window_bits: phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS as u8,
            },
        );
        let mut retained_in_place = 0u16;
        for step in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
            let retained_bit = u16::from((retained >> step) & 1);
            retained_in_place |=
                retained_bit << (phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS - 1 - step);
        }
        (
            row.end_zeta,
            u16::from(parsed.next_f_low) | retained_in_place,
            u16::from(parsed.next_g_low),
            parsed.branch_word,
            parsed.old_g0_word,
        )
    }

    fn round326_actual_branch_and_post_zeta(zeta_start: i128, old_g0_word: u8) -> (u8, i128) {
        let mut zeta = zeta_start;
        let mut branch_word = 0u8;
        for step in 0..phase_b5_execution::ROUND218_B5_BLOCK_BITS {
            let old_g0 = ((old_g0_word >> step) & 1) != 0;
            let branch = zeta < 0 && old_g0;
            if branch {
                branch_word |= 1 << step;
                zeta = -zeta - 2;
            } else {
                zeta -= 1;
            }
        }
        (branch_word, zeta)
    }

    fn round326_branch_choices(post_zeta: i128, old_g0_word: u8) -> Vec<u8> {
        let mut out = Vec::new();
        for zeta_start in -586i128..584 {
            let (branch_word, got_post) =
                round326_actual_branch_and_post_zeta(zeta_start, old_g0_word);
            if got_post == post_zeta && !out.contains(&branch_word) {
                out.push(branch_word);
            }
        }
        out.sort_unstable();
        out
    }

    fn round326_live_l_rank(zeta_start: i128, old_g0_word: u8) -> u8 {
        let (branch_word, post_zeta) =
            round326_actual_branch_and_post_zeta(zeta_start, old_g0_word);
        let choices = round326_branch_choices(post_zeta, old_g0_word);
        choices
            .iter()
            .position(|&candidate| candidate == branch_word)
            .expect("actual branch must be in Round326 branch-choice table") as u8
    }

    fn round326_expected_branch(post_zeta: i128, old_g0_word: u8, rank: u8) -> u8 {
        let choices = round326_branch_choices(post_zeta, old_g0_word);
        choices.get(rank as usize).copied().unwrap_or(0)
    }

    #[test]
    fn round326_live_l_rank_exact_cover_component_exact() {
        let zeta_bits = 11usize;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let old_g0 = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0);
        let l_rank = b.alloc_qubits(4);
        b.declare_qubit_register(&l_rank);

        emit_round326_b5_live_l_rank_exact_cover(&mut b, &zeta, &old_g0, &l_rank);

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 3);

        const CASES: usize = 37_440;
        const LANES: usize = 64;
        for batch_start in (0..CASES).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round326-b5-live-l-rank-exact-cover-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (CASES - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let zeta0 = -586i128 + (case_id / 32) as i128;
                let old0 = (case_id & 31) as u8;
                let l0 = ((case_id * 7 + 3) & 15) as u8;
                sim.set_register(&regs[0], encode_twos_zeta(zeta0, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(old0 as u64), lane);
                sim.set_register(&regs[2], U256::from(l0 as u64), lane);
            }

            sim.apply(&b.ops);
            assert_eq!(sim.global_phase(), 0, "Round326 live-L phase garbage");

            for lane in 0..active {
                let case_id = batch_start + lane;
                let zeta0 = -586i128 + (case_id / 32) as i128;
                let old0 = (case_id & 31) as u8;
                let l0 = ((case_id * 7 + 3) & 15) as u8;
                let expected = l0 ^ round326_live_l_rank(zeta0, old0);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(zeta0, zeta_bits)
                );
                assert_eq!(sim.get_register(&regs[1], lane), U256::from(old0 as u64));
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(expected as u64)
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(value, 0, "Round326 live-L scratch qubit q{idx} not clean");
            }
        }

        println!("PASS round326_b5_live_l_rank_exact_cover_exact cases={CASES}");
        println!("METRIC round326_b5_live_l_rank_exact_cover_toffoli={toffoli}");
        println!("METRIC round326_b5_live_l_rank_exact_cover_qubits={num_qubits}");
    }

    #[test]
    fn round326_branch_rank_exact_cover_cleaner_exact_9024() {
        let zeta_bits = 11usize;
        let mut b = B::new();
        let post_zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&post_zeta);
        let old_g0 = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0);
        let l_rank = b.alloc_qubits(4);
        b.declare_qubit_register(&l_rank);
        let branch = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&branch);

        emit_round326_b5_branch_rank_exact_cover_cleaner(
            &mut b, &post_zeta, &old_g0, &l_rank, &branch,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 4);

        const CASES: usize = 9_024;
        const LANES: usize = 64;
        for batch_start in (0..CASES).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round326-b5-branch-rank-exact-cover-cleaner-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (CASES - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let post0 = -591i128 + (deterministic_word(case_id, 0x706f_7374) % 1180) as i128;
                let old0 = (deterministic_word(case_id, 0x6f6c_645f) & 31) as u8;
                let rank0 = (deterministic_word(case_id, 0x7261_6e6b) & 15) as u8;
                let branch0 = (deterministic_word(case_id, 0x6272_616e) & 31) as u8;
                sim.set_register(&regs[0], encode_twos_zeta(post0, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(old0 as u64), lane);
                sim.set_register(&regs[2], U256::from(rank0 as u64), lane);
                sim.set_register(&regs[3], U256::from(branch0 as u64), lane);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "Round326 branch cleaner phase garbage"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let post0 = -591i128 + (deterministic_word(case_id, 0x706f_7374) % 1180) as i128;
                let old0 = (deterministic_word(case_id, 0x6f6c_645f) & 31) as u8;
                let rank0 = (deterministic_word(case_id, 0x7261_6e6b) & 15) as u8;
                let branch0 = (deterministic_word(case_id, 0x6272_616e) & 31) as u8;
                let expected = branch0 ^ round326_expected_branch(post0, old0, rank0);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(post0, zeta_bits)
                );
                assert_eq!(sim.get_register(&regs[1], lane), U256::from(old0 as u64));
                assert_eq!(sim.get_register(&regs[2], lane), U256::from(rank0 as u64));
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    U256::from(expected as u64)
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "Round326 branch cleaner scratch qubit q{idx} not clean"
                );
            }
        }

        println!("PASS round326_b5_branch_rank_exact_cover_cleaner_exact_9024 cases={CASES}");
        println!("METRIC round326_b5_branch_rank_exact_cover_cleaner_toffoli={toffoli}");
        println!("METRIC round326_b5_branch_rank_exact_cover_cleaner_qubits={num_qubits}");
    }

    #[test]
    fn round331_old_g0_full_eraser_rust_component_exact_9024() {
        let ops = build_round331_b5_old_g0_full_eraser_component();
        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(ops.iter().copied());
        let toffoli = ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 4);
        assert_eq!(
            regs[0].len(),
            phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS
        );
        assert_eq!(
            regs[1].len(),
            phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS
        );
        assert_eq!(regs[2].len(), phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        assert_eq!(regs[3].len(), phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        assert_eq!(num_qubits, 196);
        assert_eq!(num_bits, 1);
        assert_eq!(toffoli, 133);

        const CASES: usize = 9_024;
        const LANES: usize = 64;
        for batch_start in (0..CASES).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round331-b5-old-g0-full-eraser-rust-9024-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (CASES - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, _v0, _r0) =
                    deterministic_projective_scalar_case(case_id, SECP256K1_P);
                let parsed =
                    phase_b5_state_selector::round218_b5_low_window_parser_cell(zeta0, f0, g0);
                sim.set_register(&regs[0], U256::from(f0 as u64), lane);
                sim.set_register(&regs[1], U256::from(g0 as u64), lane);
                sim.set_register(&regs[2], U256::from(parsed.branch_word as u64), lane);
                sim.set_register(&regs[3], U256::from(parsed.old_g0_word as u64), lane);
            }

            sim.apply(&ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "Round331 old_g0 eraser phase garbage in batch {batch_start}"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, _v0, _r0) =
                    deterministic_projective_scalar_case(case_id, SECP256K1_P);
                let parsed =
                    phase_b5_state_selector::round218_b5_low_window_parser_cell(zeta0, f0, g0);
                assert_eq!(sim.get_register(&regs[0], lane), U256::from(f0 as u64));
                assert_eq!(sim.get_register(&regs[1], lane), U256::from(g0 as u64));
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(parsed.branch_word as u64),
                    "branch preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    U256::ZERO,
                    "old_g0 not erased shot {case_id}"
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "Round331 old_g0 eraser scratch qubit q{idx} not clean in batch {batch_start}"
                );
            }
        }

        println!("PASS round331_b5_old_g0_full_eraser_rust_exact_9024 cases={CASES}");
        println!("METRIC round331_b5_old_g0_full_eraser_toffoli={toffoli}");
        println!("METRIC round331_b5_old_g0_full_eraser_qubits={num_qubits}");
    }

    #[test]
    fn twos_zeta_source_window_b5_block_updates_zeta_and_transports_coefficients() {
        let p = SECP256K1_P;
        let zeta_bits = 5usize;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f_window = b.alloc_qubits(phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&branch_word);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_word);
        let next_f_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&next_f_low);
        let next_g_low = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&next_g_low);

        emit_round218_b5_twos_zeta_source_window_transport_block(
            &mut b,
            &zeta,
            &f_window,
            &g_window,
            &v,
            &r,
            &branch_word,
            &old_g0_word,
            &next_f_low,
            &next_g_low,
            p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 9);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-twos-zeta-source-window-block-component-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            (-2i128, 31u16, 1u16, U256::ZERO, U256::from(1u64)),
            (
                -1i128,
                31u16 | 512,
                1u16,
                U256::from(2u64),
                U256::from(3u64),
            ),
            (0i128, 31u16, 1u16 | 512, U256::from(3u64), U256::from(5u64)),
            (
                2i128,
                777u16,
                1001u16,
                U256::from_limbs([
                    0x243f_6a88_85a3_08d3,
                    0x1319_8a2e_0370_7344,
                    0xa409_3822_299f_31d0,
                    0x082e_fa98_ec4e_6c89,
                ]) % p,
                U256::from_limbs([
                    0x4528_21e6_38d0_1377,
                    0xbe54_66cf_34e9_0c6c,
                    0xc0ac_29b7_c97c_50dd,
                    0x3f84_d5b5_b547_0917,
                ]) % p,
            ),
        ];
        for (shot, &(zeta0, f0, g0, v0, r0)) in cases.iter().enumerate() {
            sim.set_register(&regs[0], encode_twos_zeta(zeta0, zeta_bits), shot);
            sim.set_register(&regs[1], U256::from(f0 as u64), shot);
            sim.set_register(&regs[2], U256::from(g0 as u64), shot);
            sim.set_register(&regs[3], v0, shot);
            sim.set_register(&regs[4], r0, shot);
        }

        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "two's-complement source-window block left phase garbage"
        );
        for (shot, &(zeta0, f0, g0, v0, r0)) in cases.iter().enumerate() {
            let parsed = phase_b5_execution::source_window_block_row(
                phase_b5_execution::SourceWindowSelector {
                    zeta_start: zeta0,
                    f_window: f0,
                    g_window: g0,
                    window_bits: phase_b5_execution::ROUND218_B5_SOURCE_WINDOW_BITS as u8,
                },
            );
            let row = phase_b5_execution::block_row(
                0,
                phase_b5_execution::BlockSelector {
                    zeta_start: zeta0,
                    f_low: (f0 & 31) as u8,
                    g_low: (g0 & 31) as u8,
                    width: phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8,
                },
            );
            let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
            assert_eq!(
                sim.get_register(&regs[0], shot),
                encode_twos_zeta(parsed.end_zeta, zeta_bits),
                "end zeta shot {shot}"
            );
            assert_eq!(sim.get_register(&regs[1], shot), U256::from(f0 as u64));
            assert_eq!(sim.get_register(&regs[2], shot), U256::from(g0 as u64));
            assert_eq!(sim.get_register(&regs[3], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[4], shot), expect.1, "R shot {shot}");
            assert_eq!(
                sim.get_register(&regs[5], shot),
                U256::from(parsed.branch_word as u64),
                "branch word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[6], shot),
                U256::from(parsed.old_g0_word as u64),
                "old_g0 word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[7], shot),
                U256::from(parsed.next_f_low as u64),
                "next_f_low shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[8], shot),
                U256::from(parsed.next_g_low as u64),
                "next_g_low shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(
                value, 0,
                "two's-complement source-window block scratch qubit q{idx} not clean"
            );
        }
        println!("METRIC round218_b5_twos_zeta_source_window_block_toffoli={toffoli}");
        println!("METRIC round218_b5_twos_zeta_source_window_block_qubits={num_qubits}");
    }

    #[test]
    fn twos_zeta_control_transport_block_cleans_internal_controls() {
        let p = SECP256K1_P;
        let zeta_bits = 5usize;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f_window = b.alloc_qubits(phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);

        emit_round218_b5_twos_zeta_control_transport_block(
            &mut b, &zeta, &f_window, &g_window, &v, &r, p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 5);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-twos-zeta-control-transport-block-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            (-2i128, 31u16, 1u16, U256::ZERO, U256::from(1u64)),
            (
                -1i128,
                31u16 | 512,
                1u16,
                U256::from(2u64),
                U256::from(3u64),
            ),
            (0i128, 31u16, 1u16 | 512, U256::from(3u64), U256::from(5u64)),
            (
                2i128,
                777u16,
                1001u16,
                U256::from_limbs([
                    0x243f_6a88_85a3_08d3,
                    0x1319_8a2e_0370_7344,
                    0xa409_3822_299f_31d0,
                    0x082e_fa98_ec4e_6c89,
                ]) % p,
                U256::from_limbs([
                    0x4528_21e6_38d0_1377,
                    0xbe54_66cf_34e9_0c6c,
                    0xc0ac_29b7_c97c_50dd,
                    0x3f84_d5b5_b547_0917,
                ]) % p,
            ),
        ];
        for (shot, &(zeta0, f0, g0, v0, r0)) in cases.iter().enumerate() {
            sim.set_register(&regs[0], encode_twos_zeta(zeta0, zeta_bits), shot);
            sim.set_register(&regs[1], U256::from(f0 as u64), shot);
            sim.set_register(&regs[2], U256::from(g0 as u64), shot);
            sim.set_register(&regs[3], v0, shot);
            sim.set_register(&regs[4], r0, shot);
        }

        sim.apply(&b.ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "two's-complement control transport block left phase garbage"
        );
        for (shot, &(zeta0, f0, g0, v0, r0)) in cases.iter().enumerate() {
            let row = phase_b5_execution::block_row(
                0,
                phase_b5_execution::BlockSelector {
                    zeta_start: zeta0,
                    f_low: (f0 & 31) as u8,
                    g_low: (g0 & 31) as u8,
                    width: phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8,
                },
            );
            let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
            assert_eq!(
                sim.get_register(&regs[0], shot),
                encode_twos_zeta(zeta0, zeta_bits),
                "zeta preserved shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[1], shot),
                U256::from(f0 as u64),
                "f_window preserved shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[2], shot),
                U256::from(g0 as u64),
                "g_window preserved shot {shot}"
            );
            assert_eq!(sim.get_register(&regs[3], shot), expect.0, "V shot {shot}");
            assert_eq!(sim.get_register(&regs[4], shot), expect.1, "R shot {shot}");
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(
                value, 0,
                "two's-complement control transport scratch qubit q{idx} not clean"
            );
        }
        println!("METRIC round218_b5_twos_zeta_control_transport_toffoli={toffoli}");
        println!("METRIC round218_b5_twos_zeta_control_transport_qubits={num_qubits}");
    }

    #[test]
    fn round314_source_live_hash_transport_window_block_exact_9024() {
        let p = SECP256K1_P;
        let zeta_bits = 11usize;
        let window_bits = phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let l_hash = b.alloc_qubits(ROUND314_B5_HASH_BITS);
        b.declare_qubit_register(&l_hash);
        let next_bits = window_bits - phase_b5_execution::ROUND218_B5_BLOCK_BITS;
        let next_f = b.alloc_qubits(next_bits);
        b.declare_qubit_register(&next_f);
        let next_g = b.alloc_qubits(next_bits);
        b.declare_qubit_register(&next_g);

        emit_round314_b5_source_live_hash_transport_window_block(
            &mut b, &zeta, &f_window, &g_window, &v, &r, &l_hash, &next_f, &next_g, p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 8);

        const SHOTS: usize = 9024;
        const LANES: usize = 64;
        for batch_start in (0..SHOTS).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round314-b5-source-live-hash-transport-window-9024-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (SHOTS - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                sim.set_register(&regs[0], encode_twos_zeta(zeta0, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(f0 as u64), lane);
                sim.set_register(&regs[2], U256::from(g0 as u64), lane);
                sim.set_register(&regs[3], v0, lane);
                sim.set_register(&regs[4], r0, lane);
                sim.set_register(&regs[5], U256::ZERO, lane);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "Round314 source-live hash transport left phase garbage in batch {batch_start}"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                let parsed = phase_b5_execution::source_window_block_row(
                    phase_b5_execution::SourceWindowSelector {
                        zeta_start: zeta0,
                        f_window: f0,
                        g_window: g0,
                        window_bits: phase_b5_execution::ROUND218_B5_SOURCE_WINDOW_BITS as u8,
                    },
                );
                let row = phase_b5_execution::block_row(
                    0,
                    phase_b5_execution::BlockSelector {
                        zeta_start: zeta0,
                        f_low: (f0 & 31) as u8,
                        g_low: (g0 & 31) as u8,
                        width: phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8,
                    },
                );
                let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
                let expect_hash =
                    round314_test_control_hash(parsed.branch_word, parsed.old_g0_word);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(parsed.end_zeta, zeta_bits),
                    "end zeta shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], lane),
                    U256::from(f0 as u64),
                    "f_window preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(g0 as u64),
                    "g_window preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    expect.0,
                    "V shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[4], lane),
                    expect.1,
                    "R shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[5], lane),
                    U256::from(expect_hash as u64),
                    "L hash shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[6], lane),
                    U256::from(parsed.next_f_low as u64),
                    "next_f shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[7], lane),
                    U256::from(parsed.next_g_low as u64),
                    "next_g shot {case_id}"
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "Round314 source-live hash transport scratch qubit q{idx} not clean in batch {batch_start}"
                );
            }
        }

        println!("PASS round314_b5_source_live_hash_transport_window_exact_9024 shots={SHOTS}");
        println!("METRIC round314_b5_source_live_hash_transport_window_toffoli={toffoli}");
        println!("METRIC round314_b5_source_live_hash_transport_window_qubits={num_qubits}");
    }

    #[test]
    fn round315_hash_backward_block_restores_source_and_consumes_l_exact_9024() {
        let p = SECP256K1_P;
        let zeta_bits = 11usize;
        let window_bits = phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f);
        let g = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g);
        let l_hash = b.alloc_qubits(ROUND314_B5_HASH_BITS);
        b.declare_qubit_register(&l_hash);
        let branch_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        let old_g0_word = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);

        emit_round315_b5_source_stream_backward_block_from_hash(
            &mut b,
            &zeta,
            &f,
            &g,
            0,
            &l_hash,
            &branch_word,
            &old_g0_word,
        );
        b.free_vec(&old_g0_word);
        b.free_vec(&branch_word);

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 4);

        const SHOTS: usize = 9024;
        const LANES: usize = 64;
        for batch_start in (0..SHOTS).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round315-b5-hash-backward-block-9024-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (SHOTS - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, _v0, _r0) = deterministic_projective_scalar_case(case_id, p);
                let (end_zeta, full_f, full_g, branch_word, old_g0_word) =
                    round315_expected_rotated_window_endpoint(zeta0, f0, g0);
                let l_value = round314_test_control_hash(branch_word, old_g0_word);
                sim.set_register(&regs[0], encode_twos_zeta(end_zeta, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(full_f as u64), lane);
                sim.set_register(&regs[2], U256::from(full_g as u64), lane);
                sim.set_register(&regs[3], U256::from(l_value as u64), lane);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "Round315 hash rollback left phase garbage in batch {batch_start}"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, _v0, _r0) = deterministic_projective_scalar_case(case_id, p);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(zeta0, zeta_bits),
                    "restored zeta shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], lane),
                    U256::from(f0 as u64),
                    "restored f shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(g0 as u64),
                    "restored g shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    U256::ZERO,
                    "L hash not consumed shot {case_id}"
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "Round315 hash rollback scratch qubit q{idx} not clean in batch {batch_start}"
                );
            }
        }

        println!("PASS round315_b5_hash_backward_block_exact_9024 shots={SHOTS}");
        println!("METRIC round315_b5_hash_backward_block_toffoli={toffoli}");
        println!("METRIC round315_b5_hash_backward_block_qubits={num_qubits}");
    }

    #[test]
    fn source_live_projective_scalar_transport_block_exact_9024() {
        let p = SECP256K1_P;
        let zeta_bits = 11usize;
        let window_bits = phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);

        emit_round218_b5_source_live_projective_scalar_transport_block(
            &mut b, &zeta, &f_window, &g_window, &v, &r, p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 5);

        const SHOTS: usize = 9024;
        const LANES: usize = 64;
        for batch_start in (0..SHOTS).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round218-b5-source-live-projective-scalar-transport-9024-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (SHOTS - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                sim.set_register(&regs[0], encode_twos_zeta(zeta0, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(f0 as u64), lane);
                sim.set_register(&regs[2], U256::from(g0 as u64), lane);
                sim.set_register(&regs[3], v0, lane);
                sim.set_register(&regs[4], r0, lane);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "projective-scalar transport left phase garbage in batch {batch_start}"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                let row = phase_b5_execution::block_row(
                    0,
                    phase_b5_execution::BlockSelector {
                        zeta_start: zeta0,
                        f_low: (f0 & 31) as u8,
                        g_low: (g0 & 31) as u8,
                        width: phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8,
                    },
                );
                let expect = scaled_coeff_block_fixed_pure_u256(&row, v0, r0, p);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(zeta0, zeta_bits),
                    "zeta preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], lane),
                    U256::from(f0 as u64),
                    "f_window preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(g0 as u64),
                    "g_window preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    expect.0,
                    "V shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[4], lane),
                    expect.1,
                    "R shot {case_id}"
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "projective-scalar transport scratch qubit q{idx} not clean in batch {batch_start}"
                );
            }
        }

        println!(
            "PASS round218_b5_source_live_projective_scalar_transport_exact_9024 shots={SHOTS}"
        );
        println!("METRIC round218_b5_source_live_projective_scalar_transport_toffoli={toffoli}");
        println!("METRIC round218_b5_source_live_projective_scalar_transport_qubits={num_qubits}");
    }

    #[test]
    fn round379_source_live_cheap_lft_frame_block_exact_9024() {
        let p = SECP256K1_P;
        let zeta_bits = 11usize;
        let window_bits = phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);

        emit_round379_b5_source_live_cheap_lft_frame_block(
            &mut b, &zeta, &f_window, &g_window, &v, &r, p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 5);

        const SHOTS: usize = 9024;
        const LANES: usize = 64;
        for batch_start in (0..SHOTS).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round379-b5-source-live-cheap-lft-frame-9024-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (SHOTS - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                sim.set_register(&regs[0], encode_twos_zeta(zeta0, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(f0 as u64), lane);
                sim.set_register(&regs[2], U256::from(g0 as u64), lane);
                sim.set_register(&regs[3], v0, lane);
                sim.set_register(&regs[4], r0, lane);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "cheap LFT frame left phase garbage in batch {batch_start}"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                let row = phase_b5_execution::block_row(
                    0,
                    phase_b5_execution::BlockSelector {
                        zeta_start: zeta0,
                        f_low: (f0 & 31) as u8,
                        g_low: (g0 & 31) as u8,
                        width: phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8,
                    },
                );
                let expect = cheap_lft_frame_pure_u256(row.branch_word, v0, r0, p);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(zeta0, zeta_bits),
                    "zeta preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], lane),
                    U256::from(f0 as u64),
                    "f_window preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(g0 as u64),
                    "g_window preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    expect.0,
                    "V shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[4], lane),
                    expect.1,
                    "R shot {case_id}"
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "cheap LFT frame scratch qubit q{idx} not clean in batch {batch_start}"
                );
            }
        }

        println!("PASS round379_b5_source_live_cheap_lft_frame_exact_9024 shots={SHOTS}");
        println!("METRIC round379_b5_source_live_cheap_lft_frame_toffoli={toffoli}");
        println!("METRIC round379_b5_source_live_cheap_lft_frame_qubits={num_qubits}");
    }

    #[test]
    fn round381_source_live_branch_only_cheap_lft_frame_block_exact_9024() {
        let p = SECP256K1_P;
        let zeta_bits = 11usize;
        let window_bits = phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);

        emit_round381_b5_source_live_branch_only_cheap_lft_frame_block(
            &mut b, &zeta, &f_window, &g_window, &v, &r, p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 5);

        const SHOTS: usize = 9024;
        const LANES: usize = 64;
        for batch_start in (0..SHOTS).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round381-b5-source-live-branch-only-cheap-lft-frame-9024-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (SHOTS - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                sim.set_register(&regs[0], encode_twos_zeta(zeta0, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(f0 as u64), lane);
                sim.set_register(&regs[2], U256::from(g0 as u64), lane);
                sim.set_register(&regs[3], v0, lane);
                sim.set_register(&regs[4], r0, lane);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "branch-only cheap LFT frame left phase garbage in batch {batch_start}"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                let row = phase_b5_execution::block_row(
                    0,
                    phase_b5_execution::BlockSelector {
                        zeta_start: zeta0,
                        f_low: (f0 & 31) as u8,
                        g_low: (g0 & 31) as u8,
                        width: phase_b5_execution::ROUND218_B5_BLOCK_BITS as u8,
                    },
                );
                let expect = cheap_lft_frame_pure_u256(row.branch_word, v0, r0, p);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(zeta0, zeta_bits),
                    "zeta preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], lane),
                    U256::from(f0 as u64),
                    "f_window preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(g0 as u64),
                    "g_window preserved shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    expect.0,
                    "V shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[4], lane),
                    expect.1,
                    "R shot {case_id}"
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "branch-only cheap LFT frame scratch qubit q{idx} not clean in batch {batch_start}"
                );
            }
        }

        println!(
            "PASS round381_b5_source_live_branch_only_cheap_lft_frame_exact_9024 shots={SHOTS}"
        );
        println!("METRIC round381_b5_source_live_branch_only_cheap_lft_frame_toffoli={toffoli}");
        println!("METRIC round381_b5_source_live_branch_only_cheap_lft_frame_qubits={num_qubits}");
    }

    #[test]
    fn round383_current_pattern_ranked_cheap_lft_source_block_exact_9024() {
        let p = SECP256K1_P;
        let zeta_bits = 11usize;
        let window_bits = phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);
        let l_rank = b.alloc_qubits(4);
        b.declare_qubit_register(&l_rank);
        let old_g0_history = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_history);

        emit_round383_b5_current_pattern_ranked_cheap_lft_source_block(
            &mut b,
            &zeta,
            &f_window,
            &g_window,
            &v,
            &r,
            &l_rank,
            &old_g0_history,
            p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 7);

        const SHOTS: usize = 9024;
        const LANES: usize = 64;
        for batch_start in (0..SHOTS).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round383-b5-current-pattern-ranked-cheap-lft-source-9024-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (SHOTS - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                sim.set_register(&regs[0], encode_twos_zeta(zeta0, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(f0 as u64), lane);
                sim.set_register(&regs[2], U256::from(g0 as u64), lane);
                sim.set_register(&regs[3], v0, lane);
                sim.set_register(&regs[4], r0, lane);
                sim.set_register(&regs[5], U256::ZERO, lane);
                sim.set_register(&regs[6], U256::ZERO, lane);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "Round383 current-pattern block left phase garbage in batch {batch_start}"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                let (end_zeta, full_f, full_g, branch_word, old_g0_word) =
                    round315_expected_rotated_window_endpoint(zeta0, f0, g0);
                let expect = cheap_lft_frame_pure_u256(branch_word, v0, r0, p);
                let expect_l = round326_live_l_rank(zeta0, old_g0_word);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(end_zeta, zeta_bits),
                    "end zeta shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], lane),
                    U256::from(full_f as u64),
                    "post f_window shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(full_g as u64),
                    "post g_window shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    expect.0,
                    "V shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[4], lane),
                    expect.1,
                    "R shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[5], lane),
                    U256::from(expect_l as u64),
                    "L rank shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[6], lane),
                    U256::from(old_g0_word as u64),
                    "old_g0 history shot {case_id}"
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "Round383 current-pattern block scratch qubit q{idx} not clean in batch {batch_start}"
                );
            }
        }

        println!(
            "PASS round383_b5_current_pattern_ranked_cheap_lft_source_exact_9024 shots={SHOTS}"
        );
        println!("METRIC round383_b5_current_pattern_ranked_cheap_lft_source_toffoli={toffoli}");
        println!("METRIC round383_b5_current_pattern_ranked_cheap_lft_source_qubits={num_qubits}");
    }

    #[test]
    fn round384_current_pattern_ranked_source_rollback_block_exact_9024() {
        let zeta_bits = 11usize;
        let window_bits = phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g_window);
        let l_rank = b.alloc_qubits(4);
        b.declare_qubit_register(&l_rank);
        let old_g0_history = b.alloc_qubits(phase_b5_execution::ROUND218_B5_BLOCK_BITS);
        b.declare_qubit_register(&old_g0_history);

        emit_round384_b5_current_pattern_ranked_source_rollback_block(
            &mut b,
            &zeta,
            &f_window,
            &g_window,
            &l_rank,
            &old_g0_history,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 5);

        const SHOTS: usize = 9024;
        const LANES: usize = 64;
        for batch_start in (0..SHOTS).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round384-b5-current-pattern-ranked-source-rollback-9024-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (SHOTS - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, _v0, _r0) =
                    deterministic_projective_scalar_case(case_id, SECP256K1_P);
                let (end_zeta, full_f, full_g, _branch_word, old_g0_word) =
                    round315_expected_rotated_window_endpoint(zeta0, f0, g0);
                let l_rank0 = round326_live_l_rank(zeta0, old_g0_word);
                sim.set_register(&regs[0], encode_twos_zeta(end_zeta, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(full_f as u64), lane);
                sim.set_register(&regs[2], U256::from(full_g as u64), lane);
                sim.set_register(&regs[3], U256::from(l_rank0 as u64), lane);
                sim.set_register(&regs[4], U256::from(old_g0_word as u64), lane);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "Round384 rollback block left phase garbage in batch {batch_start}"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, _v0, _r0) =
                    deterministic_projective_scalar_case(case_id, SECP256K1_P);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(zeta0, zeta_bits),
                    "restored zeta shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], lane),
                    U256::from(f0 as u64),
                    "restored f_window shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(g0 as u64),
                    "restored g_window shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    U256::ZERO,
                    "clean L rank shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[4], lane),
                    U256::ZERO,
                    "clean old_g0 history shot {case_id}"
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "Round384 rollback block scratch qubit q{idx} not clean in batch {batch_start}"
                );
            }
        }

        println!(
            "PASS round384_b5_current_pattern_ranked_source_rollback_exact_9024 shots={SHOTS}"
        );
        println!("METRIC round384_b5_current_pattern_ranked_source_rollback_toffoli={toffoli}");
        println!("METRIC round384_b5_current_pattern_ranked_source_rollback_qubits={num_qubits}");
    }

    #[test]
    fn round385_fused_advance_frame_rollback_block_exact_9024() {
        let p = SECP256K1_P;
        let zeta_bits = 11usize;
        let window_bits = phase_b5_state_selector::ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f_window);
        let g_window = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g_window);
        let v = b.alloc_qubits(N);
        b.declare_qubit_register(&v);
        let r = b.alloc_qubits(N);
        b.declare_qubit_register(&r);

        emit_round385_b5_fused_advance_frame_rollback_block(
            &mut b, &zeta, &f_window, &g_window, &v, &r, p,
        );

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 5);

        const SHOTS: usize = 9024;
        const LANES: usize = 64;
        for batch_start in (0..SHOTS).step_by(LANES) {
            let mut seed = Shake128::default();
            seed.update(b"round385-b5-fused-advance-frame-rollback-9024-v1");
            seed.update(&(batch_start as u64).to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
            let active = (SHOTS - batch_start).min(LANES);

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                sim.set_register(&regs[0], encode_twos_zeta(zeta0, zeta_bits), lane);
                sim.set_register(&regs[1], U256::from(f0 as u64), lane);
                sim.set_register(&regs[2], U256::from(g0 as u64), lane);
                sim.set_register(&regs[3], v0, lane);
                sim.set_register(&regs[4], r0, lane);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "Round385 fused rollback block left phase garbage in batch {batch_start}"
            );

            for lane in 0..active {
                let case_id = batch_start + lane;
                let (zeta0, f0, g0, v0, r0) = deterministic_projective_scalar_case(case_id, p);
                let (_end_zeta, _full_f, _full_g, branch_word, _old_g0_word) =
                    round315_expected_rotated_window_endpoint(zeta0, f0, g0);
                let expect = cheap_lft_frame_pure_u256(branch_word, v0, r0, p);
                assert_eq!(
                    sim.get_register(&regs[0], lane),
                    encode_twos_zeta(zeta0, zeta_bits),
                    "preserved zeta shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[1], lane),
                    U256::from(f0 as u64),
                    "preserved f_window shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[2], lane),
                    U256::from(g0 as u64),
                    "preserved g_window shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[3], lane),
                    expect.0,
                    "V shot {case_id}"
                );
                assert_eq!(
                    sim.get_register(&regs[4], lane),
                    expect.1,
                    "R shot {case_id}"
                );
            }

            for reg in &regs {
                zero_qubit_register(&mut sim, reg);
            }
            for (idx, value) in sim.qubits.iter().copied().enumerate() {
                assert_eq!(
                    value, 0,
                    "Round385 fused rollback block scratch qubit q{idx} not clean in batch {batch_start}"
                );
            }
        }

        println!("PASS round385_b5_fused_advance_frame_rollback_exact_9024 shots={SHOTS}");
        println!("METRIC round385_b5_fused_advance_frame_rollback_toffoli={toffoli}");
        println!("METRIC round385_b5_fused_advance_frame_rollback_qubits={num_qubits}");
    }

    #[test]
    fn pa_hook_refuses_forbidden_materialized_source_live_transport_pa() {
        let mut b = B::new();
        let tx = b.alloc_qubits(N);
        b.declare_qubit_register(&tx);
        let ty = b.alloc_qubits(N);
        b.declare_qubit_register(&ty);
        let ox = b.alloc_bits(N);
        b.declare_bit_register(&ox);
        let oy = b.alloc_bits(N);
        b.declare_bit_register(&oy);
        let before_ops = b.ops.len();

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            emit_round218_b5_source_live_transport_pa_or_fail(
                &mut b,
                &tx,
                &ty,
                &ox,
                &oy,
                SECP256K1_P,
            );
        }))
        .expect_err("Round218 PA hook must fail closed until parent cleanup is legal");

        let msg = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(msg.contains("refuses KMX emission"));
        assert!(msg.contains("phase-clean parent source advance/cleanup"));
        assert_eq!(
            b.ops.len(),
            before_ops,
            "forbidden PA hook emitted gates before refusing"
        );
    }
}
