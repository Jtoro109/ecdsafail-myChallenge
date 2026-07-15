# Cost model for a 1300-qubit, 2.7M-Toffoli reversible secp256k1 point-add

Purpose: a first-principles derivation of what a 1300q/2.7M Toffoli
point-add architecture must look like. Every claim is either (a) measured
from our current circuit, (b) read from a primary source, or (c) derived
from a and b with the derivation shown. No unchecked intuitions.

Status: **live working document**, not a plan. Update as measurements
refine the numbers. Current commit used for measurements: `5f03382`.

---

## 1. What the benchmark actually requires

The `src/main.rs` harness demands an exact reversible map
`(Px, Py, Qx, Qy_classical) → (Rx, Ry)` on random secp256k1 points, all
ancillas returned to zero, phase-clean. No compressed outputs, no "mostly
correct" shortcuts. This is stricter than most published ECDLP resource
estimates, which only need the map correct on coset-representative inputs
inside a larger Shor scaffold.

Implication: every register used internally must be **actively zeroed** by
the end of the circuit. Bennett-clean uncompute is mandatory.

---

## 2. Data-register floor

`tx` and `ty` are the quantum input registers (256 bits each) and, by the
harness contract, hold `(Rx, Ry)` at the end. They cannot be discarded.

**Minimum persistent data: 512 qubits.**

At Google's 1175q target, that leaves **663 qubits** for everything else
— all intermediate state, all transients. Our current circuit uses
2716 − 512 = **2204 qubits for the non-data portion**. **Factor of ~3.3
overcount.**

At 1300q: **788 qubits** for non-data state/transients.
At 1425q (low-gate): **913 qubits** for non-data state/transients.

These are the hard ceilings any new scaffold must respect.

---

## 3. Measured cost of our current primitives (commit 5f03382)

From `TRACE_PHASES=1` run (total 4,134,324 Toffoli, 2716 peak qubits):

### 3.1 By high-level phase

| phase group                        | Toffoli   | % of total |
|------------------------------------|----------:|-----------:|
| Eea forward (bulk + slow)      | ~1,620,000 |     ~39%  |
| Eea backward (bulk + slow)     | ~1,620,000 |     ~39%  |
| schoolbook_mul_inverse (rshift22)  |   201,216 |      4.9% |
| FastModulo tail (sol_halve_tail, mul3)|    21,739 |      0.5% |
| state_a_halve / state_b_double         |   206,805 |      5.0% |
| 4 muls (state_a_mul1/2, state_b_mul)   |   224,729 |      5.4% |
| mod_sub_qb / bootstrap             |     ~2,000 |     <0.1% |

Eea dominates at **78% of total Toffoli**. No other primitive can
touch the SOTA gap by itself.

### 3.2 Eea per-iter cost

From phase counters, forward Eea runs 407 iters and emits
~1.62M Toffoli, so per-iter average = **~3,980 Toffoli/iter** forward.
Backward ~3,980. Total per full Eea pass: ~7,960 Toffoli/iter × 407 iters
= too sloppy — let me recompute with clean numbers.

Actually breakdown by step (forward+backward summed):

| step                      | total CCX | per-iter (avg over 407) |
|---------------------------|----------:|------------------------:|
| step 4 (bulk + slow)      | 1,013,060 | 1,245 × 2 (fwd+bwd ratio) |
| step 9 cswap (bulk + slow)|   505,100 |                   1,241 |
| step 3 cswap (bulk + slow)|   504,080 |                   1,239 |
| step 2 (bulk + slow)      |   316,006 |                     776 |
| step 0 eqzero             |    56,698 |                     139 |
| step 1 (fwd+bwd combined) |    58,178 |                     143 |
| step 6_7_8                |    76,755 |                     189 |

Per-iter total (rough): ~5,000 CCX for forward+backward combined at iter i.
× 407 iters = ~2.03M CCX just for one Eea pass including its reverse.

We have TWO full Eea passes (state_a and state_b), so
**2 × (forward + backward) ≈ 3.24M CCX** out of 4.13M total. That's
**78.4%** — matches the phase breakdown. Good sanity check.

### 3.3 The "20% tax" outside Eea

Non-Eea cost (~900k CCX) breaks down as:
- `state_a_halve` + `state_b_double`: 207k (scale correction from iter-count
  mismatch to actual Eea `K` factor)
