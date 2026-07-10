# Single-Inversion Moonshot — Formal Spec

**Status:** under active design. Current live build still uses 2-Eea
(commit `21b87fd`, 4.91M Toffoli @ iters=511). This doc is the primary
artifact; any code changes must be backed by a passing falsifiable
classical replay in `single_inv_numeric.rs`.

## 0. Ground rules

- Every operation in a strategy must correspond to a reversible quantum
  op on a specific register with a specific algebraic value before and
  after. No hand-wavy "uncompute this mul".
- A strategy is **live** iff a `replay_strategy_X()` function in
  `single_inv_numeric.rs` produces `(Rx, Ry)` matching the reference on
  200/200 random curve-point pairs.
- A strategy is **worth implementing** iff (after passing 200/200) its
  counted op cost is less than the current 2-Eea scaffold's.
- We budget classical-constant multiplication by `K = 2^{-2n+1} mod p`
  as "zero Toffoli" in the replay; real reversible cost must be counted
  separately later.

## 1. Scale-factor convention (settled in commit `21b87fd`)

Under `state_a_iters = state_b_iters = 2n - 1 = 511` the raw Eea output
register r satisfies

    r_final = -v^{-1} * 2^{2n-1} mod p        (deterministic, input-indep)

with the final negation-to-positive step skipped (see `eea_forward`
comment). That is: inv_raw inside a `with_kal_inv_raw` body equals
`-v^{-1} * 2^{2n-1}`.

## 2. Register inventory before and after the point-add

Qubit registers that survive the whole circuit:

| role      | width | before | after |
|-----------|-------|--------|-------|
| tx        | n     | Px     | Rx    |
| ty        | n     | Py     | Ry    |

Classical bit registers (read-only):

| role | width | value |
|------|-------|-------|
| ox   | n     | Qx    |
| oy   | n     | Qy    |

Every ancilla we allocate between "before" and "after" must be freed
(returned to |0⟩) by the end of the body.

## 3. Core identity (classical)

Let `dx = Px - Qx`, `dy = Py - Qy`, `λ = dy / dx`. Then:

    Rx = λ² - Px - Qx     = λ² - dx - 2Qx
    Ry = λ·(Qx - Rx) - Qy = -λ·(Rx - Qx) - Qy

These formulae use exactly one inversion (compute 1/dx) and are what all
three strategies below target.

## 4. Strategy A — invert a = dx·dy (Montgomery bundle)

**Idea:** compute a single inversion of the product `dx·dy`, then use
`a^{-1}` as a "universal denominator" and extract `1/dx = dy·a^{-1}` or
`λ = dy²·a^{-1}` without a second Eea.

### Register discipline (planned)

step | tx               | ty          | ancillas live (width)
-----|------------------|-------------|----------------------------
1    | dx               | dy          | —
2    | dx               | dy          | a(n)
3    | dx               | dy          | a(n), kal_state
4    | dx               | dy          | a(n), kal_state after forward, inv_raw ⊂ kal_state
5    | dx               | dy          | a(n), kal_state, lam(n) = λ
6    | Rx-Qx            | dy          | a(n), kal_state, lam(n)
7    | Rx-Qx            | dy - λ·(Rx-Qx) = 2dy - (Ry + Qy) ?? | a(n), kal_state, lam(n)

**Obstruction surfaced by step 7:** starting from ty=dy and doing
`ty += λ·tx` (with tx = Rx-Qx) yields

    ty_new = dy + λ·(Rx-Qx) = dy - (Ry + Qy) = (Py - Qy) - Ry - Qy
           = Py - 2Qy - Ry

so ty now contains `Py - 2Qy - Ry`. Needed: Ry. Gap: `2Ry - Py + 2Qy`.
The `Py` term has no classical handle, so there is no way to finish by
classical bit ops.

### What's needed to rescue Strategy A

