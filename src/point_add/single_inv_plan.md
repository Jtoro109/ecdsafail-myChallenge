# Single-Inversion Moonshot Plan (Montgomery's trick)

## Motivation

Current 2-Kaliski structure (commit `d3aead2`, 4.18M Toffoli):
- pair1 Kaliski (forward+backward) ≈ 2.0M Toffoli
- pair2 Kaliski (forward+backward) ≈ 2.0M Toffoli
- All other ops                      ≈ 0.18M Toffoli

Kaliski ≈ **96%** of the whole circuit. Per-phase TRACE confirms: every
step-4 / step3_cswap / step9_cswap line in the profile is from a Kaliski
body. One Kaliski pass dominates ~1M Toffoli.

If we could do the whole point-add with **a single Kaliski inversion** and
pay the cost of a few extra modular multiplications, that's roughly a
**~1M Toffoli** saving: an absolute-best case of ~3.2M, ballpark the
Google low-qubit 2.7M target.

## Math (Montgomery's trick for two inversions)

secp256k1 affine add, targets `(Px, Py)` += constants `(Qx, Qy)`:

    dx = Px - Qx                        (quantum)
    dy = Py - Qy                        (quantum)
    λ  = dy * dx^{-1}                   ← inversion #1
    Rx = λ² - Px - Qx
    Ry = λ * (Qx - Rx) - Qy             ← uses λ, no extra inversion

So on paper the affine formula uses exactly **one** inversion. Our
existing scaffold actually splits the work into two Kaliskis because it
needs to uncompute `dx` and `dy` along the way and pair1/pair2 each
amortize a Kaliski. The moonshot is to refactor so only one Kaliski is
needed, at the cost of a few extra modular muls.

## Target circuit skeleton

Let `p = SECP256K1_P`, n = 256.

```
INPUT (quantum): tx = Px, ty = Py
INPUT (classical bits): ox = Qx, oy = Qy

1.  tx -= ox                 # tx now holds dx
2.  ty -= oy                 # ty now holds dy

3.  a = alloc_qubits(n)
    # a := dx * dy mod p  (cheap forward mul, only used as a Kaliski input)
    mod_mul_write_into_zero_acc(a, tx, ty)

4.  # Kaliski inverse of a into ainv (Bennett: copy-out + undo).
    # Use the existing with_kal_inv_raw(a) style, but we need ainv to
    # survive past the body. So instead:
    #   - kaliski_forward(a) → st.r holds raw = a^{-1} * 2^{2n}
    #   - classical correct: r *= K = 2^{-2n} mod p
    #   - ainv := alloc + XOR-copy r → ainv
    #   - unscale r *= 2^{2n} back
    #   - emit_inverse(kaliski_forward)
    # Now ainv = (dx*dy)^{-1}, a is still dx*dy.

5.  # Uncompute a = dx*dy by running the mul backward (Bennett).
    emit_inverse( mod_mul_write_into_zero_acc(a, tx, ty) )
    free(a)

6.  # Now we have: tx = dx, ty = dy, ainv = (dx dy)^{-1}.
    # Extract 1/dx = dy * ainv.
    inv_dx = alloc_qubits(n)
    mod_mul_write_into_zero_acc(inv_dx, ty, ainv)    # inv_dx = dy * (dx dy)^{-1} = 1/dx

7.  # Compute λ := dy / dx = dy * (1/dx)
    lam = alloc_qubits(n)
    mod_mul_write_into_zero_acc(lam, ty, inv_dx)

8.  # Rx := λ² - (Px + Qx) = λ² - (dx + 2Qx)
    # Use the usual mod_mul_sub_qq + mod_add_double_qb pattern in tx.
    mod_mul_sub_qq(tx, lam, lam)       # tx := dx - λ²
    mod_add_double_qb(tx, ox)          # tx := dx - λ² + 2Qx
    mod_neg_inplace_fast(tx)           # tx := λ² - dx - 2Qx = Rx - Qx
    # (leave tx = Rx - Qx for later +Qx fold, same pattern as today)

9.  # Ry := λ * (Qx - Rx) - Qy
    # In current coords: -(tx) = Qx - Rx, so we can rewrite
    #   Ry = -λ * tx - Qy
    # Start from ty = dy, we need to "replace" it with Ry. Going through
    # an add-mul-sub pattern keeps it reversible.
    #
    # Strategy:
    #   ty -= dy             (via uncompute of ty = dy, but that costs a Kaliski-less path…)
    #
    # Actually simpler: we never need dy again for reversibility purposes
    # because we still have ainv, inv_dx, lam as Bennett-clean intermediates.
    # But ty IS dy right now; its final value must be Ry. So:
    #   - load +Qy into ty      ty := dy + Qy = Py (restore)      (XOR classical bits, 0 CCX)
    #   - ty -= Py              now ty := 0
    #   - mul-add ty += λ * (Qx - Rx) via (tx = Rx - Qx): ty -= λ*tx
    #   - add -Qy:              ty := λ(Qx - Rx) - Qy = Ry         (trivial)
    #
    # That is, structurally the same as the current between_pair block.

10. # Finally fold tx: mod_add_qb(tx, ox) so tx := Rx.

11. # Uncompute scaffolding:
    emit_inverse(mod_mul_write_into_zero_acc(lam, ty, inv_dx))  # lam → 0
    free(lam)
    emit_inverse(mod_mul_write_into_zero_acc(inv_dx, ty_pre, ainv))
    free(inv_dx)
    # ainv was produced by Bennett; run its uncompute here.
    ...
```

