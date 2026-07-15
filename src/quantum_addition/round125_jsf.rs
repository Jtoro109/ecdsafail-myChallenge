//! Round125 JSF endpoint-operator emission helpers.
//!
//! This is a benchable lowerer for the Round125 endpoint-operator idea, not a
//! complete point-add path. It keeps the Google PA ABI registers intact, parses
//! a two-lane JSF stream from the quantum coefficient lanes, applies the signed
//! endpoint operator into scratch, then reverses the operator and parser so the
//! circuit is fuzzable as a clean roundtrip.

use crate::circuit::QubitId;

use super::B;

#[derive(Clone, Copy)]
enum Wire {
    Q(QubitId),
    Const(bool),
}

impl Wire {
    fn from_slice(bits: &[QubitId], idx: usize) -> Self {
        bits.get(idx)
            .copied()
            .map(Wire::Q)
            .unwrap_or(Wire::Const(false))
    }
}

fn xor_wire_into(b: &mut B, wire: Wire, target: QubitId) {
    match wire {
        Wire::Q(q) => b.cx(q, target),
        Wire::Const(true) => b.x(target),
        Wire::Const(false) => {}
    }
}

fn and_wire_wire_toggle(b: &mut B, left: Wire, right: Wire, target: QubitId) {
    match (left, right) {
        (Wire::Const(false), _) | (_, Wire::Const(false)) => {}
        (Wire::Const(true), Wire::Const(true)) => b.x(target),
        (Wire::Const(true), Wire::Q(q)) | (Wire::Q(q), Wire::Const(true)) => b.cx(q, target),
        (Wire::Q(a), Wire::Q(c)) => b.ccx(a, c, target),
    }
}

fn emit_choose_jsf_digit_controls(
    b: &mut B,
    x0: Wire,
    x1: Wire,
    x2: Wire,
    carry_x: QubitId,
    y0: Wire,
    y1: Wire,
    carry_y: QubitId,
    d_plus: QubitId,
    d_minus: QubitId,
) {
    let s0 = b.alloc_qubit();
    let carry1 = b.alloc_qubit();
    let s1 = b.alloc_qubit();
    let carry2 = b.alloc_qubit();
    let s2 = b.alloc_qubit();
    let t0 = b.alloc_qubit();
    let tcarry = b.alloc_qubit();
    let t1 = b.alloc_qubit();
    let sxor = b.alloc_qubit();
    let y_is_two = b.alloc_qubit();
    let flip = b.alloc_qubit();
    let sign_minus = b.alloc_qubit();

    xor_wire_into(b, x0, s0);
    b.cx(carry_x, s0);

    and_wire_wire_toggle(b, x0, Wire::Q(carry_x), carry1);

    xor_wire_into(b, x1, s1);
    b.cx(carry1, s1);

    and_wire_wire_toggle(b, x1, Wire::Q(carry1), carry2);

    xor_wire_into(b, x2, s2);
    b.cx(carry2, s2);

    xor_wire_into(b, y0, t0);
    b.cx(carry_y, t0);

    and_wire_wire_toggle(b, y0, Wire::Q(carry_y), tcarry);

    xor_wire_into(b, y1, t1);
    b.cx(tcarry, t1);

    b.cx(s1, sxor);
    b.cx(s2, sxor);

    b.x(t0);
    b.ccx(t1, t0, y_is_two);
    b.x(t0);

    b.ccx(sxor, y_is_two, flip);

    b.cx(s1, sign_minus);
    b.cx(flip, sign_minus);

    b.ccx(s0, sign_minus, d_minus);
    b.x(sign_minus);
    b.ccx(s0, sign_minus, d_plus);
    b.x(sign_minus);

    b.cx(flip, sign_minus);
    b.cx(s1, sign_minus);
    b.ccx(sxor, y_is_two, flip);
    b.x(t0);
    b.ccx(t1, t0, y_is_two);
    b.x(t0);
    b.cx(s2, sxor);
    b.cx(s1, sxor);
    b.cx(tcarry, t1);
    xor_wire_into(b, y1, t1);
    and_wire_wire_toggle(b, y0, Wire::Q(carry_y), tcarry);
    b.cx(carry_y, t0);
    xor_wire_into(b, y0, t0);
    b.cx(carry2, s2);
    xor_wire_into(b, x2, s2);
    and_wire_wire_toggle(b, x1, Wire::Q(carry1), carry2);
    b.cx(carry1, s1);
    xor_wire_into(b, x1, s1);
    and_wire_wire_toggle(b, x0, Wire::Q(carry_x), carry1);
    b.cx(carry_x, s0);
    xor_wire_into(b, x0, s0);

    b.free(sign_minus);
    b.free(flip);
    b.free(y_is_two);
    b.free(sxor);
    b.free(t1);
    b.free(tcarry);
    b.free(t0);
    b.free(s2);
    b.free(carry2);
    b.free(s1);
    b.free(carry1);
    b.free(s0);
}

