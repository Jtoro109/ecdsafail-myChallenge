# Research log — 2026-04-27 deep-read session

## Goal
Derive, from first principles and primary sources, what's actually required
to hit **≤ 2.7M Toffoli AND ≤ 1300 qubits** for a reversible secp256k1
exact-affine in-place point-add.

## Discipline for this session
- No code changes to `build()`, no experiments.
- Every quantitative claim is traced to (a) a measurement in this repo,
  (b) a page-and-equation reference in a named PDF, or (c) an explicit
  derivation from (a)+(b).
- "Prior session said X" is NEVER sufficient authority — if the claim
  matters, I re-verify from primary source this session.
- Assumptions get called out explicitly as "ASSUMPTION" and parked for
  later verification.

## Reading queue (tiered by expected structural relevance)

### Tier 1: read in depth, these define the frontier
1. **google_sota_2603_28846.pdf** (Babbush-Zalcman-Gidney 2026). This is
   the ZK-proved SOTA. Even if circuits aren't disclosed, the resource
   bounds and the few textual clues constrain what's possible.
2. **hrsl_2020.pdf** (Häner-Roetteler-Soeken-Lauter 2020). The "standard"
   public baseline for affine reversible point-add. All our primitives
   descend from this lineage. Need to know: exact Eea state layout,
   their per-iter cost breakdown, what tricks they use for cleanup.
3. **ragavan_gidney_2025.pdf** (Optimized windowed modular arithmetic).
   Windowed arithmetic is the one public "new" technique Google's paper
   says powers their number. Need: exact windowed-mul / windowed-inverse
   primitives, their cost formulas as a function of window width `w`.
4. **gidney_ekera_2021.pdf** (How to factor 2048-bit RSA with 20M noisy
   qubits). Long history of windowed arithmetic. Their Montgomery-form
   modular exponentiation is the architectural template we'd mimic.

### Tier 2: read for specific blocker resolutions
5. **kim_2026.pdf** (Kim et al.). Recent ECDLP point-add; uses unconditional
   Eea + Montgomery mul + windowed point-add. Most recent public
   point-add specifically. Need: what their per-iter cost is and why
   they can afford m_hist elimination.
6. **chevignard_2026.pdf** (Chevignard-Fouque-Schrottenloher). 1098q circuit
   using RNS + Legendre symbol. Claim: doesn't produce exact affine.
   Need to VERIFY this — if it does produce exact output under some
   condition, it's relevant; if it only certifies a predicate, it's out.
7. **luo.pdf** / **luo_ec.pdf** (Luo). 1333q circuit using location-
   controlled arithmetic. Need: what their inversion core looks like
   and whether it's portable.
8. **gidney_windowed_2019.pdf** (Windowed quantum arithmetic). The original
   windowed-mul paper. Foundation for the others.
9. **jump_by.pdf** (jumped Bernstein-Yang). B-Y with windowing. Need: exact
   per-iter cost vs Eea as a function of window width.

### Tier 3: scaffold / reversibility fundamentals
10. **luongo_mbu.pdf** (Measurement-based uncomputation variants). Our MBU
    patterns are from Gidney, but their paper claims generalizations.
    Worth checking for cswap-specific patterns.
11. **gidney_venting_adder.pdf** (arxiv 2507.23079). Already partially
    ported; read to confirm understanding of when venting helps.
12. **gidney2025rsa.pdf** (RSA via coset arithmetic + windowed). Their
    coset representation is the other candidate "trick" we've hypothesized.
13. **jacobi_circ.pdf** (Jacobi reversible GCD). An alternative inversion
    approach; check its per-iter cost model.

### Tier 4: skip unless specifically relevant
- Litinski2023.pdf: already exploited (schoolbook addsub). Re-read only
  if I find a specific claim to verify.