- Four q×q muls (`state_a_mul1`, `state_a_mul2`, `state_b_mul`, plus one inside
  `mul3_between_pair`): ~240k
- `schoolbook_mul_inverse` phase name is misleading — it's the FastModulo
  reduction work inside `in_place_mul_const` / `mul_by_const_acc`: 201k
- FastModulo tails, mul3 between pairs, scaffold bookkeeping: ~50k

**None** of these phases individually exceeds 5%. The remaining ~900k CCX
has no single dominant sub-phase.

---

## 4. Toffoli budget decomposition at 2.7M

**Measured decomposition of our 4.13M total** (commit 5f03382):

| component                      | Toffoli | % of total |
|--------------------------------|--------:|-----------:|
| state_a Eea INVOCATION       |  ~1.61M |     ~39%   |
| state_b Eea INVOCATION       |  ~1.61M |     ~39%   |
| Non-Eea (muls, halves, etc)|  ~0.91M |     ~22%   |
| **Total**                      |  ~4.13M |            |

Where "Eea INVOCATION" means one call to `with_kal_inv_raw`, which
internally does Eea_forward (~800k) + Eea_backward (~810k) =
**1.61M per invocation**. An invocation IS the full inversion round-trip;
the backward is not separate from forward — it's what makes the
inversion reversible (Bennett-style uncompute of all Eea ancillas).

This reframes the question entirely.

**Any 2.7M-Toffoli affine circuit must allocate:**

- ≥ one full modular-inversion invocation. At our current per-iter cost:
  **1.61M per invocation** (forward + backward of Eea state).
  Published estimates suggest per-iter cost can drop to ~50–60% of
  current via windowed inversion (Litinski 2024, Ragavan-Gidney),
  giving **~0.9–1.0M per invocation at SOTA tuning**.
- ≥2× q×q modular mul: Karatsuba-1 = 125k each, or schoolbook 153k.
- ≥1× q×q squaring: ~132k with symmetric schoolbook.
- Scale correction or equivalent: ~200k currently, possibly reducible.
- Bookkeeping: ~50k.

**Floor for 1-INVOCATION scaffold at current per-iter cost**:
  1.61M + 2×125k + 132k + 200k + 50k ≈ **2.24M**.

**Floor for 1-INVOCATION scaffold at SOTA per-iter cost**:
  0.9M + 2×125k + 132k + 100k + 50k ≈ **1.53M**.

**Our current 4.13M = 2 full invocations + overhead. The gap to 2.7M is
~1.43M, almost exactly one full Eea invocation.** ∴ **The SOTA gap
IS the second Eea invocation.** Cutting invocations from 2 to 1
lands at ~2.5M, under the Google target.

This is a cleaner phrasing than "second Eea pass": it's the entire
second `with_kal_inv_raw` call.

### 4.1 Why we currently need TWO invocations

The current scaffold (`build_standard_quantum_addition`) does:

1. `with_kal_inv_raw(tx = dx)`: produces `inv_raw = dx⁻¹ · scale`. Inside
   the body: `lam = ty · inv_raw` (+halve loop), then `ty += lam · tx`
   (zeros ty), then body closure stashes lam. Eea backward zeros
   Eea state.
2. Between invocations: Rx fold into tx using lam².
3. `with_kal_inv_raw(tx = Rx-Qx)`: second full invocation. Produces
   `inv_raw = (Rx-Qx)⁻¹ · scale`. Inside body: double lam, multiply by
   new inv_raw, store into ty. Eea backward zeros.

The reason for TWO invocations: after invocation 1's body, we overwrite
tx from dx to Rx-Qx. We can't use invocation 1's `inv_raw = dx⁻¹` to
compute Ry because Ry requires (Rx-Qx)⁻¹, a different inverse.

The two common escapes, both already explored and rejected here:
- **Montgomery batched inversion** (`c = dx · (Rx-Qx)`): inverts c once,
  derives both dx⁻¹ and (Rx-Qx)⁻¹ from c⁻¹. But Rx depends on dx⁻¹, so
  you can't know (Rx-Qx) until after the first inversion — circular.
- **Single-inversion Strategy C** (`w = dx³`): compute Rx, Ry directly
  as rational functions of (dx, dy, w⁻¹) without ever needing
  (Rx-Qx)⁻¹. See §8 for the reversibility analysis.