Some reversible computation that, starting from ty = `Py - 2Qy - Ry` and
using only `{lam, tx=Rx-Qx, a, kal_state, ox, oy}`, lands ty at Ry. The
only algebraic relationship that yields Py is Py = dy + Qy, and dy is
not in any live register after step 7 (ty overwrote it).

### Falsification plan for Strategy A

`replay_strategy_a()` executes steps 1–7 and checks (tx, ty) = (Rx, Ry).
Expected result: Rx will match, Ry will NOT match (off by exactly the
obstruction term above).

If `replay_strategy_a` fails as predicted, Strategy A is dead unless
we add an output register for Ry (Strategy A-prime, +n persistent
qubits).

## 5. Strategy B — invert dx only, reach Ry without Py

**Idea:** run one Eea on dx (exactly like the current state_a), get
λ into a lam register, and compute Ry from the state **before ty is
mutated**, writing Ry into a dedicated output register or into ty via
a path that only touches `dy, λ, Qy, tx'=Rx-Qx`.

Critical observation. Starting from ty = dy, consider:

    ty_target = Ry = λ·(Qx - Rx) - Qy = -λ·tx' - Qy     (tx' = Rx - Qx)

So `ty_new = ty_old + (Ry - dy) = ty_old - dy - λ·tx' - Qy`. We do have
dy classically? No — dy = Py - Qy, and Py is quantum. But note that
`ty_old = dy` already, so `ty_new = Ry = -λ·tx' - Qy - ty_old + ty_old`.
This is a tautology, not a cancellation path.

### Register discipline (planned)

step | tx               | ty                  | ancillas
-----|------------------|---------------------|-----------------------------
1    | dx               | dy                  | —
2    | dx               | dy                  | kal(dx)
3    | dx               | dy                  | kal + lam=λ
4    | Rx-Qx            | dy                  | kal + lam
5    | Rx-Qx            | dy - λ·tx' = ?      | kal + lam

At step 5:
    ty_new = dy - λ·(Rx - Qx) = dy + λ·(Qx - Rx) = dy + (Ry + Qy)
           = (Py - Qy) + Ry + Qy = Py + Ry.

So ty now holds Py + Ry. Subtracting Py reversibly requires Py as a
classical bit register, which we do not have. Dead same as A.

### The trick used by the current 2-Eea scaffold

state_a_mul2 adds into ty with **a specific scale factor that makes
ty = 0 exactly**. From the scale-factor derivation in commit `21b87fd`:
after state_a_halve, `lam_inner = -λ` (sign from the skipped negation),
so `ty += lam_inner · tx = dy - λ·dx = dy - dy = 0`. The second Eea
then writes Ry into the now-zero ty through the mul3+state_b chain.

The sign trick only works when lam_inner has a *specific* sign. So in
single-Eea land we need `lam_inner = −λ` and `ty += lam_inner · tx`
to land at 0. Strategy B-prime attempts that:

### Strategy B-prime — negative-sign state_a, replay state_b classically

1. Run Eea on dx. Body has `inv_raw = -dx^{-1} · 2^{2n-1}`.
2. Allocate lam(n). Compute `lam := dy · inv_raw` → lam = `-dy·dx^{-1}·2^{2n-1}`.
3. Halve lam (2n-1) times → lam = `-λ`.
4. `ty += lam · tx` → ty = dy + (-λ)·dx = dy - dy = 0. ✅ Py is gone.
5. Exit Eea body; Eea_backward uncomputes its state. Now we
   have: tx = dx, ty = 0, lam = -λ.
6. Compute Rx fold into tx: existing 6-step `+3Qx / neg / +Qx` chain.
   Uses lam². tx ← Rx - Qx.
7. Compute Ry into ty: need Ry = λ(Qx - Rx) - Qy = -λ·(Rx - Qx) - Qy
                         = (-λ)·tx - Qy = lam·tx - Qy.
   So: `ty += lam · tx` (writes Ry + Qy into ty since ty=0), then
       `ty -= oy` (bit register). Done. ty = Ry.
