//! Unconditional Eea inversion — no m_hist, no f_flag.
//!
//! Each iteration recomputes control bits from current (u, v_w) state.
//! This eliminates the 407-qubit m_hist register and the 1-qubit f_flag,
//! saving ~408 qubits of persistent state during the Eea body.
//!
//! Cost: ~25% more Toffoli (512 iterations instead of ~400, plus per-iter
//! flag recomputation). But massive qubit savings.
//!
//! The algorithm is the same binary almost-inverse (Eea) but every
//! iteration is "unconditional" — when v=0 (algorithm terminated), the
//! iteration just doubles r (a no-op on u,v). This matches Kim et al. 2026's
//! approach.
//!
//! Key invariant: after 2n iterations, r = ±v_in^{-1} * 2^{2n} mod p,
//! u = 1, v_w = 0, s = p. These deterministic end-values allow us to
//! free u and s after forward, reclaiming 512 qubits during the body.

use super::{
    add_nbit_qq, add_nbit_qq_fast, bit, cswap, mod_add_qq, mod_add_qq_fast,
    mod_double_inplace_fast, mod_halve_inplace_fast, mod_sub_qq_fast, sub_nbit_qq,
    sub_nbit_qq_fast, with_eq_zero_fast, with_gt, QubitId, B, SECP256K1_P,
};

use alloy_primitives::U256;

/// State for unconditional Eea — NO m_hist, NO f_flag.
pub struct UnconditionalEeaState {
    pub u: Vec<QubitId>,   // n qubits
    pub v_w: Vec<QubitId>, // n qubits
    pub r: Vec<QubitId>,   // n qubits
    pub s: Vec<QubitId>,   // n qubits
                           // No m_hist! No f_flag!
}

impl UnconditionalEeaState {
    pub fn alloc(b: &mut B, n: usize) -> Self {
        Self {
            u: b.alloc_qubits(n),
            v_w: b.alloc_qubits(n),
            r: b.alloc_qubits(n),
            s: b.alloc_qubits(n),
        }
    }

    pub fn free(self, b: &mut B) {
        b.free_vec(&self.s);
        b.free_vec(&self.r);
        b.free_vec(&self.v_w);
        b.free_vec(&self.u);
    }
}

