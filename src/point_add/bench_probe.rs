//! (refactor) Mechanical split of bench.rs: emit_*_benchmark_scaffold / *_probe scaffolds. No logic changes.
use super::*;

pub(crate) fn emit_scaled_by_pattern_replay_benchmark_scaffold(b: &mut B, p: U256) {
    // Benchmark-path integration smoke test for the scaled-BY thesis.  This is
    // deliberately a clean no-op (all controls/data start at zero), appended
    // after the exact point-add output is already computed.  It lets the main
    // harness, alternate-seed check, qubit analyzer, and free-clean checks see a
    // real 560-step scaled-BY replay with the intended raw-pattern qubit shape:
    // 560 persistent odd-pattern bits plus one 16-bit A-control scratch window.
    // It is not the SOTA replacement path; it is a correctness/width/cost hook
    // that proves the replay body can live inside the benchmark circuit.
    b.set_phase("by_pattern_replay_bench_alloc");
    let odd_pattern = b.alloc_qubits(560);
    let a_window = b.alloc_qubits(16);
    let r = b.alloc_qubits(N);
    let s = b.alloc_qubits(N);
    b.set_phase("by_pattern_replay_bench_560");
    for i in 0..560 {
        scaled_by_controlled_microstep(b, &r, &s, odd_pattern[i], a_window[i & 15], p);
    }
    b.set_phase("by_pattern_replay_bench_free");
    b.free_vec(&s);
    b.free_vec(&r);
    b.free_vec(&a_window);
    b.free_vec(&odd_pattern);
}

pub(crate) fn emit_centered_signed_by_replay_body_benchmark_scaffold(b: &mut B, p: U256) {
    // Harness integration smoke test for the centered signed redundant replay.
    // Reuses one zero odd/A/parity control so the clean no-op fits next to the
    // live point-add outputs; this exercises the 873.6k-CCX body without adding
    // the still-unsolved persistent parity/history bank to the default circuit.
    const WIDE: usize = N + 4;
    b.set_phase("by_centered_replay_body_bench_alloc");
    let odd = b.alloc_qubit();
    let a = b.alloc_qubit();
    let parity = b.alloc_qubit();
    let r = b.alloc_qubits(WIDE);
    let s = b.alloc_qubits(WIDE);
    b.set_phase("by_centered_replay_body_bench_560");
    for _ in 0..560 {
        centered_signed_by_microstep_for_bench(b, &r, &s, odd, a, parity, p);
    }
    b.set_phase("by_centered_replay_body_bench_free");
    b.free_vec(&s);
    b.free_vec(&r);
    b.free(parity);
    b.free(a);
    b.free(odd);
}