The exact uncomputation of `ainv` is the trickiest part. One viable
pattern: do NOT Bennett-copy out of the Kaliski. Instead, lift the body
(steps 6–10) inside `with_kal_inv_raw(a)`, use `inv_raw` = `ainv * 2^{2n}`
directly inside the body, and let `with_kal_inv_raw` handle reverse.
Quantities carrying a `2^{2n}` factor can be rescaled by bundling enough
halvings, the same trick as the current pair1_halve loop — but we only
pay that rescale **once**, not twice.

## Expected savings (rough)

|                         | Toffoli  |
|-------------------------|---------|
| Kaliski × 2 (current)   | ~4.0M   |
| Kaliski × 1             | ~2.0M   |
| Extra muls (+3 muls × ~75k) | +0.22M |
| pair_halve/double (×1)  | ~0.1M   |
| **Total est.**          | ~2.3M   |

Ballpark: **~45% Toffoli reduction** if this works.
Peak qubits: unchanged or better, because we never have two Kaliski
states live simultaneously.

## Risks

1. **Uncomputing `a = dx*dy`** cleanly across the Kaliski. `emit_inverse`
   on a mul is ok (it's phase-clean by construction), but stacking that
   on top of `with_kal_inv_raw`'s own reverse half is novel in this
   codebase.
2. **Scale correction on ainv.** ainv = `inv_raw * K` where
   `K = 2^{-2n} mod p`; the classical correction is a quantum × classical
   mul which costs a windowed-multiply (~40–80k Toffoli). Need to make
   sure that cost doesn't eat our savings.
3. **Pair2-cleanup ty path.** Right now pair2_cleanup uses `mod_sub_qb(ty, oy)`
   at +1.3k Toffoli. The single-inv path needs a different ty lifecycle.
   Spell out reversibly.

## Incremental validation

Prototype in 3 stages:
1. **Classical numeric check**: reimplement the whole formula in
   `src/point_add/single_inv_numeric.rs`, run 10^4 random inputs through
   both old and new formulas, check `(Rx, Ry)` matches libsecp256k1.
2. **Single-Kaliski reversible scaffold** using the existing
   `with_kal_inv_raw` on `a = dx*dy`. Drive the body with `inv_raw`, scale
   externally once, copy-out, uncompute-mul, reverse-Kaliski, verify
   5-seed phase-clean.
3. **Wire into build()** behind `SINGLE_INV=1` env gate. Measure + gate
   24 seeds before committing.
