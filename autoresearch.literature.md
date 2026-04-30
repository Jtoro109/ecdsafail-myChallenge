# Literature Sweep Notes

_Last updated: 2026-04-29._

Purpose: keep online/literature findings tied to the current low-scratch secp256k1 point-add search, so that public papers do not cause repeated detours.

## 2026-04-29 online sweep

### Google/Babbush-Zalcman-Gidney et al. 2026 (`arXiv:2603.28846`)

Relevant facts from the ZK appendix:

```text
low-qubit point-add claim: <=2,700,000 non-Clifford, <=1,175 logical qubits, <=17,000,000 ops
low-gate  point-add claim: <=2,100,000 non-Clifford, <=1,425 logical qubits, <=17,000,000 ops
validation inputs: 9,024 hash-derived secp256k1 point-add cases
full ECDLP uses w=16 and 28 windowed point additions
```

The paper explicitly withholds the circuit. It confirms the target is plausible and that average executed Toffoli/MBUC-style accounting is the right comparison, but it does not expose the missing DIV/selector primitive.

### Luo-Yang-Wang-Su-Li 2026, "Space-Efficient Quantum Algorithm for ECDLP" (`arXiv:2604.02311`)

This is the freshest public exact low-space ECDLP paper found in the arXiv sweep. It gives an exact reversible register-sharing EEA inversion:

```text
inversion width: 3n + 4 floor(log2 n) + O(1)
full ECDLP width at n=256: 1,333 logical qubits
one inversion Toffoli at n=256: about 1.97e8
asymptotic inversion Toffoli: about 204 n^2 log2(n)
```

Useful idea: they share decreasing `r` and increasing `t`, and store `(t,q,r)` / `(t',r')` in two packed work registers with location-controlled add/sub/swap. This is conceptually close to our parser/history problem.

Go/no-go for our target: **not a Toffoli route**. It misses the Google low-qubit scratch by roughly `1333 - 1175 = 158` total qubits and is two orders of magnitude too expensive in Toffoli. Treat as a register-sharing idea source only, not as a candidate point-add integration.

### Gu-Ye-Chen-Ma 2025, "Resource analysis ... improved quantum adder" (`arXiv:2510.23212`)

Focuses on carry-lookahead depth and 2D layout. It reports thousands of logical qubits and architecture/depth tradeoffs, not a low-Toffoli single point-add primitive. No direct route for the current Toffoli target.

### Measurement-based modular arithmetic papers (`arXiv:2407.20167`, `arXiv:2102.01453`)

They formalize MBUC for adders and modular arithmetic and report roughly 10--15% modular-adder Toffoli reductions in their architectures. We already use MBUC-like arithmetic and the remaining gap is structural (>1M Toffoli), so this is not a primary path. Use only for local primitive cleanup if a structural route is already viable.

### Gidney 2025 factoring / truncated residue arithmetic (`arXiv:2505.15917`)

Very relevant philosophically for low-qubit/high-approximation arithmetic, but it relies on approximate modular deviation and masked/truncated residues. Our current benchmark requires exact point-add correctness plus phase/ancilla cleanliness, so this cannot be imported directly. It reinforces the lesson that public low-qubit arithmetic often buys space with approximation or very large Toffoli counts.

### Chevignard-Fouque-Schrottenloher 2026 ePrint 2026/280

The Google paper cites this as the best public low-qubit ECDLP line, with under ~1200 logical qubits but more than 100 billion Toffoli gates. Direct ePrint PDF fetch was blocked by the site during this sweep, but the Google summary is enough for go/no-go: excellent qubit minimization, not a Toffoli candidate for a 2.7M point-add target.

## Current conclusion from public literature

No public paper found exposes a route that is both:

```text
<= 600--663 scratch beyond tx,ty
<= 2.7M point-add Toffoli
exact / phase-clean / ancilla-clean
```

The public low-qubit papers mostly trade Toffoli away by orders of magnitude. The promising search space remains custom structural primitives:

1. BY selected/window-local denominator generation below the current selector/plumbing cost.
2. Denominator-shift-free DIV recurrence that preserves the plus-minus scratch lesson without physical denominator movement.
3. Genuine phase-clean in-place DIV/multiply primitive.
4. Solinas history-carry scale correction only as a supporting optimization after a DIV body passes budget.

Immediate consequence: do not pivot to Luo/PZ-style location-controlled EEA as an integration path; use it only as inspiration for register-sharing/packing tests.
