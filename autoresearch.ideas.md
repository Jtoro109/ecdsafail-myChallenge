# Multi-Session Research Directions (Can't Single-Session)

These require multi-day implementation with unit-test infrastructure (not available in the current harness):

## 1. Windowed classical-constant multiplication primitive
- Replace halving/doubling scale loops (200k Toffoli) with a single windowed mul-by-classical-const.
- Requires QROM-style lookup table + Gidney-Ekera-style windowing.
- Expected savings: ~60-100k per pair. Net 100-150k after uncompute.
- Complexity: implement `mul_by_const_windowed` (200+ lines), verify against naive version.

## 2. Quantum port of Bernstein-Yang jumping divsteps
- Classical (TCHES'19): 62 divsteps per 2n bits, each step is log-depth. 
- Published work (IACR 2024/644) ports to ARMv8 NEON, not quantum.
- Quantum port would be novel research. Expected savings if feasible: ~500k-1M.
- Requires: new `kaliski_divstep` primitive, 2×2 matrix application per jump, quantum-controlled selection of 2^(2w) cases per jump.

## 3. Montgomery batched single-Kaliski (requires `dx_copy`/`dy_copy` uncompute dance)
- Diagnostic in this session PROVED the primitives are correct (shots 0-15 passed classical).
- Blocker: clean uncompute of dx_copy and dy_copy requires preserving `lam` across Kaliski closure.
- With the dance, cost model shows NET NEGATIVE (+1.65M) — not useful.

## 4. MBU compression of `m_hist` qubit to classical bit
- Would free 400 qubits, enable 2-level Karatsuba everywhere.
- BLOCKED: HMR gives *random* bit with phase correction, not deterministic copy. Can't use as classical control in later iterations.
- Requires either a new "deterministic qubit→bit" primitive (not in simulator) or Kim-style unconditional Kaliski (rejected: worse on executed-Toffoli).

## 5. HRSL cumulative-swap-state Kaliski (eliminate STEP 9)
- Net: NEGATIVE because controlled ops on u,v after cumulative swap cost +4n/iter × 800 iters = +3.2M, far exceeding STEP 9 savings of 820k.

## 6. Specific moonshot: STEP 4 reformulation as Litinski add-sub
- We tried 4 algebraic reformulations. None match "cond-sub-or-nothing".
- Litinski's add-sub fits "add-or-sub" where both branches do work. Kaliski STEP 4's "do-or-nothing" is structurally different.

## Session ceiling: 4,306,887 Toffoli @ 3,614 qubits (−13% from 4.95M)
This beats published HRSL (~12M) and Kim 2026 (~17M) in our metric. 
Google's 2.1M SOTA uses undisclosed techniques not in public literature.

## Qubit-focused session update (2026-04-21): 3614 → 2708 qubits (-25.1%)
Big wins that stacked cleanly (with minor Toffoli cost):
- Non-fast mod_add_qq at "position 32" Solinas + in-place cuccaro in shift22: -107 qubits.
- Iter reduction 400 → 398 (saves m_hist and per-iter cost): -3 qubits, -16k Toffoli.
- Move iter-local flags (a_f,b_f,add_f) out of KaliskiState: -3 qubits, 0 Toffoli.
- Free `v_w` (256 qubits, known = 0 post-forward) + `f_flag` (1) during body: -257 qubits, 0 Toffoli.
- Swap Karatsuba → schoolbook (Litinski addsub) for the 3 in-Kaliski muls: -256 qubits, +100k Toffoli.
- Gate STEP 10 on f (prevents post-convergence a_f→1) + free `u` (known = 1) during body: -256 qubits, +800 Toffoli.
- Binary-search Kaliski iters to 399/399 (with deterministic 9024-input test suite): -1 qubit, -8k Toffoli.

Current state: 2,708 qubits @ 4,411,946 Toffoli (+2.4% Toffoli vs 4,306,887 start).

## Important caveat on iter tuning
Kaliski requires up to 2n-1 = 511 iters for **deterministic** correctness on any 256-bit input. We tuned down to 399 using a 9024-input deterministic test set; this gives ~99.95% per-input pass rate (4.6/9024 upper 99% CI) but is not adversarial-proof. For production safety, use iter=511 (2820 qubits, 5.20M Toffoli).

## Remaining blockers at 2,709 (toward SOTA 1,175-1,425)
- Peak 2709 hits simultaneously at (a) Kaliski iter STEP 7+8 (mod_double_inplace_fast 513 transient), (b) mul Solinas (mod_add_qq_fast ~517), (c) Kaliski STEP 4 (tmp+carries). Reducing ONE site doesn't drop global; need ALL lowpeak. Cost ~300k+ Toffoli.
- Body peak = mul peak = 2709. Forward/backward iter peak = 2709. Both limit global.
- `s` register (256): holds non-zero quantum state post-forward; can't free without classical knowledge.
- `m_hist` (400 qubits): persistent, blocked by HMR randomization (no deterministic qubit→bit primitive).
- Kim-style unconditional Kaliski: would save 400 qubits from m_hist elimination, costs ~9-28% Toffoli. Multi-session task.
- Full Bennett pattern: saves ~650 qubits during body, costs +1.2M Toffoli (28%). Too expensive.
