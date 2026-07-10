//! Micro-benchmark module for exploring structural changes WITHOUT
//! touching the top-level point-add benchmark.
//!
//! Each function builds a sub-circuit in isolation, reports:
//!   - local peak qubits (upper bound on what the primitive adds on top of
//!     its input register footprint)
//!   - emitted CCX+CCZ ("Toffoli") count
//!
//! Run with:
//!   cargo test -p quantum_ecc microbench -- --nocapture
//!
//! All tests are opt-in and slow, so they are gated behind the
//! `MICROBENCH=1` environment variable.

use alloy_primitives::U256;

use super::{
    add_nbit_qq, ripple_adder_add_fast, ripple_adder_sub_fast, mod_mul_add_into_acc_karatsuba,
    mod_mul_add_into_acc_karatsuba2, mod_mul_add_into_acc_karatsuba_lowq,
    mod_mul_add_into_acc_schoolbook, mod_mul_add_qb, mod_mul_sub_qb,
    mod_mul_write_into_zero_acc_karatsuba, mod_mul_write_into_zero_acc_karatsuba2,
    mod_mul_write_into_zero_acc_schoolbook, mod_mul_write_into_zero_acc_schoolbook_lowq,
    schoolbook_mul_into_addsub, schoolbook_mul_into_addsub_inverse,
    schoolbook_mul_into_addsub_lowq, schoolbook_mul_into_addsub_lowq_inverse, B, N, SECP256K1_P,
};
use crate::circuit::{Op, OperationType, NO_BIT};

fn count_toffoli(ops: &[Op]) -> usize {
    ops.iter()
        .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
        .count()
}

#[derive(Clone, Copy)]
struct Measured {
    total_qubits: u32,
    peak_qubits: u32,
    toffoli: usize,
    conditioned_toffoli: usize,
    expected_random_toffoli: f64,
    ops: usize,
}

fn measure<F: FnOnce(&mut B)>(f: F) -> Measured {
    let mut b = B::new();
    f(&mut b);
    let (conditioned_toffoli, expected_random_toffoli) = conditioned_toffoli_profile(&b.ops);
    Measured {
        total_qubits: b.next_qubit,
        peak_qubits: b.peak_qubits,
        toffoli: count_toffoli(&b.ops),
        conditioned_toffoli,
        expected_random_toffoli,
        ops: b.ops.len(),
    }
}

fn conditioned_toffoli_profile(ops: &[Op]) -> (usize, f64) {
    let mut stack_depth = 0usize;
    let mut conditioned = 0usize;
    let mut expected = 0.0f64;
    for op in ops {
        match op.kind {
            OperationType::PushCondition => {
                stack_depth += 1;
                continue;
            }
            OperationType::PopCondition => {
                stack_depth = stack_depth.saturating_sub(1);
                continue;
            }
            _ => {}
        }
        if matches!(op.kind, OperationType::CCX | OperationType::CCZ) {
            let inline = usize::from(op.c_condition != NO_BIT);
            let depth = stack_depth + inline;
            if depth > 0 {
                conditioned += 1;
            }
            expected += 2f64.powi(-(depth as i32));
        }
    }
    (conditioned, expected)
}

fn print_row(label: &str, m: &Measured) {
    println!(
        "microbench | {:<42} | toffoli={:>8} conditioned_t={:>8} expected_random_t={:>10.1} ops={:>9} peak_q={:>5} total_q={:>5}",
        label,
        m.toffoli,
        m.conditioned_toffoli,
        m.expected_random_toffoli,
        m.ops,
        m.peak_qubits,
        m.total_qubits
    );
}

fn enabled() -> bool {
    std::env::var("MICROBENCH")
        .ok()
        .map(|v| v != "0")
        .unwrap_or(false)
}

fn fill_x(b: &mut B, n: usize) -> Vec<super::QubitId> {
    // Allocate n qubits, pretend they are inputs (no initial X to keep it
    // simple; measurements here don't care about semantic value, only cost).
    b.alloc_qubits(n)
}

/// Build just `x + a` (non-modular, n-bit) via `ripple_adder_add_fast` to
/// measure the RippleAdder-fast peak/Toffoli envelope.
fn bench_ripple_adder_add_fast(n: usize) -> Measured {
    measure(|b| {
        let x = fill_x(b, n);
        let acc = fill_x(b, n);
        let c_in = b.alloc_qubit();
        ripple_adder_add_fast(b, &x, &acc, c_in);
        b.free(c_in);
        b.free_vec(&acc);
        b.free_vec(&x);
    })
}

/// Same as above but using the non-fast `add_nbit_qq` (uses a single c_in
/// ancilla and no carry register). Trades Toffoli ↑ for peak ↓.
fn bench_ripple_adder_add_slow(n: usize) -> Measured {
    measure(|b| {
        let x = fill_x(b, n);
        let acc = fill_x(b, n);
        add_nbit_qq(b, &x, &acc);
        b.free_vec(&acc);
        b.free_vec(&x);
    })
}

