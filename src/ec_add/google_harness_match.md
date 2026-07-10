# Harness match with Google 2026 ZKP

**CONFIRMED**: Our harness matches Google's Zenodo ZKP harness
(https://zenodo.org/doi/10.5281/zenodo.19597130).

## Shared structure
- 9024 test cases per proof
- SHAKE256 XOF seeded with circuit bytes (Fiat-Shamir heuristic)
- 4 registers: target_x (256q), target_y (256q), offset_x (256b classical),
  offset_y (256b classical)
- Same secp256k1 curve constants
- Same op types: X, CX, CCX, Z, CZ, CCZ, NEG, SWAP, R, HMR, BIT_INVERT,
  BIT_STORE0, BIT_STORE1, PUSH_CONDITION, POP_CONDITION, REGISTER,
  APPEND_TO_REGISTER, DEBUG_PRINT (kickmix instruction set)
- Same simulator: u64-parallel shot execution, global phase tracking,
  register extraction
- Same correctness checks: output match, phase garbage, ancilla garbage

## Google's achievement
- Low-Qubit: **1175q, 2,700,000 Toffoli, 17,000,000 total ops**
- Low-Gate: **1425q, 2,100,000 Toffoli, 17,000,000 total ops**

Proofs available in Zenodo as:
- `proof_9024.bin` for low_qubits (SHA256: `5373e67c...`)
- `proof_9024.bin` for low_toffoli (SHA256: `04f17175...`)

**The circuits themselves are NOT released** (only the ZK proofs that
certify the existence of such circuits).

## Our current position
- **2716q, 4,180,502 Toffoli** on the same harness
- 2.3x qubits over Google's low-qubit, 1.5x over low-gate
- 1.55x Toffoli over Google's low-qubit, 2.0x over low-gate

## Gap analysis — what Google did that we haven't

From paper:
- Coset representation (Zalka/GE21): non-modular adds via padded superposition
- Windowed arithmetic (Gidney 2019): QROM table lookups for mul/add
- Venting adders (Gidney 2025, arxiv 2507.23079): 3-clean-qubit const adders
  with `4n±O(1)` Toffoli via carry-venting + phase-fix via Häner carry-xor
- Litinski scaffold (standard)

## Venting adder details (from paper we found)

Gidney's arxiv 2507.23079 "A Classical-Quantum Adder with Constant
Workspace and Linear Gates" gives:

- **3 clean ancilla + 4n±O(1) Toffoli** variant (Figure 5)
- **2 clean + (n-2) dirty ancilla + 3n±O(1) Toffoli** variant (Figure 4)

The "venting" technique: carry bits during ripple are Z-redundant
(reconstructable from the sum). Measure them in X basis (HMR-style) to
free them, then later fix the resulting phase-flip tasks via Häner
carry-xor circuits.

Python reference code: https://zenodo.org/doi/10.5281/zenodo.15866587

## Directions left to try

### A: Port Gidney's venting adder (arxiv 2507.23079)
Replace our ripple_adder_add_fast (which uses n-1 carries) with 3-clean-qubit
variant. Saves n-1 qubits at each add, costs 3n-5n extra Toffoli per add.

**Estimated effort**: 500-1000 lines of careful Rust. Multi-session.

**Expected impact**: peak drops at every add site. 2716 → 1800-2000.
Toffoli increase: ~40% from current 4M → ~6M. Still better than 66M
(HRSL Low-W) at similar qubit count.

### B: Coset representation
Most theoretically impactful but requires harness modification (c_pad
padding bits) OR accepts rare failure tolerance.

### C: Windowed arithmetic for single-pt-add
Not typically applicable (no classical window). Would require
redesigning the computation.

## Conclusion

Google's withheld circuits use established techniques (coset + windowed
+ venting + Litinski) combined in specific ways that aren't publicly
detailed. The venting adder gives us a concrete, implementable way to
approach their qubit count, at moderate Toffoli cost.

**Next session priority**: implement venting adder infrastructure as a
reusable primitive (`vent_add_qc`, `vent_sub_qc`), test at small n,
then wire into schoolbook_mul and Eea step 4.