/// One unconditional Eea iteration (forward).
///
/// Derives all control bits from current state, does the work, then
/// un-derives the control bits. No persistent history needed.
///
/// The logic mirrors `eea_iteration` but with locally-computed
/// flags instead of m_hist/f_flag.
pub fn eea_iter_unconditional_fwd(
    b: &mut B,
    p: U256,
    u: &[QubitId],
    v_w: &[QubitId],
    r: &[QubitId],
    s: &[QubitId],
    iter_idx: usize,
) {
    let n = u.len();
    let _saved_phase = b.phase;

    // ─── Compute control flags from current state ───
    // a = (v_w == 0) — i.e., "algorithm has terminated"
    // Actually in Eea: a = u[0]=0 OR (u[0]=1 AND v_w[0]=1 AND u>v)
    // Wait, let me re-derive from the original eea_iteration logic.
    //
    // Original step 0: if v_w == 0: f ^= m_i, m_i ^= (f AND v_eq_zero)
    // Original step 1: a ^= (f AND u[0]=0), m_i ^= (f AND u[0]=1 AND v_w[0]=0)
    // Original step 2: a ^= (f AND u>v AND NOT b), m_i ^= same
    //   where b = a XOR m_i
    //
    // In unconditional mode (f is always "active" = 1 until v=0):
    //   - Before termination: f=1, all operations gated by f proceed normally
    //   - After termination (v=0): the step 0 check detects v=0, sets f=0,
    //     and all subsequent steps become no-ops
    //
    // In our unconditional version, we RECOMPUTE the "is active" flag each iter
    // by checking v_w == 0. If v_w == 0, we skip everything except r *= 2.
    //
    // The branch logic when active (v_w != 0):
    //   a = NOT u[0]                           (u even → a=1, means "swap")
    //   if u[0]=1 AND v_w[0]=0: a=0, m_i=1     (v even, no swap)
    //   if u[0]=1 AND v_w[0]=1: a = (u > v)     (both odd, swap if u>v)
    //
    // Combined: a = (u[0]=0) OR (u[0]=1 AND v_w[0]=1 AND u>v)
    //           = (u[0]=0) OR (u[0]=1 AND v_w[0]=1 AND u>v)
    //
    // m_i (the "other" branch bit):
    //   m_i = (u[0]=1 AND v_w[0]=0)            (v even case)
    //       OR (u[0]=1 AND v_w[0]=1 AND NOT (u>v))  (both odd, v≥u)
    //
    // b = a XOR m_i
    // add = f AND NOT b (when f=1: add = NOT b)

    // Step 0: Check if v_w is zero (algorithm terminated)
    let v_is_zero = b.alloc_qubit();
    let or_width = if iter_idx < n { n } else { 2 * n - iter_idx };
    with_eq_zero_fast(b, &v_w[0..or_width], v_is_zero, |b| {
        // v_is_zero = 1 if v_w == 0 (terminated), 0 if still running
    });
    // Now v_is_zero holds the "is terminated" flag.
    // We'll use it as the INVERSE of f: f_active = NOT v_is_zero.

    let a_f = b.alloc_qubit();
    let b_f = b.alloc_qubit();
    let add_f = b.alloc_qubit();
    let m_i = b.alloc_qubit(); // local m_i, not stored

    // When v_w == 0 (terminated): all flags should be 0 (no-op except r *= 2)
    // When v_w != 0 (active): compute a, b, m_i, add normally

    // ─── STEP 1: compute a and m_i ───
    // a = (u[0] = 0) when active
    // m_i = (u[0]=1 AND v_w[0]=0) when active
    //
    // Using f_active = NOT v_is_zero:
    //   a_f = f_active AND (NOT u[0])
    //   m_i = f_active AND u[0] AND (NOT v_w[0])
    //
    // Compute via CCX:
    //   First compute f_active = NOT v_is_zero into a temp
    let f_active = b.alloc_qubit();
    b.x(v_is_zero);
    b.cx(v_is_zero, f_active); // f_active = NOT v_is_zero
    b.x(v_is_zero);

    // a_f = f_active AND (NOT u[0])
    b.x(u[0]);
    b.ccx(f_active, u[0], a_f);
    b.x(u[0]);

    // m_i = f_active AND u[0] AND (NOT v_w[0])
    b.x(v_w[0]);
    b.ccx(f_active, v_w[0], m_i); // m_i = f_active AND (NOT v_w[0]) ... wait need u[0] too
                                  // Actually: m_i = f_active AND u[0] AND (NOT v_w[0])
                                  // Compute u0_and_f first, then AND with NOT v_w[0]
    let u0_and_f = b.alloc_qubit();
    b.ccx(f_active, u[0], u0_and_f);
    b.ccx(u0_and_f, v_w[0], m_i); // m_i = u0_and_f AND (NOT v_w[0])
    b.x(v_w[0]);

    b.cx(a_f, b_f);
    b.cx(m_i, b_f); // b_f = a_f XOR m_i

    // ─── STEP 2: with u > v_w ───
    // a ^= (f_active AND u_gt_v AND NOT b_f)
    // m_i ^= same
    b.set_phase("kal_unc_step2");
    let cmp_width = if iter_idx < n { n } else { 2 * n - iter_idx };
    let l_gt = b.alloc_qubit();
    with_gt(b, &u[0..cmp_width], &v_w[0..cmp_width], l_gt, |b| {
        b.x(b_f);
        let f_and_gt = b.alloc_qubit();
        b.ccx(f_active, l_gt, f_and_gt);
        let t = b.alloc_qubit();
        b.ccx(f_and_gt, b_f, t); // t = f_and_gt AND NOT b_f
        b.cx(t, a_f);
        b.cx(t, m_i);
        // Uncompute t via HMR
        let tm = b.alloc_bit();
        b.hmr(t, tm);
        b.cz_if(f_and_gt, b_f, tm);
        b.free(t);
        // Uncompute f_and_gt via HMR
        let fm = b.alloc_bit();
        b.hmr(f_and_gt, fm);
        b.cz_if(f_active, l_gt, fm);
        b.free(f_and_gt);
        b.x(b_f);
    });
    b.free(l_gt);

    // ─── STEP 3: conditional swap ───
    b.set_phase("kal_unc_step3_cswap");
    let uv_width = if iter_idx < n { n } else { 2 * n - iter_idx };
    for j in 0..uv_width {
        cswap(b, a_f, u[j], v_w[j]);
    }
    let rs_width_step3 = if iter_idx + 1 < n { iter_idx + 1 } else { n };
    for j in 0..rs_width_step3 {
        cswap(b, a_f, r[j], s[j]);
    }

    // ─── STEP 4: conditional add ───
    b.set_phase("kal_unc_step4");
    // add = f_active AND NOT b_f
    b.x(b_f);
    b.ccx(f_active, b_f, add_f);
    b.x(b_f);

    // Step 4 body: if add_f: v_w -= u; s += r
    // Same structure as original but using add_f
    {
        let tmp = b.alloc_qubits(n);
        let load_width = if iter_idx < n { n } else { 2 * n - iter_idx };
        for i in 0..load_width {
            b.ccx(add_f, u[i], tmp[i]);
        }
        let tmp_sub_slice: Vec<QubitId> = tmp[0..load_width].to_vec();
        let v_w_sub_slice: Vec<QubitId> = v_w[0..load_width].to_vec();
        sub_nbit_qq_fast(b, &tmp_sub_slice, &v_w_sub_slice);

        let transform_width = if iter_idx + 1 < n { iter_idx + 1 } else { n };
        for i in 0..transform_width {
            b.cx(r[i], u[i]);
        }
        for i in 0..transform_width {
            b.ccx(add_f, u[i], tmp[i]);
        }
        for i in 0..transform_width {
            b.cx(r[i], u[i]);
        }

        let add_width = if iter_idx + 2 < n { iter_idx + 2 } else { n };
        let mut tmp_slice: Vec<QubitId> = tmp[0..transform_width].to_vec();
        let tmp_pad = if add_width > transform_width {
            let q = b.alloc_qubit();
            tmp_slice.push(q);
            Some(q)
        } else {
            None
        };
        let s_slice: Vec<QubitId> = s[0..add_width].to_vec();
        add_nbit_qq_fast(b, &tmp_slice, &s_slice);
        if let Some(q) = tmp_pad {
            b.free(q);
        }

        for i in 0..n {
            let m = b.alloc_bit();
            b.hmr(tmp[i], m);
            if i < transform_width {
                b.cz_if(add_f, r[i], m);
            } else if i < load_width {
                b.cz_if(add_f, u[i], m);
            }
        }
        b.free_vec(&tmp);
    }

    // ─── STEP 5: uncompute add ───
    b.x(b_f);
    {
        let sm = b.alloc_bit();
        b.hmr(add_f, sm);
        b.cz_if(f_active, b_f, sm);
    }
    b.x(b_f);

    // ─── STEP 6: v_w /= 2 ───
    b.set_phase("kal_unc_step6_7_8");
    for i in 0..(n - 1) {
        b.swap(v_w[i], v_w[i + 1]);
    }

    // ─── STEP 7+8: r *= 2 mod p ───
    // Unconditional: always double r, even if v=0 (this is the "no-op" for
    // terminated iterations — r just accumulates the 2^{2n} factor).
    if iter_idx < 256 {
        // r's top bit is guaranteed 0 for early iters
        for i in (0..n - 1).rev() {
            b.swap(r[i], r[i + 1]);
        }
    } else {
        mod_double_inplace_fast(b, r, p);
    }

    // ─── STEP 9: conditional swap (again) ───
    b.set_phase("kal_unc_step9_cswap");
    let uv_width9 = if iter_idx < n { n } else { 2 * n - iter_idx };
    for j in 0..uv_width9 {
        cswap(b, a_f, u[j], v_w[j]);
    }
    let rs_width_step9 = if iter_idx + 2 < n { iter_idx + 2 } else { n };
    for j in 0..rs_width_step9 {
        cswap(b, a_f, r[j], s[j]);
    }

    // ─── STEP 10: uncompute a ───
    b.x(s[0]);
    b.cx(s[0], a_f);
    b.x(s[0]);

    // ─── Uncompute all local flags ───
    // Uncompute m_i: we need to redo the computation that set m_i
    // m_i was set as: f_active AND u[0] AND NOT v_w[0] (step1 part)
    // PLUS step2 contribution: m_i ^= (f_active AND u_gt_v AND NOT b_f)
    // After step10, a_f=0. After step5, add_f=0.
    // b_f = a_f XOR m_i = 0 XOR m_i = m_i
    // So b_f currently holds m_i.
    // We need to uncompute m_i. But m_i depends on state that has been modified
    // by the iteration! The u, v_w have changed.
    //
    // This is the fundamental issue with "unconditional" iteration without
    // history: we can't uncompute m_i after the iteration changes state.
    //
    // Solution: uncompute the flags BEFORE the state-changing steps, by
    // restructuring the iteration.
    //
    // Actually, in the ORIGINAL eea_iteration, m_i is STORED in m_hist
    // and never uncomputed during forward — it's used during backward.
    // In our unconditional version, we DON'T need to uncompute m_i during
    // forward at all, because we recompute it during backward.
    //
    // So the flags a_f, b_f, m_i, add_f, f_active, u0_and_f just need to
    // be zero at iteration END. Let me verify:
    // - a_f: zeroed by step10 (cx from NOT s[0])
    // - add_f: zeroed by step5 (HMR uncompute)
    // - b_f: currently = a_f XOR m_i = 0 XOR m_i = m_i. NOT zero!
    // - m_i: not zeroed
    // - f_active: not zeroed
    // - u0_and_f: not zeroed
    //
    // This is wrong. I need to properly uncompute all local flags.
    //
    // The original eea_iteration achieves this because:
    // - a_f is zeroed by step10
    // - b_f = a_f XOR m_i, and m_i is stored (not local)
    // - add_f is zeroed by step5
    // - m_i is stored in m_hist (persistent)
    //
    // In our version, m_i must also be zeroed locally. We can zero it by
    // reversing the computation that set it. But the computation depends on
    // the ORIGINAL u, v_w state (before modification by this iteration).
    //
    // After the iteration, u and v_w have been modified. So we can't simply
    // recompute m_i from the new state.
    //
    // APPROACH: Use emit_inverse to uncompute the flag-computation block.
    // Specifically:
    // 1. Compute flags (a_f, m_i, b_f, f_active, u0_and_f) from current state
    // 2. Do the state-modifying steps (cswap, step4, step6-8, cswap, step10)
    // 3. At end, the flags a_f and add_f are already zero. But b_f, m_i,
    //    f_active, u0_and_f are not.
    // 4. We can't recompute them from modified state.
    //
    // SOLUTION: Wrap steps 1-2 in emit_inverse_hmr_safe to auto-reverse.
    // Actually no — that would reverse the state changes too.
    //
    // BETTER SOLUTION: Don't try to uncompute m_i, b_f, f_active, u0_and_f
    // during forward. Instead, free them "dirty" and handle cleanup
    // during backward. But we can't free dirty qubits — the harness checks
    // for zero.
    //
    // REAL SOLUTION: Compute flags, do state changes, then BEFORE freeing
    // the flags, reverse the flag computation using emit_inverse on just
    // the flag-computation block. But we can't use emit_inverse because
    // it reverses ALL ops including state changes.
    //
    // ACTUAL SOLUTION: Save the flag computation as a separate block,
    // then at end of iteration, manually reverse the flag computation.
    // But the flag computation depends on the ORIGINAL state (before
    // modifications), so we'd need to reverse the state changes first,
    // then reverse the flags, then redo the state changes. That triples
    // the work.
    //
    // SIMPLEST CORRECT SOLUTION: Use the original approach with m_hist
    // but allocate m_hist lazily. OR: accept that each iteration needs
    // to be self-contained by doing: compute flags → save copies →
    // state changes → restore from copies → uncompute flags → redo state
    // changes from saved flags.
    //
    // This is getting circular. The fundamental issue is that without
    // m_hist, the forward iteration CANNOT be self-cleaning because the
    // flag computation depends on pre-modification state.
    //
    // INSIGHT: The Kim 2026 approach doesn't have this problem because
    // they use WIDE r,s registers (2n+1 bits each) and don't do FastModulo
    // reduction during the loop. The iteration logic is simpler: just
    // halve/double/conditional-sub, with the only "flag" being the
    // parity bit of u and v (which is directly available from u[0], v[0])
    // and the comparison result.
    //
    // In Kim's approach, the iteration is:
    //   if u even: u >>= 1, s <<= 1 (wide shift, no mod reduction)
    //   elif v even: v >>= 1, r <<= 1
    //   elif u > v: u = (u-v)>>1, r += s, s <<= 1
    //   else: v = (v-u)>>1, s += r, r <<= 1
    //
    // The key simplification: NO modular reduction during the loop.
    // This means r and s grow to 2n bits but the arithmetic is simple
    // (no FastModulo). And the "flag" is just u[0] and v[0], which are
    // directly readable from the quantum state.
    //
    // For REVERSIBILITY: each iteration's effect is determined by
    // (u[0], v[0], u>v), all of which can be RECOMPUTED from the
    // post-iteration state (since the inverse operations are deterministic
    // given the post-iteration state). This is the key insight from
    // Luo 2025: the iteration is "history-free reversible" because
    // the control bits are functions of the current state.
    //
    // Wait, is this true? Can we determine the PRE-iteration state from
    // the POST-iteration state? Let's check:
    //
    // Case 1: u even → u >>= 1, s <<= 1
    //   Post: u' = u/2, s' = 2s. We know u was even because u' = u/2
    //   and the pre-shift u = 2*u'. And s = s'/2.
    //   But how do we know WHICH case applied? We know u[0]=0 in pre-state.
    //   In post-state, u'[n-1] could be 0 or 1 (depends on u's bit n-1).
    //   Actually, the sign of the operation is encoded in whether u' is
    //   odd/even... no, u' = u/2 which could be anything.
    //
    // The issue: from the post-iteration state alone, we can't determine
    // which branch was taken. Multiple pre-states could lead to the same
    // post-state.
    //
    // CONCLUSION: unconditional Eea WITHOUT history is NOT trivially
    // reversible. The Kim approach still needs some form of history.
    //
    // What Kim actually does: run 2n iterations unconditionally. The
    // "history" is encoded in the LENGTH REGISTERS (Luo's insight):
    // bitlen(u) + bitlen(v) decreases by exactly 1 per active iteration.
    // So the "effective length" at each step determines how many active
    // iterations have occurred, which in turn determines the sequence.
    //
    // For a FULLY HISTORY-FREE approach, we'd need to use the Luo
    // length-register technique, which is a much bigger change.
    //
    // PRACTICAL RESOLUTION: For now, keep m_hist but compress it.
    // Or: use a pebble game on m_hist to reduce it from O(n) to O(sqrt(n)).
    //
    // Actually, the simplest approach that ACTUALLY WORKS:
    // Keep m_hist in the unconditional version, but with 2n entries
    // instead of ~400. The m_hist stores just the m_i bit per iteration.
    // This costs 2n = 512 qubits... MORE than the current 407.
    //
    // Hmm. So unconditional execution doesn't save m_hist qubits at all
    // if we still need to store the history bits.
    //
    // WAIT — in the current code, m_hist[i] stores a COMBINED bit that
    // is needed for backward. The f_flag is also persistent. If we
    // eliminate f_flag and make each iteration self-contained by
    // recomputing flags, we STILL need m_hist for the backward pass.
    //
    // The ONLY way to eliminate m_hist is to make the iteration
    // reversible without stored history, which requires the Luo
    // length-register technique or an equivalent.
    //
    // ALTERNATIVE: Reduce m_hist from 407 to ~20 using a pebble game.
    // Store sqrt(407) ≈ 20 checkpoint iterations. Between checkpoints,
    // recompute the intermediate m_hist bits by re-running the forward
    // iterations. Cost: O(n * sqrt(n)) total iterations instead of O(n).
    // For n=407: ~407 * 20 = 8140 total iterations = 20x more Toffoli.
    // Too expensive.
    //
    // VERDICT: Cannot eliminate m_hist without Luo-style length registers.
    // The unconditional approach with m_hist costs 512 qubits for m_hist
    // (MORE than current 407). Dead end for qubit reduction.
    //
    // Let me abandon this approach and think differently.

    // Clean up local flags that we allocated
    b.free(u0_and_f);
    b.free(f_active);
    b.free(m_i);
    b.free(add_f);
    b.free(b_f);
    b.free(a_f);
    b.free(v_is_zero);
    b.set_phase(_saved_phase);
}

// NOTE: This module is INCOMPLETE and INCORRECT as written above.
// The unconditional iteration cannot be self-cleaning without stored
// history. The approach needs fundamental rethinking.
//
// The correct path to eliminating m_hist is Luo 2025's length-register
// technique, which is a multi-session effort.
//
// For now, the most impactful structural change is the Bennett bridge
// approach: split Eea forward/backward from body computation,
// allowing u and s to be freed during the body (already implemented
// via KAL_FREE_S and the u-freeing logic in with_kal_inv_raw).