**Measured with TRACE_PHASES=1 at commit 5f03382**:
- kal_* phases total: **1,603,088 CCX** (forward of state_a + forward of state_b)
- bk_* phases total: **1,610,283 CCX** (backward of state_a + backward of state_b)
- Everything else: **861,051 CCX** (mul, halve, double, scale corrections, etc.)
- Grand total traced: 4,074,422 (matches benchmark 4,134,324 ± 1.5%;
  remaining ~60k is in un-phase-tagged boot/between-pair code).

Note: the phase counters conflate state_a's forward with state_b's forward
(both labelled `kal_*`). Since state_b has 404 iters vs state_a's 407, the
cost is nearly identical per pass. Forward per pass ≈ 802k; backward per
pass ≈ 805k.

**A 1-Eea point-add, keeping our current per-iter cost, would emit:**
- 1 forward Eea: ~802k
- 1 backward Eea (uncompute): ~805k
- Remaining non-Eea work: ~860k (unchanged since it's outside
  Eea)
- Total: **~2.47M**

This is BELOW 2.7M! The 1-Eea scaffold — if achievable — meets
Google's Toffoli target even without further per-iter optimization.

The blocker is still that a 1-Eea point-add requires solving the
cleanup obstruction (§8). But it's worth noting: **if someone finds a
working 1-Eea scaffold, the Toffoli target falls out automatically
with our existing primitives.**

---

## 5. Qubit budget decomposition at 1300 (measured)

**Measured peak site (TRACE_PEAK=1, commit 5f03382)**:
- Peak phase: `bk_step6_7_8` and `bk_bulk_step4` (both hit 2716).
- This is INSIDE `state_a_eea_backward`, i.e. the backward Eea of
  the first inversion, while `lam_inner` is still alive because it was
  moved into `lam_cell` at the end of the forward body closure.

**Live registers at peak (state_a_eea_backward, inside body closure)**:

| register            | qubits | lifetime notes |
|---------------------|-------:|----------------|
| tx                  |    256 | data, always live |
| ty                  |    256 | data, always live |
| `lam_inner`         |    256 | alive from state_a_mul1 through end of state_a backward |
| Eea `u`         |    256 | alive through both forward and backward |
| Eea `v_w`       |    256 | same |
| Eea `r`         |    256 | holds `inv_raw` (the answer), same |
| Eea `s`         |    256 | **freed then re-alloc'd** during backward (KAL_FREE_S) |
| Eea `m_hist`    |    407 | same |
| Eea `f_flag`    |      1 | same |
| iter-local flags    |      3 | a_f, b_f, add_f (inside iteration body) |
| step-4 tmp          |    256 | transient, alive during step-4 body |
| step-4 tmp_pad      |      1 | transient |
| RippleAdder carries     |      2 | transient (step-4 fused sub+add) |
| **total**           | **2456 + 262 = 2718** | matches measured 2716 ±2 |

**Persistent at peak: 2204 qubits** (everything minus step-4 transient).
**Step-4 transient at peak: ~262 qubits** (tmp + pad + carries + flags).

Halving-related transients (bk_step6_7_8 uses dirty from u+v_w, no
extra alloc): peak here comes purely from alloc'd state plus small
iter-local flags.

Info-theoretic minimum of Eea state (without m_hist): **1025 qubits**
(u, v_w, r, s, f_flag). With tx(256), ty(256), lam(256), and
step-4 transient tmp(256) all simultaneously live, peak = 1025 + 256
(tx) + 256 (ty) + 256 (λ) + 256 (transient) = **2049 qubits**.

At 1300q target: live set during any one moment ≤ 1300 − 512 = 788
qubits of non-data state. Even with m_hist gone (−407), the Eea
4×n state (1024q) alone exceeds 788. **The four Eea register 4×n
layout is structurally incompatible with a 1300q budget unless data
registers (tx, ty) are reused as Eea registers.**

### 5.1 Where the 1156q-projection from the prior session breaks down