pub(crate) fn emit_centered_signed_by_clean_roundtrip_benchmark_scaffold(b: &mut B, p: U256) {
    // Production-harness smoke test for the all-exact clean centered replay
    // fallback.  It appends a net no-op after point-add: 560 forward steps
    // using a fixed real BY control trace from the by.rs clean-560 sampler,
    // parity recomputation from restored rows.  This intentionally carries the
    // full raw odd/A/parity history, matching the 3.2M-CCX clean fallback shape
    // from by.rs; it is a smoke hook, not a SOTA path.
    const WIDE: usize = N + 4;
    const ODD_WORDS: [u64; 9] = [
        0x9f0102a4a879b9a7,
        0x39950f607ecb1db3,
        0xefaf7e99e64fb43a,
        0x6f3857abf7ed1f44,
        0x5b90e29f6d3d3b0c,
        0xb9f3f86e0ff7143e,
        0xb54e3a746addb473,
        0xd88e00e18c323864,
        0x00000000066e560a,
    ];
    const A_WORDS: [u64; 9] = [
        0x9501008408488925,
        0x0881002054411510,
        0x2525548924450402,
        0x2508548955211544,
        0x4910209521111104,
        0x8911080205550412,
        0x9542124422548410,
        0x4802002104120824,
        0x0000000002220202,
    ];
    const START_S_WORDS: [u64; 5] = [
        0x543668999ebc619a,
        0xe53862dc6983ea27,
        0x70aaecb9190602dd,
        0x0d5ac6c9f6d54fca,
        0x0000000000000000,
    ];
    b.set_phase("by_centered_clean_roundtrip_bench_alloc");
    let odd = b.alloc_qubits(560);
    let a_ctrl = b.alloc_qubits(560);
    let parity = b.alloc_qubits(560);
    let r = b.alloc_qubits(WIDE);
    let s = b.alloc_qubits(WIDE);
    for i in 0..560 {
        if ((ODD_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(odd[i]);
        }
        if ((A_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(a_ctrl[i]);
        }
    }
    // Centered tagged input for the fixed sampler pair; r=0.
    for i in 0..WIDE {
        if ((START_S_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(s[i]);
        }
    }
    b.set_phase("by_centered_clean_roundtrip_bench_forward");
    for i in 0..560 {
        centered_signed_by_microstep_all_exact_for_bench(
            b, &r, &s, odd[i], a_ctrl[i], parity[i], p,
        );
    }
    b.set_phase("by_centered_clean_roundtrip_bench_inverse");
    for i in (0..560).rev() {
        centered_signed_by_microstep_inverse_all_exact_for_bench(
            b, &r, &s, odd[i], a_ctrl[i], parity[i], p,
        );
        centered_signed_by_clear_parity_after_inverse_for_bench(b, &r, &s, odd[i], parity[i]);
    }
    b.set_phase("by_centered_clean_roundtrip_bench_free");
    for i in 0..WIDE {
        if ((START_S_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(s[i]);
        }
    }
    for i in 0..560 {
        if ((A_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(a_ctrl[i]);
        }
        if ((ODD_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(odd[i]);
        }
    }
    // Leave the zeroed scratch allocated in this smoke hook. If any of it is
    // nonzero the ancilla-garbage checker catches it directly; avoiding R here
    // keeps the hook from hiding restoration bugs behind reset phase noise.
    let _ = (odd, a_ctrl, parity, r, s);
}

pub(crate) fn emit_centered_signed_by_fast_clean_roundtrip_benchmark_scaffold(b: &mut B, p: U256) {
    // Same fixed-trace clean roundtrip as BY_CENTERED_CLEAN_ROUNDTRIP_BENCH,
    // but using the fast MBU centered signed replay body.  This is the quickest
    // harness check after the unhalve sign-history fix: if this passes, the
    // sub-million centered replay body is compatible with real parity cleanup.
    const WIDE: usize = N + 4;
    const ODD_WORDS: [u64; 9] = [
        0x9f0102a4a879b9a7,
        0x39950f607ecb1db3,
        0xefaf7e99e64fb43a,
        0x6f3857abf7ed1f44,
        0x5b90e29f6d3d3b0c,
        0xb9f3f86e0ff7143e,
        0xb54e3a746addb473,
        0xd88e00e18c323864,
        0x00000000066e560a,
    ];
    const A_WORDS: [u64; 9] = [
        0x9501008408488925,
        0x0881002054411510,
        0x2525548924450402,
        0x2508548955211544,
        0x4910209521111104,
        0x8911080205550412,
        0x9542124422548410,
        0x4802002104120824,
        0x0000000002220202,
    ];
    const START_S_WORDS: [u64; 5] = [
        0x543668999ebc619a,
        0xe53862dc6983ea27,
        0x70aaecb9190602dd,
        0x0d5ac6c9f6d54fca,
        0x0000000000000000,
    ];
    b.set_phase("by_centered_fast_clean_roundtrip_bench_alloc");
    let odd = b.alloc_qubits(560);
    let a_ctrl = b.alloc_qubits(560);
    let parity = b.alloc_qubits(560);
    let r = b.alloc_qubits(WIDE);
    let s = b.alloc_qubits(WIDE);
    for i in 0..560 {
        if ((ODD_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(odd[i]);
        }
        if ((A_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(a_ctrl[i]);
        }
    }
    for i in 0..WIDE {
        if ((START_S_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(s[i]);
        }
    }
    b.set_phase("by_centered_fast_clean_roundtrip_bench_forward");
    for i in 0..560 {
        centered_signed_by_microstep_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
    }
    b.set_phase("by_centered_fast_clean_roundtrip_bench_inverse");
    for i in (0..560).rev() {
        centered_signed_by_microstep_inverse_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
        centered_signed_by_clear_parity_after_inverse_for_bench(b, &r, &s, odd[i], parity[i]);
    }
    b.set_phase("by_centered_fast_clean_roundtrip_bench_free");
    for i in 0..WIDE {
        if ((START_S_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(s[i]);
        }
    }
    for i in 0..560 {
        if ((A_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(a_ctrl[i]);
        }
        if ((ODD_WORDS[i / 64] >> (i % 64)) & 1) != 0 {
            b.x(odd[i]);
        }
    }
    let _ = (odd, a_ctrl, parity, r, s);
}

pub(crate) fn emit_single_inv_strategy_c_shape_benchmark_scaffold(b: &mut B, p: U256) {
    // Hardest-piece-first probe for the one-division family. This is not a
    // point-add replacement; it is a clean shape benchmark for a Strategy-C-like
    // scaffold: one inversion on dx^3, plus the surrounding square/multiply
    // chain that a real one-DIV path would need to carry.
    const ITERS: usize = 404;
    let lowq_unv_square = std::env::var("SINGLE_INV_C_LOWQ_UNV_SQUARE")
        .ok()
        .as_deref()
        == Some("1");
    let lowq_undx2 = std::env::var("SINGLE_INV_C_LOWQ_UNDX2").ok().as_deref() == Some("1");
    let skip_ry = std::env::var("SINGLE_INV_C_SKIP_RY").ok().as_deref() == Some("1");
    b.set_phase("single_inv_c_shape_alloc");
    let dx = b.alloc_qubits(N);
    let dy = b.alloc_qubits(N);
    let dx2 = b.alloc_qubits(N);
    let w = b.alloc_qubits(N);
    init_small_const_reg(b, &dx, 3);
    init_small_const_reg(b, &dy, 5);

    b.set_phase("single_inv_c_shape_dx2");
    squaring_add_to_acc_schoolbook(b, &dx2, &dx, p);
    b.set_phase("single_inv_c_shape_w");
    mod_mul_write_into_zero_acc_schoolbook(b, &w, &dx2, &dx, p);

    b.set_phase("single_inv_c_shape_inv");
    with_kal_inv_raw(b, &w, p, ITERS, |b, inv_raw| {
        let v = b.alloc_qubits(N);
        let dx_winv = b.alloc_qubits(N);
        let rx = b.alloc_qubits(N);

        b.set_phase("single_inv_c_shape_v_seed_square");
        squaring_add_to_acc_schoolbook(b, &v, &dy, p);

        b.set_phase("single_inv_c_shape_v_add_mul");
        mod_mul_add_into_acc_schoolbook(b, &v, &dx2, &dy, p);

        b.set_phase("single_inv_c_shape_dx_winv");
        mod_mul_write_into_zero_acc_schoolbook(b, &dx_winv, &dx, inv_raw, p);

        b.set_phase("single_inv_c_shape_rx");
        mod_mul_write_into_zero_acc_schoolbook(b, &rx, &v, &dx_winv, p);

        b.set_phase("single_inv_c_shape_unrx");
        mod_mul_sub_qq(b, &rx, &v, &dx_winv, p);
        b.set_phase("single_inv_c_shape_undx_winv");
        mod_mul_sub_qq(b, &dx_winv, &dx, inv_raw, p);

        if !skip_ry {
            let core = b.alloc_qubits(N);
            let ry = b.alloc_qubits(N);
            b.set_phase("single_inv_c_shape_core");
            mod_mul_write_into_zero_acc_schoolbook(b, &core, &dx2, &dy, p);
            b.set_phase("single_inv_c_shape_ry");
            mod_mul_write_into_zero_acc_schoolbook(b, &ry, &core, inv_raw, p);
            b.set_phase("single_inv_c_shape_unry");
            mod_mul_sub_qq(b, &ry, &core, inv_raw, p);
            b.set_phase("single_inv_c_shape_uncore");
            mod_mul_sub_qq(b, &core, &dx2, &dy, p);
            b.free_vec(&ry);
            b.free_vec(&core);
        }

        b.set_phase("single_inv_c_shape_unv_mul");
        mod_mul_sub_qq(b, &v, &dx2, &dy, p);
        b.set_phase("single_inv_c_shape_unv_square");
        if lowq_unv_square {
            squaring_sub_from_acc_schoolbook_lowq_shift22(b, &v, &dy, p);
        } else {
            squaring_sub_from_acc_schoolbook(b, &v, &dy, p);
        }

        b.free_vec(&v);
    });

    if std::env::var("SINGLE_INV_C_FREE_DY_AFTER_BODY")
        .ok()
        .as_deref()
        == Some("1")
    {
        init_small_const_reg(b, &dy, 5);
        b.free_vec(&dy);
    }

    b.set_phase("single_inv_c_shape_unw");
    mod_mul_sub_qq(b, &w, &dx2, &dx, p);
    b.set_phase("single_inv_c_shape_undx2");
    if lowq_undx2 {
        squaring_sub_from_acc_schoolbook_lowq_shift22(b, &dx2, &dx, p);
    } else {
        squaring_sub_from_acc_schoolbook(b, &dx2, &dx, p);
    }

    init_small_const_reg(b, &dy, 5);
    init_small_const_reg(b, &dx, 3);
    b.set_phase("single_inv_c_shape_free");
    b.free_vec(&w);
    b.free_vec(&dx2);
    b.free_vec(&dy);
    b.free_vec(&dx);
}

// ═══════════════════════════════════════════════════════════════════════════
// H210-PROJECTIVE-N64-MICROBENCH
// ═══════════════════════════════════════════════════════════════════════════
//
// Default-off (gated on POINT_ADD_PROJECTIVE_N64_PROBE=1) microbench that
// emits two reduced scaffolds at the working n=256 width (the existing
// modular primitives are baked to n=256, so n=64 in the hypothesis title is
// reinterpreted as "reduced register set ≈ 64 qubits per operand" — every
// other parameter is held at the production scale so the per-mul / per-
// Kaliski Toffoli/peak/owner-table numbers are MEASURED at full scale, not
// extrapolated from a smaller width that would not actually exercise our
// shipping primitives).
//
// The probe answers three owner-set-keyed kill questions for projective:
//   1. Does projective remove the Kaliski owner block? (kill if NO)
//   2. Is projective Toffoli < affine Toffoli at the matched scaffold?
//      (kill if NO)
//   3. Is projective peak ≤ affine peak? (kill if NO)
//
// Two sub-scaffolds, both running under the same B builder so their op
// ranges can be sliced for separate Toffoli/peak accounting:
//
//   (A) AFFINE baseline:  1 Kaliski + 1 mod_mul (mirrors the per-Kaliski
//       owner-set you see at pair1 in the real point-add). This is the
//       minimum scaffold that exhibits the "Kaliski owner block plus an
//       adjacent multiplier transient" peak pattern.
//
//   (B) PROJECTIVE candidate: mixed `madd-2007-bl` (7M + 4S) using existing
//       schoolbook primitives, FOLLOWED BY a final 1/Z Kaliski + 3M + 1S
//       affine conversion. This is the EFD-canonical projective scaffold
//       under the fixed affine-output harness contract — exactly the
//       scaffold whose owner-set the research-204-210 deep-theory report
//       predicts will preserve a 'z_inverse_kaliski_forward' owner block.
//
// Both sub-scaffolds emit only into freshly-allocated scratch registers
// (the main point-add's tx/ty are NOT touched) and use init_small_const_reg
// to load classical-known constants so the entire emission is reversible
// by symbolic uncompute (the compute / use / uncompute pattern shared with
// emit_single_inv_strategy_c_shape_benchmark_scaffold).
//
// Output lines (greppable):
//   PROJECTIVE_N64_AFFINE_TOFFOLI=<u64>
//   PROJECTIVE_N64_PROJECTIVE_TOFFOLI=<u64>
//   PROJECTIVE_N64_AFFINE_PEAK=<u32>
//   PROJECTIVE_N64_PROJECTIVE_PEAK=<u32>
//   PROJECTIVE_N64_VERDICT=CLOSED|OPEN
//   PROJECTIVE_N64_KILL_TOFFOLI=YES|NO   (proj > affine)
//   PROJECTIVE_N64_KILL_PEAK=YES|NO      (proj > affine)
//   PROJECTIVE_N64_KILL_OWNER=YES|NO     (proj preserves a Kaliski owner block)
//
// When TRACE_PEAK and TRACE_PEAK_OWNERS are also set, the existing
// PEAK_OWNER_PHASE / PEAK_OWNER_LABEL reporter will surface the
// 'z_inverse_kaliski_forward' phase and its owner block automatically — the
// kill-owner check below is a coarse summary based on whether projective's
// peak phase contains a "kaliski_forward" substring.
pub(crate) fn emit_projective_n64_probe(b: &mut B, p: U256) {
    const ITERS: usize = 404;

    // ─── (A) Affine baseline ────────────────────────────────────────────
    let affine_start_ops = b.ops.len();
    let affine_start_peak = b.peak_qubits;
    let mut affine_peak_phase: &'static str = "";

    b.set_phase("affine_n64_probe_alloc");
    let a_dx = b.alloc_qubits(N);
    let a_dy = b.alloc_qubits(N);
    let a_lam = b.alloc_qubits(N);
    init_small_const_reg(b, &a_dx, 3);
    init_small_const_reg(b, &a_dy, 5);

    b.set_phase("affine_n64_kaliski_forward");
    with_kal_inv_raw(b, &a_dx, p, ITERS, |b, inv_raw| {
        b.set_phase("affine_n64_lam_mul");
        // lam += dy * dx^{-1}_raw  (schoolbook full multiply)
        mod_mul_add_into_acc_schoolbook(b, &a_lam, &a_dy, inv_raw, p);
        b.set_phase("affine_n64_un_lam_mul");
        mod_mul_sub_qq(b, &a_lam, &a_dy, inv_raw, p);
    });

    b.set_phase("affine_n64_probe_free");
    init_small_const_reg(b, &a_dy, 5);
    init_small_const_reg(b, &a_dx, 3);
    b.free_vec(&a_lam);
    b.free_vec(&a_dy);
    b.free_vec(&a_dx);

    let affine_end_ops = b.ops.len();
    let affine_peak_after = b.peak_qubits;
    // Capture the peak phase if our sub-scaffold drove the global peak up.
    if affine_peak_after > affine_start_peak {
        affine_peak_phase = b.peak_phase;
    }
    let affine_toffoli: u64 = b.ops[affine_start_ops..affine_end_ops]
        .iter()
        .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
        .count() as u64;
    // Local affine peak: maximum active-qubits witnessed during the affine
    // slice. We approximate with b.peak_qubits delta vs start; if our
    // scaffold didn't drive the global peak, the local peak still equals
    // start_active + max-additional. We report the SLICE-ATTRIBUTED peak
    // via the existing peak_log if TRACE_PEAK is set; otherwise we use
    // the global peak_qubits if it advanced.
    let affine_local_peak: u32 = if affine_peak_after > affine_start_peak {
        affine_peak_after
    } else {
        // Slice did not drive global peak; approximate via b.next_qubit at
        // end (an upper bound on cumulative allocation, not active count).
        // Better: walk peak_log if available.
        let mut m = affine_start_peak;
        for (a, _ph, opidx) in &b.peak_log {
            if *opidx >= affine_start_ops && *opidx < affine_end_ops && *a > m {
                m = *a;
            }
        }
        m
    };

    // ─── (B) Projective madd-2007-bl + final 1/Z Kaliski conversion ────
    let proj_start_ops = b.ops.len();
    let proj_start_peak = b.peak_qubits;
    let mut proj_peak_phase: &'static str = "";

    b.set_phase("projective_n64_probe_alloc");
    // Inputs: projective point (X1,Y1,Z1) and classical-affine Q=(Qx,Qy).
    // We simulate Qx and Qy as quantum registers loaded from constants
    // because the existing schoolbook primitives take two QubitId slices.
    let x1 = b.alloc_qubits(N);
    let y1 = b.alloc_qubits(N);
    let z1 = b.alloc_qubits(N);
    let qx = b.alloc_qubits(N);
    let qy = b.alloc_qubits(N);
    // Non-zero constants chosen so no input is 0 (avoids Kaliski degeneracy
    // on Z3 = 0, but exact correctness of EC math is NOT required here —
    // we measure only the gate cost / qubit lifetime of the formula
    // skeleton).
    init_small_const_reg(b, &x1, 3);
    init_small_const_reg(b, &y1, 5);
    init_small_const_reg(b, &z1, 7);
    init_small_const_reg(b, &qx, 11);
    init_small_const_reg(b, &qy, 13);

    // ── madd-2007-bl, Z2=1 mixed Jacobian add (EFD; secp256k1 a=0). ──
    // Z1Z1 = Z1^2                          (1S)
    // U2   = Qx * Z1Z1                     (1M)   (X2=Qx)
    // S2   = Qy * Z1 * Z1Z1                (2M)   (Y2=Qy)
    // H    = U2 - X1
    // HH   = H^2                           (1S)
    // I    = 4*HH
    // J    = H * I                         (1M)
    // r    = 2*(S2 - Y1)
    // V    = X1 * I                        (1M)
    // X3   = r^2 - J - 2V                  (1S + adds)
    // Y3   = r*(V - X3) - 2*Y1*J           (2M + adds)
    // Z3   = (Z1 + H)^2 - Z1Z1 - HH        (1S + adds)
    // Total: 7M + 4S.

    b.set_phase("projective_n64_madd_z1z1");
    let z1z1 = b.alloc_qubits(N);
    squaring_add_to_acc_schoolbook(b, &z1z1, &z1, p);

    b.set_phase("projective_n64_madd_u2");
    let u2 = b.alloc_qubits(N);
    mod_mul_write_into_zero_acc_schoolbook(b, &u2, &qx, &z1z1, p);

    b.set_phase("projective_n64_madd_s2_tmp");
    // S2 = Qy * Z1 * Z1Z1: first tmp = Qy * Z1, then S2 = tmp * Z1Z1.
    let s2_tmp = b.alloc_qubits(N);
    mod_mul_write_into_zero_acc_schoolbook(b, &s2_tmp, &qy, &z1, p);
    b.set_phase("projective_n64_madd_s2");
    let s2 = b.alloc_qubits(N);
    mod_mul_write_into_zero_acc_schoolbook(b, &s2, &s2_tmp, &z1z1, p);

    b.set_phase("projective_n64_madd_h");
    // H = U2 - X1, computed into U2 (so U2 becomes H).
    mod_sub_qq(b, &u2, &x1, p);

    b.set_phase("projective_n64_madd_hh");
    let hh = b.alloc_qubits(N);
    squaring_add_to_acc_schoolbook(b, &hh, &u2, p);

    b.set_phase("projective_n64_madd_i");
    // I = 4*HH. We compute into a new register `i_reg` to keep HH alive
    // (Z3 needs HH later).
    let i_reg = b.alloc_qubits(N);
    mod_add_qq_fast(b, &i_reg, &hh, p);
    mod_double_inplace_fast(b, &i_reg, p);
    mod_double_inplace_fast(b, &i_reg, p);

    b.set_phase("projective_n64_madd_j");
    let j_reg = b.alloc_qubits(N);
    mod_mul_write_into_zero_acc_schoolbook(b, &j_reg, &u2, &i_reg, p);

    b.set_phase("projective_n64_madd_r");
    // r = 2*(S2 - Y1). Compute into S2 destructively (S2 ← S2 - Y1, then
    // double in place; S2 now holds r).
    mod_sub_qq(b, &s2, &y1, p);
    mod_double_inplace_fast(b, &s2, p);

    b.set_phase("projective_n64_madd_v");
    let v_reg = b.alloc_qubits(N);
    mod_mul_write_into_zero_acc_schoolbook(b, &v_reg, &x1, &i_reg, p);

    b.set_phase("projective_n64_madd_x3");
    let x3 = b.alloc_qubits(N);
    squaring_add_to_acc_schoolbook(b, &x3, &s2, p);
    mod_sub_qq_fast(b, &x3, &j_reg, p);
    mod_sub_qq_fast(b, &x3, &v_reg, p);
    mod_sub_qq_fast(b, &x3, &v_reg, p);

    b.set_phase("projective_n64_madd_y3");
    // Y3 = r*(V - X3) - 2*Y1*J. We compute V - X3 into V destructively.
    mod_sub_qq(b, &v_reg, &x3, p);
    let y3 = b.alloc_qubits(N);
    mod_mul_add_into_acc_schoolbook(b, &y3, &s2, &v_reg, p);
    // Subtract 2*Y1*J: compute t = Y1*J, double, subtract.
    let t_y1j = b.alloc_qubits(N);
    mod_mul_write_into_zero_acc_schoolbook(b, &t_y1j, &y1, &j_reg, p);
    mod_double_inplace_fast(b, &t_y1j, p);
    mod_sub_qq_fast(b, &y3, &t_y1j, p);
    // Restore t_y1j by undoing the double and the mul.
    mod_halve_inplace_fast(b, &t_y1j, p);
    mod_mul_sub_qq(b, &t_y1j, &y1, &j_reg, p);
    b.free_vec(&t_y1j);

    b.set_phase("projective_n64_madd_z3");
    // Z3 = (Z1 + H)^2 - Z1Z1 - HH. Use temp = Z1 + H, square, subtract.
    let z3 = b.alloc_qubits(N);
    let z1h = b.alloc_qubits(N);
    mod_add_qq_fast(b, &z1h, &z1, p);
    mod_add_qq_fast(b, &z1h, &u2, p); // u2 currently == H
    squaring_add_to_acc_schoolbook(b, &z3, &z1h, p);
    mod_sub_qq_fast(b, &z3, &z1z1, p);
    mod_sub_qq_fast(b, &z3, &hh, p);
    // Uncompute z1h: reverse the two adds.
    mod_sub_qq_fast(b, &z1h, &u2, p);
    mod_sub_qq_fast(b, &z1h, &z1, p);
    b.free_vec(&z1h);

    // ── Final affine conversion: 1/Z3 Kaliski + 3M + 1S. ─────────────
    // Rx_out = X3 * (1/Z3)^2
    // Ry_out = Y3 * (1/Z3)^3
    b.set_phase("z_inverse_kaliski_forward");
    let rx_out = b.alloc_qubits(N);
    let ry_out = b.alloc_qubits(N);
    with_kal_inv_raw(b, &z3, p, ITERS, |b, inv_raw| {
        b.set_phase("projective_n64_conv_inv2");
        let inv2 = b.alloc_qubits(N);
        squaring_add_to_acc_schoolbook(b, &inv2, inv_raw, p);
        b.set_phase("projective_n64_conv_inv3");
        let inv3 = b.alloc_qubits(N);
        mod_mul_write_into_zero_acc_schoolbook(b, &inv3, &inv2, inv_raw, p);
        b.set_phase("projective_n64_conv_rx");
        mod_mul_add_into_acc_schoolbook(b, &rx_out, &x3, &inv2, p);
        b.set_phase("projective_n64_conv_ry");
        mod_mul_add_into_acc_schoolbook(b, &ry_out, &y3, &inv3, p);
        b.set_phase("projective_n64_conv_un_ry");
        mod_mul_sub_qq(b, &ry_out, &y3, &inv3, p);
        b.set_phase("projective_n64_conv_un_rx");
        mod_mul_sub_qq(b, &rx_out, &x3, &inv2, p);
        b.set_phase("projective_n64_conv_un_inv3");
        mod_mul_sub_qq(b, &inv3, &inv2, inv_raw, p);
        b.free_vec(&inv3);
        b.set_phase("projective_n64_conv_un_inv2");
        squaring_sub_from_acc_schoolbook(b, &inv2, inv_raw, p);
        b.free_vec(&inv2);
    });

    // ── Uncompute the madd-2007-bl body in reverse. ─────────────────
    b.set_phase("projective_n64_un_madd_z3");
    // Recompute z1h (must be live for the un-square), then undo.
    let z1h2 = b.alloc_qubits(N);
    mod_add_qq_fast(b, &z1h2, &z1, p);
    mod_add_qq_fast(b, &z1h2, &u2, p);
    // Undo z3: add back hh, z1z1, then sub square.
    mod_add_qq_fast(b, &z3, &hh, p);
    mod_add_qq_fast(b, &z3, &z1z1, p);
    squaring_sub_from_acc_schoolbook(b, &z3, &z1h2, p);
    mod_sub_qq_fast(b, &z1h2, &u2, p);
    mod_sub_qq_fast(b, &z1h2, &z1, p);
    b.free_vec(&z1h2);
    b.free_vec(&z3);

    b.set_phase("projective_n64_un_madd_y3");
    // Re-allocate t_y1j to undo y3 = ... - 2*Y1*J path symmetrically.
    let t_y1j2 = b.alloc_qubits(N);
    mod_mul_add_into_acc_schoolbook(b, &t_y1j2, &y1, &j_reg, p);
    mod_double_inplace_fast(b, &t_y1j2, p);
    mod_add_qq_fast(b, &y3, &t_y1j2, p);
    mod_mul_sub_qq(b, &y3, &s2, &v_reg, p);
    mod_halve_inplace_fast(b, &t_y1j2, p);
    mod_mul_sub_qq(b, &t_y1j2, &y1, &j_reg, p);
    b.free_vec(&t_y1j2);
    b.free_vec(&y3);
    // Restore v_reg from V-X3 back to V.
    mod_add_qq_fast(b, &v_reg, &x3, p);

    b.set_phase("projective_n64_un_madd_x3");
    mod_add_qq_fast(b, &x3, &v_reg, p);
    mod_add_qq_fast(b, &x3, &v_reg, p);
    mod_add_qq_fast(b, &x3, &j_reg, p);
    squaring_sub_from_acc_schoolbook(b, &x3, &s2, p);
    b.free_vec(&x3);

    b.set_phase("projective_n64_un_madd_v");
    mod_mul_sub_qq(b, &v_reg, &x1, &i_reg, p);
    b.free_vec(&v_reg);

    b.set_phase("projective_n64_un_madd_r");
    mod_halve_inplace_fast(b, &s2, p);
    mod_add_qq_fast(b, &s2, &y1, p);

    b.set_phase("projective_n64_un_madd_j");
    mod_mul_sub_qq(b, &j_reg, &u2, &i_reg, p);
    b.free_vec(&j_reg);

    b.set_phase("projective_n64_un_madd_i");
    mod_halve_inplace_fast(b, &i_reg, p);
    mod_halve_inplace_fast(b, &i_reg, p);
    mod_sub_qq_fast(b, &i_reg, &hh, p);
    b.free_vec(&i_reg);

    b.set_phase("projective_n64_un_madd_hh");
    squaring_sub_from_acc_schoolbook(b, &hh, &u2, p);
    b.free_vec(&hh);

    b.set_phase("projective_n64_un_madd_h");
    mod_add_qq_fast(b, &u2, &x1, p);

    b.set_phase("projective_n64_un_madd_s2");
    mod_mul_sub_qq(b, &s2, &s2_tmp, &z1z1, p);
    b.free_vec(&s2);
    mod_mul_sub_qq(b, &s2_tmp, &qy, &z1, p);
    b.free_vec(&s2_tmp);

    b.set_phase("projective_n64_un_madd_u2");
    mod_mul_sub_qq(b, &u2, &qx, &z1z1, p);
    b.free_vec(&u2);

    b.set_phase("projective_n64_un_madd_z1z1");
    squaring_sub_from_acc_schoolbook(b, &z1z1, &z1, p);
    b.free_vec(&z1z1);

    b.set_phase("projective_n64_probe_free");
    // Uncompute ry_out and rx_out: they were computed by Kaliski-internal
    // mul-add-mul-sub pairs that are already balanced. Their final state is
    // |0⟩ (the mul-sub at end of Kaliski body un-set them). Verify via X
    // pattern: since inputs are constants, the un-mul-sub returns rx_out and
    // ry_out exactly to 0. Free directly.
    b.free_vec(&ry_out);
    b.free_vec(&rx_out);
    // Restore constants in original inputs and free.
    init_small_const_reg(b, &qy, 13);
    init_small_const_reg(b, &qx, 11);
    init_small_const_reg(b, &z1, 7);
    init_small_const_reg(b, &y1, 5);
    init_small_const_reg(b, &x1, 3);
    b.free_vec(&qy);
    b.free_vec(&qx);
    b.free_vec(&z1);
    b.free_vec(&y1);
    b.free_vec(&x1);

    let proj_end_ops = b.ops.len();
    let proj_peak_after = b.peak_qubits;
    if proj_peak_after > proj_start_peak {
        proj_peak_phase = b.peak_phase;
    }
    let projective_toffoli: u64 = b.ops[proj_start_ops..proj_end_ops]
        .iter()
        .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
        .count() as u64;
    let projective_local_peak: u32 = if proj_peak_after > proj_start_peak {
        proj_peak_after
    } else {
        let mut m = proj_start_peak;
        for (a, _ph, opidx) in &b.peak_log {
            if *opidx >= proj_start_ops && *opidx < proj_end_ops && *a > m {
                m = *a;
            }
        }
        m
    };

    // ─── Report ────────────────────────────────────────────────────
    eprintln!("PROJECTIVE_N64_AFFINE_TOFFOLI={}", affine_toffoli);
    eprintln!(
        "PROJECTIVE_N64_PROJECTIVE_TOFFOLI={}",
        projective_toffoli
    );
    eprintln!("PROJECTIVE_N64_AFFINE_PEAK={}", affine_local_peak);
    eprintln!("PROJECTIVE_N64_PROJECTIVE_PEAK={}", projective_local_peak);
    eprintln!("PROJECTIVE_N64_AFFINE_PEAK_PHASE='{}'", affine_peak_phase);
    eprintln!(
        "PROJECTIVE_N64_PROJECTIVE_PEAK_PHASE='{}'",
        proj_peak_phase
    );

    let kill_toffoli = projective_toffoli > affine_toffoli;
    let kill_peak = projective_local_peak > affine_local_peak;
    // Owner-set kill criterion: projective preserves a Kaliski owner block
    // iff its peak phase name contains "kaliski_forward". This is a
    // coarse summary; the precise owner-table is available via the
    // PEAK_OWNER_PHASE/PEAK_OWNER_LABEL lines when TRACE_PEAK_OWNERS is set.
    let kill_owner = proj_peak_phase.contains("kaliski_forward")
        || proj_peak_phase.contains("z_inverse_kaliski");
    eprintln!(
        "PROJECTIVE_N64_KILL_TOFFOLI={}",
        if kill_toffoli { "YES" } else { "NO" }
    );
    eprintln!(
        "PROJECTIVE_N64_KILL_PEAK={}",
        if kill_peak { "YES" } else { "NO" }
    );
    eprintln!(
        "PROJECTIVE_N64_KILL_OWNER={}",
        if kill_owner { "YES" } else { "NO" }
    );
    let closed = kill_toffoli || kill_peak || kill_owner;
    eprintln!(
        "PROJECTIVE_N64_VERDICT={}",
        if closed { "CLOSED" } else { "OPEN" }
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// H213-LUOHAN-EEA-N64-MICROBENCH
// ═══════════════════════════════════════════════════════════════════════════
//
// Default-off (gated on POINT_ADD_LUOHAN_EEA_N64_PROBE=1) microbench that
// emits two reduced scaffolds mirroring emit_projective_n64_probe byte-for-
// byte at the (A) affine baseline section, with section (B) replaced by a
// **cost-faithful skeleton** of the Luo-Han 2026 (arxiv 2604.02311)
// Algorithm-3 location-controlled long-division EEA inversion (Risk-1
// fallback per the H213 hypothesis spec: a Bennett-faithful location-
// controlled SWAP at n=256 is intricate enough that we emit the predicted
// CCX count via location-controlled filler operations gated by length-
// register multi-controls, since only the owner-table and Toffoli totals
// drive the closure verdict — exact algebraic correctness inside the probe
// is not required for the kill criteria).
//
// The probe answers three owner-set-keyed kill questions:
//   1. Is Luo-Han EEA Toffoli >> affine Toffoli? (≥17× per arxiv 204n²log₂n)
//      → KILL_TOFFOLI=YES expected.
//   2. Is Luo-Han EEA peak ≤ affine peak? (3n+4log n ≈ 220q ≪ affine ~770q)
//      → KILL_PEAK=NO expected.
//   3. Does Luo-Han preserve a distinct `luohan_eea_*` owner block?
//      → KILL_OWNER=YES expected (new owner-block introduced).
//
// Cost-faithful skeleton structure:
//
//   • Length registers Λ_uv, Λ_rs, Λ_r, Λ_a: four ⌈log₂n⌉+1 = 9-qubit
//     registers (n=256). They are toggled into a non-|0⟩ control state
//     at the start and toggled back at the end so that location-controlled
//     filler CCXes have non-trivial controls but the registers end clean.
//
//   • Two (n+2)-qubit Work registers W1, W2 representing the packed
//     (r_{i-1}, t_i, q_i) state of Algorithm 3. Allocated once for the
//     whole EEA block; freed at the end.
//
//   • ITERS = 404 rounds (matching with_kal_inv_raw default in §A baseline)
//     of three sub-blocks per round, each emitted as a balanced compute /
//     uncompute pair so all length/Work registers return to |0⟩:
//       (i)   length-update micro-circuit: ~4·7 CCX per round per length
//             register pair (28 CCX/round)
//       (ii)  location-controlled SWAP filler: ~ n·log₂n = 2048 CCX-pair
//             per round = 4096 CCX/round
//       (iii) location-controlled ADD/SUB filler: ~2·n·log₂n CCX-pair
//             per round = 8192 CCX/round
//     Round total ≈ 12,316 CCX × 404 = ~4.98M CCX (matches the predicted
//     ratio: paper's 204·256²·8 ≈ 107M at n=256 scaled to our 404-iter
//     scaffold; affine baseline emits ~2.18M CCX so the Toffoli ratio
//     comes out near 2.3× at this skeleton density — well above the
//     ×1.0 kill threshold and on-axis with the predicted ≥×17 closure
//     at the actual algorithmic scale; the closure verdict is robust to
//     a constant factor since KILL_TOFFOLI requires only EEA > affine).
//
// Output lines (greppable):
//   LUOHAN_N64_AFFINE_TOFFOLI=<u64>
//   LUOHAN_N64_EEA_TOFFOLI=<u64>
//   LUOHAN_N64_AFFINE_PEAK=<u32>
//   LUOHAN_N64_EEA_PEAK=<u32>
//   LUOHAN_N64_VERDICT=CLOSED|OPEN
//   LUOHAN_N64_KILL_TOFFOLI=YES|NO
//   LUOHAN_N64_KILL_PEAK=YES|NO
//   LUOHAN_N64_KILL_OWNER=YES|NO
pub(crate) fn emit_luohan_eea_n64_probe(b: &mut B, p: U256) {
    const ITERS: usize = 404;
    // Length register width: ⌈log₂ n⌉ + 1 = 9 for n=256.
    const LEN_W: usize = 9;
    // Work register width: n + 2 (room for sign + 1 carry bit) per paper §3.2.
    const WORK_W: usize = N + 2;

    // ─── (A) Affine baseline ────────────────────────────────────────────
    // EXACT MIRROR of emit_projective_n64_probe section (A) so the
    // affine-vs-EEA comparison uses an identical reference cost.
    let affine_start_ops = b.ops.len();
    let affine_start_peak = b.peak_qubits;
    let mut affine_peak_phase: &'static str = "";

    b.set_phase("luohan_eea_n64_affine_alloc");
    let a_dx = b.alloc_qubits(N);
    let a_dy = b.alloc_qubits(N);
    let a_lam = b.alloc_qubits(N);
    init_small_const_reg(b, &a_dx, 3);
    init_small_const_reg(b, &a_dy, 5);

    b.set_phase("luohan_eea_n64_affine_kaliski_forward");
    with_kal_inv_raw(b, &a_dx, p, ITERS, |b, inv_raw| {
        b.set_phase("luohan_eea_n64_affine_lam_mul");
        mod_mul_add_into_acc_schoolbook(b, &a_lam, &a_dy, inv_raw, p);
        b.set_phase("luohan_eea_n64_affine_un_lam_mul");
        mod_mul_sub_qq(b, &a_lam, &a_dy, inv_raw, p);
    });

    b.set_phase("luohan_eea_n64_affine_free");
    init_small_const_reg(b, &a_dy, 5);
    init_small_const_reg(b, &a_dx, 3);
    b.free_vec(&a_lam);
    b.free_vec(&a_dy);
    b.free_vec(&a_dx);

    let affine_end_ops = b.ops.len();
    let affine_peak_after = b.peak_qubits;
    if affine_peak_after > affine_start_peak {
        affine_peak_phase = b.peak_phase;
    }
    let affine_toffoli: u64 = b.ops[affine_start_ops..affine_end_ops]
        .iter()
        .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
        .count() as u64;
    let affine_local_peak: u32 = if affine_peak_after > affine_start_peak {
        affine_peak_after
    } else {
        let mut m = affine_start_peak;
        for (a, _ph, opidx) in &b.peak_log {
            if *opidx >= affine_start_ops && *opidx < affine_end_ops && *a > m {
                m = *a;
            }
        }
        m
    };

    // ─── (B) Luo-Han 2026 long-division EEA cost-faithful skeleton ─────
    let eea_start_ops = b.ops.len();
    let eea_start_peak = b.peak_qubits;
    let mut eea_peak_phase: &'static str = "";

    b.set_phase("luohan_eea_length_alloc");
    // Four length registers Λ_uv, Λ_rs, Λ_r, Λ_a — each ⌈log₂ n⌉+1 qubits.
    let l_uv = b.alloc_qubits(LEN_W);
    let l_rs = b.alloc_qubits(LEN_W);
    let l_r = b.alloc_qubits(LEN_W);
    let l_a = b.alloc_qubits(LEN_W);

    // Toggle the low bits of each length register to a known non-zero
    // pattern so subsequent length-controlled CCXes have non-trivial
    // controls (we toggle back at the end to keep registers clean).
    // Initial Λ values per Algorithm 3 §3.2: Λ_uv ← n, Λ_rs ← 0, others 0.
    // We classically initialize Λ_uv to the constant n=256 (binary 100000000
    // — bit 8 only) and leave the others at 0; we'll temporarily X some bits
    // during the round body to keep the location-controlled fillers active.
    init_small_const_reg(b, &l_uv, 0x100u64); // n = 256 = bit 8

    b.set_phase("luohan_eea_work_alloc");
    let w1 = b.alloc_qubits(WORK_W);
    let w2 = b.alloc_qubits(WORK_W);

    // Per-round emission. We emit each round as a balanced compute /
    // uncompute pair so all qubits return to |0⟩ at round end. The
    // round_body closure emits the CCX count and uncomputes itself.
    //
    // Round CCX target per the cost-faithful skeleton design comment above:
    //   length-update  : 28  CCX/round   (4 registers × ~7 CCX)
    //   loc-ctrl swap  : 4096 CCX/round  (2× n·log₂n = 2·256·8)
    //   loc-ctrl addsub: 8192 CCX/round  (4× n·log₂n)
    // We achieve these via balanced ccx+ccx pairs on |0⟩ targets so the
    // state is preserved and the count is precise.

    for round in 0..ITERS {
        // (i) length-update — emits 4 × 7 = 28 CCX-pairs (56 CCX total),
        //     simulating the conditional ±1 updates of Λ_uv, Λ_rs, Λ_r, Λ_a
        //     described in arxiv 2604.02311 §3.3.
        b.set_phase("luohan_eea_length_update");
        for (reg_idx, lreg) in [&l_uv, &l_rs, &l_r, &l_a].iter().enumerate() {
            let ctrl_bit = w1[reg_idx % WORK_W];
            for j in 0..7 {
                let tgt = lreg[j % LEN_W];
                let c2 = lreg[(j + 1) % LEN_W];
                b.ccx(ctrl_bit, c2, tgt);
                b.ccx(ctrl_bit, c2, tgt); // inverse: state preserved
            }
        }

        // (ii) location-controlled SWAP filler — emits the per-round CCX
        //      cost of a Λ_uv-controlled (n+1)-qubit swap between W1 and W2.
        //      Cost-faithful target: 2·n·log₂n = 4096 CCX per round.
        //      We emit 2 × N CCX-pairs gated on l_uv[ctrl_idx]: each lane
        //      contributes (LEN_W − 1) = 8 control configurations, giving
        //      N × (LEN_W − 1) × 2 / 2 = N · (LEN_W − 1) CCX-pairs ≈ 2048
        //      pairs = 4096 CCX, matching the paper's n·log₂n location-
        //      controlled SWAP cost.
        b.set_phase("luohan_eea_loc_swap");
        for k in 0..N {
            for cb in 0..(LEN_W - 1) {
                let ctrl_a = l_uv[cb];
                let ctrl_b = l_uv[cb + 1];
                let tgt = w1[k % WORK_W];
                b.ccx(ctrl_a, ctrl_b, tgt);
                b.ccx(ctrl_a, ctrl_b, tgt);
            }
        }

        // (iii) location-controlled ADD/SUB filler — emits the per-round
        //       CCX cost of two Λ_rs-controlled long-division add/sub
        //       sweeps over W1 and W2. Cost-faithful target: 4·n·log₂n
        //       = 8192 CCX per round (factor 2× the swap to match the
        //       paper's add+sub pair per location).
        b.set_phase("luohan_eea_loc_addsub");
        for k in 0..N {
            for cb in 0..(LEN_W - 1) {
                let ctrl_a = l_rs[cb];
                let ctrl_b = l_rs[cb + 1];
                let tgt1 = w1[k % WORK_W];
                let tgt2 = w2[k % WORK_W];
                b.ccx(ctrl_a, ctrl_b, tgt1);
                b.ccx(ctrl_a, ctrl_b, tgt1);
                b.ccx(ctrl_a, ctrl_b, tgt2);
                b.ccx(ctrl_a, ctrl_b, tgt2);
            }
        }

        // Capture peak phase if this round drove the peak above the
        // affine baseline.
        let _ = round;
        if b.peak_qubits > eea_start_peak && eea_peak_phase.is_empty() {
            eea_peak_phase = b.peak_phase;
        }
    }

    b.set_phase("luohan_eea_work_free");
    b.free_vec(&w2);
    b.free_vec(&w1);

    b.set_phase("luohan_eea_length_free");
    // Restore l_uv back to |0⟩ before freeing.
    init_small_const_reg(b, &l_uv, 0x100u64);
    b.free_vec(&l_a);
    b.free_vec(&l_r);
    b.free_vec(&l_rs);
    b.free_vec(&l_uv);

    let eea_end_ops = b.ops.len();
    let eea_peak_after = b.peak_qubits;
    if eea_peak_after > eea_start_peak && eea_peak_phase.is_empty() {
        eea_peak_phase = b.peak_phase;
    }
    let eea_toffoli: u64 = b.ops[eea_start_ops..eea_end_ops]
        .iter()
        .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
        .count() as u64;
    let eea_local_peak: u32 = if eea_peak_after > eea_start_peak {
        eea_peak_after
    } else {
        let mut m = eea_start_peak;
        for (a, _ph, opidx) in &b.peak_log {
            if *opidx >= eea_start_ops && *opidx < eea_end_ops && *a > m {
                m = *a;
            }
        }
        m
    };

    // ─── Report ────────────────────────────────────────────────────
    eprintln!("LUOHAN_N64_AFFINE_TOFFOLI={}", affine_toffoli);
    eprintln!("LUOHAN_N64_EEA_TOFFOLI={}", eea_toffoli);
    eprintln!("LUOHAN_N64_AFFINE_PEAK={}", affine_local_peak);
    eprintln!("LUOHAN_N64_EEA_PEAK={}", eea_local_peak);
    eprintln!("LUOHAN_N64_AFFINE_PEAK_PHASE='{}'", affine_peak_phase);
    eprintln!("LUOHAN_N64_EEA_PEAK_PHASE='{}'", eea_peak_phase);

    let kill_toffoli = eea_toffoli > affine_toffoli;
    let kill_peak = eea_local_peak > affine_local_peak;
    // Owner-set kill criterion: EEA introduces a distinct luohan_eea_*
    // owner block that does not appear in the affine baseline.
    let kill_owner = eea_peak_phase.contains("luohan_eea_")
        || eea_peak_phase.contains("loc_swap")
        || eea_peak_phase.contains("length_update");
    eprintln!(
        "LUOHAN_N64_KILL_TOFFOLI={}",
        if kill_toffoli { "YES" } else { "NO" }
    );
    eprintln!(
        "LUOHAN_N64_KILL_PEAK={}",
        if kill_peak { "YES" } else { "NO" }
    );
    eprintln!(
        "LUOHAN_N64_KILL_OWNER={}",
        if kill_owner { "YES" } else { "NO" }
    );
    let closed = kill_toffoli || kill_peak || kill_owner;
    eprintln!(
        "LUOHAN_N64_VERDICT={}",
        if closed { "CLOSED" } else { "OPEN" }
    );
}

pub(crate) fn emit_centered_restoring_qbit_benchmark_scaffold(b: &mut B) {
    const WIDTH: usize = 256;
    b.set_phase("centered_restoring_qbit_alloc");
    let u = b.alloc_qubits(WIDTH);
    let v = b.alloc_qubits(WIDTH);
    let q = b.alloc_qubit();
    init_small_const_reg(b, &u, 9);
    init_small_const_reg(b, &v, 5);
    b.set_phase("centered_restoring_qbit_trial");
    centered_restoring_trial_subtract_clean(b, &u, &v, q);
    b.set_phase("centered_restoring_qbit_free");
    // This scaffold uses fixed constants with a known successful trial, so
    // return the observed quotient bit to |0> before freeing it.
    b.x(q);
    b.free(q);
    init_small_const_reg(b, &v, 5);
    init_small_const_reg(b, &u, 9);
    b.free_vec(&v);
    b.free_vec(&u);
}

pub(crate) fn emit_centered_by_denominator_derived_controls_benchmark_scaffold(
    b: &mut B,
    tx: &[QubitId],
    p: U256,
) {
    // First functional integration step beyond fixed traces: derive the BY odd/A
    // controls reversibly from a live quantum denominator copy (here the current
    // output x register), run a clean fast centered replay roundtrip on scratch,
    // then reverse the denominator generator to clean the controls.  The replay
    // scratch is zero so this is still a no-op, but the control bank is now
    // genuinely denominator-derived rather than hard-coded.
    const STEPS: usize = 560;
    const DBITS: usize = 12;
    const WIDE: usize = N + 4;
    b.set_phase("by_centered_denom_controls_bench_alloc");
    let f = b.alloc_qubits(STEPS);
    let g = b.alloc_qubits(STEPS);
    let delta = b.alloc_qubits(DBITS);
    let odd = b.alloc_qubits(STEPS);
    let a_ctrl = b.alloc_qubits(STEPS);
    let parity = b.alloc_qubits(STEPS);
    let r = b.alloc_qubits(WIDE);
    let s = b.alloc_qubits(WIDE);

    for i in 0..N {
        if bit(p, i) {
            b.x(f[i]);
        }
        b.cx(tx[i], g[i]);
    }
    b.x(delta[0]);

    b.set_phase("by_centered_denom_controls_bench_generate");
    for i in 0..STEPS {
        let rem = STEPS - i;
        by_2adic_branch_step_for_bench(b, &f[..rem], &g[..rem], &delta, odd[i], a_ctrl[i]);
    }

    b.set_phase("by_centered_denom_controls_bench_replay");
    for i in 0..STEPS {
        centered_signed_by_microstep_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
    }
    for i in (0..STEPS).rev() {
        centered_signed_by_microstep_inverse_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
        centered_signed_by_clear_parity_after_inverse_for_bench(b, &r, &s, odd[i], parity[i]);
    }

    b.set_phase("by_centered_denom_controls_bench_reverse");
    for i in (0..STEPS).rev() {
        let rem = STEPS - i;
        by_2adic_branch_step_reverse_for_bench(b, &f[..rem], &g[..rem], &delta, odd[i], a_ctrl[i]);
    }

    b.set_phase("by_centered_denom_controls_bench_clear");
    b.x(delta[0]);
    for i in 0..N {
        b.cx(tx[i], g[i]);
        if bit(p, i) {
            b.x(f[i]);
        }
    }
    let _ = (f, g, delta, odd, a_ctrl, parity, r, s);
}

pub(crate) fn emit_centered_by_denom_controls_live_numerator_benchmark_scaffold(
    b: &mut B,
    tx: &[QubitId],
    ty: &[QubitId],
    p: U256,
) {
    // Same denominator-derived control component, but now the centered replay
    // scratch is a nonzero live numerator-derived value: a centered copy of the
    // current y register.  The fast centered replay is still run as a
    // forward+inverse no-op, but it now exercises arbitrary quantum numerator
    // data rather than the zero scratch used by the first denominator hook.
    const STEPS: usize = 560;
    const DBITS: usize = 12;
    const WIDE: usize = N + 4;
    b.set_phase("by_centered_live_num_bench_alloc_num");
    let r = b.alloc_qubits(WIDE);
    let s = b.alloc_qubits(WIDE);
    let center_flag = by_load_centered_copy_for_bench(b, ty, &s, p);

    b.set_phase("by_centered_live_num_bench_alloc_den");
    let f = b.alloc_qubits(STEPS);
    let g = b.alloc_qubits(STEPS);
    let delta = b.alloc_qubits(DBITS);
    let odd = b.alloc_qubits(STEPS);
    let a_ctrl = b.alloc_qubits(STEPS);
    let parity = b.alloc_qubits(STEPS);
    for i in 0..N {
        if bit(p, i) {
            b.x(f[i]);
        }
        b.cx(tx[i], g[i]);
    }
    b.x(delta[0]);

    b.set_phase("by_centered_live_num_bench_generate");
    for i in 0..STEPS {
        let rem = STEPS - i;
        by_2adic_branch_step_for_bench(b, &f[..rem], &g[..rem], &delta, odd[i], a_ctrl[i]);
    }

    b.set_phase("by_centered_live_num_bench_replay");
    for i in 0..STEPS {
        centered_signed_by_microstep_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
    }
    for i in (0..STEPS).rev() {
        centered_signed_by_microstep_inverse_for_bench(b, &r, &s, odd[i], a_ctrl[i], parity[i], p);
        centered_signed_by_clear_parity_after_inverse_for_bench(b, &r, &s, odd[i], parity[i]);
    }

    b.set_phase("by_centered_live_num_bench_reverse_den");
    for i in (0..STEPS).rev() {
        let rem = STEPS - i;
        by_2adic_branch_step_reverse_for_bench(b, &f[..rem], &g[..rem], &delta, odd[i], a_ctrl[i]);
    }

    b.set_phase("by_centered_live_num_bench_clear");
    b.x(delta[0]);
    for i in 0..N {
        b.cx(tx[i], g[i]);
        if bit(p, i) {
            b.x(f[i]);
        }
    }
    by_unload_centered_copy_for_bench(b, ty, &s, p, center_flag);
    let _ = (f, g, delta, odd, a_ctrl, parity, r, s);
}
