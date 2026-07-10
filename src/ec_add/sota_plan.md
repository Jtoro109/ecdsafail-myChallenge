# Concrete plan to reach SOTA (matching Google's 1175q/2.7M)

## Current state (committed)
- **4.18M Toffoli / 2716q** on harness identical to Google's ZKP harness
- Beats HRSL Low-T (19M) by 4.6×
- Matches Litinski 2023 frontier
- **Gap to Google**: 1.55× Toffoli, 2.3× qubits

## Identified peak-triggering sites (via TRACE_EACH_PEAK)

| Phase | Peak cause | Qubits wasted |
|---|---|---:|
| state_a_mul1 | 254-qubit alloc burst at ops_idx=5294430 | 254 |
| state_a_mul2 | same (correction-3 padding) | 254 |
| bk_bulk_step6_7_8 | `csub_nbit_const_fast` internal alloc of n-wide `a` + n-1 carries | ~254 |
| bk_step6_7_8 | same | ~254 |

**All peak sites share the same root cause**: an n-wide ancilla (either
the mul's correction-3 padding OR the const-sub's materialized
constant) combined with n-1 RippleAdder carries. Both allocations are n
qubits each, total 2n transient.

## Step-by-step plan (session 2-4)

### Session 2: port Gidney's venting adder (arxiv 2507.23079)

Primitives to implement in Rust under `src/ec_add/venting.rs`:

1. **`xor_right_shifted_carries_into`** — Häner carry-xor: `Q_dst ^= carry(Q_src, offset, cin) >> 1`. ~n CCX, 0 ancilla. Direct port from
   `/tmp/gidney_venting/code/src/constadd/_carry_xor.py`.

2. **`add_using_2_clean_qubits_and_venting_carries`** — streaming vented
   add with phase-flips left behind. 2 clean ancilla, ~n CCX, n-3 vent bits.
   Direct port from `_add_2_clean_vented.py`.

3. **`iadd_using_3_clean_qubits`** — composed full adder via split +
   vent + phase-fix. 3 clean ancilla, 4n CCX, 0 dirty ancilla (uses
   "borrow back and forth"). Port from `_add_3_clean.py`.

4. **`iadd_using_linear_dirty_qubits_2_clean_qubits`** — 2 clean + n-2
   dirty, 3n CCX. Port from `_add_n_dirty_2_clean.py`.

**Testing**: port `_add_*_test.py` to Rust tests. Verify correctness
on classical basis states at small n (8, 16, 32, 64). Extend to
correctness tests at n=256 using our Simulator.

**Wiring**: replace `csub_nbit_const_fast` in `mod_halve_inplace_fast`
with venting variant. Replace correction-3 full-width sub in
`schoolbook_mul_into_addsub` with venting variant.

**Expected impact**: peak 2716 → ~2460 (save ~256q at both peak
sites). Toffoli 4.18M → ~5.5M (+30% from 3n vs n per const-add).

**Effort**: ~6-8 hours. Complexity: high (careful RippleAdder-derivative
primitives). Low risk if Python reference is faithfully ported.

### Session 3: Luo-style register sharing (arxiv 2506.xxxxx / Luo 2025)

Goal: reduce Eea state `u, v, r, s` from 4n to ~3n by sharing
storage via length registers.

Key primitives:
- **`loc_controlled_cswap`**: cswap between qubit pairs determined by
  a log-wide length register Λ. ~n log n CCX.
- **`loc_controlled_add/sub`**: add/sub parameterized by Λ.
- **`luo_iteration_forward/backward`**: Eea iter using Work1, Work2
  registers (each 2n wide, shared between u/v and r/s respectively).

**Expected impact**: Eea state 1432q → ~800q. Peak 2460 → ~1800.
Toffoli 5.5M → ~40-100M (due to location-controlled arithmetic's
n log n cost per op).

**Effort**: ~10-15 hours. Complexity: very high. 

**Note**: this is a QUBIT vs TOFFOLI tradeoff. If user accepts ~50M
Toffoli, this gets us to ~1800q at ~50M — close to Luo 2025's public
numbers.

### Session 4: Coset representation (Zalka / Gidney-Ekera 2021)

Goal: cheaper modular adds via non-modular adds on coset-form
registers.

Key changes:
- Represent registers in coset form: `|k mod p⟩` → `Σ|jN + k⟩`.
- Add `c_pad` = O(log n) padding bits per register.
- Replace `mod_add_qq` (10n Toffoli) with `non_mod_add_qq` (4n Toffoli).
- At circuit boundaries, convert to/from canonical form.

**Expected impact**: 60% reduction on modular-add cost (which is a big
chunk of our Eea step 4). Toffoli 5.5M → ~3M.

**Blocker**: harness expects canonical output `x mod p`. Coset registers
hold `jN + k`. Workarounds:
  (a) Final fold-to-canonical at circuit end (adds ~2n Toffoli).
  (b) Harness modification to check `register mod p == expected`.
  (c) Use coset ONLY inside subroutines, not at I/O boundary.

**Effort**: ~8-12 hours (core primitive) + careful scheduling.

## Final projected SOTA-adjacent

After all 3 sessions:
- **Qubits**: ~1500-1800 (vs Google 1175)
- **Toffoli**: ~3-5M (vs Google 2.7M)
- **Within 1.5× Google on both axes** — the publicly-reachable frontier.

To match Google EXACTLY likely requires additional novel research
(e.g., a specific sequencing of these techniques that Google found but
didn't publish).

## Immediate next step (next session)

Start with **venting adder port**. It's the highest-impact / lowest-risk
single primitive. Gidney's Python code is reference quality. Porting
4-5 functions into Rust + tests = 1 session of focused work.

Target delivery:
- `src/ec_add/venting.rs`: ~500 lines
- `src/ec_add/venting_test.rs`: ~300 lines
- Wired into `mod_halve_inplace_fast` and `schoolbook_mul` as opt-in
  behind env var.
- Peak reduction from 2716 → ~2460 demonstrated in autoresearch.