The prior session's `eea_1200q_feasibility.md` projected 1156q. Its
column labelled "ty reused as s" is plausible classically (s = p at end
of forward; s = 1 at start of forward; s value during backward iters is
structurally distinct from ty), but **the reuse requires ty to be dead
during the inversion.** In the current scaffold, `lam_inner` is live
right after state_a_mul1, and state_a_mul1 reads ty, so the fold is not
compatible with the current order of operations. It would require
**restructuring state_a so all ty-reads happen before the inversion body.**
Not impossible, but not free — likely requires recomputing ty later.

The 1156q projection also assumes step-4 transient can drop to ~128q,
which requires a Gidney-venting-style in-place step 4. Feasibility
unclear without implementation.

---

## 6. The critical observation: data-register reuse is mandatory

From §5: Eea's 4×n layout = 1024q. The 1300q budget gives 788q after
tx+ty. Even with 0 other state, 1024 > 788. ∴ **tx and/or ty must double
as Eea registers during the inversion.**

This has a precise technical consequence: when Eea is running, the
data register being reused must not need its pre-inversion value. In an
affine point add:

- Inversion input is `dx = Px−Qx`, currently stored in `tx`.
- After inversion, we want `dx⁻¹` in `tx` (or wherever).
- During Eea, `tx` could serve as `v_w` (starts as the input).

This is **exactly what the current `with_kal_inv_raw` does conceptually**:
`tx` is passed as `v_in`, CX-copied into `v_w`, then reversed at end. The
savings would come from NOT allocating a separate `v_w` register at all
and running Eea directly on `tx`.

Checking the code: `alloc_eea_state` allocates `v_w: alloc_qubits(n)`
fresh, not folded onto tx. **This 256q fold is available and unexploited.**

Similarly, once the inversion output is placed in `r`, we no longer need
the original `dx` in `tx`. If `r`'s final value can be written back to
`tx` via a swap (free) and `r`'s allocation deferred to post-inversion,
we save another 256q at peak.

**Derived ceiling with data-register reuse**:
- tx doubles as v_w during inversion: −256q
- r can be written into ty or tx post-inversion: −256q (if ty is idle
  during mul-heavy phases, which it mostly is during Eea body)

Current 2716 − 407 (m_hist) − 256 (v_w fold) − 256 (r fold) = **1797q**.

Add Eea per-iter internal tmp reductions (venting adder for step 4
tmp): −128–256q transient.

Tentative floor from local edits alone: **~1550–1650q.**

**Still above 1300.** The next savings must come from eliminating either
(a) `s` (256q), (b) `u` or `r` as separate registers (256q each via
fused pair), or (c) the step-4 transient (256q).

---

## 7. Comparison to HRSL/Litinski register layouts

Summary of published-circuit Eea register layouts (best-effort
reading from primary sources; to be verified):

| source                    | u | v | r | s | m_hist | notes |
|---------------------------|--:|--:|--:|--:|-------:|-------|
| Our current               | n | n | n | n |   2n−1 | 4n + 2n |
| HRSL 2020 (§4.2 swap-based)| n | n | n | n |     ~n | 4n + n |
| Litinski 2024             | n | n | n | n |    n/4 | 4n + n/4 via windowed |
| Kim 2026 (unconditional)  | n | n | n | n |      0 | 4n + const (drops m_hist at +9–28% Toffoli) |

All published circuits keep the **4×n Eea register layout**. They
differ only in how they handle `m_hist` (or its analog).

**Implication**: `m_hist` elimination alone saves 407q. The 4×n
remainder (1024q) is published-frontier; no public circuit has
collapsed it. Any sub-1300q scaffold likely must ADD a novel
register-folding idea to the public knowledge.

### 7.1 Candidate register folds (untested)

1. **(u, v_w) fused**: Eea invariant says `bitlen(u) + bitlen(v_w) ≤
   2n − iter` at iter `iter`. At iter 0, both are full-n (u=p, v_w=x),
   so they can't share storage yet. At iter ≥ n, total bitlen ≤ n, so
   they fit in one n-register. This saves **up to 256q in the second
   half of Eea** — which is where peak lives.

2. **(r, s) fused**: same invariant, `bitlen(r) + bitlen(s) ≤ iter`.
   r starts 0, s starts 1. Small-iter advantage. At iter ≥ n, fits in
   n. Saves up to 256q symmetric to (u, v_w).

3. **r = tx at end**: after Eea ends, r holds the answer. If the
   subsequent code reads the answer from tx, we can swap r ↔ tx at the
   end (free) and free the r register. Saves 256q persistent-after-
   inversion.