fn toggle_next_carry(
    b: &mut B,
    input_bit: Wire,
    carry_in: QubitId,
    d_minus: QubitId,
    carry_out: QubitId,
) {
    b.cx(d_minus, carry_out);
    and_wire_wire_toggle(b, input_bit, Wire::Q(carry_in), carry_out);
}

fn toggle_or_into(b: &mut B, left: QubitId, right: QubitId, target: QubitId) {
    b.cx(left, target);
    b.cx(right, target);
    b.ccx(left, right, target);
}

fn toggle_first_occupied(b: &mut B, occupied: QubitId, seen: QubitId, first: QubitId) {
    b.x(seen);
    b.ccx(occupied, seen, first);
    b.x(seen);
}

fn emit_controlled_operator_unit(
    b: &mut B,
    control: QubitId,
    source: &[QubitId],
    acc: &[QubitId],
    salt: usize,
) {
    debug_assert_eq!(source.len(), acc.len());
    for j in 0..acc.len() {
        b.ccx(control, source[(j + salt) % source.len()], acc[j]);
    }
}

struct StepControls {
    d0_plus: QubitId,
    d0_minus: QubitId,
    d1_plus: QubitId,
    d1_minus: QubitId,
    nz0: QubitId,
    nz1: QubitId,
    occupied: QubitId,
    first: QubitId,
}

impl StepControls {
    fn alloc(b: &mut B) -> Self {
        Self {
            d0_plus: b.alloc_qubit(),
            d0_minus: b.alloc_qubit(),
            d1_plus: b.alloc_qubit(),
            d1_minus: b.alloc_qubit(),
            nz0: b.alloc_qubit(),
            nz1: b.alloc_qubit(),
            occupied: b.alloc_qubit(),
            first: b.alloc_qubit(),
        }
    }

    fn free(self, b: &mut B) {
        b.free(self.first);
        b.free(self.occupied);
        b.free(self.nz1);
        b.free(self.nz0);
        b.free(self.d1_minus);
        b.free(self.d1_plus);
        b.free(self.d0_minus);
        b.free(self.d0_plus);
    }
}

fn toggle_digit_controls(
    b: &mut B,
    x0: &[QubitId],
    x1: &[QubitId],
    carry0: &[QubitId],
    carry1: &[QubitId],
    bit: usize,
    controls: &StepControls,
) {
    emit_choose_jsf_digit_controls(
        b,
        Wire::from_slice(x0, bit),
        Wire::from_slice(x0, bit + 1),
        Wire::from_slice(x0, bit + 2),
        carry0[bit],
        Wire::from_slice(x1, bit),
        Wire::from_slice(x1, bit + 1),
        carry1[bit],
        controls.d0_plus,
        controls.d0_minus,
    );
    emit_choose_jsf_digit_controls(
        b,
        Wire::from_slice(x1, bit),
        Wire::from_slice(x1, bit + 1),
        Wire::from_slice(x1, bit + 2),
        carry1[bit],
        Wire::from_slice(x0, bit),
        Wire::from_slice(x0, bit + 1),
        carry0[bit],
        controls.d1_plus,
        controls.d1_minus,
    );
}

fn compute_derived_controls(b: &mut B, seen: QubitId, controls: &StepControls) {
    b.cx(controls.d0_plus, controls.nz0);
    b.cx(controls.d0_minus, controls.nz0);
    b.cx(controls.d1_plus, controls.nz1);
    b.cx(controls.d1_minus, controls.nz1);
    toggle_or_into(b, controls.nz0, controls.nz1, controls.occupied);
    toggle_first_occupied(b, controls.occupied, seen, controls.first);
}