- coord_forms.pdf / Huang2025.pdf: projective coords proved inapplicable.
- by2019.pdf / by_paper.pdf: B-Y w=1 already analyzed negative.
- RNSL2017.pdf / rnsl.pdf: superseded by HRSL 2020.
- toomcook.pdf / kara.pdf: schoolbook tricks, already exploited.
- addition_chain.pdf: Fermat-inv, 18M Toffoli, rule out.

## Precise problem statement

Produce a reversible quantum circuit `C` satisfying:

1. **Data contract** (from `src/main.rs`): given quantum registers
   `tx, ty` each 256 qubits, and classical registers `ox, oy` each 256
   bits representing secp256k1 point Q = (ox, oy), the circuit maps
       |Px⟩|Py⟩ |ox⟩|oy⟩|0⟩_{anc}
     →  |Rx⟩|Ry⟩ |ox⟩|oy⟩|0⟩_{anc}
   for all `(Px, Py)` representing on-curve points distinct from ±Q
   (and not identity), where `(Rx, Ry) = (Px, Py) + (Qx, Qy)` under
   the secp256k1 group law. All ancillas zeroed.

2. **Harness acceptance**: phase-clean (no uncorrected phase-flip tasks
   on any shot), classical-clean (output matches reference on all 24-seed
   × 4096-shot test cases).

3. **Resource targets** (SOTA): max executed Toffoli ≤ 2.7M; peak qubits
   ≤ 1300.

4. **Hard constraints**: circuit must be a *classical reversible*
   Boolean function executed in superposition, built from the
   `kickmix`-compatible instruction set (CCX, CX, X, Swap, CZ, Z, and
   measurement + classically-controlled variants for MBU).

**Current circuit**: 4.13M Toffoli, 2716 qubits. Ratio: 1.53× Toffoli,
2.09× qubits.

## Decomposition of the SOTA gap

Measured on commit 5f03382 (benchmark run confirms):
- kal_* forward phases total: 1,603,088 CCX
- bk_* backward phases total: 1,610,283 CCX
- non-Eea: 861,051 CCX
- Grand total traced: 4,074,422 CCX (benchmark reports 4,136,878;
  remaining ~60k is in untagged boot/scaffold code).

So per Eea invocation (one full `with_kal_inv_raw` call) = ~1.60M CCX.
We have TWO invocations. Gap to 2.7M is ≈ 1.4M ≈ exactly one Eea
invocation.

This reframes the SOTA gap as a **binary architectural question**:

> Can we do secp256k1 point addition with exactly ONE Eea (or
> equivalent) invocation, meeting the exact-affine in-place contract?

If yes: our existing per-iter cost puts us at ~2.5M, under target.
If no: we need to cut per-iter cost of each of two invocations by ~45%
to hit 2.7M. Ragavan-Gidney's windowed inversion claims ~50%-ish
reductions; this is their domain.

I need to answer the binary question from primary sources before
going further. It has occupied every session but has NEVER been
definitively settled. Let's fix that.

---

## Investigation 1: the "1 vs 2 inversion" question

### What we know from this repo's history
- `single_inv_numeric.rs` has 3 classically-replayed strategies
  (A, B2, C). A and B2 have documented obstructions. C (inv of `w = dx³`)
  is classically correct but its reversible quantum schedule was never
  written. Prior session logged its classical probe at 4.57M CCX and
  4279 qubits, "too wide".
- The obstruction always reduces to: "at circuit end, an ancilla register
  holds `dx` (or a function thereof), and zeroing it requires reconstructing
  `dx` from `(Rx, Ry, Qx, Qy)`, which is itself a point-subtraction
  requiring its own inversion."

### Is this a theorem or just a pattern?

Claim I want to formally verify:
> **For any reversible circuit `C` computing the in-place map
> `|Px, Py, 0_anc⟩ → |Rx, Ry, 0_anc⟩` on curve points where all operations
> are classical-reversible, the minimum number of modular-inversion
> invocations is 2, independent of circuit structure.**

