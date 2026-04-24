# SOTA gap analysis — April 24 research dive

## Google 2026 (secp256k1) per single point-addition resource claim

From `/tmp/google_ecc/src/main.tex`:

- **Low-qubit**: 2.7M Toffoli / 1175 qubits / 17M total ops
- **Low-gate**: 2.1M Toffoli / 1425 qubits / 17M total ops

Windowed point-add: `Q += P[k]` where `P[k]` is a 2^16-entry classical
table indexed by a 16-qubit register `k`. Affine math itself is identical
to Litinski 2023's 6-step scaffold with 2 inversions + 2 muls + 1 sq + adds.

## Literature survey — per single point-add Toffoli at n=256

| | Toffoli (PA) | qubits | per-Kaliski forward | status |
|---|---:|---:|---:|---|
| Google (withheld) low-qubit | **2.7M** | 1175 | ~1.0M (estimated) | withheld |
| Google (withheld) low-gate  | **2.1M** | 1425 | ~0.8M (estimated) | withheld |
| Litinski 2023               | ~4.2M    | ~2500 | 1.7M (26n²) | public |
| HRSL 2020 Low T (Toffoli-opt) | ~19M (4.6M Toffoli@1:4) | 2619 | ~3M | public |
| HRSL 2020 Low W (qubit-opt) | ~66M     | 2124 | big | public |
| Kim 2026 Q-opt              | ~30M     | 6475 | big | public |
| **Our build()**             | **4.18M** | 2716 | **1.62M** | *here* |
| Luo 2025                    | ~500M    | 1333 | ~200M | public |
| Chevignard 2026             | >3000M   | 1100 | RNS | public |

## Cost breakdown of our build()

From TRACE_PHASES:
```
Kaliski internals (kal_* + bk_*):            3.24M Toffoli (78%)
pair1_halve + pair2_double (scale correct):   207k Toffoli (5%)
pair1_mul1 + pair1_mul2 + pair2_mul + mul3:   239k Toffoli (6%)
sol_halve_tail:                                 8k (0%)
affine scaffold + const adds/subs:            ~450k (11%)
---
avg executed Toffoli total:                  4.18M
```

Each Kaliski (fwd + bwd measurement-uncompute) costs ~1.62M.
Per iteration: ~15n CCX (≈3.9k at n=256) for 407 iters × 2 inversions.

## Where the gap to Google 2.7M comes from

