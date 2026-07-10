//! Round218 B=5 source-live half-delta program generator.
//!
//! This file is intentionally disjoint from `mod.rs` and
//! `round218_b5_transport.rs`.  It is a real executable data/program model for
//! the surviving Round218 resource row: 590 half-delta steps, grouped as 118
//! blocks of five steps, over the secp256k1 field.  The KMX integration point
//! is still outside this file; the objects here are the exact block selectors
//! and block matrices that a lowerer consumes.

pub const ROUND218_B5_PA_QUBITS: usize = 1_562;
pub const ROUND218_B5_PA_TOFFOLI: usize = 2_203_351;
pub const ROUND218_B5_STEPS: usize = 590;
pub const ROUND218_B5_BLOCK_BITS: usize = 5;
pub const ROUND218_B5_SOURCE_WINDOW_BITS: usize = 2 * ROUND218_B5_BLOCK_BITS;
pub const ROUND218_B5_BLOCKS: usize = ROUND218_B5_STEPS / ROUND218_B5_BLOCK_BITS;
pub const ROUND218_B5_CLASSIFICATION: &str =
    "ROUND218_B5_SOURCE_LIVE_HALF_DELTA_PROGRAM_Q1562_T2203351";
pub const SECP256K1_P_HEX: &str =
    "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F";

const BLOCK_SCALE: i128 = 1i128 << ROUND218_B5_BLOCK_BITS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceRow {
    pub classification: &'static str,
    pub qubits: usize,
    pub toffoli: usize,
    pub steps: usize,
    pub block_bits: usize,
    pub blocks: usize,
    pub prime_hex: &'static str,
}

