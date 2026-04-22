# Pair1 mul2 findings

A direct hypothesis test was run on the strict failing case `k = 4`:
- replace `pair1_mul2 = mod_mul_add_into_acc_schoolbook(...)`
- with `mod_mul_add_into_acc_karatsuba2(...)`
- keep the specialized bulk-prefix replacement active.

## Result
This did **not** fix the phase bug.

Observed effect:
- baseline experimental (`pair1_mul2` schoolbook):
  - `classical mismatches = 0`
  - `phase-garbage batches = 1`
  - first phase mask: `0x0000040000000000`
- replacing `pair1_mul2` with karatsuba2:
  - `classical mismatches = 2`
  - `phase-garbage batches = 3`
  - first phase mask: `0x0000000000000040`

## Interpretation
`pair1_mul2` is clearly part of the phase-sensitive region, but the failure is
not just “schoolbook mul-add is wrong”. Swapping in a different multiplier made
things worse and changed the phase signature.

So the remaining bug is more likely about the phase context / interface into
`pair1_mul2` than about that one routine in isolation.
