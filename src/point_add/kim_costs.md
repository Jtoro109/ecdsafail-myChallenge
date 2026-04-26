# Kim inversion cost checkpoint (April 25)

Purpose: preserve a **measured** answer to the question
"can we just drop in the existing Kim-style unconditional inversion path
and get closer to 1200q / 2.7M?"

## Commands

```bash
cargo test kim_inv_ -- --nocapture
```

## Measured results

From `src/point_add/kim_inv_circuit.rs` tests:

- **Kim forward inversion at n=256, 2n rounds**
  - Toffoli: **1,252,352**
  - peak qubits: **3590**

- **Full Bennett-clean `kim_inv(x, out)` primitive**
  - Toffoli: **2,530,240**
  - peak qubits: **4102**

The full primitive leaves `x` unchanged, cleans all ancilla, and writes
`±x^-1 * 2^(2n) mod p` to `out`.

## Interpretation

This is a valuable negative result:

1. **Toffoli is not the blocker.**
   A single Bennett-clean Kim inversion at 2.53M CCX is not absurdly far from
   the whole point-add budget.

2. **Qubits are the blocker.**
   4102q for one clean inversion primitive is nowhere near the user's ~1200q
   target, and it is also far above our current 2716q live-build peak.

3. **Direct Kim drop-in is dead.**
   Even before adding the surrounding affine scaffold, two inversions plus
   multipliers would be far outside the qubit budget.

## Practical conclusion

Use the Kim work only as:

- a classical/reference validation harness for unconditional execution,
- a source of ideas about deterministic scaling / postponed reduction,
- not as a live `build()` replacement.

If we revisit this line, the only plausible next step is a **compact-state**
variant (Google/Luo-like register sharing, coset/windowed arithmetic, or other
major architecture change), not the current wide-state Kim primitive.
