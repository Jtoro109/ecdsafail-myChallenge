# Creative ambitious attempts log (April 24)

## Summary
User asked for ambitious exploration beyond public literature. Here's
what was attempted and what failed:

## Attempt 1: Classical m_hist via HMR + phase correction
**Goal**: Save 407q by measuring each iter's m_i bit to classical.

**Implementation**: Full `_mc` variants of bulk forward/backward with
HMR after step 2 + `cz_if` phase corrections for the two CCX
contributions to m_i.

**Result**: 320 phase-garbage batches across 5 seeds × 4096 shots.
Classical correctness passed (0 mismatches).

**Root cause**: HMR gives a RANDOM classical bit, not the stored quantum
value. For single-use cases (like AND uncompute), phase correction
disentangles the measurement-collapsed qubit coherently. But when we
need to "reload" the qubit's value later (in backward pass), the
random classical bit cannot represent the entangled-with-input quantum
value.

**Lesson**: Measurement-uncompute only works for qubits that are NOT
used after measurement. m_hist is used in backward, so this pattern
doesn't apply.

## Attempt 2: Step-9 removal with persistent a_f
**Goal**: Save 660k Toffoli by removing the redundant swap-back cswap
at end of each Eea iter.

**Classical validation first**: Python simulation showed the two
variants diverge. After 407 iters only 13/30 trials match final
(u, v, r, s) even with a final parity swap correction.

**Root cause**: The a_k value is computed from CURRENT register
contents. Without step 9, register contents evolve differently between
rounds (accumulating unswapped state), so a_{k+1} in the no-step-9
variant is different from the with-step-9 variant. The algorithms
COMPUTE DIFFERENT TRAJECTORIES, not just differing by a swap.

## Attempt 3: Rowwise mul (n-wide tmp vs 2n)
**Goal**: Save 256q at peak via streaming rowwise schoolbook.

**Implementation**: Added `mod_mul_add_rowwise_into_acc` behind
`KAL_ROWWISE_MUL=1` env var. Used `cmod_add_qq(acc, tmp, y[i], p)` for
each row followed by `mod_double_inplace_fast(tmp)`.

**Result at state_a_mul1 only**: correctness passes, peak unchanged at
2716 (other phases hit same peak), Toffoli +300k.

**Result when applied to all three mul call sites**: 1 phase-garbage
batch appears (similar to the pre-existing bug with `schoolbook_lowq`).

**Root cause**: The rowwise mul's uncompute via halve-back has a rare
edge case that leaks phase. Likely related to intermediate tmp value
crossing a boundary that mod_halve doesn't handle cleanly.

## Attempt 4: Non-fast ripple_adder in backward step 4
**Goal**: Save ~256q at backward step 4 by using `ripple_adder_sub/add`
instead of `_fast` variants (no carry ancilla).

**Result**: Correctness passes, peak UNCHANGED (forward mul1 still
dominant peak). Toffoli +330k.

## Attempt 5: Litinski fast-inverse lowq mul (existing dead code)
**Goal**: Wire `schoolbook_mul_into_addsub_lowq` into state_a_mul1 to
save carry qubits.

**Result**: 1 phase-garbage batch in 20480 shots (pre-existing bug
documented in the code).

## Attempt 6: Jacobian coordinates analysis
**Goal**: Eliminate one inversion by using Jacobian internally.

**Classical validation**: 100/100 trials match affine point-add.

**Cost analysis**: Jacobian add = 4 muls + 3 squarings ≈ 1.4M Toffoli
forward + 1.4M backward = 2.8M. Plus 1 Eea (~1.6M) + 2 muls
for affine conversion. Total: ~5.5M Toffoli — WORSE than current 4.18M.

**Qubit analysis**: Jacobian temps during add peak at ~2560q (8
registers live at some point). Plus Eea at end: ~2700q. Not a
peak reduction.

## Attempt 7: Coset representation (Zalka/GE21)
**Potential**: 33% modular-add cost reduction (2n → ~1.5n CCX).

**Blocker**: Our test harness checks `get_register == expected mod p`.
Coset-form registers hold `jN + x`, which matches mod p but exceeds
n bits. Either need to change harness (out of scope per user constraint
"Only edit files under src/ec_add/") or use c_pad=0 and accept
rare errors (fails our 100% correctness gate).

## Attempt 8: Montgomery-form for scaling factor reuse
**Potential**: Fold state_a_halve (407 halvings, 103k CCX) and state_b_double
(404 doublings, 103k CCX) into a single Montgomery conversion at start
and end. Saves ~200k CCX.

**Blocker**: Requires ALL operations (muls, adds) to be in Montgomery
form. Full circuit rewrite.

## Attempt 9: Fewer iters (e.g., 300-style)
**Tested**: 405, 401 work but gives only 0.2% improvement. Going below
~400 breaks the 24-seed test (tested: 390 fails badly).

## Attempt 10: Bulk halve via mul by classical constant 2^(-407)
**Cost analysis**: Constant has 117 set bits. `in_place_mul_const`
costs 2 × (hamming + 255) × n ≈ 321k CCX vs 104k for 407 individual
halvings. **3x WORSE.** No win.

## What we've learned

**The hard constraint**: public-literature methods cannot simultaneously
achieve:
- ≤1200 qubits peak
- ≤5M Toffoli per point-add
- Reversible 256-bit secp256k1 affine point-add with in-place output

**Google's withheld circuit is the only known method that does all
three.** Their paper mentions coset representation + windowed arithmetic
+ Litinski scaffold, but the specific construction is withheld.

## What's left to try (higher-risk, multi-session)

1. **Full Luo 2025 rewrite**: 1333q, ~200M Toffoli. ~20 hours of work.
2. **Coset representation + harness modification**: ~2000q, ~3M Toffoli.
   Requires harness change or `c_pad=0` with failure tolerance.
3. **Windowed arithmetic for single-point-add**: requires QROM
   lookup tables, structured nontrivially for single-pt-add.
4. **Custom hybrid**: combine techniques selectively. Research.

## Current best, committed
- 4.18M Toffoli / 2716q
- Matches Litinski 2023 frontier
- Beats HRSL Low-T (19M) by 4.5x
- 1.5x over Google's claimed low-qubit (2.7M / 1175q)
- 2.5x qubits over Google (2716 vs 1175)