fn uncompute_derived_controls(b: &mut B, seen: QubitId, controls: &StepControls) {
    toggle_first_occupied(b, controls.occupied, seen, controls.first);
    toggle_or_into(b, controls.nz0, controls.nz1, controls.occupied);
    b.cx(controls.d1_minus, controls.nz1);
    b.cx(controls.d1_plus, controls.nz1);
    b.cx(controls.d0_minus, controls.nz0);
    b.cx(controls.d0_plus, controls.nz0);
}

fn emit_operator_units(
    b: &mut B,
    controls: &StepControls,
    source: &[QubitId],
    acc: &[QubitId],
    bit: usize,
) {
    emit_controlled_operator_unit(b, controls.d0_plus, source, acc, bit);
    emit_controlled_operator_unit(b, controls.d0_minus, source, acc, bit + 17);
    emit_controlled_operator_unit(b, controls.d1_plus, source, acc, bit + 31);
    emit_controlled_operator_unit(b, controls.d1_minus, source, acc, bit + 47);
    emit_controlled_operator_unit(b, controls.occupied, source, acc, bit + 71);
    emit_controlled_operator_unit(b, controls.occupied, source, acc, bit + 97);
    emit_controlled_operator_unit(b, controls.first, source, acc, bit + 127);
}

fn emit_forward_step(
    b: &mut B,
    x0: &[QubitId],
    x1: &[QubitId],
    carry0: &[QubitId],
    carry1: &[QubitId],
    seen: &[QubitId],
    source: &[QubitId],
    acc: &[QubitId],
    bit: usize,
) {
    let controls = StepControls::alloc(b);
    toggle_digit_controls(b, x0, x1, carry0, carry1, bit, &controls);
    compute_derived_controls(b, seen[bit], &controls);
    emit_operator_units(b, &controls, source, acc, bit);
    toggle_next_carry(
        b,
        Wire::from_slice(x0, bit),
        carry0[bit],
        controls.d0_minus,
        carry0[bit + 1],
    );
    toggle_next_carry(
        b,
        Wire::from_slice(x1, bit),
        carry1[bit],
        controls.d1_minus,
        carry1[bit + 1],
    );
    toggle_or_into(b, seen[bit], controls.occupied, seen[bit + 1]);
    uncompute_derived_controls(b, seen[bit], &controls);
    toggle_digit_controls(b, x0, x1, carry0, carry1, bit, &controls);
    controls.free(b);
}

fn emit_reverse_step(
    b: &mut B,
    x0: &[QubitId],
    x1: &[QubitId],
    carry0: &[QubitId],
    carry1: &[QubitId],
    seen: &[QubitId],
    source: &[QubitId],
    acc: &[QubitId],
    bit: usize,
) {
    let controls = StepControls::alloc(b);
    toggle_digit_controls(b, x0, x1, carry0, carry1, bit, &controls);
    compute_derived_controls(b, seen[bit], &controls);
    toggle_or_into(b, seen[bit], controls.occupied, seen[bit + 1]);
    toggle_next_carry(
        b,
        Wire::from_slice(x1, bit),
        carry1[bit],
        controls.d1_minus,
        carry1[bit + 1],
    );
    toggle_next_carry(
        b,
        Wire::from_slice(x0, bit),
        carry0[bit],
        controls.d0_minus,
        carry0[bit + 1],
    );
    emit_operator_units(b, &controls, source, acc, bit);
    uncompute_derived_controls(b, seen[bit], &controls);
    toggle_digit_controls(b, x0, x1, carry0, carry1, bit, &controls);
    controls.free(b);
}

pub(super) fn emit_round125_jsf_operator_roundtrip(
    b: &mut B,
    coeff0: &[QubitId],
    coeff1: &[QubitId],
) {
    assert_eq!(coeff0.len(), coeff1.len());
    assert!(!coeff0.is_empty());

    let width = coeff0.len();
    let digit_bits = width + 1;
    let source = b.alloc_qubits(width);
    let acc = b.alloc_qubits(width);
    let carry0 = b.alloc_qubits(digit_bits + 1);
    let carry1 = b.alloc_qubits(digit_bits + 1);
    let seen = b.alloc_qubits(digit_bits + 1);

    b.set_phase("round125_jsf_source_seed");
    for &q in &source {
        b.x(q);
    }

    b.set_phase("round125_jsf_forward_operator");
    for bit in 0..digit_bits {
        emit_forward_step(
            b, coeff0, coeff1, &carry0, &carry1, &seen, &source, &acc, bit,
        );
    }

    b.set_phase("round125_jsf_reverse_operator");
    for bit in (0..digit_bits).rev() {
        emit_reverse_step(
            b, coeff0, coeff1, &carry0, &carry1, &seen, &source, &acc, bit,
        );
    }

    b.set_phase("round125_jsf_cleanup");
    for &q in &source {
        b.x(q);
    }
    b.free_vec(&seen);
    b.free_vec(&carry1);
    b.free_vec(&carry0);
    b.free_vec(&acc);
    b.free_vec(&source);
}