8. Uncompute lam = -λ. This is the one open sub-problem: lam was built
   via "dy · inv_raw · halvings" inside the Eea body, but the body
   already exited. We need to either uncompute lam outside the body, or
   structure so lam is freed inside the body before exit.

Strategy B-prime has two versions:

- **B1**: keep lam live across the Eea body exit; uncompute later
  using dy, inv_raw_recomputed. That means running state_a Eea *twice*
  (once forward, once as part of uncompute). NOT a saving — same cost
  as the existing 2-Eea up to constants.
- **B2**: do Rx fold and Ry computation (steps 6–7) *inside the Eea
  body*, before body exit. That keeps dy, inv_raw live and lam can be
  uncomputed symmetrically (mul2 inverse). Peak qubit higher (lam lives
  during body), but only ONE Eea pass total.

### Falsification plan for Strategy B2

`replay_strategy_b2()` does steps 1–8 in order, tracking every register
as a U256. End check: (tx, ty) == (Rx, Ry).

## 6. Strategy C — direct Montgomery batch (two inversions → one)

Montgomery's trick: to invert both `a` and `b`, compute `ab`, invert
once, then multiply back. That's `a^{-1} = b·(ab)^{-1}` and
`b^{-1} = a·(ab)^{-1}`.

In our case we only need one inversion (1/dx), but we could view state_a
+ state_b as two inversions (of dx and of (Rx-Qx)) and batch them. The
existing 2-Eea scaffold does invert both of these.

### Register discipline (planned)

step | tx          | ty      | ancillas
-----|-------------|---------|-----------------------------
1    | dx          | dy      | —
2    | dx          | dy      | pre_rx(n) = computed classically from tx, lam_sq?
...

This strategy requires computing (Rx - Qx) *before* we have λ, so we
can multiply dx · (Rx - Qx) and invert that product. But Rx depends on
λ which depends on 1/dx. That's circular unless we use a different
definition of (Rx - Qx).

**Algebraic unknotting:**

    Rx - Qx = λ² - Px - 2Qx = (dy/dx)² - (dx + 2Qx) - Qx  ... wait this
                                                           has dx on both
                                                           sides.

    = (dy² - dx²(dx + 2Qx) - dx²·Qx) / dx²    ... let's denote dx² = w
    = (dy² - w·(Px + Qx)) / w

So `dx²·(Rx - Qx) = dy² - dx²·(Px + Qx)`. That means:

    dx³·(Rx - Qx) = dx·(dy² - dx²·(Px + Qx))

Product `dx · (Rx - Qx) · dx² = dx³(Rx-Qx)` is computable from
{dx, dy, Px, Qx}. So we CAN compute `dx·(Rx-Qx)` = `[dx³(Rx-Qx)] / dx²`
and invert that product.

But `1/(dx(Rx-Qx)) * dx = 1/(Rx-Qx)` and `1/(dx(Rx-Qx)) * (Rx-Qx) = 1/dx`.
So one Eea on `w = dx·(Rx-Qx)` gives us BOTH inverses.

This is the direction the user hinted at. Needs more algebra to make
it a reversible sequence, but the math closes.

### Falsification plan for Strategy C

`replay_strategy_c()` assumes we can compute `w = dx·(Rx - Qx)`
classically from live registers, invert it once, and derive both λ and
Ry from w^{-1}. The replay should reproduce the reference.

## 7. Cost accounting (preliminary, before implementing)

Current 2-Eea at iters=511:
- 2 × Eea (511 iters each)          ≈ 4.50M Toffoli (scaled from 4.18M/407)
- 4 quantum muls                         ≈ 0.27M
- Other                                  ≈ 0.14M
- **Total**                              ≈ 4.91M (matches commit)