pub const ROUND218_B5_RESOURCE_ROW: ResourceRow = ResourceRow {
    classification: ROUND218_B5_CLASSIFICATION,
    qubits: ROUND218_B5_PA_QUBITS,
    toffoli: ROUND218_B5_PA_TOFFOLI,
    steps: ROUND218_B5_STEPS,
    block_bits: ROUND218_B5_BLOCK_BITS,
    blocks: ROUND218_B5_BLOCKS,
    prime_hex: SECP256K1_P_HEX,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HalfDeltaState {
    pub zeta: i128,
    pub f: i128,
    pub g: i128,
    pub v: i128,
    pub r: i128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    PositiveOdd,
    NonbranchEven,
    NonbranchOdd,
}

impl StepKind {
    pub const fn branch_bit(self) -> u8 {
        match self {
            Self::PositiveOdd => 1,
            Self::NonbranchEven | Self::NonbranchOdd => 0,
        }
    }

    pub const fn old_g0(self) -> u8 {
        match self {
            Self::PositiveOdd | Self::NonbranchOdd => 1,
            Self::NonbranchEven => 0,
        }
    }

    pub const fn numerator_matrix(self) -> Matrix2 {
        match self {
            Self::PositiveOdd => Matrix2 {
                a00: 0,
                a01: 2,
                a10: -1,
                a11: 1,
            },
            Self::NonbranchEven => Matrix2 {
                a00: 2,
                a01: 0,
                a10: 0,
                a11: 1,
            },
            Self::NonbranchOdd => Matrix2 {
                a00: 2,
                a01: 0,
                a10: 1,
                a11: 1,
            },
        }
    }

    pub fn apply_scaled_coeff_mod(self, v: i128, r: i128, p: i128) -> (i128, i128) {
        scaled_coeff_step_mod(v, r, self.branch_bit() != 0, self.old_g0() != 0, p)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Matrix2 {
    pub a00: i128,
    pub a01: i128,
    pub a10: i128,
    pub a11: i128,
}

impl Matrix2 {
    pub const IDENTITY: Self = Self {
        a00: 1,
        a01: 0,
        a10: 0,
        a11: 1,
    };

    pub fn mul(self, rhs: Self) -> Self {
        Self {
            a00: self.a00 * rhs.a00 + self.a01 * rhs.a10,
            a01: self.a00 * rhs.a01 + self.a01 * rhs.a11,
            a10: self.a10 * rhs.a00 + self.a11 * rhs.a10,
            a11: self.a10 * rhs.a01 + self.a11 * rhs.a11,
        }
    }

    pub fn det(self) -> i128 {
        self.a00 * self.a11 - self.a01 * self.a10
    }

    pub fn apply_integer_div(self, f: i128, g: i128, denominator: i128) -> (i128, i128) {
        let nf = self.a00 * f + self.a01 * g;
        let ng = self.a10 * f + self.a11 * g;
        assert_eq!(nf.rem_euclid(denominator), 0, "f numerator is not integral");
        assert_eq!(ng.rem_euclid(denominator), 0, "g numerator is not integral");
        (nf / denominator, ng / denominator)
    }

    pub fn apply_mod(self, v: i128, r: i128, p: i128) -> (i128, i128) {
        (
            modp(self.a00 * v + self.a01 * r, p),
            modp(self.a10 * v + self.a11 * r, p),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockSelector {
    pub zeta_start: i128,
    pub f_low: u8,
    pub g_low: u8,
    pub width: u8,
}

impl BlockSelector {
    pub fn from_state(state: HalfDeltaState) -> Self {
        let mask = (1i128 << ROUND218_B5_BLOCK_BITS) - 1;
        Self {
            zeta_start: state.zeta,
            f_low: state.f.rem_euclid(1i128 << ROUND218_B5_BLOCK_BITS) as u8,
            g_low: state.g.rem_euclid(1i128 << ROUND218_B5_BLOCK_BITS) as u8,
            width: ROUND218_B5_BLOCK_BITS as u8,
        }
        .masked(mask as u8)
    }

    fn masked(mut self, mask: u8) -> Self {
        self.f_low &= mask;
        self.g_low &= mask;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockMatrix {
    pub numerator: Matrix2,
    pub denominator_log2: u8,
}

impl BlockMatrix {
    pub fn denominator(self) -> i128 {
        1i128 << self.denominator_log2
    }

    pub fn apply_scaled_mod(self, v: i128, r: i128, p: i128) -> (i128, i128) {
        assert_odd_field_modulus(p);
        let (mut v, mut r) = self.numerator.apply_mod(v, r, p);
        for _ in 0..self.denominator_log2 {
            v = halve_mod(v, p);
            r = halve_mod(r, p);
        }
        (v, r)
    }

    pub fn apply(self, state: HalfDeltaState, end_zeta: i128, p: i128) -> HalfDeltaState {
        let (f, g) = self
            .numerator
            .apply_integer_div(state.f, state.g, self.denominator());
        let (v, r) = self.numerator.apply_mod(state.v, state.r, p);
        HalfDeltaState {
            zeta: end_zeta,
            f,
            g,
            v,
            r,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockRow {
    pub block_index: usize,
    pub start_step: usize,
    pub selector: BlockSelector,
    pub step_kinds: [StepKind; ROUND218_B5_BLOCK_BITS],
    pub branch_word: u8,
    pub old_g0_word: u8,
    pub end_zeta: i128,
    pub matrix: BlockMatrix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceWindowSelector {
    pub zeta_start: i128,
    pub f_window: u16,
    pub g_window: u16,
    pub window_bits: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceWindowBlockRow {
    pub selector: SourceWindowSelector,
    pub branch_word: u8,
    pub old_g0_word: u8,
    pub end_zeta: i128,
    pub next_f_low: u8,
    pub next_g_low: u8,
}

impl BlockRow {
    pub fn apply_scaled_coeff_matrix_mod(&self, v: i128, r: i128, p: i128) -> (i128, i128) {
        self.matrix.apply_scaled_mod(v, r, p)
    }

    pub fn apply_scaled_coeff_stepwise_mod(&self, v: i128, r: i128, p: i128) -> (i128, i128) {
        apply_scaled_coeff_steps_mod(self.step_kinds, v, r, p)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round218B5Program {
    pub resource: ResourceRow,
    pub block_count: usize,
}

impl Round218B5Program {
    pub fn new() -> Self {
        Self {
            resource: ROUND218_B5_RESOURCE_ROW,
            block_count: ROUND218_B5_BLOCKS,
        }
    }

    pub fn block_row(&self, block_index: usize, selector: BlockSelector) -> BlockRow {
        assert!(block_index < self.block_count, "block index out of range");
        block_row(block_index, selector)
    }

    pub fn row_for_state(&self, block_index: usize, state: HalfDeltaState) -> BlockRow {
        self.block_row(block_index, BlockSelector::from_state(state))
    }
}

impl Default for Round218B5Program {
    fn default() -> Self {
        Self::new()
    }
}

pub fn block_row(block_index: usize, selector: BlockSelector) -> BlockRow {
    assert_eq!(selector.width as usize, ROUND218_B5_BLOCK_BITS);
    assert!(block_index < ROUND218_B5_BLOCKS, "block index out of range");
    assert!(
        selector.f_low < BLOCK_SCALE as u8,
        "selector f_low out of range"
    );
    assert!(
        selector.g_low < BLOCK_SCALE as u8,
        "selector g_low out of range"
    );

    let (step_kinds, end_zeta) = derive_step_kinds(selector);
    let mut numerator = Matrix2::IDENTITY;
    let mut branch_word = 0u8;
    let mut old_g0_word = 0u8;
    for (idx, kind) in step_kinds.iter().copied().enumerate() {
        numerator = kind.numerator_matrix().mul(numerator);
        branch_word |= kind.branch_bit() << idx;
        old_g0_word |= kind.old_g0() << idx;
    }
    let matrix = BlockMatrix {
        numerator,
        denominator_log2: ROUND218_B5_BLOCK_BITS as u8,
    };
    assert_eq!(matrix.numerator.det(), matrix.denominator());

    BlockRow {
        block_index,
        start_step: block_index * ROUND218_B5_BLOCK_BITS,
        selector,
        step_kinds,
        branch_word,
        old_g0_word,
        end_zeta,
        matrix,
    }
}

pub fn source_window_block_row(selector: SourceWindowSelector) -> SourceWindowBlockRow {
    assert_eq!(
        selector.window_bits as usize, ROUND218_B5_SOURCE_WINDOW_BITS,
        "Round218 B=5 source window must be 2B bits"
    );
    let window_mod = 1i128 << ROUND218_B5_SOURCE_WINDOW_BITS;
    assert!(
        selector.f_window < window_mod as u16,
        "selector f_window out of range"
    );
    assert!(
        selector.g_window < window_mod as u16,
        "selector g_window out of range"
    );
    assert!(
        selector.f_window & 1 == 1,
        "source-window half-delta domain requires odd f"
    );

    let mut zeta = selector.zeta_start;
    let mut f = selector.f_window as i128;
    let mut g = selector.g_window as i128;
    let mut width = ROUND218_B5_SOURCE_WINDOW_BITS;
    let mut branch_word = 0u8;
    let mut old_g0_word = 0u8;

    for step in 0..ROUND218_B5_BLOCK_BITS {
        let old_g0 = (g & 1) as u8;
        let branch = (zeta < 0 && old_g0 != 0) as u8;
        branch_word |= branch << step;
        old_g0_word |= old_g0 << step;

        let next_width = width - 1;
        let next_mask = (1i128 << next_width) - 1;
        if branch != 0 {
            let next_f = g;
            let next_g = (g - f) / 2;
            zeta = -zeta - 2;
            f = next_f & next_mask;
            g = next_g & next_mask;
        } else {
            let next_g = (g + old_g0 as i128 * f) / 2;
            zeta -= 1;
            f &= next_mask;
            g = next_g & next_mask;
        }
        width = next_width;
    }

    SourceWindowBlockRow {
        selector,
        branch_word,
        old_g0_word,
        end_zeta: zeta,
        next_f_low: f as u8,
        next_g_low: g as u8,
    }
}

pub fn apply_block_row(state: HalfDeltaState, row: &BlockRow, p: i128) -> HalfDeltaState {
    let expected = BlockSelector::from_state(state);
    assert_eq!(
        expected, row.selector,
        "block row selector does not match live state"
    );
    row.matrix.apply(state, row.end_zeta, p)
}

pub fn scaled_coeff_step_mod(
    v: i128,
    r: i128,
    branch: bool,
    old_g0: bool,
    p: i128,
) -> (i128, i128) {
    assert_odd_field_modulus(p);
    assert!(
        old_g0 || !branch,
        "positive half-delta branch requires old_g0=1"
    );

    let mut v = modp(v, p);
    let mut r = modp(r, p);
    if !branch && old_g0 {
        r = modp(r + v, p);
    }
    if branch {
        r = modp(r - v, p);
    }
    r = halve_mod(r, p);
    if branch {
        v = modp(v + 2 * r, p);
    }
    (v, r)
}

pub fn apply_scaled_coeff_steps_mod(
    step_kinds: [StepKind; ROUND218_B5_BLOCK_BITS],
    mut v: i128,
    mut r: i128,
    p: i128,
) -> (i128, i128) {
    assert_odd_field_modulus(p);
    for kind in step_kinds {
        (v, r) = kind.apply_scaled_coeff_mod(v, r, p);
    }
    (v, r)
}

pub fn half_delta_step_scaled_coeff_mod(
    state: HalfDeltaState,
    p: i128,
) -> (HalfDeltaState, StepKind) {
    let (mut next, kind) = half_delta_step_mod(state, p);
    let (v, r) = kind.apply_scaled_coeff_mod(state.v, state.r, p);
    next.v = v;
    next.r = r;
    (next, kind)
}

pub fn apply_steps_scaled_coeff(
    mut state: HalfDeltaState,
    steps: usize,
    p: i128,
) -> HalfDeltaState {
    for _ in 0..steps {
        state = half_delta_step_scaled_coeff_mod(state, p).0;
    }
    state
}

pub fn apply_block_row_scaled_coeff(
    state: HalfDeltaState,
    row: &BlockRow,
    p: i128,
) -> HalfDeltaState {
    let expected = BlockSelector::from_state(state);
    assert_eq!(
        expected, row.selector,
        "block row selector does not match live state"
    );
    let (f, g) = row
        .matrix
        .numerator
        .apply_integer_div(state.f, state.g, row.matrix.denominator());
    let (v, r) = row.apply_scaled_coeff_matrix_mod(state.v, state.r, p);
    HalfDeltaState {
        zeta: row.end_zeta,
        f,
        g,
        v,
        r,
    }
}

pub fn apply_program_blocks_scaled_coeff(
    program: &Round218B5Program,
    mut state: HalfDeltaState,
    blocks: usize,
    p: i128,
) -> HalfDeltaState {
    assert!(blocks <= program.block_count);
    for block_index in 0..blocks {
        let row = program.row_for_state(block_index, state);
        state = apply_block_row_scaled_coeff(state, &row, p);
    }
    state
}

pub fn half_delta_step_mod(state: HalfDeltaState, p: i128) -> (HalfDeltaState, StepKind) {
    assert!(state.f & 1 == 1, "half-delta domain requires odd f");
    let old_g0 = state.g.rem_euclid(2) as u8;
    if state.zeta < 0 && old_g0 != 0 {
        (
            HalfDeltaState {
                zeta: -state.zeta - 2,
                f: state.g,
                g: (state.g - state.f) / 2,
                v: modp(2 * state.r, p),
                r: modp(state.r - state.v, p),
            },
            StepKind::PositiveOdd,
        )
    } else {
        (
            HalfDeltaState {
                zeta: state.zeta - 1,
                f: state.f,
                g: (state.g + old_g0 as i128 * state.f) / 2,
                v: modp(2 * state.v, p),
                r: modp(state.r + old_g0 as i128 * state.v, p),
            },
            if old_g0 == 0 {
                StepKind::NonbranchEven
            } else {
                StepKind::NonbranchOdd
            },
        )
    }
}

pub fn apply_steps(mut state: HalfDeltaState, steps: usize, p: i128) -> HalfDeltaState {
    for _ in 0..steps {
        state = half_delta_step_mod(state, p).0;
    }
    state
}

pub fn apply_program_blocks(
    program: &Round218B5Program,
    mut state: HalfDeltaState,
    blocks: usize,
    p: i128,
) -> HalfDeltaState {
    assert!(blocks <= program.block_count);
    for block_index in 0..blocks {
        let row = program.row_for_state(block_index, state);
        state = apply_block_row(state, &row, p);
    }
    state
}

fn derive_step_kinds(selector: BlockSelector) -> ([StepKind; ROUND218_B5_BLOCK_BITS], i128) {
    let mut zeta = selector.zeta_start;
    let mut f_low = selector.f_low as i128;
    let mut g_low = selector.g_low as i128;
    let mut kinds = [StepKind::NonbranchEven; ROUND218_B5_BLOCK_BITS];

    for (step, slot) in kinds.iter_mut().enumerate() {
        let remaining = ROUND218_B5_BLOCK_BITS - step;
        let modulus = 1i128 << remaining;
        f_low = f_low.rem_euclid(modulus);
        g_low = g_low.rem_euclid(modulus);
        assert_eq!(f_low & 1, 1, "selector leaves the odd-f half-delta domain");

        let old_g0 = (g_low & 1) as u8;
        let kind = if zeta < 0 && old_g0 != 0 {
            let next_f = g_low;
            let next_g = ((g_low - f_low) / 2).rem_euclid(modulus >> 1);
            zeta = -zeta - 2;
            f_low = next_f;
            g_low = next_g;
            StepKind::PositiveOdd
        } else {
            let next_g = ((g_low + old_g0 as i128 * f_low) / 2).rem_euclid(modulus >> 1);
            zeta -= 1;
            g_low = next_g;
            if old_g0 == 0 {
                StepKind::NonbranchEven
            } else {
                StepKind::NonbranchOdd
            }
        };
        *slot = kind;
    }

    (kinds, zeta)
}

fn modp(x: i128, p: i128) -> i128 {
    x.rem_euclid(p)
}

fn halve_mod(x: i128, p: i128) -> i128 {
    let x = modp(x, p);
    if x & 1 == 0 {
        x / 2
    } else {
        (x + p) / 2
    }
}

fn assert_odd_field_modulus(p: i128) {
    assert!(
        p > 1 && p & 1 == 1,
        "scaled coefficient semantics requires an odd field modulus"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(p: i128, h: i128) -> HalfDeltaState {
        HalfDeltaState {
            zeta: -1,
            f: p,
            g: h,
            v: 0,
            r: 1,
        }
    }

    fn coefficient_samples(p: i128) -> [i128; 9] {
        [0, 1, 2, 3, (p - 1) / 2, p / 2, p - 3, p - 2, p - 1]
    }

    fn pow2_mod(exp: usize, p: i128) -> i128 {
        let mut out = 1i128;
        for _ in 0..exp {
            out = modp(out + out, p);
        }
        out
    }

    fn inv_mod_toy(a: i128, p: i128) -> i128 {
        let (mut old_r, mut r) = (p, modp(a, p));
        let (mut old_t, mut t) = (0i128, 1i128);
        while r != 0 {
            let q = old_r / r;
            (old_r, r) = (r, old_r - q * r);
            (old_t, t) = (t, old_t - q * t);
        }
        assert_eq!(old_r, 1, "{a} is not invertible modulo {p}");
        modp(old_t, p)
    }

    fn full_source_block_trace(zeta_start: i128, f: i128, g: i128) -> (i128, i128, i128, u8, u8) {
        let mut state = HalfDeltaState {
            zeta: zeta_start,
            f,
            g,
            v: 0,
            r: 0,
        };
        let mut branch_word = 0u8;
        let mut old_g0_word = 0u8;
        for step in 0..ROUND218_B5_BLOCK_BITS {
            let (next, kind) = half_delta_step_mod(state, 43);
            branch_word |= kind.branch_bit() << step;
            old_g0_word |= kind.old_g0() << step;
            state = next;
        }
        (state.zeta, state.f, state.g, branch_word, old_g0_word)
    }

    #[test]
    fn resource_row_is_the_round218_b5_target() {
        let program = Round218B5Program::new();
        println!("METRIC round218_b5_program_q={}", program.resource.qubits);
        println!("METRIC round218_b5_program_t={}", program.resource.toffoli);
        println!("METRIC round218_b5_program_blocks={}", program.block_count);
        assert_eq!(program.resource.qubits, 1_562);
        assert_eq!(program.resource.toffoli, 2_203_351);
        assert_eq!(program.resource.steps, 590);
        assert_eq!(program.resource.block_bits, 5);
        assert_eq!(program.block_count, 118);
        assert_eq!(program.resource.prime_hex, SECP256K1_P_HEX);
    }

    #[test]
    fn scaled_coefficient_step_matches_half_delta_matrix_over_fields() {
        for p in [43i128, 101, 251, 65_537] {
            for kind in [
                StepKind::PositiveOdd,
                StepKind::NonbranchEven,
                StepKind::NonbranchOdd,
            ] {
                for v in coefficient_samples(p) {
                    for r in coefficient_samples(p) {
                        let by_controls = kind.apply_scaled_coeff_mod(v, r, p);
                        let (mut by_matrix_v, mut by_matrix_r) =
                            kind.numerator_matrix().apply_mod(v, r, p);
                        by_matrix_v = halve_mod(by_matrix_v, p);
                        by_matrix_r = halve_mod(by_matrix_r, p);
                        assert_eq!(
                            by_controls,
                            (by_matrix_v, by_matrix_r),
                            "p={p} kind={kind:?} v={v} r={r}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn embedded_reference_block_matrices_are_stable() {
        let even = block_row(
            0,
            BlockSelector {
                zeta_start: 8,
                f_low: 1,
                g_low: 0,
                width: 5,
            },
        );
        assert_eq!(even.old_g0_word, 0b00000);
        assert_eq!(even.branch_word, 0b00000);
        assert_eq!(
            even.matrix.numerator,
            Matrix2 {
                a00: 32,
                a01: 0,
                a10: 0,
                a11: 1
            }
        );

        let odd = block_row(
            0,
            BlockSelector {
                zeta_start: 8,
                f_low: 1,
                g_low: 1,
                width: 5,
            },
        );
        assert_eq!(odd.old_g0_word, 0b11111);
        assert_eq!(odd.branch_word, 0b00000);
        assert_eq!(
            odd.matrix.numerator,
            Matrix2 {
                a00: 32,
                a01: 0,
                a10: 31,
                a11: 1
            }
        );

        let positive = block_row(
            0,
            BlockSelector {
                zeta_start: -1,
                f_low: 31,
                g_low: 1,
                width: 5,
            },
        );
        assert_eq!(positive.old_g0_word, 0b00011);
        assert_eq!(positive.branch_word, 0b00011);
        assert_eq!(
            positive.matrix.numerator,
            Matrix2 {
                a00: -16,
                a01: 16,
                a10: -1,
                a11: -1
            }
        );
    }

    #[test]
    fn source_window_block_row_matches_full_integer_sources() {
        let low_mod = 1i128 << ROUND218_B5_BLOCK_BITS;
        let window_shift = ROUND218_B5_SOURCE_WINDOW_BITS;
        for zeta_start in [-9i128, -1, 0, 7] {
            for f_window in (1u16..(1u16 << ROUND218_B5_SOURCE_WINDOW_BITS)).step_by(97) {
                if f_window & 1 == 0 {
                    continue;
                }
                for g_window in (0u16..(1u16 << ROUND218_B5_SOURCE_WINDOW_BITS)).step_by(113) {
                    let row = source_window_block_row(SourceWindowSelector {
                        zeta_start,
                        f_window,
                        g_window,
                        window_bits: ROUND218_B5_SOURCE_WINDOW_BITS as u8,
                    });
                    for f_high in [-3i128, 0, 5] {
                        for g_high in [-4i128, 0, 7] {
                            let f = f_window as i128 + (f_high << window_shift);
                            let g = g_window as i128 + (g_high << window_shift);
                            let (end_zeta, end_f, end_g, branch_word, old_g0_word) =
                                full_source_block_trace(zeta_start, f, g);
                            assert_eq!(row.branch_word, branch_word);
                            assert_eq!(row.old_g0_word, old_g0_word);
                            assert_eq!(row.end_zeta, end_zeta);
                            assert_eq!(row.next_f_low as i128, end_f.rem_euclid(low_mod));
                            assert_eq!(row.next_g_low as i128, end_g.rem_euclid(low_mod));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn source_window_refines_b5_low_selector_controls() {
        let low_mask = ((1u16 << ROUND218_B5_BLOCK_BITS) - 1) as u16;
        for zeta_start in [-9i128, -1, 0, 4] {
            for f_window in (1u16..(1u16 << ROUND218_B5_SOURCE_WINDOW_BITS)).step_by(2) {
                for g_window in 0u16..(1u16 << ROUND218_B5_SOURCE_WINDOW_BITS) {
                    let window = source_window_block_row(SourceWindowSelector {
                        zeta_start,
                        f_window,
                        g_window,
                        window_bits: ROUND218_B5_SOURCE_WINDOW_BITS as u8,
                    });
                    let low = block_row(
                        0,
                        BlockSelector {
                            zeta_start,
                            f_low: (f_window & low_mask) as u8,
                            g_low: (g_window & low_mask) as u8,
                            width: ROUND218_B5_BLOCK_BITS as u8,
                        },
                    );
                    assert_eq!(window.branch_word, low.branch_word);
                    assert_eq!(window.old_g0_word, low.old_g0_word);
                }
            }
        }
    }

    #[test]
    fn b5_scaled_block_matrix_matches_stepwise_scaled_coefficients_on_toy_fields() {
        for p in [43i128, 101, 251, 65_537] {
            for zeta in [-9, -5, -1, 0, 3, 8, 13] {
                for f_low in [1u8, 3, 5, 9, 17, 23, 31] {
                    for g_low in [0u8, 1, 2, 7, 16, 23, 30, 31] {
                        let row = block_row(
                            0,
                            BlockSelector {
                                zeta_start: zeta,
                                f_low,
                                g_low,
                                width: ROUND218_B5_BLOCK_BITS as u8,
                            },
                        );
                        for v in coefficient_samples(p) {
                            for r in coefficient_samples(p) {
                                let by_matrix = row.apply_scaled_coeff_matrix_mod(v, r, p);
                                let by_steps = row.apply_scaled_coeff_stepwise_mod(v, r, p);
                                assert_eq!(
                                    by_matrix, by_steps,
                                    "p={p} selector={:?} v={v} r={r}",
                                    row.selector
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn generated_b5_blocks_match_stepwise_python_round56_semantics() {
        let p = 251;
        let program = Round218B5Program::new();
        for h in 1..p {
            let state = seed(p, h);
            let by_blocks = apply_program_blocks(&program, state, 11, p);
            let by_blocks_plus_one = half_delta_step_mod(by_blocks, p).0;
            let stepwise = apply_steps(state, 56, p);
            assert_eq!(by_blocks_plus_one, stepwise, "Round56 mismatch for h={h}");
        }
    }

    #[test]
    fn generated_b5_blocks_match_stepwise_toy_primes_through_590_steps() {
        let program = Round218B5Program::new();
        for p in [43i128, 101, 251] {
            for h in 1..p {
                let state = seed(p, h);
                let by_blocks = apply_program_blocks(&program, state, ROUND218_B5_BLOCKS, p);
                let stepwise = apply_steps(state, ROUND218_B5_STEPS, p);
                assert_eq!(by_blocks, stepwise, "p={p} h={h}");
            }
        }
    }

    #[test]
    fn unscaled_full_source_coefficients_are_signed_scaled_inverse_on_toys() {
        let scale = |p| pow2_mod(ROUND218_B5_STEPS, p);
        for p in [43i128, 101, 251] {
            for h in 1..p {
                let out = apply_steps(seed(p, h), ROUND218_B5_STEPS, p);
                assert_eq!(out.g, 0, "p={p} h={h}");
                assert_eq!(out.r, 0, "p={p} h={h}");
                assert!(
                    out.f == 1 || out.f == -1,
                    "unexpected final f for p={p} h={h}: {}",
                    out.f
                );

                let sign_corrected_v = if out.f < 0 { modp(-out.v, p) } else { out.v };
                let expect = modp(scale(p) * inv_mod_toy(h, p), p);
                assert_eq!(sign_corrected_v, expect, "p={p} h={h}");
            }
        }
    }

    #[test]
    fn generated_b5_blocks_scaled_coefficients_match_stepwise_sampled_states() {
        let program = Round218B5Program::new();
        for p in [43i128, 101, 251] {
            for h in 1..p {
                let state = seed(p, h);
                let by_blocks =
                    apply_program_blocks_scaled_coeff(&program, state, ROUND218_B5_BLOCKS, p);
                let stepwise = apply_steps_scaled_coeff(state, ROUND218_B5_STEPS, p);
                assert_eq!(by_blocks, stepwise, "p={p} h={h}");
            }
        }
    }

    #[test]
    fn selector_low_bits_are_sufficient_for_each_b5_block_on_domain() {
        let p = 251;
        for zeta in -8..=8 {
            for f_low in (1u8..32).step_by(2) {
                for g_low in 0u8..32 {
                    let selector = BlockSelector {
                        zeta_start: zeta,
                        f_low,
                        g_low,
                        width: 5,
                    };
                    let row = block_row(0, selector);
                    for f_hi in -2..=2 {
                        for g_hi in -2..=2 {
                            let state = HalfDeltaState {
                                zeta,
                                f: f_low as i128 + f_hi * BLOCK_SCALE,
                                g: g_low as i128 + g_hi * BLOCK_SCALE,
                                v: 17,
                                r: 29,
                            };
                            if state.f & 1 == 0 {
                                continue;
                            }
                            let block = apply_block_row(state, &row, p);
                            let stepwise = apply_steps(state, ROUND218_B5_BLOCK_BITS, p);
                            assert_eq!(block, stepwise, "selector={selector:?} state={state:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_generated_block_matrix_has_half_delta_determinant() {
        for zeta in -16..=16 {
            for f_low in (1u8..32).step_by(2) {
                for g_low in 0u8..32 {
                    let row = block_row(
                        0,
                        BlockSelector {
                            zeta_start: zeta,
                            f_low,
                            g_low,
                            width: 5,
                        },
                    );
                    assert_eq!(row.matrix.denominator(), 32);
                    assert_eq!(row.matrix.numerator.det(), 32);
                }
            }
        }
    }
}