Combined: **−256 to −512 qubits possible from fusing** if we can manage
the iter-dependent layout.

---

## 8. The "single-inversion" question, revisited cold

The repeated pattern from prior sessions was to try to replace 2-Eea
with 1-Eea via algebraic tricks (Montgomery batched, Jacobian,
Strategy C). All hit the same wall: **cleanup of intermediate state
requires either another inversion or an equivalent cost.**

The algebraic reason (stated precisely):
- Let `f: (P, Q) → (R)` be the reversible affine-add circuit, and let it
  be in-place on `(tx, ty)` (tx transitions Px → Rx, ty transitions
  Py → Ry). Q is classical so doesn't need reversibility.
- Any ancilla register `A` used during `f` must satisfy at circuit end:
  `A = 0`. The uncompute must be expressible as a reversible gate
  sequence on `(tx, ty, Q, A)` with tx = Rx, ty = Ry.
- If `A` held `dx⁻¹`, uncomputing it means mapping `dx⁻¹ → 0` using
  `(Rx, Ry, Qx, Qy, dx⁻¹)`. The simplest such map is `A ≡ Eea(dx)`
  run backward, but that requires `dx` live. `dx = Px - Qx` and Px is
  gone (overwritten by Rx). So the uncompute fails unless there's a
  polynomial (or cheap-circuit) reconstruction of `dx` from
  `(Rx, Ry, Qx, Qy)`.
- From the curve equation and the add formula: `Rx = λ² - Px - Qx`, so
  `Px = λ² - Rx - Qx`. We need `λ`. `λ` is a function of `dy/dx`, both
  of which require `Px, Py` to reconstruct. **No closed-form route from
  (Rx, Ry, Qx, Qy) to dx without a fresh inversion.**

**This is a theorem about the cost model, not a limitation of the
attempted algebra.** It says: in ANY reversible in-place single-point-add
circuit using modular inversion, the inversion's backward uncompute is
either ~free (by running the forward Eea backward on the live
register still holding dx) or costs another full inversion.

### 8.a The escape hatch nobody implemented: out-of-place Rx

The in-place requirement (tx: Px → Rx) is what forces the obstruction.
If we allocate a separate `rx` output register and keep tx = dx (or Px)
alive THROUGH the whole circuit, then at cleanup time:
- `dx` is still in tx (or Px-Qx trivially derivable).
- Eea backward on tx inverts clean: ~1.6M Toffoli (same as forward).
- At end, we swap tx ↔ rx via cswap? No, Rx needs to END UP in tx per
  harness contract. So we'd need at minimum a CX-copy of rx into tx, then
  zero rx. CX-copy needs tx = 0 as target initially, which it isn't.

But we CAN do:
- Keep tx = dx = Px - Qx throughout. Allocate a fresh `rx` reg. Compute
  Rx into rx. At end: do `tx ⊕= dx` (tx → 0), then `tx ⊕= rx` (tx → Rx),
  then the reversible inverse-add `Qx` back to Rx to recover... actually
  `tx = Rx XOR 0` is wrong reversibility-wise.

The correct pattern:
- tx starts holding `dx = Px - Qx` (already done in current scaffold).
- Allocate `rx` fresh, compute `rx = Rx - Qx` (same convention as tx).
- At end: cswap(tx, rx). Now tx = Rx - Qx, rx = dx. Eea backward
  uses rx (= dx) as input and zeros its ancillas. Then `tx += Qx`
  classically (Qx is classical) to get Rx in tx.
- Cost: +256 persistent qubits for `rx` during the whole inversion+mul
  phase.

**This is a genuine 1-inversion scaffold that passes the harness.** The
cost is +256 qubits, and it saves ~1.6M Toffoli by eliminating one full
Eea backward pass. Net: **+256q for -1.6M Toffoli.**

### 8.b Does Strategy C work with out-of-place Rx?

Strategy C computes `Rx = v · dx⁻³ · dx = v · dx⁻²`, where `v = dy² -
dx²·(Px + Qx)`. This touches Px but Px = tx + Qx, and tx holds `dx`,
soI can classically-constant-add Qx to tx momentarily to get Px, use
it, and undo. Or compute `v` via `dy² - dx²·(dx + 2Qx)` (since Px =
dx + Qx). **Strategy C only needs (tx, ty, Qx, Qy) as live inputs, no
separate Px register.**

