# SOTA rebuild plan (REVISED for 1200q hard cap)

## What we know (from Google 2026 + literature)

SOTA is **two distinct operating points**:

| config     | qubits | Toffoli/Shor | Toffoli/point-add (implied) |
|------------|-------:|-------------:|----------------------------:|
| low-qubit  |   1200 |           90M|                       ~2.8M |
| low-gate   |   1450 |           70M|                       ~2.2M |
| Litinski'23|  ~2500 |          200M|                       ~6.2M |
| Chevignard |   1100 |         >100G|                      >3000M |

So:
- **At 1200 qubits the Toffoli budget is ~90M/32 ≈ 2.8M per single point-add**.
- **At 2500+ qubits the Toffoli budget is ~6M per single point-add**.
- We are currently at **4.18M Toffoli @ 2716 qubits**, which is *between* the Litinski operating point and the low-gate operating point.

## User hard cap: 1200 qubits

Given 1200 qubit cap, **we cannot** use any architecture with:
- Wide `r,s` registers (2n+1 = 513 each) — Kim-style
- 2n m_hist + 2n both_odd_hist = 1024 flags
- 4n Eea internal state (u, v, r, s at n each)

All of the above blow through 1200q when combined with `tx, ty` = 512 input qubits.

Budget accounting:
```
tx, ty                : 512 qubits (fixed, input-output)
Leftover budget       : 688 qubits for EVERYTHING else
```

## What fits in 688 qubits of ancilla

**Luo's register sharing** (3n + O(log n)): Work1 (n+3) + Work2 (n+3) + Length regs (~30) + Control regs (~7) + misc = **~560 qubits total for inversion**.

That leaves **~130 qubits** for the surrounding affine scaffold, which must share heavily across operations.

Toffoli cost of Luo's modular inversion: **~204n² log n ≈ 107M per inversion** at n=256. Way over our 4.18M budget per point-add.

## The unavoidable tradeoff

**Low-qubit = high-Toffoli.** There is no known algorithm that is BOTH <= 1200 qubits AND <= 4M Toffoli. The frontier is:
- Chevignard'26:    ~1100q, >100G Toffoli
- Google low-qubit: ~1200q, ~90M Toffoli / Shor (=~2.8M/pt-add)
- Google low-gate:  ~1450q, ~70M Toffoli / Shor
- Litinski:         ~2500q, ~50M Toffoli / Shor
- Ours:              2716q,  ~1.3G Toffoli / Shor (4.18M/pt-add × 32 windows)

The true SOTA point at 1200 qubits is ~2.8M Toffoli per point-add, which would be ~90M Toffoli total for full ECDLP. This is Google's withheld circuit.

## What our metric actually measures

Our harness measures **average executed Toffoli per single point-add**. It doesn't amortize across windowed Shor (as Google does), so the fair comparison IS per point-add:

| | qubits | Toffoli/point-add |
|--|-------:|------------------:|
| our build()     | 2716 |             4.18M |
| Google low-qubit| 1200 |             ~2.8M |
| Google low-gate | 1450 |             ~2.2M |

So the target at 1200q is ~2.8M, and at 1450q is ~2.2M.

## Status of what was built this session

Kim-style wide-r inversion primitive: **works correctly, but at 4102q peak** — the WRONG operating point for the 1200q target. Useful as proof-of-concept but must be replaced with a narrow-state Luo/PZ-style inversion to hit the user's cap.

## Revised path to 1200q / 2.8M Toffoli

The Kim work is reusable as a classical validation harness but not as a live primitive. The correct primitive for 1200q is:

1. **Luo 2025's Algorithm 3**: single (n+3) Work register holding (t, q, r) via register sharing, with a matching Work2 for (t', r'). 4-phase binary long-division EEA.
2. Input-dependent phase progression (Phase 1-4 cycles driven by classical-flag comparisons) instead of uniform 2n iterations.
3. Per-step cost is small (~10-20 CCX), but ~5.76n steps per EEA iteration, and multiple EEA iterations. Total Toffoli comes out at Luo's stated 204n² log n ≈ 107M.

But wait — that's 107M per inversion, and we do 2 inversions, so 214M. Way worse than Google's 90M total. That means Luo alone ≠ SOTA.

**Google's unpublished trick**: likely some combination of:
- Luo-style register sharing +
- Batched windowed arithmetic reducing the inversion to amortized cost across a ZK-proofed ECDLP instance +
- Montgomery representation throughout to avoid explicit FastModulo reductions +
- Possibly spooky pebbling / qubit recycling via measurement

**We cannot match SOTA exactly without their unpublished circuit.** But we can target the publicly-reachable frontier: Luo 2025's 1333q / ~976n³ Toffoli at n=256 = ~16B Toffoli TOTAL = ~500M per point-add. Even that is worse than our current 4.18M.

## Honest conclusion

The **public** literature does not contain a 1200q-compatible algorithm with <= 4M Toffoli per point-add. Google's circuit is withheld. To match them we need either:
(a) independently rediscover their tricks, or
(b) accept a different operating point (e.g., 1600-2000q with ~3M Toffoli).

For this session, realistic targets given what's built:

**Target A (ambitious)**: build a Luo-style inversion in 500-600 qubits, giving a total point-add peak of ~1300-1400 qubits. Toffoli will be much higher than 4.18M — perhaps 50-100M per point-add. This matches Chevignard's operating point.

**Target B (conservative)**: port HRSL's 8n ≈ 2060q point-add scaffold (which is closer to our current budget and has a published, reproducible Toffoli cost). This drops qubits from 2716 to ~2060 without an explosion in Toffoli.

**Target C (realistic)**: keep our current ~2700q scaffold and focus on Toffoli reductions via Kim-style wide-r inversion (the work done this session). Target: ~3M Toffoli per point-add at ~4000q.

**The user wants 1200q. That requires Target A: accept 50-100M Toffoli per point-add.** That's a different metric than our current harness optimizes.

## Recommendation

Given the gap between 4.18M Toffoli and what's achievable at 1200q in the public literature, we should:
1. Explicitly acknowledge that the "both low qubits AND low Toffoli" configuration is Google's withheld circuit and not publicly reachable.
2. Pick an operating point: either (a) rebuild for 1200q accepting higher Toffoli, or (b) optimize Toffoli at current qubit count.
3. Update the session goal / primary metric accordingly.
