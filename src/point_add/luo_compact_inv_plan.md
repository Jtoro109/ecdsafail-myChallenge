# Luo-style compact inversion: implementation plan

## Goal
Reduce Kaliski state from **1432q (4n + iters + 1)** to **~560q (3n + O(log n))**
following Luo 2025's register-sharing strategy.

## Source
Luo 2025 §3.2 (register sharing) + §3.3 (stepwise optimization) + Algorithm 3.

## Core idea: "location-controlled arithmetic"

The standard Kaliski keeps 4 n-bit registers `u, v, r, s` live throughout
all iterations. Luo observes:

- At any iteration k, exactly ONE of (u, v) is being "actively shrunk"
  (halved or subtracted). The other is waiting.
- Similarly for (r, s).
- We can use ONE shared "length register" Λ to track the effective
  bitlengths, and pack the logical values into a SINGLE shared storage
  indexed by Λ.

This is Proos-Zalka's register sharing made EXACT via length registers
(Luo's contribution over PZ).

## Concrete register layout

Luo's Algorithm 3 uses (for ECDLP, n=256):
- `Work1` register: 2n bits = 512 qubits. Stores concatenated (u, v) with
  a moving boundary given by `Λ_uv`.
- `Work2` register: 2n bits = 512 qubits. Stores concatenated (r, s) with
  moving boundary given by `Λ_rs`.  
- `Λ_uv`: log₂(2n) = 9 qubits. Length of u portion in Work1.
- `Λ_rs`: log₂(2n) = 9 qubits. Length of r portion in Work2.
- Iteration history: 2n bits = 512 qubits (same as our m_hist but for Luo's
  two control bits per iter).
- Total: 2n + 2n + 2·log n + 2n + O(1) ≈ 1547 qubits... hmm

Actually Luo claims 3n+4log n = 800 for inversion. Let me re-read.

Table 1: 3n + 4⌊log₂n⌋ + O(1) for inversion.

The breakdown must share more aggressively. Let me think:
- u+v share 2n qubits via length (but that doesn't save vs 2n = 2n).
- UNLESS they share the SAME n-qubit register, with Λ_uv telling us
  which bits are u vs v at any point.

If u and v collectively fit in n bits (sum of bitlengths ≤ n at all
times), they can share a SINGLE n-bit register. Similarly for r, s.

**Is sum(bitlen(u), bitlen(v)) ≤ n throughout?** Kaliski invariant:
gcd(u_0, v_0) = gcd(u_k, v_k). Initially u_0=p (n bits), v_0=x (n bits).
So sum(bitlen(u_0), bitlen(v_0)) = 2n at start. NOT ≤ n.

Hmm. So PZ's share-in-n-bits idea doesn't directly apply. Let me re-read
Luo more carefully.

## Revised reading

Luo's insight (§3.2): in the CLASSICAL execution, after each Kaliski
step, one of u or v shrinks by at least 1 bit. The sum bitlen(u) +
bitlen(v) decreases monotonically by ≥1 per iter. Starting at 2n,
after n iters it's ≤ n. So for the second half of the algorithm, u+v
DO share in n bits.

Luo's register sharing works for **the second half** but not the first.
The first half still needs 2n bits. Hmm.

Actually looking at Luo Table 1: `3n + 4⌊log₂n⌋` includes the HALF where
sharing works. First half uses 2n + n + log n structure (Work1=2n for
u,v, Work2=n for r OR s since one is small, Λ=log n).

Wait n=256, 3n = 768. That matches a structure like:
- Work1 (u || v): 2n = 512 qubits throughout
- Work2 (just the larger of r, s): n = 256 qubits, with the smaller
  stored in a smaller subregister (at most log n bits early on)
- Λ_uv: log n
- Λ_rs: log n
- Iter controls: 2 × 2n = 4n? No, 4 log n according to Luo's formula.

Hmm the 4 log n is the lengths, not iter history. Where's iter history
stored?

Re-read §3.3: Luo says iteration history is encoded IN the lengths
register via a cumulative counter, not per-iter bits. Each iter's
decision bit (swap / sub / halve) is NOT stored; instead, the ALGORITHM'S
REVERSIBILITY uses the CURRENT length registers at each step to decide
what to uncompute.

This is the key structural insight. **History-free reversibility via
length-register coupling.**

## Detailed sketch of Luo Algorithm 3

(Following Luo 2025 pseudocode, adapted.)

```
Input: v ∈ ℤ/pℤ, v ≠ 0
Output: v^{-1} mod p
Registers:
  W1[0..2n]:    shared (u, v). Initially [p || v], i.e. W1[0..n]=p,
                W1[n..2n]=v. Concatenated.
  W2[0..2n]:    shared (r, s). Initially [0 || 1].
  Λuv:          log(2n) bits, = n (pointer to u/v boundary in W1)
  Λrs:          log(2n) bits, = 0 initially

For k = 0 to 2n-1:
  1. Identify "active" halves based on Λuv, Λrs.
  2. Determine control bits (a, b) from Λuv and W1[Λuv-1] (lsb of v).
  3. Conditional operations based on (a, b):
     - if a: swap u, v (logically: rotate W1 based on Λuv).
     - if both odd: v -= u (in-place on W1).
     - v /= 2 (shift-right on v portion, updating Λuv).
     - r *= 2 (in W2, updating Λrs).
  4. Update Λuv, Λrs.

End for.

After loop: extract v^{-1} from W2 at position Λrs.
```

The REVERSIBILITY comes from: Λuv, Λrs uniquely determine the iteration
history (they only increase/decrease by 0 or 1 per iter, and the
direction is determined by the control bits which are recomputable
from W1/W2 state).

## Why this saves qubits

- W1, W2 together = 4n qubits — same as our (u, v, r, s).
- Λuv, Λrs = 2 log n = 18 qubits.
- NO iter history = **saves 512 qubits vs our m_hist (407) + other.**
- Total: 4n + 2 log n = 1042 qubits. Matches Luo's 3n+4logn if only
  3n of W1+W2 is kept live (using further sharing tricks in §3.3).

## Why this is HARDER than it looks

- Each iter's operations are CONTROLLED BY LENGTH REGISTERS (not direct
  qubit indices). This requires "location-controlled arithmetic" —
  operations where the position of data is quantum (indexed by Λ).
  Location-controlled add/sub is ~5-10x more expensive per op than
  fixed-position.
- Solinas reduction gets complex (needs location-dependent fold).

## Cost expected

Luo claims 204 n² log n Toffoli per inversion. For n=256, that's
204 × 65536 × 8 ≈ 107M Toffoli. Per point-add with 2 inversions:
~215M. Compare our 4.18M.

**So Luo's trade: -50% qubits, +50x Toffoli.**

## Implementation steps

1. Implement `alloc_luo_state(n)` returning `LuoState { W1, W2, Luv, Lrs }`.
2. Implement `loc_controlled_add/sub` — parameterized arithmetic.
3. Implement `luo_iteration_forward` and `luo_iteration_backward`.
4. Wire through `kaliski_forward` / `_backward` behind env var
   `KAL_LUO_COMPACT=1`.
5. Test at n=256 and measure qubit + Toffoli.

Each step is substantial. Full implementation: multi-session effort
(estimated 10-20 hours total).

## Alternative I'll try next

Given session constraints, focus on **location-agnostic compressions
that don't require full rewrite**:
- "Windowed cswap": merge 16 consecutive iters' cswaps into one
  lookup operation.
- Time-folding: share state between forward+mul1+backward at peak
  via controlled-free scheme.

Both are speculative. Main path remains Luo.