/// Reverse direction of ripple_adder_add_fast (for symmetry sanity only).
fn bench_ripple_adder_sub_fast(n: usize) -> Measured {
    measure(|b| {
        let x = fill_x(b, n);
        let acc = fill_x(b, n);
        let c_in = b.alloc_qubit();
        ripple_adder_sub_fast(b, &x, &acc, c_in);
        b.free(c_in);
        b.free_vec(&acc);
        b.free_vec(&x);
    })
}

/// write_into_zero schoolbook: tmp_ext and inv done inside the primitive.
fn bench_mul_basic_write() -> Measured {
    let p = SECP256K1_P;
    measure(|b| {
        let x = fill_x(b, N);
        let y = fill_x(b, N);
        let acc = fill_x(b, N); // assumed zero
        mod_mul_write_into_zero_acc_schoolbook(b, &acc, &x, &y, p);
        b.free_vec(&acc);
        b.free_vec(&y);
        b.free_vec(&x);
    })
}

fn bench_mul_add_schoolbook() -> Measured {
    let p = SECP256K1_P;
    measure(|b| {
        let x = fill_x(b, N);
        let y = fill_x(b, N);
        let acc = fill_x(b, N);
        mod_mul_add_into_acc_schoolbook(b, &acc, &x, &y, p);
        b.free_vec(&acc);
        b.free_vec(&y);
        b.free_vec(&x);
    })
}

/// write_into_zero schoolbook low-q variant. Same semantics as
/// `bench_mul_basic_write` but using the lowq addsub internals. We
/// observed in build() that wiring this in at state_a_mul1 produces a
/// deterministic phase-garbage batch (1/20480 shots on ALT_SEED tag=5),
/// so this is here purely to measure cost, NOT as a drop-in.
fn bench_mul_basic_lowq_write() -> Measured {
    let p = SECP256K1_P;
    measure(|b| {
        let x = fill_x(b, N);
        let y = fill_x(b, N);
        let acc = fill_x(b, N);
        mod_mul_write_into_zero_acc_schoolbook_lowq(b, &acc, &x, &y, p);
        b.free_vec(&acc);
        b.free_vec(&y);
        b.free_vec(&x);
    })
}

fn bench_mul_fast_write() -> Measured {
    let p = SECP256K1_P;
    measure(|b| {
        let x = fill_x(b, N);
        let y = fill_x(b, N);
        let acc = fill_x(b, N);
        mod_mul_write_into_zero_acc_karatsuba(b, &acc, &x, &y, p);
        b.free_vec(&acc);
        b.free_vec(&y);
        b.free_vec(&x);
    })
}

fn bench_mul_add_karatsuba() -> Measured {
    let p = SECP256K1_P;
    measure(|b| {
        let x = fill_x(b, N);
        let y = fill_x(b, N);
        let acc = fill_x(b, N);
        mod_mul_add_into_acc_karatsuba(b, &acc, &x, &y, p);
        b.free_vec(&acc);
        b.free_vec(&y);
        b.free_vec(&x);
    })
}

fn bench_mul_fast2_write() -> Measured {
    let p = SECP256K1_P;
    measure(|b| {
        let x = fill_x(b, N);
        let y = fill_x(b, N);
        let acc = fill_x(b, N);
        mod_mul_write_into_zero_acc_karatsuba2(b, &acc, &x, &y, p);
        b.free_vec(&acc);
        b.free_vec(&y);
        b.free_vec(&x);
    })
}

/// Isolated schoolbook_mul_into_addsub forward+inverse pair.
fn bench_schoolbook_addsub_pair(lowq: bool) -> Measured {
    measure(|b| {
        let x = fill_x(b, N);
        let y = fill_x(b, N);
        let tmp_ext = b.alloc_qubits(2 * N);
        if lowq {
            schoolbook_mul_into_addsub_lowq(b, &x, &y, &tmp_ext);
            schoolbook_mul_into_addsub_lowq_inverse(b, &x, &y, &tmp_ext);
        } else {
            schoolbook_mul_into_addsub(b, &x, &y, &tmp_ext);
            schoolbook_mul_into_addsub_inverse(b, &x, &y, &tmp_ext);
        }
        b.free_vec(&tmp_ext);
        b.free_vec(&y);
        b.free_vec(&x);
    })
}

/// Forward-only schoolbook_mul_into_addsub (fast vs lowq).
fn bench_schoolbook_addsub_forward(lowq: bool) -> Measured {
    measure(|b| {
        let x = fill_x(b, N);
        let y = fill_x(b, N);
        let tmp_ext = b.alloc_qubits(2 * N);
        if lowq {
            schoolbook_mul_into_addsub_lowq(b, &x, &y, &tmp_ext);
        } else {
            schoolbook_mul_into_addsub(b, &x, &y, &tmp_ext);
        }
        b.free_vec(&tmp_ext);
        b.free_vec(&y);
        b.free_vec(&x);
    })
}

fn bench_mul_add_karatsuba2() -> Measured {
    let p = SECP256K1_P;
    measure(|b| {
        let x = fill_x(b, N);
        let y = fill_x(b, N);
        let acc = fill_x(b, N);
        mod_mul_add_into_acc_karatsuba2(b, &acc, &x, &y, p);
        b.free_vec(&acc);
        b.free_vec(&y);
        b.free_vec(&x);
    })
}