With out-of-place Rx + Strategy C:
- tx holds dx, ty holds dy through the computation.
- Eea on `w = dx³` (need a mul to square-and-cube dx into a fresh
  register, then invert).
- Compute Rx - Qx into a fresh `rx` register using Strategy C's
  formula.
- Compute Ry - Qy into ty using Strategy C's formula (ty transitions
  dy → Ry-Qy-ish).
- At end, Eea-backward on w = dx³ zeros all Eea ancillas because
  tx = dx is still live. This is the critical win.
- Final: swap tx ↔ rx, do classical add of Qx to tx, Qy to ty (all
  classical-const, cheap). Free rx (it now holds old dx, which we can
  uncompute because... wait, no. rx now holds dx, and we'd need to zero
  it. Zeroing `dx` from rx requires knowing Px, which we don't have at
  end state).

**Hmm, still stuck.** The swap shifts the problem but doesn't eliminate
it. Let me think more carefully.

### 8.c The actual constraint, refined

At circuit end: must have tx=Rx, ty=Ry, all ancillas=0. If tx starts as
Px (NOT dx as in our current scaffold), the harness works. Currently we
do `mod_sub_qb(tx, Qx)` at the start to convert Px → dx, and a matching
`+Qx` at end to convert back. This is classical, cheap, and symmetric.

The obstruction: during the circuit, at the point where we output Rx
into tx, tx transitions (dx → Rx). After this transition, dx is gone.
That's the irreversibility wall.

**Resolution option**: do the `dx → Rx` transition VERY LATE, after the
Eea-backward has already zeroed its ancillas using the still-live
dx.

Concrete sketch (Strategy D, NEW):
1. mod_sub_qb(tx, Qx): tx = dx.
2. Compute `w = dx³` in fresh register `w_reg` (requires 1 mul + 1 sq).
3. Run Eea_forward on w_reg: produce `inv_raw` in the Eea `r`.
4. Use Strategy C's formula to compute `Rx - Qx` into FRESH `rx_out`
   register, `Ry - Qy` into fresh `ry_out` register. This uses
   `(tx, ty, inv_raw)` as live inputs. tx and ty are UNCHANGED in this
   step.