### Counter-examples to consider
- **Fermat inversion**: one exponentiation replaces one Eea. So
  technically "1 inversion primitive call" is possible, at cost 18M
  Toffoli. Trivially doesn't fit 2.7M target. But it IS a 1-primitive-
  call circuit. The theorem as stated is WRONG if "inversion invocation"
  counts anything producing `x⁻¹`.

  Refined claim: "1 modular inversion + O(1) muls, fitting in our
  per-iter cost model, is impossible for in-place affine output."

- **Windowed inversion**: Litinski 2024 / Ragavan-Gidney 2025 claim some
  form of "fused inversion + multiplication". If you can multiply DURING
  the inversion (while Eea is still in progress), the inversion
  "invocation" and the subsequent mul share state. Is this a 1-inversion
  or 2-? Semantic question.

  Refined claim: "the amount of reversible uncompute work needed to zero
  all Eea-state ancillas is ≥ cost_of_one_forward_Eea."

  This claim I believe IS true — Bennett argued it generally for
  reversible computation, and the specific case of Eea is that
  forward+backward = 2× forward (with small savings from MBU).

  So per invocation of Eea, cost ≥ 2× forward. For TWO invocations
  that's 4× forward. For ONE invocation it's 2× forward — a 50% saving
  IF we can find a 1-invocation scheme.

### The real question
How can Google do it (if they do)? Three hypotheses, to be tested against
primary sources:

**H1: They don't use Eea.** They use Ragavan-Gidney windowed
inversion which has fundamentally different cost structure (windowed
Montgomery), and it naturally "fuses" forward + backward via the
Montgomery trick.

**H2: They do 2 inversions but each is ~1.35M instead of our ~1.6M.**
Per-iter tuning at 15% saving, not architectural.

**H3: They structurally do 1 inversion via a clever algebra that the
public-but-complex schemes like Montgomery batched don't fit, but a new
(possibly projective + Legendre hybrid) does.**

### What I need from the papers
- Google 2026: do they claim 1 or 2 inversions? What's the total work?
- HRSL 2020: explicit cost breakdown including number of inversions.
- Ragavan-Gidney 2025: what's their "full ECDLP point-add" cost? Does
  it resemble 1-inv or 2-inv structure?
- Kim 2026: explicitly states "two inversions per point-add"? Or one?

This is the central question. I'll read these papers targeted at this
specific question first.