fn bench_mul_add_qb() -> Measured {
    let p = SECP256K1_P;
    measure(|b| {
        let x = fill_x(b, N);
        let y = b.alloc_bits(N);
        let acc = fill_x(b, N);
        mod_mul_add_qb(b, &acc, &x, &y, p);
        b.free_vec(&acc);
        b.free_vec(&x);
    })
}

fn bench_mul_sub_qb() -> Measured {
    let p = SECP256K1_P;
    measure(|b| {
        let x = fill_x(b, N);
        let y = b.alloc_bits(N);
        let acc = fill_x(b, N);
        mod_mul_sub_qb(b, &acc, &x, &y, p);
        b.free_vec(&acc);
        b.free_vec(&x);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_adders() {
        if !enabled() {
            return;
        }
        for n in [64, 128, 256] {
            print_row(
                &format!("ripple_adder_add_fast n={}", n),
                &bench_ripple_adder_add_fast(n),
            );
            print_row(
                &format!("ripple_adder_add_slow n={}", n),
                &bench_ripple_adder_add_slow(n),
            );
            print_row(
                &format!("ripple_adder_sub_fast n={}", n),
                &bench_ripple_adder_sub_fast(n),
            );
        }
    }

    #[test]
    fn bench_muls() {
        if !enabled() {
            return;
        }
        let sb_w = bench_mul_basic_write();
        let sb_lw = bench_mul_basic_lowq_write();
        let sb_a = bench_mul_add_schoolbook();
        let k1_w = bench_mul_fast_write();
        let k1_a = bench_mul_add_karatsuba();
        let k2_w = bench_mul_fast2_write();
        let k2_a = bench_mul_add_karatsuba2();
        print_row("mul_basic_write_zero", &sb_w);
        print_row("mul_basic_lowq_write_zero", &sb_lw);
        print_row("mul_basic_add", &sb_a);
        println!(
            "microbench | summary | schoolbook_fast→schoolbook_lowq (write_zero): toff {:+}, peak {:+}",
            sb_lw.toffoli as i64 - sb_w.toffoli as i64,
            sb_lw.peak_qubits as i64 - sb_w.peak_qubits as i64
        );
        print_row("mul_fast1_write_zero", &k1_w);
        print_row("mul_fast1_add", &k1_a);
        print_row("mul_fast2_write_zero", &k2_w);
        print_row("mul_fast2_add", &k2_a);

        // Summary deltas to drive structural choices at a glance.
        println!(
            "microbench | summary | schoolbook→karatsuba1: toff {:+}, peak {:+}",
            k1_a.toffoli as i64 - sb_a.toffoli as i64,
            k1_a.peak_qubits as i64 - sb_a.peak_qubits as i64
        );
        println!(
            "microbench | summary | schoolbook→karatsuba2: toff {:+}, peak {:+}",
            k2_a.toffoli as i64 - sb_a.toffoli as i64,
            k2_a.peak_qubits as i64 - sb_a.peak_qubits as i64
        );
        println!(
            "microbench | summary | karatsuba1→karatsuba2: toff {:+}, peak {:+}",
            k2_a.toffoli as i64 - k1_a.toffoli as i64,
            k2_a.peak_qubits as i64 - k1_a.peak_qubits as i64
        );
        print_row(
            "schoolbook_addsub_fast   (forward+inverse)",
            &bench_schoolbook_addsub_pair(false),
        );
        print_row(
            "schoolbook_addsub_lowq   (forward+inverse)",
            &bench_schoolbook_addsub_pair(true),
        );
        print_row(
            "schoolbook_addsub_fast   (forward only)",
            &bench_schoolbook_addsub_forward(false),
        );
        print_row(
            "schoolbook_addsub_lowq   (forward only)",
            &bench_schoolbook_addsub_forward(true),
        );

        // New lowq Karatsuba variant: should show +Toffoli, -peak_q vs karatsuba1.
        let lowq_add = measure(|b| {
            let p = SECP256K1_P;
            let x = fill_x(b, N);
            let y = fill_x(b, N);
            let acc = fill_x(b, N);
            mod_mul_add_into_acc_karatsuba_lowq(b, &acc, &x, &y, p);
            b.free_vec(&acc);
            b.free_vec(&y);
            b.free_vec(&x);
        });
        print_row("mul_fast1_lowq_add", &lowq_add);
    }

    #[test]
    fn bench_qb_muls() {
        if !enabled() {
            return;
        }
        let add = bench_mul_add_qb();
        let sub = bench_mul_sub_qb();
        print_row("mul_add_qb_push_condition", &add);
        print_row("mul_sub_qb_push_condition", &sub);
        println!(
            "microbench | summary | qb add expected/static ratio={:.4} sub expected/static ratio={:.4}",
            add.expected_random_toffoli / add.toffoli as f64,
            sub.expected_random_toffoli / sub.toffoli as f64
        );
    }
}