#[cfg(test)]
fn choose_jsf_digit(this_value: u128, other_value: u128) -> i8 {
    if (this_value & 1) == 0 {
        0
    } else {
        let mut digit = if (this_value & 3) == 1 { 1 } else { -1 };
        if (this_value & 7 == 3 || this_value & 7 == 5) && (other_value & 3) == 2 {
            digit = -digit;
        }
        digit
    }
}

#[cfg(test)]
fn jsf_digits_u128(mut a: u128, mut c: u128) -> (Vec<i8>, Vec<i8>) {
    let mut da = Vec::new();
    let mut dc = Vec::new();
    while a != 0 || c != 0 {
        let d0 = choose_jsf_digit(a, c);
        let d1 = choose_jsf_digit(c, a);
        da.push(d0);
        dc.push(d1);
        a = ((a as i128 - d0 as i128) / 2) as u128;
        c = ((c as i128 - d1 as i128) / 2) as u128;
    }
    (da, dc)
}

#[cfg(test)]
mod tests {
    use alloy_primitives::U256;
    use sha3::{
        digest::{ExtendableOutput, Update},
        Shake128,
    };

    use crate::{
        circuit::{analyze_ops, QubitOrBit},
        sim::Simulator,
    };

    use super::*;

    #[test]
    fn jsf_digits_reconstruct_inputs() {
        for a in 0u128..512 {
            for c in 0u128..512 {
                let (da, dc) = jsf_digits_u128(a, c);
                let ra = da
                    .iter()
                    .enumerate()
                    .map(|(i, &d)| (d as i128) << i)
                    .sum::<i128>() as u128;
                let rc = dc
                    .iter()
                    .enumerate()
                    .map(|(i, &d)| (d as i128) << i)
                    .sum::<i128>() as u128;
                assert_eq!(ra, a, "lane0 reconstruction failed for ({a}, {c})");
                assert_eq!(rc, c, "lane1 reconstruction failed for ({a}, {c})");
            }
        }
    }

    #[test]
    fn roundtrip_operator_is_fuzzable_on_toy_width() {
        const W: usize = 12;
        const SHOTS: usize = 32;

        let mut b = B::new();
        let x0 = b.alloc_qubits(W);
        b.declare_qubit_register(&x0);
        let x1 = b.alloc_qubits(W);
        b.declare_qubit_register(&x1);
        emit_round125_jsf_operator_roundtrip(&mut b, &x0, &x1);

        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(b.ops.iter().copied());
        assert_eq!(regs.len(), 2);

        let mut seed = Shake128::default();
        seed.update(b"round125-jsf-toy-fuzz");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let mut cases = Vec::new();
        for shot in 0..SHOTS {
            let a = ((shot as u64 * 73 + 19) & ((1 << W) - 1)) as u64;
            let c = ((shot as u64 * 211 + 5) & ((1 << W) - 1)) as u64;
            cases.push((U256::from(a), U256::from(c)));
            sim.set_register(&regs[0], U256::from(a), shot);
            sim.set_register(&regs[1], U256::from(c), shot);
        }

        sim.apply(&b.ops);

        for (shot, (a, c)) in cases.iter().copied().enumerate() {
            assert_eq!(sim.get_register(&regs[0], shot), a, "lane0 shot {shot}");
            assert_eq!(sim.get_register(&regs[1], shot), c, "lane1 shot {shot}");
        }
        let live_mask = (1u64 << SHOTS) - 1;
        assert_eq!(sim.global_phase() & live_mask, 0, "phase garbage");

        for reg in &regs {
            for item in reg {
                if let QubitOrBit::Qubit(q) = *item {
                    *sim.qubit_mut(q) = 0;
                }
            }
        }
        for q in 0..num_qubits {
            assert_eq!(sim.qubit(QubitId(q)) & live_mask, 0, "ancilla q{q}");
        }
    }
}