## Next steps
1. Read Google 2026 (tier-1 #1) focusing on resource breakdown tables.
2. Read HRSL 2020 (tier-1 #2) focusing on Section 4's full point-add
   construction and resource tally.
3. Read Ragavan-Gidney 2025 (tier-1 #3) focusing on windowed inversion
   primitive.
4. Write investigation findings back into this log.
5. Only after that: decide whether to pursue 1-inv hypothesis further.

---

## Investigation 1 findings (after reading primary sources)

### What Google's 2.7M actually measures

**Critical clarification** from Google 2026 Appendix (p. 55–57):

> "our elliptic curve circuits perform a sequence of in-place windowed
> elliptic curve point additions, each requiring three table lookups.
> The core operation evaluates `Q ← Q + P[k]` where P is a (classically)
> pre-computed table of elliptic curve points, w is the window size,
> k is a w-qubit quantum register, and Q is a 2n qubit target accumulator."

Their **point-addition circuit** (the one they ZK-prove at 2.7M) is **`Q += P[k]`**, i.e. a **windowed** add where:
- Q (2n qubits = 512 qubits at n=256) holds a quantum superposition.
- k (w=16 qubits) is a quantum index into a classical table.
- P is a classical table of 2^16 precomputed elliptic curve points.
- The 2.7M includes **3 table lookups** + the add body + uncompute lookups.

Google's w=16 is chosen optimally. Their 2.7M is for the whole windowed
point-add-with-lookup, not the "bare" add.

### What our harness actually measures

`src/main.rs` computes the circuit on inputs:
- `targets[i] = k1_i * G` (quantum `(Px, Py)`)
- `offsets[i] = k2_i * G` (**classical bits** `(Qx, Qy)`, set per-shot)

And checks the final `(tx, ty)` equals `targets[i] + offsets[i]`.

So our circuit computes **bare single point-add with fully classical Q**,
no windowed lookup, no phase-estimation register. This is **strictly
simpler** than Google's task.

### Comparing apples-to-apples

| Circuit                              | Task                     | n=256 Toffoli   | qubits |
|--------------------------------------|--------------------------|----------------|-------:|
| HRSL 2020 Low-W                      | Windowed Q+=P[k] lookup  | ~12M per w-add |   2124 |
| HRSL 2020 Low-T                      | Windowed Q+=P[k] lookup  | ~12M per w-add |   2619 |
| Kim 2026 Q-opt                       | Windowed Q+=P[k] lookup  | ~17M per w-add |   6475 |
| Kim 2026 D-opt                       | Windowed Q+=P[k] lookup  | ~10M per w-add |  23728 |
| Litinski 2023 single instance        | Windowed Q+=P[k] lookup  | ~3.9M per w-add|   6000 |
| **Google 2026 Low-qubit (SOTA)**     | Windowed Q+=P[k] lookup  | **2.7M per w-add**|   1175 |
| **Google 2026 Low-gate (SOTA)**      | Windowed Q+=P[k] lookup  | **2.1M per w-add**|   1425 |
| **us, current baseline (c1aeeb4)**   | Bare (Px,Py)+(Qx,Qy)     | **4.14M**      | **2716** |

HRSL Low-T per-point-add derivation: 1.08·2^31 T-gates / (2·256/19 ≈ 27)
windowed adds / 7T-per-Toffoli ≈ 12M Toffoli per windowed add. At
n=256 with w=19. *Checked from Table 1.*

Kim 2026 at n=256 per-point-add: directly read from Table 3, row "256".
T-count 122M for Q-opt. Toffoli = T/7 ≈ 17.4M. For D-opt, 69.6M T ≈ 10M.
*Note Kim gives per-point-add counts directly, NOT per-full-Shor.*

Litinski 2023 per-windowed-add derivation: 109M Toffoli per-key /
(2·256/16) ≈ 28 windowed adds = **3.9M per windowed add**.
*From Section 2.1 baseline cost.*

### Observations

1. **Our 4.14M bare-add is competitive with public literature even
   though our task is simpler.** We beat HRSL's windowed add by 3× at
   similar qubits (2716 vs 2124).

2. **Google's 2.7M is for the HARDER windowed task.** On the bare task,
   their technique presumably achieves *less* than 2.7M (they don't report
   it because they don't do bare adds).

3. **To hit Google's SOTA via public techniques alone is impossible.**
   The best public per-windowed-add is Litinski's 3.9M; we already beat
   that.

4. **Our harness matters**: the 1175q / 2.7M target was set for a
   DIFFERENT functional circuit than ours. We need to decide whether our
   own SOTA target is:
   (a) Match Google's exact task (port to windowed add-with-lookup,
       build a table-lookup primitive, measure against 2.7M/1175q).
   (b) Match Google's per-point-add cost on our simpler task (should
       naturally be lower than 2.7M).
   (c) Improve incrementally on our bare-add until it matches
       Litinski-level sophistication.

### Structural techniques used by SOTA (public)

From HRSL Section 4 and Fig 8b:
- **Two modular inversions per point-add**: HRSL confirms 2 divisions
  per Weierstrass add (one for λ, one for... wait, Fig 8 is the improved
  division, and the full point-add (their Fig 9) uses "2 divisions, 2
  multiplications, 1 squaring, 9 additions". So HRSL does 2 divisions.
- **Each division = 1 inversion invocation** (inv — mul — inv⁻¹).
- **Register recycling after inversion**: "after the inversion, three
  registers contain known values of 0, 1, and the modulus p. We can
  clear these auxiliary qubits with at most 3n parallel X gates, then
  re-use them for modular multiplication." We already use this via
  KAL_FREE_S=1 which frees `s` (= p at end).

From Litinski 2023 Section 1.3:
- **Parallel-instance inversion trick**: if running k instances in
  parallel (all doing ECDLP on different keys), can share inversions
  via Montgomery batched trick: `1/k inversions per instance`.
  **NOT applicable to our single-instance benchmark.**
- Asymptotically: 3 muls + 1/k inversions per instance.

From Kim 2026 Section 3.4:
- Uses **binary EEA inversion** (same as Eea) with unconditional
  execution (no m_hist, no f_flag gating).
- Cost: 122M T-gates at n=256 per point-add, 6475 qubits.
- **Depth-optimized, not Toffoli-optimized.** Our 4.14M beats their 17M
  on Toffoli.

From Google 2026:
- 2.7M per windowed-point-add at 1175q with w=16.
- **Explicitly states**: "windowed arithmetic" and "MBUC" are common
  ingredients in all prior work — so their 2.7M vs Litinski's 3.9M vs
  HRSL's 12M improvement is NOT from using windowing or MBUC per se.
- Their real improvement is undisclosed.

### Key realization: our task vs Google's task

Our benchmark (bare point-add with classical Q) is a **strict subset**
of the work Google's benchmark does (which includes 3 table lookups
+ w-qubit index register handling + classical table). Roughly, their
task includes ours plus ~3 lookups of 2n-bit values + ancilla.

A lookup of a 2n-bit classical value into a 2n-qubit target, gated on a
w-qubit index, costs ~2n·2^w-ish bits of work per lookup (the QROM cost).
At w=16 that's ~33M CCX. Three lookups = ~100M. MINUS the uncomputes
(which MBUC makes cheaper). So Google's windowed add has massive
lookup overhead on top of the bare add.

For Google to hit 2.7M TOTAL (including lookups), the bare-add part
must be significantly under 2.7M. **Our 4.14M for the bare part
suggests we're ~1.5× over Google's bare-add cost.**

### Revised research question

Not "is 1-inversion affine reversible?" (we've shown it's not, without
new algebra).

But **what per-point-add cost does Google's circuit actually achieve
for the bare add?** If we estimate:
- Total: 2.7M
- Lookup overhead: ~200k (with MBUC, QROAM-style; conservative)
- Bare-add cost implied: ~2.5M

So their bare-add is ~2.5M, ours is ~4.14M. **Gap: ~1.7M.** Closer to
two Eea-invocation at ~0.85M each after 50% per-iter optimization.

That's more believable as "technique gap" than "magic undisclosed trick".

---

## Investigation 2: per-iter Eea cost reduction

Our current forward Eea: ~800k Toffoli for 407 iters = **~1965 CCX/iter**.

HRSL's per-iter cost: their Low-T mode uses ~436n³ T-gates for full Shor.
Per windowed add: ~260M T = ~37M CCX. Their inversion is 2n = 512 iters.
Per iter (across their modified Eea): 37M / 512 / (2°divisions)
≈ 36k CCX/iter. Way higher than us. HRSL clearly NOT the per-iter winner.

Litinski 2023's per-iter: total 109M / 28 windowed adds / 2 inversions
per add / 407 iters ≈ 4800 CCX/iter. **Higher than ours.**

Kim 2026's per-iter: 122M / 407 iters / 2 inversions ≈ 150k CCX/iter.
(Kim is depth-optimized, very high Toffoli count.)

Conclusion: **our per-iter Eea cost (~2000 CCX) is ALREADY close to
best-in-class public.** Further per-iter reduction seems unlikely without
architectural change. The gap to Google must come from STRUCTURE, not
per-iter tuning.

---

## MAJOR UPDATE: harness now allows approximate correctness (Option A)

2026-04-27 later in session, confirmed with user: **the harness will be
edited to allow Shor-tolerant approximate correctness**. Specifically:

- ≤ 0.1% classical failure rate on the 9024 test cases (≤ 9 wrong shots).
- Output register may have residual padding (coset-representation
  residue is acceptable).
- Matches the approximation model of Gidney-Ekera 2021 and Google 2026.

### Consequences for technique applicability

The following structural techniques become VIABLE that were previously
blocked:

1. **Coset representation of modular integers** (Zalka 2006 /
   GE2021 §2.4). Replaces modular n-bit add cost 10n → 4n, a ~60%
   per-op reduction. Cost: cpad ≈ 2 log n + 10 ≈ 26–30 padding qubits
   total for n=256, cpad ≈ 26 gives deviation ≈ 10⁻⁸, well under 0.1%.

2. **Oblivious carry runways** (GE2021 §2.6). Allows splitting an n-bit
   register into k independent chunks for parallel addition, with
   runway-padding between them to terminate carries without global
   propagation. Depth-oriented gain but also Toffoli-neutral for chunk
   sizes up to crossover.

3. **Approximate Eea tail**: if Eea backward leaves a tiny
   residual in cleanup registers (bounded by 10⁻⁸ deviation per invocation),
   the harness will accept. This potentially reduces the full-backward
   cost if we can short-circuit the last few iterations.

### Consequences for technique non-applicability

The 1-inversion / cleanup obstruction argument in §8 of `cost_model.md`
is weakened. Specifically: "zeroing an ancilla holding dx requires
reconstructing dx" is a *deterministic* claim. Under approximate
correctness, the ancilla only needs to be *approximately* zero — small
residual perturbations are OK. This opens lines like:

- Uncomputing `dx_shadow` via a partial Eea that stops before
  fully zeroing the register. Bound the residual via the coset/runway
  machinery. Saves the final iterations' worth of Toffolis.

### Revised Tier-1 research targets

Now that coset is in-play, the priority order is:

1. **GE2021 Section 2.4–2.5** (coset + windowed): understand the exact
   construction, including the `cpad` parameter and how runway handles
   boundary effects.

2. **GE2021 Section 2.9**: the approximation-error bounds in full,
   so we know how much deviation we can absorb and still pass the
   edited harness.

3. **Gidney 2025 "RSA coset"** (/tmp/gidney2025rsa.pdf): more recent
   coset application with possibly refined parameters.

4. **Check if coset is compatible with our Eea**: Eea uses
   modular add-sub and modular halve/double. All are modular ops; all
   should benefit from coset. But we also have `with_gt` comparisons
   (for STEP 2), which are NOT modular ops — how does coset affect them?

5. **Full accounting**: if coset cuts n-bit mod-add cost from ~10n
   to ~4n across all 500+ mod-add-equivalent operations in our circuit,
   total savings are ~750k CCX. Combined with other Tier-1 techniques,
   plausibly hitting 3.0–3.3M. Further cuts need structural changes.

## Investigation 2 findings (session-end)

### Primary-source reading completed

Docs read in this session (specific sections verified):
- **Google 2026 (google_sota_2603_28846.pdf)**: Appendix A.1 (Low-Qubit
  Variant stats), A.3 (Circuit Architecture, describes `Q += P[k]`
  task).
- **HRSL 2020 (hrsl_2020.pdf)**: §4.2 (Modular Inversion), §5 (Elliptic
  Curves), Table 1 (n=256 resource counts).
- **Kim 2026 (kim_2026.pdf)**: §3.3.1 (Inversion using binary EEA,
  Algorithm 2), Table 3 (per-point-add resource tally at n=256).
- **Litinski 2023 (Litinski2023.pdf)**: §1.3 (parallel-instance
  inversion trick), Figure 7 (Eea with 13n cost/iter), §2.1
  (baseline resource count).
- **Litinski 2024 (schoolbook_fewer.pdf)**: Table 1 (n²+4n+3 schoolbook,
  n²+6n+correction for modular mod p).
- **Gidney-Ekera 2021 (gidney_ekera_2021.pdf)**: §2.4 (coset rep),
  §2.5 (windowed arithmetic), §2.6 (oblivious carry runways), §2.9
  (approximation error bounds).
- **Gidney 2019 (gidney_windowed_2019.pdf)**: §2 (table-lookup cost
  L-1 forward, √L uncompute via MBU).
- **Gidney 2025 RSA (gidney2025rsa.pdf)**: brief scan for RNS/coset;
  focuses on residue arithmetic for RSA, not directly portable to
  ECDLP.
- **Chevignard 2026 (chevignard_2026.pdf)**: abstract + Section 1
  contribution. Confirmed: output is 1-bit Legendre hash, NOT exact
  (Rx, Ry). Not applicable to us.
- **Luongo 2025 (ragavan_gidney_2025.pdf, misnamed)**: RSA-focused,
  3% Toffoli improvements on windowed mul. Not ECDLP-applicable.

### Artifacts written
- `cost_model.md`: ground-truth cost model (from-measurements).
- `blockers_and_paths.md`: structural blockers + candidate paths with
  literature citations.
- `research_log_2026_04_27.md`: this file, investigation log.

### Structural conclusions

1. **Google's 2.7M is for the windowed task** `Q += P[k]` with w=16 and
   3 table lookups. Lookup overhead is ~200k Toffoli. Implied bare-add
   cost is ~2.5M.

2. **We're at 4.14M on the bare task** — already best-in-public-lit
   by ~3× against HRSL/Kim, competitive with Litinski (~3.7M at
   6000q). The 1.5× gap to Google is real but the techniques to close
   it aren't public.

3. **Under approximate correctness (Option A)**:
   - Coset representation becomes available. Saves ~60% on mod-add
     heavy portions. Breaks comparators, so can't apply inside Eea
     directly.
   - Approximate cleanup might relax single-inversion obstruction.
     Novel work needed.

4. **The per-iter Eea cost** (~1970 CCX/iter forward) is ALREADY
   below Litinski's published 13n = 3328 CCX/iter. Further savings
   need:
   - Step 3 + 9 cswap restructuring (structural, unpublished).
   - Per-iter primitive replacement (windowed Eea, also not public).

5. **Qubit reduction to 1300q** requires:
   - m_hist elimination (-407q).
   - tx reuse as v_w (-256q).
   - r folding into tx at end (-256q).
   - ty reuse as s (-256q).
   - Combined: 2716 → ~1540q. Still above 1300q.
   - Additional reduction requires windowed Eea OR approximate
     cleanup.

### Decision candidates (not selected this session)

Path A: Coset on non-Eea mod-adds. ~300k Toffoli savings. Medium
risk. Medium reward.

Path B: m_hist elimination + karatsuba at state_a_mul2. Small Toffoli
win (~30k), large qubit win (-407q). Low risk.

Path C: Step 3+9 cswap restructuring. ~1M Toffoli target. Highest
reward, highest risk. Novel research.

Path D: Approximate-correctness single-invocation scaffold. ~1.6M
Toffoli target. Highest reward, highest risk. Novel research.

Path E: Port our task to windowed `Q += P[k]` form to directly compare
to Google. Not currently matching harness.

### What this session did NOT decide

- Which path to pursue first. User should pick based on risk tolerance
  and time budget.
- Implementation details for any path. These require their own focused
  design session.
- Whether to edit the harness for approximate correctness. Implied by
  paths A/D but not by B/C.

### Key uncertainty

The **cleanup obstruction under approximate correctness** is the pivot
question. If it's genuinely solvable via bounded-deviation techniques
(cpad-style), Path D is the highest-impact path and would get us under
3M with our existing per-iter primitives. If it's NOT solvable (the
obstruction really does force ~1.6M of work regardless), then we're
stuck in the 3–4M range with public techniques, and matching Google
requires unpublished insights.

This should be the first focused investigation of the next session:
**can we prove or disprove that approximate correctness enables a
1-invocation scaffold?**