Dividing their budget roughly:
- 1 Kaliski fwd+bwd at ~1.3M (20% cheaper than ours, plus fewer iters)
- Still 2 inversions per point-add (Litinski-style can't collapse to 1)  
- Muls/sq/adds at ~400k combined
- Total ≈ 2.7M (matches their claim)

**Google's savings vs Litinski/us must come from two places:**

1. **Cheaper per-iter Kaliski**: ~15n → ~10n CCX/iter. Removes ~30-35% of Kaliski.
2. **Lower iter count**: 407 → ~350 via a tighter convergence bound or windowed iters.

## What tricks close the gap

Concrete optimizations found in public literature that apply to Kaliski:

### 1. Merge step3_cswap and step9_cswap into one
Per iter we do 2× (u,v)-cswap and 2× (r,s)-cswap at roughly 6.4n CCX
combined. HRSL Fig 6b's swap-based formulation implies **one cswap pair
at iter entry + one at iter exit = same 2 cswaps**. But if we can fuse
the incoming swap at round k+1 with the outgoing swap at round k (when
the same `a_f` carries over via m_hist[k] ⊕ m_hist[k+1]), we could save
one cswap per iter.

### 2. Eliminate `add_dummy` / `b_f` tracking (minor)
We already drop step0_eqzero in bulk_prefix3 (saves ~n per iter). Can
further merge step1/step5 flag tracking into single ccx + measurement
uncompute. Saves ~0.5n per iter.

### 3. Reduce r-doubling cost
`mod_double_inplace_fast` costs ~n CCX (constant add for Solinas
correction). For the first 255 iters we use `mod_double_no_corr` (0
CCX). For the remaining ~150 iters we pay ~n each = 38k per inversion.
Trick: delay the Solinas correction to a single bulk correction at the
end of Kaliski (like Kim 2026's "postponed modular reduction") —
possible but requires widening r from n to n+k bits transiently.

### 4. Reduce step4 width via iter_idx bounds more aggressively
Our step4 is ~6.4n/iter. At iter 0, r and s have bit-length 0 and 1, so
the cond-sub/add can be done on width 1-2 bits, not n. We already
truncate via `transform_width`, `load_width`, `add_width`, `sub_width`.
Can we truncate harder? In bulk prefix (first 255 iters), u and v have
bit-lengths bounded, so the cond-sub on v-=u only needs width bit-len(u)
not full n. Currently we use full n. Could save up to 50% on step4 in
bulk.

### 5. Fewer iters via tighter BY-style bound
Our pair iters = 407. Classical Kaliski termination probability → 1 at
~256 iters for most inputs. If we run 350 iters + a Bernstein-Yang style
check that covers the rare tail, we could cut iters to 350 with
~acceptable classical failure rate (which Shor's algorithm tolerates
per the paper ~1% wrong point-adds is fine).

## Path to 2.9M Toffoli (below Google's 2.7M by ~7%)

Combine:
- Trick 3 (r-doubling Solinas postponement): -38k
- Trick 4 (step4 width truncation in bulk): -400k to -800k
- Trick 5 (iters 407 → 350): -330k across 2 inversions

Total: **-0.8M to -1.2M**. From 4.18M → 3.0-3.4M. Approaches SOTA.

## Path to true SOTA (2.1M)

Additionally need a 33% cheaper per-iter Kaliski (15n → 10n). Only
achievable via:
- **Algorithm 2 of Kim 2026** (unconditional execution + Montgomery form)
  — saves flag bookkeeping but pays for wider r register during loop
- **Luo 2025 register sharing** — but Luo's arithmetic per step is more
  complex, overall Toffoli higher

No public reference implementation hits Google's 2.1-2.7M per PA at ≤2500q.
Google's circuit is genuinely novel beyond what the published literature
shows.

## Practical plan

Rather than try to reinvent Google's witheld circuit, pursue the
publicly-reachable frontier: ~3.0M Toffoli at current 2716q. Then push
qubits down separately.

## Literature tricks found during deep-dive (April 24)

### Trick A: Coset representation of modular integers (Zalka / Gidney-Ekerå 2021)
Encode `k mod N` as superposition `Σ|jN+k⟩`. Non-modular adders perform
approx-modular addition in **4n Toffoli** vs **10n** for modular adder
(2.5x speedup). Padding cost O(log n).

**Blocker**: changes the semantics of register contents. Our test harness
checks `get_register(.) == expected mod p`, but coset-encoded registers
hold `k + jN` for a specific j per shot. Need either:
- harness modification to treat classical basis states as coset samples and
  check `get_register % p == expected`, or
- switch to coset only for subroutines that do many internal additions
  (inversion inner loop), not at the I/O boundary.

### Trick B: Windowed classical-quantum addition (Gidney 2019)
For each `q_reg += k` where `k` is a classical constant, group k bits of
control qubits and do one QROM lookup of precomputed `k1*P + k2*Q + ...`
followed by a single non-controlled add. Saves factor `w` over iterated
controlled adds.

**Applicability**: in our build, `mod_add_qb(tx, ox, p)` does a controlled
add of classical bit register. If we fuse k of these, we'd save 3x+.

### Trick C: Oblivious carry runways (Gidney-Ekera 2021)
Split an n-bit adder register into pieces with "runways" where carries are
clipped. Turns n-bit addition into ~n/k + log pieces. Needed for parallel
execution but also reduces ancilla footprint.

### Trick D: Approximate point addition (Proos-Zalka)
Ellliptic-curve point add needs to yield correct result with high prob
(~99%) not exactly, as long as total error budget across Shor's algorithm
is < 1%. Google's paper explicitly uses this. That means we could drop the
`exceptional cases` (flag f1, f2, f3, f4 in Litinski step 6) entirely,
saving ~5n Toffoli per point-add. Our current code already largely ignores
exceptions.

### Trick E: Signed-window signed-point-add (HRSL)
Window size 2^w with sign bit. Half the table size. Already assumed in
Google/Litinski/HRSL.

## What combining them gives

Per Gidney-Ekera 2021, **coset + windowed = 24n³ Toffoli / lg²(n)** for RSA
mod-exp at n=2048. For our point-add, analogously:
- Coset saves 60% on modular addition (4n vs 10n).
- Windowed saves factor ~w=16 on controlled-adds within inversion.

If both are applied to Kaliski: per-iter cost drops from ~15n to ~6-8n.
Total: 2 inversions × 2n iters × 7n CCX/iter = 28n² ≈ **1.8M Toffoli for both
inversions combined**. Plus muls/sq/affine: ~500k. **Total ~2.3M per
point-add**. Achievable SOTA ballpark.

But coset rep needs harness and entire-build change. Windowed classical-q
add needs QROM lookup infrastructure.

## Action plan

**Path A (pragmatic)**: Micro-optimize current build to ~3.5M:
- iters 407→402 (−28k, proven)
- Aggressive step4 width truncation in late iters (−~40k)
- Step3_cswap + step9_cswap merge (HRSL Fig 6b, −~500k if viable)

Estimated: 3.5M @ 2716q. Still above SOTA but a clear step forward.

**Path B (architectural)**: Build coset-representation infrastructure
behind a feature flag. Requires harness upgrade. Rough target: 2.5M @
2800q. Substantial rewrite, takes multiple sessions.

**Path C (structural)**: Build windowed-point-add infrastructure. Requires
QROM lookup tables. Not directly applicable to single-point-add benchmark,
only to full Shor loop.

Going with Path A for near-term progress.
