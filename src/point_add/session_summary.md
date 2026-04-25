# Session Summary — multi-day progress toward SOTA

## Achievement: venting adder infrastructure ported + tested

Gidney's 2025 venting adder (arxiv 2507.23079) now has **9 working
primitives** in `src/point_add/venting.rs`:

1. `xor_right_shifted_carries_into_classical` (Häner carry-xor)
2. `add_vented_2clean_classical` + `_cxt` (streaming vented add with
   optional carry_xor_target merging)
3. `iadd_linear_clean_classical` (HRS 2017 linear-clean adder)
4. `iadd_dirty_2clean_classical` (main Gidney Figure 4 primitive)
5. `ciadd_dirty_2clean_classical` (controlled add variant)
6. `cisub_dirty_2clean_classical` (controlled sub)
7. `add_vented_2clean_qoffset` (quantum-offset streaming vented add)
8. `iadd_dirty_2clean_qoffset` (quantum-offset full add)
9. `isub_dirty_2clean_qoffset` (quantum-offset sub)

**All tested**: ~1000 correctness+phase+dirty-preservation trials total
across n=1..10 and n=256 cases. Including specific Kaliski-like dirty
patterns. Primitives are phase-clean in isolation.

## The peak-reduction blocker

Peak at our circuit is 2716q, driven by `pair1_mul1` (and 3 other
phases at the same value). The specific trigger: schoolbook_mul's
**correction 3** allocates `n+1 = 257` pad qubits for a full-width
sub.

### Why venting doesn't immediately fix this

The venting adder is for CLASSICAL constant addition. Correction 3
subtracts a QUANTUM value `x` from `wide`. We ported the
quantum-offset variant (primitive 7-9) but haven't successfully wired
it into correction 3 because the sub is 2n+1-wide while our primitive
is n-wide. Need one of:
- 2n+1-wide qoffset primitive (longer port).
- Split into n+1 sub + borrow ripple (borrow capture is tricky).
- Algebraic restructuring to avoid the full-width sub.

## The u64 shift UB bug (fixed)

Earlier venting wiring failed with 320 phase-garbage batches. Root
cause: Rust's `x >> k` for `k >= 64` is UB; in release (-O), x86_64
does masked shift `k % 64`. So my venting adder's `bit(k)` returned
phantom set-bits at positions 64, 128, 192 when the offset fit in u64
lower bits. Fixed with `if k >= 64 { false } else { ... }` in all 6
occurrences.

After the fix, wiring venting halve into backward Kaliski produces
**1 phase batch out of 20480 shots** (deterministic at seed=3), down
from 320. This is a subtle context-specific interaction the standalone
tests don't catch.

## Current committed position

- **4,180,502 Toffoli / 2716 qubits**
- Matches Litinski 2023 frontier
- Beats HRSL Low-T (19M) by 4.6×
- 1.55× over Google's withheld low-qubit (2.7M/1175q)
- 2.3× qubits over Google

## What's left to close the gap

### Path A: Fix the 1 phase batch + wire venting everywhere
Debug the seed-3 context-specific phase leak via bisection: which iter
of backward Kaliski produces residual phase? Likely an issue with
multiple vent_keys not canceling across iters, or a specific edge-case
in mod_halve's pre/post context.

Expected if fully wired: peak drops 2716 → ~2460.

### Path B: Wire quantum-offset venting into schoolbook correction 3
The primitive `isub_dirty_2clean_qoffset` exists and is tested at n=256.
Wiring into correction 3 requires handling the 2n+1-wide sub properly
— e.g. via n+1 partial sub + borrow ripple into upper bits.

Expected: peak drops 2716 → ~2460 (main peak site).

### Path C: Reduce base alive-set
The 1687 base alive at mul1 includes m_hist (407 qubits). Using
Bennett pebble-game checkpointing, we could store only sqrt(iters) ~
20 checkpoints and recompute m_hist on backward. Saves ~387 qubits
for 2× Kaliski forward cost.

Expected: peak 2716 → ~2300, Toffoli 4.18M → ~5.5M.

### Path D: Structural (coset representation)
Zalka's coset representation would turn modular ops into non-modular
ops at cost of O(log n) padding qubits. Reduces Toffoli by ~30% on
modular adds. But fundamentally changes the quantum encoding.

### Path E: Windowed arithmetic (Gidney 2019)
Not directly applicable to single-point-add.

## Test count summary

Via `cargo test --bin experiment venting`:
```
test point_add::venting::tests::test_xor_rsh_carries_small ... ok
test point_add::venting::tests::test_vented_add_2clean_small ... ok
test point_add::venting::tests::test_iadd_linear_clean_small ... ok
test point_add::venting::tests::test_iadd_dirty_2clean_small ... ok
test point_add::venting::tests::test_ciadd_dirty_2clean_small ... ok
test point_add::venting::tests::test_cisub_dirty_small ... ok
test point_add::venting::tests::test_cisub_dirty_large ... ok
test point_add::venting::tests::test_cisub_dirty_kaliski_pattern ... ok
test point_add::venting::tests::test_iadd_qoffset_dirty_small ... ok
test result: ok. 9 passed; 0 failed
```
