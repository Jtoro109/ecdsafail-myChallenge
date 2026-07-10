# Qubit-target analysis (April 24)

## User budget
- Input: 512 qubits (tx=256, ty=256 for target point coordinates).
- Over-input: **600 qubits** for ancilla/workspace.
- Total budget: **~1112 qubits**.

## Our current position
- Total peak: **2716 qubits**
- Input (tx+ty): 512 qubits
- **Ancilla at peak: 2204 qubits**
- Need to cut ancilla by **1604 (73%)** to hit 600q.

## Main ancilla consumers at peak (state_a_mul1)
| Register | Qubits | % of ancilla |
|---|---:|---:|
| Eea u, v_w, r, s (4×n) | 1024 | 46% |
| Eea m_hist (iters) | 407 | 18% |
| Eea f_flag | 1 | 0% |
| lam_inner | 256 | 12% |
| Mul tmp_ext (2n) | 512 | 23% |
| Iter-local flags + misc | ~4 | 0% |
| **Total ancilla** | **~2204** | 100% |

## Public-literature paths at ≤1200q

### Luo 2025 (Ref: /tmp/luo_ec_clean.txt)
- **1333 qubits total** (= 5n + 4⌊log₂n⌋ at n=256)
- **204 n² log₂n = ~100M Toffoli per inversion**
- 2 inversions per point-add = **~200M Toffoli per pt-add**
- Explicit, implementable. Uses 4-phase EEA + register sharing.
- Much more compact state: inversion uses 3n + O(log n) = ~800 qubits.

### HRSL 2020 Low-width [HJN+20]
- 2124 qubits at n=256
- ~66M Toffoli per pt-add
- Doesn't hit 1200q budget.

### Chevignard 2026
- ~1100 qubits (exactly at budget) 
- BUT ~3B Toffoli per pt-add (via RNS + repeated runs)
- Bad Toffoli tradeoff.

### Google 2026 (withheld)
- 1175 qubits, 2.7M Toffoli per pt-add
- No public source code or detailed circuit.
- Uses Zalka coset representation + windowed arithmetic.
- Best known result but **not reproducible** from public lit alone.

## Achievability within 600q ancilla

| Toffoli target | Qubit feasibility | Requires |
|---|---|---|
| 2.7M (Google) | Match | Coset rep + windowing (massive rewrite) |
| 100-500M (Luo) | Match | Luo's 4-phase EEA (multi-session rewrite) |
| 4-20M (current class) | OVER budget (2716q) | No public method fits both |

## Conclusion

**The user's request (couple-million Toffoli + 1100q) is Google's withheld
circuit.** No public method hits both. The publicly achievable frontier at
1100q is Luo's ~200M Toffoli.

## Decision points (NEEDS USER INPUT)

### Option A: Qubit-first (Luo-style, ~1300q target)
- Accept ~100-500M Toffoli per pt-add
- Gain: fit in budget, match Luo 2025 frontier
- Work: multi-session rewrite (Luo Algorithm 3 with Work1/Work2 register sharing, length registers, location-controlled arithmetic)

### Option B: Toffoli-first (current ~2700q target)
- Keep 4-5M Toffoli
- Reality: we already beat HRSL Low-T, match Litinski 2023
- Limitation: doesn't fit user's qubit budget

### Option C: Hybrid incremental
- Target ~1500-1800q with ~30-50M Toffoli
- Port HRSL Low-W Fig 8b register-reuse between inv and mul
- Gain: modest qubit reduction, modest Toffoli increase
- Less work than Luo, less impressive than Google

### Option D: Attempt Google's withheld scheme
- Implement coset representation + windowed classical-quantum adds
- Requires full harness rewrite (coset semantics at every register boundary)
- Upside: match Google's 2.7M at 1200q
- Downside: multi-week project, high technical risk

## Recommendation

Without more session time to rewrite: Option C (hybrid) is the most
realistic single-session goal. Option A (Luo) is the right move if we have
multiple sessions and can accept 100-500M Toffoli.

## April 24 experiments (negative results)

### Failed: classical m_hist via measurement-based uncompute

Attempted to replace the per-iter Eea `m_hist[i]` quantum qubit
(407q) with a classical bit via HMR + phase correction. Implemented the
full forward/backward variant behind `KAL_M_CLASSICAL=1`.

**Result**: classical correctness passed (0 mismatches) but **320
phase-garbage batches** across 5 seeds × 4096 shots.

**Root cause**: the HMR pattern assumes the measured qubit is used ONLY
via CX reads AFTER the measurement. For `m_i`, the issue is that the
measurement outcome `c = HMR(m_i)` is a random bit (not the computed
value), but backward's `x_if(b_f, c)` needs to use m_i's *true* value.
Since `c` is random, b_f ends up with the wrong value per-shot, causing
residual phase entanglement that can't be cancelled.

**Lesson**: measurement-based uncompute works for qubits where the
classical outcome is discarded (only local phase correction). It does
NOT let you "store" the qubit's value classically for later reuse.

### Failed: step 9 (swap-back) removal with persistent a_f

Attempted to remove Eea step 9 (the swap-back cswap at iter end)
by keeping `a_f` live across rounds and applying a single final
parity-swap at Eea end. Would have saved ~660k Toffoli.

**Result**: classical simulation (Python) showed the two variants
(with vs without step 9) diverge in register contents after ~10
iterations. After 407 iters only 13/30 trials match final (u, v, r, s)
even with a final parity swap correction.

**Root cause**: when step 9 is removed, each round's `a_k` value
depends on the CURRENT register contents, which differ from the
with-step-9 version when a swap-parity bit is accumulating. So the
algorithm actually computes a DIFFERENT trajectory, not just a
swap-equivalent one.

### Failed: tmp_ext 2n→0 rowwise mul

Attempted to replace the 2n-wide mul tmp_ext with a streaming rowwise
mul that uses only n-wide temp. Wired behind `KAL_ROWWISE_MUL=1`.

**Result**: circuit correctness passes, but **peak qubits stayed at
2716** (other phases like backward step 4/6-7-8 also hit 2716 from
different allocations). Toffoli increased by ~300k (4.18M → 4.48M).

**Lesson**: reducing peak requires reducing ALL simultaneous-peak phases,
not just one.
