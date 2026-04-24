//! Measure the exact Toffoli cost of each modular arithmetic primitive in
//! isolation. Test-only; emits numbers via eprintln for the planner.
//!
//! We don't need these for live correctness — just for honest cost accounting
//! so we can sanity-check SOTA reachability.

#![cfg(test)]

use super::{
    mod_add_qb, mod_add_qc, mod_add_qq, mod_mul_add_into_acc_schoolbook,
    mod_mul_sub_qq, mod_mul_write_into_zero_acc_schoolbook, mod_neg_inplace_fast, mod_sub_qb, N,
    SECP256K1_P,
};
use super::{B, QubitId};
use crate::circuit::OperationType;

fn count_ccx(ops: &[crate::circuit::Op]) -> usize {
    ops.iter()
        .filter(|o| matches!(o.kind, OperationType::CCX | OperationType::CCZ))
        .count()
}

fn new_builder_with_reg(n: usize) -> (B, Vec<QubitId>) {
    let mut b = B::new();
    let r = b.alloc_qubits(n);
    (b, r)
}

#[test]
fn cost_mul_write_schoolbook_n256() {
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let x = b.alloc_qubits(N);
    let y = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_mul_write_into_zero_acc_schoolbook(&mut b, &acc, &x, &y, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_mul_write_into_zero_acc_schoolbook(n=256): {} CCX", ccx);
}

#[test]
fn cost_mul_add_schoolbook_n256() {
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let x = b.alloc_qubits(N);
    let y = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_mul_add_into_acc_schoolbook(&mut b, &acc, &x, &y, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_mul_add_into_acc_schoolbook(n=256): {} CCX", ccx);
}

#[test]
fn cost_mul_sub_qq_n256() {
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let x = b.alloc_qubits(N);
    let y = b.alloc_qubits(N);
    let start = b.ops.len();
    mod_mul_sub_qq(&mut b, &acc, &x, &y, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_mul_sub_qq(n=256): {} CCX", ccx);
}

#[test]
fn cost_sub_qb_n256() {
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let bits = b.alloc_bits(N);
    let start = b.ops.len();
    mod_sub_qb(&mut b, &acc, &bits, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_sub_qb(n=256): {} CCX", ccx);
}

#[test]
fn cost_neg_inplace_fast_n256() {
    let (mut b, r) = new_builder_with_reg(N);
    let p = SECP256K1_P;
    let start = b.ops.len();
    mod_neg_inplace_fast(&mut b, &r, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("mod_neg_inplace_fast(n=256): {} CCX", ccx);
}
#[test]
fn cost_squaring_sub_n256() {
    use super::*;
    use crate::circuit::OperationType;
    fn count_ccx(ops: &[crate::circuit::Op]) -> usize {
        ops.iter().filter(|o| matches!(o.kind, OperationType::CCX | OperationType::CCZ)).count()
    }
    let mut b = B::new();
    let p = SECP256K1_P;
    let acc = b.alloc_qubits(N);
    let x = b.alloc_qubits(N);
    let start = b.ops.len();
    // mod_mul_sub_qq with same register is a squaring
    mod_mul_sub_qq(&mut b, &acc, &x, &x, p);
    let end = b.ops.len();
    let ccx = count_ccx(&b.ops[start..end]);
    eprintln!("squaring via mod_mul_sub_qq: {} CCX", ccx);
}