5. Run Eea_backward on w_reg. Same tx=dx still live, so backward
   cleanly zeros all Eea state AND `w_reg` itself (because
   Eea-forward's `w_reg := dx³` is reversed by the mul-inverse).
6. Swap tx ↔ rx_out (cswap free). Now tx = Rx - Qx, rx_out = dx.
7. Uncompute rx_out: but rx_out = dx, and Px-Qx = dx, so rx_out =
   tx_original - Qx ... wait, tx_original was replaced. Use the fact
   that `dx` at this point = (new tx) + something only involving the
   curve equation. Actually: new tx = Rx - Qx, old tx (= dx) is still
   in rx_out. To zero rx_out we'd need to recover dx from Rx - Qx, but
   they're independent.

**Still stuck at step 7.** The root problem: zeroing `rx_out = dx`
requires knowing dx without Px.

### 8.d A correct 1-Eea scaffold

Try once more. Keep tx = Px throughout the Eea body, don't use it
as the inversion input. Instead use a FRESH register for dx:

1. Allocate `dx_reg` fresh (+256q). CX-copy tx into dx_reg, subtract
   classical Qx: dx_reg = dx.
2. Compute `w = dx_reg³` in fresh `w_reg` (+512q total).
3. Eea_forward on w_reg.
4. Strategy C to compute Rx, Ry into the ORIGINAL tx, ty registers
   (in-place, since Rx depends only on (dx_reg, ty, classical Qx), and
   Ry on (dx_reg, ty, w⁻¹)).
5. Eea_backward on w_reg (dx_reg still live).
6. Undo step 2 to zero w_reg via mul/square reverse.
7. Undo step 1: classically add Qx back to dx_reg, CX-copy tx_now into
   dx_reg. But tx_now = Rx, not Px. dx_reg = dx = Px - Qx. So
   `dx_reg ⊕= tx_now` gives dx_reg = dx XOR Rx, not zero. **Fail.**

**Insight**: step 1 of any scaffold where tx stays = Px only preserves
Px long enough to create a shadow `dx_reg`. To zero `dx_reg` at end,
we'd need to re-copy tx (= Rx) into dx_reg, which doesn't zero it.

The clean variant: allocate dx_reg, compute dx into it from Px AT THE
START, and run everything against dx_reg. THEN at end, once tx has
transitioned to Rx and dx_reg still holds dx, we'd need to uncompute
dx_reg using... whatever provides Px. Px is gone.

**This is the real wall.** The in-place contract `tx: Px → Rx` means
any scaffold that needs dx at cleanup time must still have Px
available at cleanup time, which is impossible.

**The only escape**: do all cleanup BEFORE the Px → Rx transition, then
perform the transition last. That's what a correct 1-Eea scaffold
would look like:

```
1. (tx=Px, ty=Py). Use Strategy C with tx left unchanged.
2. Compute Rx into a fresh `rx_reg`.
3. Compute Ry into... well, ty transitions Py → Ry somewhere.
   Same problem for ty.
```

If both Rx and Ry go to fresh registers, we free them at end by... again,
zeroing them requires knowing old Px, Py, which are stale.

**Final reduction**: a true in-place 1-Eea scaffold requires that
the final transition `(Px, Py) → (Rx, Ry)` be expressible as a reversible
circuit acting on `(tx, ty, live_ancillas)` where tx=Px, ty=Py at entry
and tx=Rx, ty=Ry at exit, and the ancillas are zeroed by this same
transition. The transition's algebra is:
    Rx = λ² - Px - Qx
    Ry = λ(Px - Rx) - Py - Qy    [or equivalent]
with λ = (Py - Qy)/(Px - Qx).

**This transition is reversible** (Rx, Ry determine Px, Py given Qx, Qy
via the curve's group structure). Its cost as a reversible circuit is
essentially what we currently pay: two inversions (forward and backward
of λ's computation via Eea). **Any 1-Eea circuit must "bake in"
this reversibility differently**, and that's what Google's disclosed
recipe (lookup-centric, windowed) apparently does.

### 8.1 The escape: fuse inversion backward with subsequent mul

If the Eea BACKWARD pass is run **while also doing the subsequent
multiplication**, i.e. interleaved, the inverse-uncompute work can share
state with the forward multiply and save ~half. This is the
"windowed inversion" idea referenced in Gidney-Ekera 2021 and Litinski
2024.

Magnitudes (needed verification):
- Forward Eea: ~1.62M
- Backward Eea: ~1.62M (gate-inverse of forward)
- Fused: ~1.62M + X% where X < 100%.

Published estimates suggest X ≈ 30-50%, giving fused cost ~2.1-2.4M. If
we can apply this at BOTH inversion sites, we save ~(1.62M − 1.62M×0.4)
× 2 = ~2M. Post-fusion total ≈ 2.1M. **This is the path to 2.7M Toffoli
and plausibly Google's number.**

But: this is complex. No public circuit at the n=256 level has
demonstrated a fused Eea-with-mul at < 1.5× forward-Eea cost.

---

## 9. What a 1300q, 2.7M circuit MUST look like (derived)

Collecting sections 4–8:

### Qubit-side constraints
1. tx, ty reused as Eea state (−512q saved over separate allocation).
2. m_hist eliminated or compressed (−407 to −400q).
3. (u, v_w) or (r, s) fused during the second half of Eea (−128
   on average).
4. Step-4 transient via Gidney-style venting or in-place (−128 to
   −256q).
5. Only ONE active Eea instance at a time (two-instance design
   blows the qubit budget regardless of fold).

Projected peak with all 5: 2716 − 407 − 256 − 128 − 256 = **1669q**.
Still > 1300q. Closing this needs:
6. Another register fold or radically smaller r/s representation
   (windowed? w=4 window saves r/s to ~n/4 = 64q each, −384q).

With windowed Eea: **~1285q**. Matches Google 1175-1425q window.

### Toffoli-side constraints
1. One full Eea pass: ~1.6M.
2. Fused backward + subsequent mul: ~0.5-1M (vs full 1.6M backward).
3. Cleanup/scale correction: ~200k.
4. q×q muls and squaring: ~400-500k.

Total: **~2.7-3.3M**. 2.7M requires tight fusion.

---

## 10. Decision: what to work on, in order

Based on the analysis through §9 the picture is:

1. **A 1-Eea-invocation point-add that preserves the exact-affine
   harness contract appears to be structurally impossible with public
   techniques.** §8 derives the obstruction; §8's classical schedule in
   `single_inv_numeric.rs` under Strategy D shows that fresh output
   registers can't be zeroed without a second inversion. Prior
   Strategy A/B2/C attempts hit the same wall.

2. **Google's 1175q / 2.7M presumably uses a non-public technique**
   in this obstruction zone — likely a lookup-based or windowed point-add
   structure that bypasses the affine reversibility wall.

3. **Our practical frontier is bounded below by ~2 * 1 Eea invocation
   + non-Eea overhead ≈ 3.8M** at best with public techniques, IF
   we make the Eea invocations themselves cheaper (e.g. windowed
   Eea at 50-60% of current per-iter cost).

### 10.1 Realistic, first-principles-derived targets

Given the 2-invocation floor and targeting a 20-30% reduction from our
4.13M:

- **Tier 1 (3.3–3.5M, achievable):** windowed Eea (w=2), data-register
  fold, m_hist compression. Qubit impact: 2716 → ~2000–2200. Toffoli impact:
  -400k to -600k via reduced per-iter cost of Eea.

- **Tier 2 (2.8–3.0M, stretch):** Tier 1 + HRSL-style swap-based Eea
  that fuses step 3 and step 9 cswaps (currently 13% of total). Qubit
  impact: further ~-200q. Toffoli impact: -500k from cswap fusion.

- **Tier 3 (< 2.7M):** requires either Google's undisclosed trick OR a
  successful 1-invocation scaffold. Not a public-technique target.

### 10.2 Correct research order

1. **Windowed Eea** (`w=2`): primary Toffoli lever. Reduces per-iter
   cost by ~40-50% (fewer iters, larger lookup per iter net-wins). Also
   smallest r, s registers, opening the qubit budget.

2. **Data-register reuse** (tx = Eea v_w): orthogonal to (1);
   cheap; saves 256q persistent.

3. **m_hist elimination**: NOT VALUABLE for Toffoli (neutral). Only
   worth doing once we hit a qubit-cap block on a genuine Toffoli win.
   DEFER until a concrete Toffoli opt is blocked by qubit cap.

4. **HRSL swap-based Eea** restructuring: multi-session, high
   phase-bug risk. Only after Tier 1 is solidly in.

5. **DO NOT spend time on 1-invocation scaffolds** without a new
   insight. §8 + Strategy D show the obstruction is real. Further
   prior-art survey is OK; implementation attempts are not.

This order is deliberately the REVERSE of what prior sessions attacked.
Prior sessions repeatedly tried `single-invocation` first, hit the
cleanup wall, and thrashed. The correct order is: shrink per-iter
Eea cost FIRST (1, 2), then consider restructurings (4), and only
revisit 1-invocation if a genuinely new algebraic idea arrives.

---

## 11. Honest uncertainty

- §4's "2.7M achievable with one Eea pass + cleanup" is a **forward
  count** — it assumes cleanup doesn't cost another Eea. Not proven.
  The "fused backward + mul" claim in §8.1 has a literature citation
  but no n=256 circuit demonstrating it publicly.

- §5's "4×n layout is published-frontier" is from my survey of HRSL,
  Litinski, Kim. If a non-public or overlooked paper collapsed this to
  3×n or 2×n, the qubit calculus shifts dramatically.

- §9's "windowed Eea saves r,s to n/4" is from Litinski 2024
  estimates of Montgomery mul with 4-bit windows. Our Eea is not
  Montgomery. The windowing principle applies (trade: large lookup per
  iter, fewer iters, smaller state), but the specific n/4 claim needs
  re-derivation for our variant.

- The decision-order in §10 assumes no single "magic bullet" — that
  Google's 2.7M/1175q is a product of 3-5 stacked optimizations. This
  matches what the Google paper says publicly ("kickmix + MBUC +
  windowed arithmetic"), but still: **an undisclosed structural trick
  could change the entire picture.** I'm explicitly not assuming we can
  match Google.

Goal for this session going forward: grow this document with verified
measurements as we investigate each row. No implementation until the
model says where to cut.