Strategy A (dead): N/A
Strategy B2 (best hope): 1 × Eea at iters=511 + 3 muls (lam= dy·inv;
lam² for Rx; lam·tx for Ry; plus mul uncompute). Ballpark:
- 1 × Eea                             ≈ 2.25M
- +3 muls and uncomputes                  ≈ 0.4M
- rescale (halve-by-2^{2n-1})             ≈ 0.1M (windowed)
- other                                   ≈ 0.14M
- **Total**                               ≈ 2.9M

That's 2.0M below current. Worth implementing **only if** the
classical replay works end-to-end.

Strategy C: potentially 1 × Eea + more muls, different structure;
cost depends on how cleanly `w = dx·(Rx - Qx)` factors.

## 8. Results of 200-trial replay tests

| strategy | Rx    | Ry    | ancilla leak         | verdict            |
|----------|-------|-------|----------------------|--------------------|
| ref      | 200/200 | 200/200 | n/a                  | sanity             |
| A        | 200/200 | off by +dy | —                    | **DEAD** (Py trapped in ty) |
| B2       | 200/200 | 200/200 | lam_copy = -λ leaked | alive with caveat  |
| C        | 200/200 | 200/200 | **NONE**             | **alive**          |

Replay code lives in `src/ec_add/single_inv_numeric.rs`. Run with
`cargo test --release -p quantum_ecc -- single_inv_numeric --nocapture`.

## 9. Honest op-count estimate for Strategy C at n=256

Strategy C does one Eea on `w = dx³` and then derives (Rx, Ry) from
the algebraic identities

    v   = dy² - dx²·(Px + Qx)
    Rx  = v·(dx·w⁻¹)
    Ry  = (dy·(dx²·Qx − v) − w·Qy)·w⁻¹

Forward op count (n=256, Toffoli):

| op                          | cost  |
|-----------------------------|------:|
| dx² (squaring)              | ~130k |
| dx³ = dx·dx²                 | ~150k |
| dy² (squaring)              | ~130k |
| dx²·(Px+Qx), quantum×quantum | ~150k |
| v = dy² - ...               | ~1.5k |
| 1 Eea on w, iters=511   | ~2.25M|
| dx·w⁻¹                       | ~150k |
| Rx = v·(dx·w⁻¹)              | ~150k |
| dx²·Qx (quantum·classical)   | ~80k  |
| dy·core                     | ~150k |
| w·Qy (quantum·classical)    | ~80k  |
| Ry = numer·w⁻¹               | ~150k |

Forward subtotal: ~3.67M.

Uncomputation (Bennett) of all temporaries: roughly doubles the mul
cost, ~1M extra.

Eea scale correction (w⁻¹ carries 2^{2n-1} factor; must be removed):
windowed classical-constant multiply, ~0.1M.

**Total estimate: ~3.7 – 4.3M Toffoli**

Compared to current 4.91M at iters=511: **saving ~0.6 – 1.2M (-13% to -24%)**.

That's the realistic ceiling, not the "3M" napkin number from earlier
monologues. Still a legitimate win if it can be implemented cleanly.

## 10. What to do next

- [ ] Lock down the Eea scale-factor / sign arithmetic for w = dx³.
      Specifically: the raw Eea output on w is `-w⁻¹·2^{2n-1}`; every
      mul that consumes it needs to know it's negative-signed.
- [ ] Re-check whether the existing `mod_mul_*` primitives can be used
      verbatim for the Strategy C chain, or if any of the ops want a
      new subroutine (e.g., `v := dy² - dx²(Px+Qx)` wants a single
      combined mul-sub to avoid extra allocations).
- [ ] Budget peak qubits for Strategy C. We already allocate n qubits
      each for `dx², dx³, dy², v, dx·w⁻¹, core, numer`, so naive ~7n extra
      persistent qubits during the chain. That overshoots the 2800q cap
      by a lot. Will need Bennett-style interleaving (free dx² after dx³
      is computed, etc.) to keep peak ≤ the program.md 3700q cap.
- [ ] Only AFTER peak + Toffoli both pencil-out under caps: write the
      reversible scaffold behind `SINGLE_INV=1` env gate.
