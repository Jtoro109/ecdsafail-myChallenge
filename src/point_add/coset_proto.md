# Coset/padded prototype checkpoint (April 25)

This file records the first concrete reversible prototype of the "coset"
idea inside `src/point_add/`.

## What was implemented

`src/point_add/coset_proto.rs` adds an **exact Bennett-clean padded-add
prototype** for secp256k1:

- accumulate repeated additions in an `(n + c_pad)`-bit non-modular workspace,
- compute a canonical `mod p` output **once** into a fresh register by folding
  the high padding bits using the exact identity
  `2^256 ≡ 2^32 + 977 (mod p)`,
- uncompute the padded workspace back to zero.

This is not a full Google-style coset implementation. It is a targeted probe of
one possible landing zone: replacing a short chain of modular adds with
padded/non-mod adds plus one exact cleanup.

## Commands

```bash
cargo test coset_proto -- --nocapture
```

## Measured results

### Classical-bit add chain (`mod_add_qb` style)

- `reps=3, cpad=2`
  - direct: `3072 CCX`, peak `1285`
  - coset proto: `4102 CCX`, peak `1799`
  - delta: `+1030 CCX`, `+514q`

- `reps=8, cpad=4`
  - direct: `8192 CCX`, peak `1285`
  - coset proto: `9264 CCX`, peak `1801`
  - delta: `+1072 CCX`, `+516q`

- `reps=12, cpad=4`
  - direct: `12288 CCX`, peak `1285`
  - coset proto: `11336 CCX`, peak `1801`
  - delta: `-952 CCX`, `+516q`

- `reps=16, cpad=5`
  - direct: `16384 CCX`, peak `1285`
  - coset proto: `14720 CCX`, peak `1802`
  - delta: `-1664 CCX`, `+517q`

### Quantum-register add chain (`mod_add_qq_fast` style)

- `reps=3, cpad=2`
  - direct: `3072 CCX`, peak `1285`
  - coset proto: `4102 CCX`, peak `2055`
  - delta: `+1030 CCX`, `+770q`

- `reps=8, cpad=4`
  - direct: `8192 CCX`, peak `1285`
  - coset proto: `9264 CCX`, peak `2057`
  - delta: `+1072 CCX`, `+772q`

- `reps=12, cpad=4`
  - direct: `12288 CCX`, peak `1285`
  - coset proto: `11336 CCX`, peak `2057`
  - delta: `-952 CCX`, `+772q`

- `reps=16, cpad=5`
  - direct: `16384 CCX`, peak `1285`
  - coset proto: `14720 CCX`, peak `2058`
  - delta: `-1664 CCX`, `+773q`

## Interpretation

The key observation is a **crossover**:

- below about **12 repeated adds**, exact canonicalization/cleanup dominates and
  the padded prototype loses,
- at about **12-16 repeated adds**, the padded prototype starts to win on
  Toffoli,
- but it still carries a large **qubit tax**: roughly `+516q` for classical-bit
  chains and `+772q` for quantum-register chains in these toy setups.

So:

- **Short affine correction chains are NOT a good first landing spot for
  coset/padded arithmetic.**
- The first plausible landing zone is a **long arithmetic region** with at
  least a dozen adds/subs sharing one cleanup.
- Even there, we must solve the qubit tax by reusing an already-live wide
  workspace instead of allocating the padded accumulator on top of the current
  live set.

## Current verdict

This quickly invalidates the easiest coset insertion point.
The next credible coset experiments should target:

1. long add/sub regions (not 3-8 add chains),
2. QROM/windowed batches where many adds share one cleanup,
3. or a wider scaffold rewrite where the output can remain non-canonical until
   the very end.
