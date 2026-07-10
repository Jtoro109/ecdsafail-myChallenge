//! Gate-level Round218 B=5 low-state selector parser.
//!
//! For a fixed `zeta_start`, this lowers the map
//!
//!   |f_low, g_low, 0, 0> -> |f_low, g_low, branch_word, old_g0_word>
//!
//! over the odd-`f_low` half-delta domain.  The emitted circuit is an XOR
//! network of positive-control monomials synthesized from the exact B=5
//! `round218_b5_program::block_row` table.  Because the network only toggles
//! output qubits as functions of the unchanged low-state inputs, the matching
//! uncompute is the same gate sequence applied a second time.
//!
//! The dynamic-zeta transducer below extends the same parser to
//!
//!   |zeta_start, f_low, g_low, 0, 0, 0>
//!     -> |zeta_start, f_low, g_low, branch_word, old_g0_word, end_zeta>
//!
//! with offset-binary zeta encodings.  Invalid zeta codes and even-`f_low`
//! inputs are totalized to zero outputs.

use alloy_primitives::U256;

use crate::circuit::{BitId, Op, QubitId};

use super::{round218_b5_program, B};

pub const ROUND218_B5_LOW_STATE_BITS: usize = round218_b5_program::ROUND218_B5_BLOCK_BITS;
pub const ROUND218_B5_LOW_WINDOW_BITS: usize = 2 * ROUND218_B5_LOW_STATE_BITS;
pub const ROUND218_B5_SELECTOR_INPUT_BITS: usize = 2 * ROUND218_B5_LOW_STATE_BITS;
pub const ROUND218_B5_SELECTOR_OUTPUT_BITS: usize = 2 * ROUND218_B5_LOW_STATE_BITS;
pub const ROUND218_B5_SELECTOR_MAX_SCRATCH: usize = ROUND218_B5_SELECTOR_INPUT_BITS - 2;
pub const ROUND218_B5_LOW_WINDOW_PARSER_INPUT_BITS: usize = 2 * ROUND218_B5_LOW_WINDOW_BITS;
pub const ROUND218_B5_LOW_WINDOW_PARSER_OUTPUT_BITS: usize = 4 * ROUND218_B5_LOW_STATE_BITS;
pub const ROUND218_B5_LOW_WINDOW_PARSER_MIN_RETAINED_BITS: usize = ROUND218_B5_LOW_STATE_BITS;
pub const ROUND218_B5_DYNAMIC_ZETA_PREFIX_OUTPUT_BITS: usize = ROUND218_B5_SELECTOR_OUTPUT_BITS;
pub const ROUND218_B5_DYNAMIC_ZETA_SYNTHESIS_INPUT_LIMIT: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Round218B5LowWindowParserOutput {
    pub branch_word: u8,
    pub old_g0_word: u8,
    pub next_f_low: u8,
    pub next_g_low: u8,
}

impl Round218B5LowWindowParserOutput {
    pub fn packed(self) -> u32 {
        assert!(self.branch_word < (1u8 << ROUND218_B5_LOW_STATE_BITS));
        assert!(self.old_g0_word < (1u8 << ROUND218_B5_LOW_STATE_BITS));
        assert!(self.next_f_low < (1u8 << ROUND218_B5_LOW_STATE_BITS));
        assert!(self.next_g_low < (1u8 << ROUND218_B5_LOW_STATE_BITS));
        u32::from(self.branch_word)
            | (u32::from(self.old_g0_word) << ROUND218_B5_LOW_STATE_BITS)
            | (u32::from(self.next_f_low) << (2 * ROUND218_B5_LOW_STATE_BITS))
            | (u32::from(self.next_g_low) << (3 * ROUND218_B5_LOW_STATE_BITS))
    }
}

pub fn round218_b5_low_window_parser_cell(
    zeta_start: i128,
    f_window: u16,
    g_window: u16,
) -> Round218B5LowWindowParserOutput {
    round218_b5_low_window_parser_cell_with_retained_word(zeta_start, f_window, g_window).0
}

pub fn round218_b5_low_window_parser_retained_word(
    zeta_start: i128,
    f_window: u16,
    g_window: u16,
) -> u8 {
    round218_b5_low_window_parser_cell_with_retained_word(zeta_start, f_window, g_window).1
}

pub fn round218_b5_low_window_parser_reconstruct_input(
    zeta_start: i128,
    parsed: Round218B5LowWindowParserOutput,
    retained_word: u8,
) -> (u16, u16) {
    let low_mask = (1u8 << ROUND218_B5_LOW_STATE_BITS) - 1;
    assert_eq!(parsed.branch_word & !low_mask, 0);
    assert_eq!(parsed.old_g0_word & !low_mask, 0);
    assert_eq!(parsed.next_f_low & !low_mask, 0);
    assert_eq!(parsed.next_g_low & !low_mask, 0);
    assert_eq!(retained_word & !low_mask, 0);

    let mut zeta = zeta_start;
    for step in 0..ROUND218_B5_LOW_STATE_BITS {
        let branch = ((parsed.branch_word >> step) & 1) != 0;
        let old_g0 = ((parsed.old_g0_word >> step) & 1) != 0;
        assert!(
            !branch || old_g0,
            "branch step {step} is impossible with old_g0=0"
        );
        assert_eq!(
            branch,
            zeta < 0 && old_g0,
            "branch/old_g0 words are not legal for zeta_start={zeta_start}"
        );
        if branch {
            zeta = -zeta - 2;
        } else {
            zeta -= 1;
        }
    }

    let mut f = i128::from(parsed.next_f_low);
    let mut g = i128::from(parsed.next_g_low);
    for step in (0..ROUND218_B5_LOW_STATE_BITS).rev() {
        let after_width = ROUND218_B5_LOW_WINDOW_BITS - step - 1;
        let modulus = 1i128 << (after_width + 1);
        let retained_bit = i128::from((retained_word >> step) & 1);
        let next_f_full = f + (retained_bit << after_width);
        let branch = ((parsed.branch_word >> step) & 1) != 0;
        let old_g0 = i128::from((parsed.old_g0_word >> step) & 1);

        let (prev_f, prev_g) = if branch {
            let prev_g = next_f_full;
            ((prev_g - 2 * g).rem_euclid(modulus), prev_g)
        } else {
            let prev_f = next_f_full;
            (prev_f, (2 * g - old_g0 * prev_f).rem_euclid(modulus))
        };
        assert_eq!(prev_f & 1, 1, "reconstructed f left the odd domain");
        f = prev_f;
        g = prev_g;
    }

    (f as u16, g as u16)
}

fn round218_b5_low_window_parser_cell_with_retained_word(
    zeta_start: i128,
    f_window: u16,
    g_window: u16,
) -> (Round218B5LowWindowParserOutput, u8) {
    let window_limit = 1u16 << ROUND218_B5_LOW_WINDOW_BITS;
    assert!(f_window < window_limit, "f_window out of 10-bit range");
    assert!(g_window < window_limit, "g_window out of 10-bit range");
    assert_eq!(f_window & 1, 1, "low-window parser requires odd f");

    let mut zeta = zeta_start;
    let mut f = i128::from(f_window);
    let mut g = i128::from(g_window);
    let mut branch_word = 0u8;
    let mut old_g0_word = 0u8;
    let mut retained_word = 0u8;

    for step in 0..ROUND218_B5_LOW_STATE_BITS {
        let width = ROUND218_B5_LOW_WINDOW_BITS - step;
        let modulus = 1i128 << width;
        let next_modulus = modulus >> 1;
        f = f.rem_euclid(modulus);
        g = g.rem_euclid(modulus);
        assert_eq!(f & 1, 1, "low-window parser left the odd-f domain");

        let old_g0 = (g & 1) as u8;
        let branch = zeta < 0 && old_g0 != 0;
        old_g0_word |= old_g0 << step;
        let (next_f, next_g) = if branch {
            branch_word |= 1u8 << step;
            zeta = -zeta - 2;
            (g, (g - f) / 2)
        } else {
            zeta -= 1;
            (f, (g + i128::from(old_g0) * f) / 2)
        };

        retained_word |= (((next_f >> (width - 1)) & 1) as u8) << step;
        f = next_f.rem_euclid(next_modulus);
        g = next_g.rem_euclid(next_modulus);
    }

    (
        Round218B5LowWindowParserOutput {
            branch_word,
            old_g0_word,
            next_f_low: f as u8,
            next_g_low: g as u8,
        },
        retained_word,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Round218B5DynamicZetaTransducerSpec {
    pub zeta_min: i128,
    pub zeta_max: i128,
    pub end_zeta_min: i128,
    pub end_zeta_max: i128,
}

impl Round218B5DynamicZetaTransducerSpec {
    pub fn new(zeta_min: i128, zeta_max: i128) -> Self {
        assert!(zeta_min <= zeta_max, "dynamic zeta range is empty");
        let (end_zeta_min, end_zeta_max) = round218_b5_dynamic_zeta_end_range(zeta_min, zeta_max);
        Self {
            zeta_min,
            zeta_max,
            end_zeta_min,
            end_zeta_max,
        }
    }

    pub fn with_end_range(
        zeta_min: i128,
        zeta_max: i128,
        end_zeta_min: i128,
        end_zeta_max: i128,
    ) -> Self {
        assert!(zeta_min <= zeta_max, "dynamic zeta range is empty");
        assert!(
            end_zeta_min <= end_zeta_max,
            "dynamic end-zeta range is empty"
        );
        let (required_min, required_max) = round218_b5_dynamic_zeta_end_range(zeta_min, zeta_max);
        assert!(
            end_zeta_min <= required_min && required_max <= end_zeta_max,
            "dynamic end-zeta range [{end_zeta_min}, {end_zeta_max}] does not cover exact range [{required_min}, {required_max}]"
        );
        Self {
            zeta_min,
            zeta_max,
            end_zeta_min,
            end_zeta_max,
        }
    }

    pub fn with_same_zeta_range(zeta_min: i128, zeta_max: i128) -> Self {
        Self::with_end_range(zeta_min, zeta_max, zeta_min, zeta_max)
    }

    pub fn start_zeta_values(self) -> usize {
        inclusive_i128_len(self.zeta_min, self.zeta_max)
    }

    pub fn end_zeta_values(self) -> usize {
        inclusive_i128_len(self.end_zeta_min, self.end_zeta_max)
    }

    pub fn start_zeta_bits(self) -> usize {
        bits_for_values(self.start_zeta_values())
    }

    pub fn end_zeta_bits(self) -> usize {
        bits_for_values(self.end_zeta_values())
    }

    pub fn input_bits(self) -> usize {
        self.start_zeta_bits() + ROUND218_B5_SELECTOR_INPUT_BITS
    }

    pub fn output_bits(self) -> usize {
        ROUND218_B5_DYNAMIC_ZETA_PREFIX_OUTPUT_BITS + self.end_zeta_bits()
    }

    pub fn encode_start_zeta(self, zeta: i128) -> usize {
        assert!(
            (self.zeta_min..=self.zeta_max).contains(&zeta),
            "zeta_start={zeta} outside dynamic range [{}, {}]",
            self.zeta_min,
            self.zeta_max
        );
        (zeta - self.zeta_min) as usize
    }

    pub fn decode_start_code(self, code: usize) -> Option<i128> {
        (code < self.start_zeta_values()).then_some(self.zeta_min + code as i128)
    }

    pub fn encode_end_zeta(self, zeta: i128) -> usize {
        assert!(
            (self.end_zeta_min..=self.end_zeta_max).contains(&zeta),
            "end_zeta={zeta} outside dynamic output range [{}, {}]",
            self.end_zeta_min,
            self.end_zeta_max
        );
        (zeta - self.end_zeta_min) as usize
    }
}

pub fn build_round218_b5_low_state_selector_component(zeta_start: i128) -> Vec<Op> {
    let mut b = B::new();
    let f_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&f_low);
    let g_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&g_low);
    let branch_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&branch_word);
    let old_g0_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&old_g0_word);
    let scratch = b.alloc_qubits(round218_b5_low_state_selector_scratch_qubits(zeta_start));
    if !scratch.is_empty() {
        b.declare_qubit_register(&scratch);
    }

    b.set_phase("round218_b5_low_state_selector_component");
    emit_round218_b5_low_state_selector_with_scratch(
        &mut b,
        &f_low,
        &g_low,
        zeta_start,
        &branch_word,
        &old_g0_word,
        &scratch,
    );
    b.ops
}

pub fn build_round218_b5_dynamic_zeta_transducer_component(
    spec: Round218B5DynamicZetaTransducerSpec,
) -> Vec<Op> {
    let mut b = B::new();
    let zeta_start = b.alloc_qubits(spec.start_zeta_bits());
    if !zeta_start.is_empty() {
        b.declare_qubit_register(&zeta_start);
    }
    let f_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&f_low);
    let g_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&g_low);
    let branch_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&branch_word);
    let old_g0_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&old_g0_word);
    let end_zeta = b.alloc_qubits(spec.end_zeta_bits());
    if !end_zeta.is_empty() {
        b.declare_qubit_register(&end_zeta);
    }
    let scratch = b.alloc_qubits(round218_b5_dynamic_zeta_transducer_scratch_qubits(spec));
    if !scratch.is_empty() {
        b.declare_qubit_register(&scratch);
    }

    b.set_phase("round218_b5_dynamic_zeta_transducer_component");
    emit_round218_b5_dynamic_zeta_transducer_with_scratch(
        &mut b,
        spec,
        &zeta_start,
        &f_low,
        &g_low,
        &branch_word,
        &old_g0_word,
        &end_zeta,
        &scratch,
    );
    b.ops
}

pub fn build_round218_b5_low_window_parser_component(zeta_start: i128) -> Vec<Op> {
    let mut b = B::new();
    let f_window = b.alloc_qubits(ROUND218_B5_LOW_WINDOW_BITS);
    b.declare_qubit_register(&f_window);
    let g_window = b.alloc_qubits(ROUND218_B5_LOW_WINDOW_BITS);
    b.declare_qubit_register(&g_window);
    let branch_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&branch_word);
    let old_g0_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&old_g0_word);
    let next_f_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&next_f_low);
    let next_g_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&next_g_low);

    b.set_phase("round218_b5_low_window_parser_component");
    emit_round218_b5_low_window_parser(
        &mut b,
        &f_window,
        &g_window,
        zeta_start,
        &branch_word,
        &old_g0_word,
        &next_f_low,
        &next_g_low,
    );
    b.ops
}

pub fn build_round218_b5_dynamic_window_parser_component(
    spec: Round218B5DynamicZetaTransducerSpec,
    window_bits: usize,
) -> Vec<Op> {
    assert!(
        window_bits >= ROUND218_B5_LOW_STATE_BITS,
        "dynamic window parser needs at least a B=5 source window"
    );
    let next_bits = window_bits - ROUND218_B5_LOW_STATE_BITS;
    let mut b = B::new();
    let zeta_start = b.alloc_qubits(spec.start_zeta_bits());
    if !zeta_start.is_empty() {
        b.declare_qubit_register(&zeta_start);
    }
    let f_window = b.alloc_qubits(window_bits);
    b.declare_qubit_register(&f_window);
    let g_window = b.alloc_qubits(window_bits);
    b.declare_qubit_register(&g_window);
    let branch_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&branch_word);
    let old_g0_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&old_g0_word);
    let end_zeta = b.alloc_qubits(spec.end_zeta_bits());
    if !end_zeta.is_empty() {
        b.declare_qubit_register(&end_zeta);
    }
    let next_f = b.alloc_qubits(next_bits);
    if !next_f.is_empty() {
        b.declare_qubit_register(&next_f);
    }
    let next_g = b.alloc_qubits(next_bits);
    if !next_g.is_empty() {
        b.declare_qubit_register(&next_g);
    }

    b.set_phase("round218_b5_dynamic_window_parser_component");
    emit_round218_b5_dynamic_window_parser(
        &mut b,
        spec,
        &zeta_start,
        &f_window,
        &g_window,
        &branch_word,
        &old_g0_word,
        &end_zeta,
        &next_f,
        &next_g,
    );
    b.ops
}

pub fn build_round218_b5_twos_zeta_window_parser_component(
    zeta_bits: usize,
    window_bits: usize,
) -> Vec<Op> {
    assert!(
        zeta_bits >= 3,
        "two's-complement zeta parser needs at least 3 signed bits"
    );
    assert!(
        window_bits >= ROUND218_B5_LOW_STATE_BITS,
        "two's-complement window parser needs at least a B=5 source window"
    );
    let next_bits = window_bits - ROUND218_B5_LOW_STATE_BITS;
    let mut b = B::new();
    let zeta = b.alloc_qubits(zeta_bits);
    b.declare_qubit_register(&zeta);
    let f_window = b.alloc_qubits(window_bits);
    b.declare_qubit_register(&f_window);
    let g_window = b.alloc_qubits(window_bits);
    b.declare_qubit_register(&g_window);
    let branch_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&branch_word);
    let old_g0_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&old_g0_word);
    let next_f = b.alloc_qubits(next_bits);
    if !next_f.is_empty() {
        b.declare_qubit_register(&next_f);
    }
    let next_g = b.alloc_qubits(next_bits);
    if !next_g.is_empty() {
        b.declare_qubit_register(&next_g);
    }

    b.set_phase("round218_b5_twos_zeta_window_parser_component");
    emit_round218_b5_twos_zeta_window_parser(
        &mut b,
        &zeta,
        &f_window,
        &g_window,
        &branch_word,
        &old_g0_word,
        &next_f,
        &next_g,
    );
    b.ops
}

pub fn build_round218_b5_source_stream_forward_block_component(
    zeta_bits: usize,
    window_bits: usize,
) -> Vec<Op> {
    assert!(
        zeta_bits >= 3,
        "source-stream forward block needs at least 3 signed zeta bits"
    );
    assert!(
        window_bits >= ROUND218_B5_LOW_STATE_BITS,
        "source-stream forward block needs at least a B=5 source window"
    );
    let mut b = B::new();
    let zeta = b.alloc_qubits(zeta_bits);
    b.declare_qubit_register(&zeta);
    let f_window = b.alloc_qubits(window_bits);
    b.declare_qubit_register(&f_window);
    let g_window = b.alloc_qubits(window_bits);
    b.declare_qubit_register(&g_window);
    let branch_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&branch_word);
    let old_g0_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&old_g0_word);

    b.set_phase("round218_b5_source_stream_forward_block_component");
    emit_round218_b5_source_stream_forward_block(
        &mut b,
        &zeta,
        &f_window,
        &g_window,
        0,
        &branch_word,
        &old_g0_word,
    );
    b.ops
}

pub fn build_round218_b5_twos_zeta_control_word_parser_component(
    zeta_bits: usize,
    window_bits: usize,
) -> Vec<Op> {
    assert!(
        zeta_bits >= 3,
        "two's-complement zeta parser needs at least 3 signed bits"
    );
    assert!(
        window_bits >= ROUND218_B5_LOW_STATE_BITS,
        "two's-complement control parser needs at least a B=5 source window"
    );
    let mut b = B::new();
    let zeta = b.alloc_qubits(zeta_bits);
    b.declare_qubit_register(&zeta);
    let f_window = b.alloc_qubits(window_bits);
    b.declare_qubit_register(&f_window);
    let g_window = b.alloc_qubits(window_bits);
    b.declare_qubit_register(&g_window);
    let branch_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&branch_word);
    let old_g0_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    b.declare_qubit_register(&old_g0_word);

    b.set_phase("round218_b5_twos_zeta_control_word_parser_component");
    emit_round218_b5_twos_zeta_control_word_parser(
        &mut b,
        &zeta,
        &f_window,
        &g_window,
        &branch_word,
        &old_g0_word,
    );
    b.ops
}

pub fn round218_b5_low_state_selector_term_counts(
    zeta_start: i128,
) -> [usize; ROUND218_B5_SELECTOR_OUTPUT_BITS] {
    let masks = selector_anf_masks(zeta_start);
    let mut counts = [0usize; ROUND218_B5_SELECTOR_OUTPUT_BITS];
    for (idx, output_masks) in masks.iter().enumerate() {
        counts[idx] = output_masks.len();
    }
    counts
}

pub fn round218_b5_low_state_selector_scratch_qubits(zeta_start: i128) -> usize {
    selector_anf_masks(zeta_start)
        .iter()
        .flat_map(|output_masks| output_masks.iter())
        .map(|mask| mask.count_ones() as usize)
        .max()
        .unwrap_or(0)
        .saturating_sub(2)
}

pub fn round218_b5_dynamic_zeta_end_range(zeta_min: i128, zeta_max: i128) -> (i128, i128) {
    assert!(zeta_min <= zeta_max, "dynamic zeta range is empty");
    let mut out_min = i128::MAX;
    let mut out_max = i128::MIN;
    for zeta_start in zeta_min..=zeta_max {
        for f_low in (1u8..(1u8 << ROUND218_B5_LOW_STATE_BITS)).step_by(2) {
            for g_low in 0u8..(1u8 << ROUND218_B5_LOW_STATE_BITS) {
                let row = round218_b5_program::block_row(
                    0,
                    round218_b5_program::BlockSelector {
                        zeta_start,
                        f_low,
                        g_low,
                        width: ROUND218_B5_LOW_STATE_BITS as u8,
                    },
                );
                out_min = out_min.min(row.end_zeta);
                out_max = out_max.max(row.end_zeta);
            }
        }
    }
    (out_min, out_max)
}

pub fn round218_b5_dynamic_zeta_transducer_term_counts(
    spec: Round218B5DynamicZetaTransducerSpec,
) -> Vec<usize> {
    dynamic_zeta_transducer_anf_masks(spec)
        .into_iter()
        .map(|output_masks| output_masks.len())
        .collect()
}

pub fn round218_b5_dynamic_zeta_transducer_scratch_qubits(
    spec: Round218B5DynamicZetaTransducerSpec,
) -> usize {
    dynamic_zeta_transducer_anf_masks(spec)
        .iter()
        .flat_map(|output_masks| output_masks.iter())
        .map(|mask| mask.count_ones() as usize)
        .max()
        .unwrap_or(0)
        .saturating_sub(2)
}

pub(super) fn emit_round218_b5_low_state_selector(
    b: &mut B,
    f_low: &[QubitId],
    g_low: &[QubitId],
    zeta_start: i128,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    let scratch = b.alloc_qubits(round218_b5_low_state_selector_scratch_qubits(zeta_start));
    emit_round218_b5_low_state_selector_with_scratch(
        b,
        f_low,
        g_low,
        zeta_start,
        branch_word,
        old_g0_word,
        &scratch,
    );
    b.free_vec(&scratch);
}

pub(super) fn emit_round218_b5_low_state_selector_uncompute(
    b: &mut B,
    f_low: &[QubitId],
    g_low: &[QubitId],
    zeta_start: i128,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    emit_round218_b5_low_state_selector(b, f_low, g_low, zeta_start, branch_word, old_g0_word);
}

pub(super) fn emit_round218_b5_low_state_selector_with_scratch(
    b: &mut B,
    f_low: &[QubitId],
    g_low: &[QubitId],
    zeta_start: i128,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    scratch: &[QubitId],
) {
    assert_eq!(f_low.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(g_low.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(old_g0_word.len(), ROUND218_B5_LOW_STATE_BITS);

    let masks = selector_anf_masks(zeta_start);
    let required_scratch = masks
        .iter()
        .flat_map(|output_masks| output_masks.iter())
        .map(|mask| mask.count_ones() as usize)
        .max()
        .unwrap_or(0)
        .saturating_sub(2);
    assert!(
        scratch.len() >= required_scratch,
        "Round218 B=5 selector needs {required_scratch} scratch qubits for zeta_start={zeta_start}, got {}",
        scratch.len()
    );

    let mut controls = Vec::with_capacity(ROUND218_B5_SELECTOR_INPUT_BITS);
    controls.extend_from_slice(f_low);
    controls.extend_from_slice(g_low);

    for (output_idx, output_masks) in masks.iter().enumerate() {
        let target = if output_idx < ROUND218_B5_LOW_STATE_BITS {
            branch_word[output_idx]
        } else {
            old_g0_word[output_idx - ROUND218_B5_LOW_STATE_BITS]
        };
        for &mask in output_masks {
            emit_monomial_toggle(b, &controls, mask, target, scratch);
        }
    }
}

pub(super) fn emit_round218_b5_low_state_selector_uncompute_with_scratch(
    b: &mut B,
    f_low: &[QubitId],
    g_low: &[QubitId],
    zeta_start: i128,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    scratch: &[QubitId],
) {
    emit_round218_b5_low_state_selector_with_scratch(
        b,
        f_low,
        g_low,
        zeta_start,
        branch_word,
        old_g0_word,
        scratch,
    );
}

pub(super) fn emit_round218_b5_dynamic_zeta_transducer(
    b: &mut B,
    spec: Round218B5DynamicZetaTransducerSpec,
    zeta_start: &[QubitId],
    f_low: &[QubitId],
    g_low: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    end_zeta: &[QubitId],
) {
    let scratch = b.alloc_qubits(round218_b5_dynamic_zeta_transducer_scratch_qubits(spec));
    emit_round218_b5_dynamic_zeta_transducer_with_scratch(
        b,
        spec,
        zeta_start,
        f_low,
        g_low,
        branch_word,
        old_g0_word,
        end_zeta,
        &scratch,
    );
    b.free_vec(&scratch);
}

pub(super) fn emit_round218_b5_dynamic_zeta_transducer_uncompute(
    b: &mut B,
    spec: Round218B5DynamicZetaTransducerSpec,
    zeta_start: &[QubitId],
    f_low: &[QubitId],
    g_low: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    end_zeta: &[QubitId],
) {
    emit_round218_b5_dynamic_zeta_transducer(
        b,
        spec,
        zeta_start,
        f_low,
        g_low,
        branch_word,
        old_g0_word,
        end_zeta,
    );
}

pub(super) fn emit_round218_b5_dynamic_zeta_transducer_with_scratch(
    b: &mut B,
    spec: Round218B5DynamicZetaTransducerSpec,
    zeta_start: &[QubitId],
    f_low: &[QubitId],
    g_low: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    end_zeta: &[QubitId],
    scratch: &[QubitId],
) {
    assert_eq!(zeta_start.len(), spec.start_zeta_bits());
    assert_eq!(f_low.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(g_low.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(old_g0_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(end_zeta.len(), spec.end_zeta_bits());

    let masks = dynamic_zeta_transducer_anf_masks(spec);
    let required_scratch = masks
        .iter()
        .flat_map(|output_masks| output_masks.iter())
        .map(|mask| mask.count_ones() as usize)
        .max()
        .unwrap_or(0)
        .saturating_sub(2);
    assert!(
        scratch.len() >= required_scratch,
        "Round218 B=5 dynamic-zeta transducer needs {required_scratch} scratch qubits, got {}",
        scratch.len()
    );

    let mut controls = Vec::with_capacity(spec.input_bits());
    controls.extend_from_slice(zeta_start);
    controls.extend_from_slice(f_low);
    controls.extend_from_slice(g_low);

    for (output_idx, output_masks) in masks.iter().enumerate() {
        let target = dynamic_zeta_transducer_target(output_idx, branch_word, old_g0_word, end_zeta);
        for &mask in output_masks {
            emit_monomial_toggle_u64(b, &controls, mask, target, scratch);
        }
    }
}

pub(super) fn emit_round218_b5_dynamic_zeta_transducer_uncompute_with_scratch(
    b: &mut B,
    spec: Round218B5DynamicZetaTransducerSpec,
    zeta_start: &[QubitId],
    f_low: &[QubitId],
    g_low: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    end_zeta: &[QubitId],
    scratch: &[QubitId],
) {
    emit_round218_b5_dynamic_zeta_transducer_with_scratch(
        b,
        spec,
        zeta_start,
        f_low,
        g_low,
        branch_word,
        old_g0_word,
        end_zeta,
        scratch,
    );
}

pub(super) fn emit_round218_b5_low_window_parser(
    b: &mut B,
    f_window: &[QubitId],
    g_window: &[QubitId],
    zeta_start: i128,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    next_f_low: &[QubitId],
    next_g_low: &[QubitId],
) {
    assert_eq!(f_window.len(), ROUND218_B5_LOW_WINDOW_BITS);
    assert_eq!(g_window.len(), ROUND218_B5_LOW_WINDOW_BITS);
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(old_g0_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(next_f_low.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(next_g_low.len(), ROUND218_B5_LOW_STATE_BITS);

    let branch_tmp = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    let old_g0_tmp = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);

    let selector_scratch =
        b.alloc_qubits(round218_b5_low_state_selector_scratch_qubits(zeta_start));
    emit_round218_b5_low_state_selector_with_scratch(
        b,
        &f_window[..ROUND218_B5_LOW_STATE_BITS],
        &g_window[..ROUND218_B5_LOW_STATE_BITS],
        zeta_start,
        &branch_tmp,
        &old_g0_tmp,
        &selector_scratch,
    );
    b.free_vec(&selector_scratch);

    for i in 0..ROUND218_B5_LOW_STATE_BITS {
        b.cx(branch_tmp[i], branch_word[i]);
        b.cx(old_g0_tmp[i], old_g0_word[i]);
    }
    emit_round218_b5_window_update_copy(
        b,
        f_window,
        g_window,
        &branch_tmp,
        &old_g0_tmp,
        next_f_low,
        next_g_low,
    );

    let selector_scratch =
        b.alloc_qubits(round218_b5_low_state_selector_scratch_qubits(zeta_start));
    emit_round218_b5_low_state_selector_uncompute_with_scratch(
        b,
        &f_window[..ROUND218_B5_LOW_STATE_BITS],
        &g_window[..ROUND218_B5_LOW_STATE_BITS],
        zeta_start,
        &branch_tmp,
        &old_g0_tmp,
        &selector_scratch,
    );
    b.free_vec(&selector_scratch);

    b.free_vec(&old_g0_tmp);
    b.free_vec(&branch_tmp);
}

pub(super) fn emit_round218_b5_dynamic_window_parser(
    b: &mut B,
    spec: Round218B5DynamicZetaTransducerSpec,
    zeta_start: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    end_zeta: &[QubitId],
    next_f: &[QubitId],
    next_g: &[QubitId],
) {
    assert_eq!(zeta_start.len(), spec.start_zeta_bits());
    assert!(
        f_window.len() >= ROUND218_B5_LOW_STATE_BITS,
        "dynamic window parser needs at least B=5 source bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(old_g0_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(end_zeta.len(), spec.end_zeta_bits());
    assert_eq!(
        next_f.len(),
        f_window.len() - ROUND218_B5_LOW_STATE_BITS,
        "next_f must retain window_bits-B low bits"
    );
    assert_eq!(next_g.len(), next_f.len());

    let scratch = b.alloc_qubits(round218_b5_dynamic_zeta_transducer_scratch_qubits(spec));
    emit_round218_b5_dynamic_zeta_transducer_with_scratch(
        b,
        spec,
        zeta_start,
        &f_window[..ROUND218_B5_LOW_STATE_BITS],
        &g_window[..ROUND218_B5_LOW_STATE_BITS],
        branch_word,
        old_g0_word,
        end_zeta,
        &scratch,
    );
    b.free_vec(&scratch);

    emit_round218_b5_window_update_copy(
        b,
        f_window,
        g_window,
        branch_word,
        old_g0_word,
        next_f,
        next_g,
    );
}

pub(super) fn emit_round218_b5_twos_zeta_window_parser(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    next_f: &[QubitId],
    next_g: &[QubitId],
) {
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta parser needs at least 3 signed bits"
    );
    assert!(
        zeta.len() < 256,
        "two's-complement zeta parser constants assume <256 bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert!(
        f_window.len() >= ROUND218_B5_LOW_STATE_BITS,
        "source window must contain at least B=5 bits"
    );
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(old_g0_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(
        next_f.len(),
        f_window.len() - ROUND218_B5_LOW_STATE_BITS,
        "next_f must retain window_bits-B low bits"
    );
    assert_eq!(next_g.len(), next_f.len());

    let f_work = b.alloc_qubits(f_window.len());
    let g_work = b.alloc_qubits(g_window.len());

    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }

    let sign = zeta[zeta.len() - 1];
    for step in 0..ROUND218_B5_LOW_STATE_BITS {
        let width = f_window.len() - step;
        b.cx(g_work[0], old_g0_word[step]);
        b.ccx(sign, g_work[0], branch_word[step]);
        emit_round218_b5_low_window_apply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_word[step],
        );
        emit_round218_b5_twos_zeta_update_step(b, zeta, branch_word[step]);
    }

    for i in 0..next_f.len() {
        b.cx(f_work[i], next_f[i]);
        b.cx(g_work[i], next_g[i]);
    }

    for step in (0..ROUND218_B5_LOW_STATE_BITS).rev() {
        let width = f_window.len() - step;
        emit_round218_b5_low_window_unapply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_word[step],
        );
    }

    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }

    b.free_vec(&g_work);
    b.free_vec(&f_work);
}

pub(super) fn emit_round218_b5_twos_zeta_control_word_parser(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta parser needs at least 3 signed bits"
    );
    assert!(
        zeta.len() < 256,
        "two's-complement zeta parser constants assume <256 bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert!(
        f_window.len() >= ROUND218_B5_LOW_STATE_BITS,
        "source window must contain at least B=5 bits"
    );
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(old_g0_word.len(), ROUND218_B5_LOW_STATE_BITS);

    let zeta_work = b.alloc_qubits(zeta.len());
    let f_work = b.alloc_qubits(f_window.len());
    let g_work = b.alloc_qubits(g_window.len());

    for i in 0..zeta.len() {
        b.cx(zeta[i], zeta_work[i]);
    }
    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }

    let sign = zeta_work[zeta_work.len() - 1];
    for step in 0..ROUND218_B5_LOW_STATE_BITS {
        let width = f_window.len() - step;
        b.cx(g_work[0], old_g0_word[step]);
        b.ccx(sign, g_work[0], branch_word[step]);
        emit_round218_b5_low_window_apply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_word[step],
        );
        emit_round218_b5_twos_zeta_update_step(b, &zeta_work, branch_word[step]);
    }

    for step in (0..ROUND218_B5_LOW_STATE_BITS).rev() {
        let width = f_window.len() - step;
        emit_round218_b5_twos_zeta_update_step_inverse(b, &zeta_work, branch_word[step]);
        emit_round218_b5_low_window_unapply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_word[step],
        );
    }

    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }
    for i in 0..zeta.len() {
        b.cx(zeta[i], zeta_work[i]);
    }

    b.free_vec(&g_work);
    b.free_vec(&f_work);
    b.free_vec(&zeta_work);
}

pub(super) fn emit_round218_b5_twos_zeta_control_word_parser_uncompute(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta parser needs at least 3 signed bits"
    );
    assert!(
        zeta.len() < 256,
        "two's-complement zeta parser constants assume <256 bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert!(
        f_window.len() >= ROUND218_B5_LOW_STATE_BITS,
        "source window must contain at least B=5 bits"
    );
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(old_g0_word.len(), ROUND218_B5_LOW_STATE_BITS);

    let zeta_work = b.alloc_qubits(zeta.len());
    let f_work = b.alloc_qubits(f_window.len());
    let g_work = b.alloc_qubits(g_window.len());

    for i in 0..zeta.len() {
        b.cx(zeta[i], zeta_work[i]);
    }
    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }

    for step in 0..ROUND218_B5_LOW_STATE_BITS {
        let width = f_window.len() - step;
        emit_round218_b5_low_window_apply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_word[step],
        );
        emit_round218_b5_twos_zeta_update_step(b, &zeta_work, branch_word[step]);
    }

    for step in (0..ROUND218_B5_LOW_STATE_BITS).rev() {
        let width = f_window.len() - step;
        emit_round218_b5_twos_zeta_update_step_inverse(b, &zeta_work, branch_word[step]);
        emit_round218_b5_low_window_unapply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_word[step],
        );
        let sign = zeta_work[zeta_work.len() - 1];
        b.ccx(sign, g_work[0], branch_word[step]);
        b.cx(g_work[0], old_g0_word[step]);
    }

    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }
    for i in 0..zeta.len() {
        b.cx(zeta[i], zeta_work[i]);
    }

    b.free_vec(&g_work);
    b.free_vec(&f_work);
    b.free_vec(&zeta_work);
}

pub(super) fn emit_round218_b5_twos_zeta_branch_word_parser(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    branch_word: &[QubitId],
) {
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta parser needs at least 3 signed bits"
    );
    assert!(
        zeta.len() < 256,
        "two's-complement zeta parser constants assume <256 bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert!(
        f_window.len() >= ROUND218_B5_LOW_STATE_BITS,
        "source window must contain at least B=5 bits"
    );
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);

    let old_g0_tmp = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    let zeta_work = b.alloc_qubits(zeta.len());
    let f_work = b.alloc_qubits(f_window.len());
    let g_work = b.alloc_qubits(g_window.len());

    for i in 0..zeta.len() {
        b.cx(zeta[i], zeta_work[i]);
    }
    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }

    let sign = zeta_work[zeta_work.len() - 1];
    for step in 0..ROUND218_B5_LOW_STATE_BITS {
        let width = f_window.len() - step;
        b.cx(g_work[0], old_g0_tmp[step]);
        b.ccx(sign, g_work[0], branch_word[step]);
        emit_round218_b5_low_window_apply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_tmp[step],
        );
        emit_round218_b5_twos_zeta_update_step(b, &zeta_work, branch_word[step]);
    }

    for step in (0..ROUND218_B5_LOW_STATE_BITS).rev() {
        let width = f_window.len() - step;
        emit_round218_b5_twos_zeta_update_step_inverse(b, &zeta_work, branch_word[step]);
        emit_round218_b5_low_window_unapply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_tmp[step],
        );
        b.cx(g_work[0], old_g0_tmp[step]);
    }

    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }
    for i in 0..zeta.len() {
        b.cx(zeta[i], zeta_work[i]);
    }

    b.free_vec(&g_work);
    b.free_vec(&f_work);
    b.free_vec(&zeta_work);
    b.free_vec(&old_g0_tmp);
}

pub(super) fn emit_round218_b5_twos_zeta_branch_word_parser_uncompute(
    b: &mut B,
    zeta: &[QubitId],
    f_window: &[QubitId],
    g_window: &[QubitId],
    branch_word: &[QubitId],
) {
    assert!(
        zeta.len() >= 3,
        "two's-complement zeta parser needs at least 3 signed bits"
    );
    assert!(
        zeta.len() < 256,
        "two's-complement zeta parser constants assume <256 bits"
    );
    assert_eq!(g_window.len(), f_window.len());
    assert!(
        f_window.len() >= ROUND218_B5_LOW_STATE_BITS,
        "source window must contain at least B=5 bits"
    );
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);

    let old_g0_tmp = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
    let zeta_work = b.alloc_qubits(zeta.len());
    let f_work = b.alloc_qubits(f_window.len());
    let g_work = b.alloc_qubits(g_window.len());

    for i in 0..zeta.len() {
        b.cx(zeta[i], zeta_work[i]);
    }
    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }

    for step in 0..ROUND218_B5_LOW_STATE_BITS {
        let width = f_window.len() - step;
        b.cx(g_work[0], old_g0_tmp[step]);
        emit_round218_b5_low_window_apply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_tmp[step],
        );
        emit_round218_b5_twos_zeta_update_step(b, &zeta_work, branch_word[step]);
    }

    for step in (0..ROUND218_B5_LOW_STATE_BITS).rev() {
        let width = f_window.len() - step;
        emit_round218_b5_twos_zeta_update_step_inverse(b, &zeta_work, branch_word[step]);
        emit_round218_b5_low_window_unapply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_tmp[step],
        );
        let sign = zeta_work[zeta_work.len() - 1];
        b.ccx(sign, g_work[0], branch_word[step]);
        b.cx(g_work[0], old_g0_tmp[step]);
    }

    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }
    for i in 0..zeta.len() {
        b.cx(zeta[i], zeta_work[i]);
    }

    b.free_vec(&g_work);
    b.free_vec(&f_work);
    b.free_vec(&zeta_work);
    b.free_vec(&old_g0_tmp);
}

pub(super) fn emit_round218_b5_twos_zeta_update_step(b: &mut B, zeta: &[QubitId], branch: QubitId) {
    let modulus = U256::from(1u64) << zeta.len();
    let mask = modulus - U256::from(1u64);
    super::add_nbit_const(b, zeta, mask);
    for &q in zeta {
        b.cx(branch, q);
    }
    super::cadd_nbit_const(b, zeta, modulus - U256::from(2u64), branch);
}

pub(super) fn emit_round218_b5_twos_zeta_update_step_inverse(
    b: &mut B,
    zeta: &[QubitId],
    branch: QubitId,
) {
    let modulus = U256::from(1u64) << zeta.len();
    let mask = modulus - U256::from(1u64);
    super::csub_nbit_const(b, zeta, modulus - U256::from(2u64), branch);
    for &q in zeta {
        b.cx(branch, q);
    }
    super::sub_nbit_const(b, zeta, mask);
}

pub(super) fn emit_round218_b5_source_stream_forward_block(
    b: &mut B,
    zeta: &[QubitId],
    f: &[QubitId],
    g: &[QubitId],
    step0: usize,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    assert_eq!(
        branch_word.len(),
        round218_b5_program::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(old_g0_word.len(), branch_word.len());
    assert_eq!(f.len(), g.len());
    assert!(
        step0 + branch_word.len() <= f.len(),
        "source stream block exceeds source width"
    );

    let sign = zeta[zeta.len() - 1];
    for j in 0..branch_word.len() {
        let step = step0 + j;
        let width = f.len() - step;
        b.cx(g[0], old_g0_word[j]);
        b.ccx(sign, g[0], branch_word[j]);
        emit_round218_b5_low_window_apply_step(b, f, g, width, branch_word[j], old_g0_word[j]);
        emit_round218_b5_twos_zeta_update_step(b, zeta, branch_word[j]);
    }
}

pub(super) fn emit_round218_b5_source_stream_forward_block_from_bits(
    b: &mut B,
    zeta: &[QubitId],
    f: &[QubitId],
    g: &[QubitId],
    step0: usize,
    branch_bits: &[BitId],
    old_g0_bits: &[BitId],
) {
    assert_eq!(
        branch_bits.len(),
        round218_b5_program::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(old_g0_bits.len(), branch_bits.len());
    assert_eq!(f.len(), g.len());
    assert!(
        step0 + branch_bits.len() <= f.len(),
        "source stream measured-control block exceeds source width"
    );

    let add_ctrl = b.alloc_bit();
    for j in 0..branch_bits.len() {
        let step = step0 + j;
        let width = f.len() - step;
        emit_round218_b5_low_window_apply_step_from_bits(
            b,
            f,
            g,
            width,
            branch_bits[j],
            old_g0_bits[j],
            add_ctrl,
        );
        emit_round218_b5_twos_zeta_update_step_from_bit(b, zeta, branch_bits[j]);
    }
    b.bit_store0(add_ctrl);
}

pub(super) fn emit_round218_b5_source_stream_forward_block_from_controls(
    b: &mut B,
    zeta: &[QubitId],
    f: &[QubitId],
    g: &[QubitId],
    step0: usize,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    assert_eq!(
        branch_word.len(),
        round218_b5_program::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(old_g0_word.len(), branch_word.len());
    assert_eq!(f.len(), g.len());
    assert!(
        step0 + branch_word.len() <= f.len(),
        "source stream control block exceeds source width"
    );

    for j in 0..branch_word.len() {
        let step = step0 + j;
        let width = f.len() - step;
        emit_round218_b5_low_window_apply_step(b, f, g, width, branch_word[j], old_g0_word[j]);
        emit_round218_b5_twos_zeta_update_step(b, zeta, branch_word[j]);
    }
}

pub(super) fn emit_round218_b5_source_stream_backward_block(
    b: &mut B,
    zeta: &[QubitId],
    f: &[QubitId],
    g: &[QubitId],
    step0: usize,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
) {
    assert_eq!(
        branch_word.len(),
        round218_b5_program::ROUND218_B5_BLOCK_BITS
    );
    assert_eq!(old_g0_word.len(), branch_word.len());
    assert_eq!(f.len(), g.len());
    assert!(
        step0 + branch_word.len() <= f.len(),
        "source stream block exceeds source width"
    );

    for j in (0..branch_word.len()).rev() {
        let step = step0 + j;
        let width = f.len() - step;
        emit_round218_b5_twos_zeta_update_step_inverse(b, zeta, branch_word[j]);
        emit_round218_b5_low_window_unapply_step(b, f, g, width, branch_word[j], old_g0_word[j]);
        let sign = zeta[zeta.len() - 1];
        b.ccx(sign, g[0], branch_word[j]);
        b.cx(g[0], old_g0_word[j]);
    }
}

fn emit_round218_b5_low_window_apply_step_from_bits(
    b: &mut B,
    f_work: &[QubitId],
    g_work: &[QubitId],
    width: usize,
    branch: BitId,
    old_g0: BitId,
    add_ctrl: BitId,
) {
    b.bit_store0(add_ctrl);
    b.bit_invert_if(add_ctrl, old_g0);
    b.bit_invert_if(add_ctrl, branch);
    b.push_condition(add_ctrl);
    super::add_nbit_qq(b, &f_work[..width], &g_work[..width]);
    b.pop_condition();
    b.bit_store0(add_ctrl);

    b.push_condition(branch);
    for i in 0..width {
        b.swap(f_work[i], g_work[i]);
    }
    for &q in &g_work[..width] {
        b.x(q);
    }
    super::add_nbit_const(b, &g_work[..width], U256::from(1u64));
    super::add_nbit_qq(b, &f_work[..width], &g_work[..width]);
    b.pop_condition();

    emit_rotate_down_one(b, &g_work[..width]);
}

fn emit_round218_b5_twos_zeta_update_step_from_bit(b: &mut B, zeta: &[QubitId], branch: BitId) {
    let modulus = U256::from(1u64) << zeta.len();
    let mask = modulus - U256::from(1u64);
    super::add_nbit_const(b, zeta, mask);
    b.push_condition(branch);
    for &q in zeta {
        b.x(q);
    }
    super::add_nbit_const(b, zeta, modulus - U256::from(2u64));
    b.pop_condition();
}

pub(super) fn emit_round218_b5_window_update_copy(
    b: &mut B,
    f_window: &[QubitId],
    g_window: &[QubitId],
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    next_f: &[QubitId],
    next_g: &[QubitId],
) {
    assert_eq!(g_window.len(), f_window.len());
    assert!(
        f_window.len() >= ROUND218_B5_LOW_STATE_BITS,
        "source window must contain at least B=5 bits"
    );
    assert_eq!(branch_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(old_g0_word.len(), ROUND218_B5_LOW_STATE_BITS);
    assert_eq!(
        next_f.len(),
        f_window.len() - ROUND218_B5_LOW_STATE_BITS,
        "next_f must retain window_bits-B low bits"
    );
    assert_eq!(next_g.len(), next_f.len());

    let f_work = b.alloc_qubits(f_window.len());
    let g_work = b.alloc_qubits(g_window.len());

    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }

    for step in 0..ROUND218_B5_LOW_STATE_BITS {
        let width = f_window.len() - step;
        emit_round218_b5_low_window_apply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_word[step],
        );
    }

    for i in 0..next_f.len() {
        b.cx(f_work[i], next_f[i]);
        b.cx(g_work[i], next_g[i]);
    }

    for step in (0..ROUND218_B5_LOW_STATE_BITS).rev() {
        let width = f_window.len() - step;
        emit_round218_b5_low_window_unapply_step(
            b,
            &f_work,
            &g_work,
            width,
            branch_word[step],
            old_g0_word[step],
        );
    }

    for i in 0..f_window.len() {
        b.cx(f_window[i], f_work[i]);
        b.cx(g_window[i], g_work[i]);
    }

    b.free_vec(&g_work);
    b.free_vec(&f_work);
}

pub(super) fn emit_round218_b5_low_window_apply_step(
    b: &mut B,
    f_work: &[QubitId],
    g_work: &[QubitId],
    width: usize,
    branch: QubitId,
    old_g0: QubitId,
) {
    let add_ctrl = b.alloc_qubit();
    b.cx(old_g0, add_ctrl);
    b.cx(branch, add_ctrl);
    emit_controlled_add_mod_power_two(b, &f_work[..width], &g_work[..width], add_ctrl);
    b.cx(branch, add_ctrl);
    b.cx(old_g0, add_ctrl);
    b.free(add_ctrl);

    for i in 0..width {
        super::cswap(b, branch, f_work[i], g_work[i]);
    }
    emit_controlled_neg_mod_power_two(b, &g_work[..width], branch);
    emit_controlled_add_mod_power_two(b, &f_work[..width], &g_work[..width], branch);
    emit_rotate_down_one(b, &g_work[..width]);
}

pub(super) fn emit_round218_b5_low_window_unapply_step(
    b: &mut B,
    f_work: &[QubitId],
    g_work: &[QubitId],
    width: usize,
    branch: QubitId,
    old_g0: QubitId,
) {
    emit_rotate_up_one(b, &g_work[..width]);
    emit_controlled_sub_mod_power_two(b, &f_work[..width], &g_work[..width], branch);
    emit_controlled_neg_mod_power_two(b, &g_work[..width], branch);
    for i in 0..width {
        super::cswap(b, branch, f_work[i], g_work[i]);
    }

    let add_ctrl = b.alloc_qubit();
    b.cx(old_g0, add_ctrl);
    b.cx(branch, add_ctrl);
    emit_controlled_sub_mod_power_two(b, &f_work[..width], &g_work[..width], add_ctrl);
    b.cx(branch, add_ctrl);
    b.cx(old_g0, add_ctrl);
    b.free(add_ctrl);
}

fn emit_controlled_add_mod_power_two(
    b: &mut B,
    addend: &[QubitId],
    acc: &[QubitId],
    ctrl: QubitId,
) {
    assert_eq!(addend.len(), acc.len());
    let gated = b.alloc_qubits(addend.len());
    for i in 0..addend.len() {
        b.ccx(ctrl, addend[i], gated[i]);
    }
    super::add_nbit_qq(b, &gated, acc);
    for i in 0..addend.len() {
        b.ccx(ctrl, addend[i], gated[i]);
    }
    b.free_vec(&gated);
}

fn emit_controlled_sub_mod_power_two(
    b: &mut B,
    subtrahend: &[QubitId],
    acc: &[QubitId],
    ctrl: QubitId,
) {
    assert_eq!(subtrahend.len(), acc.len());
    let gated = b.alloc_qubits(subtrahend.len());
    for i in 0..subtrahend.len() {
        b.ccx(ctrl, subtrahend[i], gated[i]);
    }
    super::sub_nbit_qq(b, &gated, acc);
    for i in 0..subtrahend.len() {
        b.ccx(ctrl, subtrahend[i], gated[i]);
    }
    b.free_vec(&gated);
}

fn emit_controlled_neg_mod_power_two(b: &mut B, reg: &[QubitId], ctrl: QubitId) {
    for &q in reg {
        b.cx(ctrl, q);
    }
    super::cadd_nbit_const(b, reg, alloy_primitives::U256::from(1u64), ctrl);
}

fn emit_rotate_down_one(b: &mut B, reg: &[QubitId]) {
    for i in 0..reg.len().saturating_sub(1) {
        b.swap(reg[i], reg[i + 1]);
    }
}

fn emit_rotate_up_one(b: &mut B, reg: &[QubitId]) {
    for i in (0..reg.len().saturating_sub(1)).rev() {
        b.swap(reg[i], reg[i + 1]);
    }
}

fn selector_anf_masks(zeta_start: i128) -> Vec<Vec<u16>> {
    (0..ROUND218_B5_SELECTOR_OUTPUT_BITS)
        .map(|output_idx| selector_output_anf_masks(zeta_start, output_idx))
        .collect()
}

fn dynamic_zeta_transducer_target(
    output_idx: usize,
    branch_word: &[QubitId],
    old_g0_word: &[QubitId],
    end_zeta: &[QubitId],
) -> QubitId {
    if output_idx < ROUND218_B5_LOW_STATE_BITS {
        branch_word[output_idx]
    } else if output_idx < ROUND218_B5_SELECTOR_OUTPUT_BITS {
        old_g0_word[output_idx - ROUND218_B5_LOW_STATE_BITS]
    } else {
        end_zeta[output_idx - ROUND218_B5_SELECTOR_OUTPUT_BITS]
    }
}

fn dynamic_zeta_transducer_anf_masks(spec: Round218B5DynamicZetaTransducerSpec) -> Vec<Vec<u64>> {
    assert!(
        spec.input_bits() <= ROUND218_B5_DYNAMIC_ZETA_SYNTHESIS_INPUT_LIMIT,
        "Round218 B=5 dynamic-zeta ANF has {} inputs; limit is {}",
        spec.input_bits(),
        ROUND218_B5_DYNAMIC_ZETA_SYNTHESIS_INPUT_LIMIT
    );
    (0..spec.output_bits())
        .map(|output_idx| dynamic_zeta_transducer_output_anf_masks(spec, output_idx))
        .collect()
}

fn dynamic_zeta_transducer_output_anf_masks(
    spec: Round218B5DynamicZetaTransducerSpec,
    output_idx: usize,
) -> Vec<u64> {
    assert!(output_idx < spec.output_bits());
    let input_bits = spec.input_bits();
    let mut truth = vec![0u8; 1usize << input_bits];
    let zeta_bits = spec.start_zeta_bits();
    let zeta_mask = low_usize_mask(zeta_bits);
    let low_mask = low_usize_mask(ROUND218_B5_LOW_STATE_BITS);

    for (input, value) in truth.iter_mut().enumerate() {
        let zeta_code = input & zeta_mask;
        let Some(zeta_start) = spec.decode_start_code(zeta_code) else {
            continue;
        };
        let low_state = input >> zeta_bits;
        let f_low = (low_state & low_mask) as u8;
        if f_low & 1 == 0 {
            continue;
        }
        let g_low = ((low_state >> ROUND218_B5_LOW_STATE_BITS) & low_mask) as u8;
        let row = round218_b5_program::block_row(
            0,
            round218_b5_program::BlockSelector {
                zeta_start,
                f_low,
                g_low,
                width: ROUND218_B5_LOW_STATE_BITS as u8,
            },
        );
        *value = dynamic_zeta_transducer_output_bit(spec, &row, output_idx);
    }

    anf_masks_from_truth(input_bits, truth)
}

fn dynamic_zeta_transducer_output_bit(
    spec: Round218B5DynamicZetaTransducerSpec,
    row: &round218_b5_program::BlockRow,
    output_idx: usize,
) -> u8 {
    if output_idx < ROUND218_B5_LOW_STATE_BITS {
        (row.branch_word >> output_idx) & 1
    } else if output_idx < ROUND218_B5_SELECTOR_OUTPUT_BITS {
        let bit_idx = output_idx - ROUND218_B5_LOW_STATE_BITS;
        (row.old_g0_word >> bit_idx) & 1
    } else {
        let bit_idx = output_idx - ROUND218_B5_SELECTOR_OUTPUT_BITS;
        ((spec.encode_end_zeta(row.end_zeta) >> bit_idx) & 1) as u8
    }
}

fn selector_output_anf_masks(zeta_start: i128, output_idx: usize) -> Vec<u16> {
    assert!(output_idx < ROUND218_B5_SELECTOR_OUTPUT_BITS);
    let mut truth = [0u8; 1usize << ROUND218_B5_SELECTOR_INPUT_BITS];

    for (input, value) in truth.iter_mut().enumerate() {
        let f_low = (input & ((1usize << ROUND218_B5_LOW_STATE_BITS) - 1)) as u8;
        if f_low & 1 == 0 {
            continue;
        }
        let g_low = ((input >> ROUND218_B5_LOW_STATE_BITS)
            & ((1usize << ROUND218_B5_LOW_STATE_BITS) - 1)) as u8;
        let row = round218_b5_program::block_row(
            0,
            round218_b5_program::BlockSelector {
                zeta_start,
                f_low,
                g_low,
                width: ROUND218_B5_LOW_STATE_BITS as u8,
            },
        );
        let word = if output_idx < ROUND218_B5_LOW_STATE_BITS {
            row.branch_word
        } else {
            row.old_g0_word
        };
        let bit_idx = output_idx % ROUND218_B5_LOW_STATE_BITS;
        *value = (word >> bit_idx) & 1;
    }

    for bit in 0..ROUND218_B5_SELECTOR_INPUT_BITS {
        for mask in 0..truth.len() {
            if (mask & (1usize << bit)) != 0 {
                truth[mask] ^= truth[mask ^ (1usize << bit)];
            }
        }
    }

    truth
        .iter()
        .enumerate()
        .filter_map(|(mask, &coeff)| (coeff != 0).then_some(mask as u16))
        .collect()
}

fn anf_masks_from_truth(input_bits: usize, mut truth: Vec<u8>) -> Vec<u64> {
    assert!(input_bits < u64::BITS as usize);
    assert_eq!(truth.len(), 1usize << input_bits);
    for bit in 0..input_bits {
        for mask in 0..truth.len() {
            if (mask & (1usize << bit)) != 0 {
                truth[mask] ^= truth[mask ^ (1usize << bit)];
            }
        }
    }

    truth
        .iter()
        .enumerate()
        .filter_map(|(mask, &coeff)| (coeff != 0).then_some(mask as u64))
        .collect()
}

fn emit_monomial_toggle(
    b: &mut B,
    controls: &[QubitId],
    mask: u16,
    target: QubitId,
    scratch: &[QubitId],
) {
    let selected: Vec<QubitId> = controls
        .iter()
        .enumerate()
        .filter_map(|(idx, &control)| ((mask & (1u16 << idx)) != 0).then_some(control))
        .collect();
    emit_multi_control_x(b, &selected, target, scratch);
}

fn emit_monomial_toggle_u64(
    b: &mut B,
    controls: &[QubitId],
    mask: u64,
    target: QubitId,
    scratch: &[QubitId],
) {
    let selected: Vec<QubitId> = controls
        .iter()
        .enumerate()
        .filter_map(|(idx, &control)| ((mask & (1u64 << idx)) != 0).then_some(control))
        .collect();
    emit_multi_control_x(b, &selected, target, scratch);
}

fn emit_multi_control_x(b: &mut B, controls: &[QubitId], target: QubitId, scratch: &[QubitId]) {
    match controls.len() {
        0 => b.x(target),
        1 => b.cx(controls[0], target),
        2 => b.ccx(controls[0], controls[1], target),
        n => {
            assert!(
                scratch.len() >= n - 2,
                "not enough scratch for {n}-controlled X"
            );
            b.ccx(controls[0], controls[1], scratch[0]);
            for idx in 2..n - 1 {
                b.ccx(scratch[idx - 2], controls[idx], scratch[idx - 1]);
            }
            b.ccx(scratch[n - 3], controls[n - 1], target);
            for idx in (2..n - 1).rev() {
                b.ccx(scratch[idx - 2], controls[idx], scratch[idx - 1]);
            }
            b.ccx(controls[0], controls[1], scratch[0]);
        }
    }
}

fn inclusive_i128_len(min: i128, max: i128) -> usize {
    assert!(min <= max);
    let len = max
        .checked_sub(min)
        .and_then(|delta| delta.checked_add(1))
        .expect("dynamic zeta range length overflow");
    usize::try_from(len).expect("dynamic zeta range is too large for this target")
}

fn bits_for_values(values: usize) -> usize {
    assert!(values > 0, "need at least one encodable value");
    usize::BITS as usize - (values - 1).leading_zeros() as usize
}

fn low_usize_mask(bits: usize) -> usize {
    if bits == 0 {
        0
    } else {
        assert!(bits < usize::BITS as usize);
        (1usize << bits) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::{analyze_ops, OperationType};
    use crate::sim::Simulator;
    use sha3::{
        digest::{ExtendableOutput, Update},
        Shake128,
    };

    #[derive(Clone, Debug)]
    struct SelectorTestWires {
        f_low: Vec<QubitId>,
        g_low: Vec<QubitId>,
        branch_word: Vec<QubitId>,
        old_g0_word: Vec<QubitId>,
        scratch: Vec<QubitId>,
    }

    #[derive(Clone, Debug)]
    struct DynamicZetaTestWires {
        zeta_start: Vec<QubitId>,
        f_low: Vec<QubitId>,
        g_low: Vec<QubitId>,
        branch_word: Vec<QubitId>,
        old_g0_word: Vec<QubitId>,
        end_zeta: Vec<QubitId>,
        scratch: Vec<QubitId>,
    }

    fn build_selector_test_circuit(zeta_start: i128, roundtrip: bool) -> (B, SelectorTestWires) {
        let mut b = B::new();
        let f_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&f_low);
        let g_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&g_low);
        let branch_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&branch_word);
        let old_g0_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&old_g0_word);
        let scratch = b.alloc_qubits(round218_b5_low_state_selector_scratch_qubits(zeta_start));
        if !scratch.is_empty() {
            b.declare_qubit_register(&scratch);
        }

        b.set_phase("round218_b5_selector_test_compute");
        emit_round218_b5_low_state_selector_with_scratch(
            &mut b,
            &f_low,
            &g_low,
            zeta_start,
            &branch_word,
            &old_g0_word,
            &scratch,
        );
        if roundtrip {
            b.set_phase("round218_b5_selector_test_uncompute");
            emit_round218_b5_low_state_selector_uncompute_with_scratch(
                &mut b,
                &f_low,
                &g_low,
                zeta_start,
                &branch_word,
                &old_g0_word,
                &scratch,
            );
        }

        (
            b,
            SelectorTestWires {
                f_low,
                g_low,
                branch_word,
                old_g0_word,
                scratch,
            },
        )
    }

    fn build_dynamic_zeta_test_circuit(
        spec: Round218B5DynamicZetaTransducerSpec,
        roundtrip: bool,
    ) -> (B, DynamicZetaTestWires) {
        let mut b = B::new();
        let zeta_start = b.alloc_qubits(spec.start_zeta_bits());
        if !zeta_start.is_empty() {
            b.declare_qubit_register(&zeta_start);
        }
        let f_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&f_low);
        let g_low = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&g_low);
        let branch_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&branch_word);
        let old_g0_word = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&old_g0_word);
        let end_zeta = b.alloc_qubits(spec.end_zeta_bits());
        if !end_zeta.is_empty() {
            b.declare_qubit_register(&end_zeta);
        }
        let scratch = b.alloc_qubits(round218_b5_dynamic_zeta_transducer_scratch_qubits(spec));
        if !scratch.is_empty() {
            b.declare_qubit_register(&scratch);
        }

        b.set_phase("round218_b5_dynamic_zeta_test_compute");
        emit_round218_b5_dynamic_zeta_transducer_with_scratch(
            &mut b,
            spec,
            &zeta_start,
            &f_low,
            &g_low,
            &branch_word,
            &old_g0_word,
            &end_zeta,
            &scratch,
        );
        if roundtrip {
            b.set_phase("round218_b5_dynamic_zeta_test_uncompute");
            emit_round218_b5_dynamic_zeta_transducer_uncompute_with_scratch(
                &mut b,
                spec,
                &zeta_start,
                &f_low,
                &g_low,
                &branch_word,
                &old_g0_word,
                &end_zeta,
                &scratch,
            );
        }

        (
            b,
            DynamicZetaTestWires {
                zeta_start,
                f_low,
                g_low,
                branch_word,
                old_g0_word,
                end_zeta,
                scratch,
            },
        )
    }

    fn exhaustive_cases() -> Vec<(u8, u8)> {
        let mut cases = Vec::new();
        for f_low in (1u8..32).step_by(2) {
            for g_low in 0u8..32 {
                cases.push((f_low, g_low));
            }
        }
        cases
    }

    fn zeta_cases() -> &'static [i128] {
        &[-9, -5, -2, -1, 0, 1, 4, 9]
    }

    fn dynamic_zeta_test_spec() -> Round218B5DynamicZetaTransducerSpec {
        Round218B5DynamicZetaTransducerSpec::new(-2, 2)
    }

    fn dynamic_zeta_exhaustive_cases(
        spec: Round218B5DynamicZetaTransducerSpec,
    ) -> Vec<(i128, u8, u8)> {
        let mut cases = Vec::new();
        for zeta_start in spec.zeta_min..=spec.zeta_max {
            for (f_low, g_low) in exhaustive_cases() {
                cases.push((zeta_start, f_low, g_low));
            }
        }
        cases
    }

    fn set_word<R: sha3::digest::XofReader>(
        sim: &mut Simulator<'_, R>,
        qs: &[QubitId],
        value: u8,
        shot: usize,
    ) {
        for (idx, &q) in qs.iter().enumerate() {
            if ((value >> idx) & 1) != 0 {
                *sim.qubit_mut(q) |= 1u64 << shot;
            } else {
                *sim.qubit_mut(q) &= !(1u64 << shot);
            }
        }
    }

    fn get_word<R: sha3::digest::XofReader>(
        sim: &Simulator<'_, R>,
        qs: &[QubitId],
        shot: usize,
    ) -> u8 {
        let mut out = 0u8;
        for (idx, &q) in qs.iter().enumerate() {
            out |= (((sim.qubit(q) >> shot) & 1) as u8) << idx;
        }
        out
    }

    fn set_usize_word<R: sha3::digest::XofReader>(
        sim: &mut Simulator<'_, R>,
        qs: &[QubitId],
        value: usize,
        shot: usize,
    ) {
        for (idx, &q) in qs.iter().enumerate() {
            if ((value >> idx) & 1) != 0 {
                *sim.qubit_mut(q) |= 1u64 << shot;
            } else {
                *sim.qubit_mut(q) &= !(1u64 << shot);
            }
        }
    }

    fn get_usize_word<R: sha3::digest::XofReader>(
        sim: &Simulator<'_, R>,
        qs: &[QubitId],
        shot: usize,
    ) -> usize {
        let mut out = 0usize;
        for (idx, &q) in qs.iter().enumerate() {
            out |= (((sim.qubit(q) >> shot) & 1) as usize) << idx;
        }
        out
    }

    fn encode_twos_zeta(zeta: i128, bits: usize) -> alloy_primitives::U256 {
        let modulus = 1i128 << bits;
        alloy_primitives::U256::from(zeta.rem_euclid(modulus) as u64)
    }

    fn expected_rotated_window_endpoint(
        zeta_start: i128,
        f_window: u16,
        g_window: u16,
    ) -> (i128, u16, u16, u8, u8) {
        let parsed = round218_b5_low_window_parser_cell(zeta_start, f_window, g_window);
        let retained = round218_b5_low_window_parser_retained_word(zeta_start, f_window, g_window);
        let row = round218_b5_program::source_window_block_row(
            round218_b5_program::SourceWindowSelector {
                zeta_start,
                f_window,
                g_window,
                window_bits: ROUND218_B5_LOW_WINDOW_BITS as u8,
            },
        );
        let mut retained_in_place = 0u16;
        for step in 0..ROUND218_B5_LOW_STATE_BITS {
            let retained_bit = u16::from((retained >> step) & 1);
            retained_in_place |= retained_bit << (ROUND218_B5_LOW_WINDOW_BITS - 1 - step);
        }
        (
            row.end_zeta,
            u16::from(parsed.next_f_low) | retained_in_place,
            u16::from(parsed.next_g_low),
            parsed.branch_word,
            parsed.old_g0_word,
        )
    }

    fn zero_qubit_register<R: sha3::digest::XofReader>(
        sim: &mut Simulator<'_, R>,
        reg: &[crate::circuit::QubitOrBit],
    ) {
        for item in reg {
            if let crate::circuit::QubitOrBit::Qubit(q) = *item {
                *sim.qubit_mut(q) = 0;
            }
        }
    }

    fn expected_row(zeta_start: i128, f_low: u8, g_low: u8) -> round218_b5_program::BlockRow {
        round218_b5_program::block_row(
            0,
            round218_b5_program::BlockSelector {
                zeta_start,
                f_low,
                g_low,
                width: ROUND218_B5_LOW_STATE_BITS as u8,
            },
        )
    }

    fn exact_low_window_reference(
        zeta_start: i128,
        f_window: u16,
        g_window: u16,
    ) -> Round218B5LowWindowParserOutput {
        let mut zeta = zeta_start;
        let mut f = i128::from(f_window);
        let mut g = i128::from(g_window);
        let mut branch_word = 0u8;
        let mut old_g0_word = 0u8;

        for step in 0..ROUND218_B5_LOW_STATE_BITS {
            assert_eq!(f.rem_euclid(2), 1);
            let old_g0 = g.rem_euclid(2) as u8;
            old_g0_word |= old_g0 << step;
            if zeta < 0 && old_g0 != 0 {
                branch_word |= 1u8 << step;
                let next_f = g;
                let next_g = (g - f) / 2;
                zeta = -zeta - 2;
                f = next_f;
                g = next_g;
            } else {
                let next_g = (g + i128::from(old_g0) * f) / 2;
                zeta -= 1;
                g = next_g;
            }
        }

        Round218B5LowWindowParserOutput {
            branch_word,
            old_g0_word,
            next_f_low: f.rem_euclid(1i128 << ROUND218_B5_LOW_STATE_BITS) as u8,
            next_g_low: g.rem_euclid(1i128 << ROUND218_B5_LOW_STATE_BITS) as u8,
        }
    }

    fn assert_scratch_zero<R: sha3::digest::XofReader>(
        sim: &Simulator<'_, R>,
        scratch: &[QubitId],
    ) {
        for &q in scratch {
            assert_eq!(sim.qubit(q), 0, "selector scratch q{} is not clean", q.0);
        }
    }

    #[test]
    fn low_window_parser_cell_matches_exact_half_delta_for_all_odd_windows() {
        let window_limit = 1u16 << ROUND218_B5_LOW_WINDOW_BITS;
        for &zeta_start in zeta_cases() {
            for f_window in (1u16..window_limit).step_by(2) {
                for g_window in 0u16..window_limit {
                    let parsed = round218_b5_low_window_parser_cell(zeta_start, f_window, g_window);
                    let reference = exact_low_window_reference(zeta_start, f_window, g_window);
                    assert_eq!(
                        parsed, reference,
                        "zeta={zeta_start} f_window={f_window} g_window={g_window}"
                    );

                    let row = expected_row(
                        zeta_start,
                        (f_window & ((1u16 << ROUND218_B5_LOW_STATE_BITS) - 1)) as u8,
                        (g_window & ((1u16 << ROUND218_B5_LOW_STATE_BITS) - 1)) as u8,
                    );
                    assert_eq!(parsed.branch_word, row.branch_word);
                    assert_eq!(parsed.old_g0_word, row.old_g0_word);
                }
            }
        }
    }

    #[test]
    fn low_window_parser_output_has_exact_32_way_collision_classes() {
        assert_eq!(ROUND218_B5_LOW_WINDOW_PARSER_MIN_RETAINED_BITS, 5);
        let window_limit = 1u16 << ROUND218_B5_LOW_WINDOW_BITS;
        let domain_size = 1usize << (ROUND218_B5_LOW_WINDOW_PARSER_INPUT_BITS - 1);
        let expected_image_size = domain_size >> ROUND218_B5_LOW_WINDOW_PARSER_MIN_RETAINED_BITS;
        let expected_preimages = 1u16 << ROUND218_B5_LOW_WINDOW_PARSER_MIN_RETAINED_BITS;

        for &zeta_start in zeta_cases() {
            let mut counts = vec![0u16; 1usize << ROUND218_B5_LOW_WINDOW_PARSER_OUTPUT_BITS];
            for f_window in (1u16..window_limit).step_by(2) {
                for g_window in 0u16..window_limit {
                    let parsed = round218_b5_low_window_parser_cell(zeta_start, f_window, g_window);
                    counts[parsed.packed() as usize] += 1;
                }
            }

            let mut image_size = 0usize;
            let mut min_preimages = u16::MAX;
            let mut max_preimages = 0u16;
            for count in counts.into_iter().filter(|&count| count != 0) {
                image_size += 1;
                min_preimages = min_preimages.min(count);
                max_preimages = max_preimages.max(count);
            }

            assert_eq!(image_size, expected_image_size, "zeta={zeta_start}");
            assert_eq!(min_preimages, expected_preimages, "zeta={zeta_start}");
            assert_eq!(max_preimages, expected_preimages, "zeta={zeta_start}");
        }
    }

    #[test]
    fn retained_word_is_sufficient_for_low_window_parser_reversal() {
        let window_limit = 1u16 << ROUND218_B5_LOW_WINDOW_BITS;
        for &zeta_start in zeta_cases() {
            for f_window in (1u16..window_limit).step_by(2) {
                for g_window in 0u16..window_limit {
                    let parsed = round218_b5_low_window_parser_cell(zeta_start, f_window, g_window);
                    let retained =
                        round218_b5_low_window_parser_retained_word(zeta_start, f_window, g_window);
                    assert!(retained < (1u8 << ROUND218_B5_LOW_WINDOW_PARSER_MIN_RETAINED_BITS));
                    assert_eq!(
                        round218_b5_low_window_parser_reconstruct_input(
                            zeta_start, parsed, retained,
                        ),
                        (f_window, g_window),
                        "zeta={zeta_start} f_window={f_window} g_window={g_window}"
                    );
                }
            }
        }
    }

    #[test]
    fn low_window_parser_gate_matches_reference_and_cleans() {
        let zeta_start = -1;
        let ops = build_round218_b5_low_window_parser_component(zeta_start);
        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(ops.iter().copied());
        let toffoli = ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 6);
        assert_eq!(regs[0].len(), ROUND218_B5_LOW_WINDOW_BITS);
        assert_eq!(regs[1].len(), ROUND218_B5_LOW_WINDOW_BITS);
        for reg in &regs[2..] {
            assert_eq!(reg.len(), ROUND218_B5_LOW_STATE_BITS);
        }

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-low-window-parser-gate-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            (1u16, 0u16),
            (31u16, 1u16),
            (31u16 | 512, 1u16),
            (31u16, 1u16 | 512),
            (341u16, 682u16),
            (513u16, 511u16),
            (777u16, 1001u16),
            (1023u16, 1023u16),
        ];
        for (shot, &(f_window, g_window)) in cases.iter().enumerate() {
            sim.set_register(
                &regs[0],
                alloy_primitives::U256::from(f_window as u64),
                shot,
            );
            sim.set_register(
                &regs[1],
                alloy_primitives::U256::from(g_window as u64),
                shot,
            );
        }

        sim.apply(&ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "low-window parser left phase garbage"
        );
        for (shot, &(f_window, g_window)) in cases.iter().enumerate() {
            let parsed = round218_b5_low_window_parser_cell(zeta_start, f_window, g_window);
            assert_eq!(
                sim.get_register(&regs[0], shot),
                alloy_primitives::U256::from(f_window as u64),
                "f_window shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[1], shot),
                alloy_primitives::U256::from(g_window as u64),
                "g_window shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[2], shot),
                alloy_primitives::U256::from(parsed.branch_word as u64),
                "branch_word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[3], shot),
                alloy_primitives::U256::from(parsed.old_g0_word as u64),
                "old_g0_word shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[4], shot),
                alloy_primitives::U256::from(parsed.next_f_low as u64),
                "next_f_low shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[5], shot),
                alloy_primitives::U256::from(parsed.next_g_low as u64),
                "next_g_low shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for (idx, value) in sim.qubits.iter().copied().enumerate() {
            assert_eq!(value, 0, "low-window parser scratch q{idx} is not clean");
        }
        println!("METRIC round218_b5_low_window_parser_toffoli={toffoli}");
        println!("METRIC round218_b5_low_window_parser_qubits={num_qubits}");
    }

    #[test]
    fn twos_zeta_control_word_parser_matches_reference_and_cleans_work() {
        let zeta_bits = 5usize;
        let window_bits = ROUND218_B5_LOW_WINDOW_BITS;
        let ops = build_round218_b5_twos_zeta_control_word_parser_component(zeta_bits, window_bits);
        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(ops.iter().copied());
        let toffoli = ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 5);
        assert_eq!(regs[0].len(), zeta_bits);
        assert_eq!(regs[1].len(), window_bits);
        assert_eq!(regs[2].len(), window_bits);
        assert_eq!(regs[3].len(), ROUND218_B5_LOW_STATE_BITS);
        assert_eq!(regs[4].len(), ROUND218_B5_LOW_STATE_BITS);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-twos-zeta-control-word-parser-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            (-4i128, 31u16, 1u16),
            (-2i128, 31u16 | 512, 1u16),
            (-1i128, 31u16, 1u16 | 512),
            (0i128, 341u16, 682u16),
            (2i128, 513u16, 511u16),
            (3i128, 777u16, 1001u16),
            (6i128, 1023u16, 1023u16),
            (-7i128, 1u16, 0u16),
        ];
        for (shot, &(zeta_start, f_window, g_window)) in cases.iter().enumerate() {
            sim.set_register(&regs[0], encode_twos_zeta(zeta_start, zeta_bits), shot);
            sim.set_register(
                &regs[1],
                alloy_primitives::U256::from(f_window as u64),
                shot,
            );
            sim.set_register(
                &regs[2],
                alloy_primitives::U256::from(g_window as u64),
                shot,
            );
        }

        sim.apply(&ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "control-word parser left phase garbage"
        );
        for (shot, &(zeta_start, f_window, g_window)) in cases.iter().enumerate() {
            let parsed = round218_b5_low_window_parser_cell(zeta_start, f_window, g_window);
            assert_eq!(
                sim.get_register(&regs[0], shot),
                encode_twos_zeta(zeta_start, zeta_bits),
                "zeta shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[1], shot),
                alloy_primitives::U256::from(f_window as u64),
                "f_window shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[2], shot),
                alloy_primitives::U256::from(g_window as u64),
                "g_window shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[3], shot),
                alloy_primitives::U256::from(parsed.branch_word as u64),
                "branch shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[4], shot),
                alloy_primitives::U256::from(parsed.old_g0_word as u64),
                "old_g0 shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for q in 0..num_qubits {
            assert_eq!(
                sim.qubit(QubitId(q)),
                0,
                "control-word parser scratch q{q} not clean"
            );
        }

        let mut roundtrip = B::new();
        let rt_zeta = roundtrip.alloc_qubits(zeta_bits);
        roundtrip.declare_qubit_register(&rt_zeta);
        let rt_f = roundtrip.alloc_qubits(window_bits);
        roundtrip.declare_qubit_register(&rt_f);
        let rt_g = roundtrip.alloc_qubits(window_bits);
        roundtrip.declare_qubit_register(&rt_g);
        let rt_branch = roundtrip.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        roundtrip.declare_qubit_register(&rt_branch);
        let rt_old_g0 = roundtrip.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        roundtrip.declare_qubit_register(&rt_old_g0);
        emit_round218_b5_twos_zeta_control_word_parser(
            &mut roundtrip,
            &rt_zeta,
            &rt_f,
            &rt_g,
            &rt_branch,
            &rt_old_g0,
        );
        emit_round218_b5_twos_zeta_control_word_parser_uncompute(
            &mut roundtrip,
            &rt_zeta,
            &rt_f,
            &rt_g,
            &rt_branch,
            &rt_old_g0,
        );
        let rt_ops = roundtrip.ops;
        let (rt_num_qubits, rt_num_bits, _rt_num_registers, rt_regs) =
            analyze_ops(rt_ops.iter().copied());
        let mut seed = Shake128::default();
        seed.update(b"round218-b5-twos-zeta-control-word-parser-roundtrip-v1");
        let mut xof = seed.finalize_xof();
        let mut rt_sim = Simulator::new(rt_num_qubits as usize, rt_num_bits as usize, &mut xof);
        for (shot, &(zeta_start, f_window, g_window)) in cases.iter().enumerate() {
            rt_sim.set_register(&rt_regs[0], encode_twos_zeta(zeta_start, zeta_bits), shot);
            rt_sim.set_register(
                &rt_regs[1],
                alloy_primitives::U256::from(f_window as u64),
                shot,
            );
            rt_sim.set_register(
                &rt_regs[2],
                alloy_primitives::U256::from(g_window as u64),
                shot,
            );
        }
        rt_sim.apply(&rt_ops);
        assert_eq!(
            rt_sim.global_phase(),
            0,
            "control-word parser roundtrip left phase garbage"
        );
        for (shot, &(zeta_start, f_window, g_window)) in cases.iter().enumerate() {
            assert_eq!(
                rt_sim.get_register(&rt_regs[0], shot),
                encode_twos_zeta(zeta_start, zeta_bits),
                "roundtrip zeta shot {shot}"
            );
            assert_eq!(
                rt_sim.get_register(&rt_regs[1], shot),
                alloy_primitives::U256::from(f_window as u64),
                "roundtrip f shot {shot}"
            );
            assert_eq!(
                rt_sim.get_register(&rt_regs[2], shot),
                alloy_primitives::U256::from(g_window as u64),
                "roundtrip g shot {shot}"
            );
            assert_eq!(
                rt_sim.get_register(&rt_regs[3], shot),
                alloy_primitives::U256::ZERO,
                "roundtrip branch shot {shot}"
            );
            assert_eq!(
                rt_sim.get_register(&rt_regs[4], shot),
                alloy_primitives::U256::ZERO,
                "roundtrip old_g0 shot {shot}"
            );
        }
        for reg in &rt_regs {
            zero_qubit_register(&mut rt_sim, reg);
        }
        for q in 0..rt_num_qubits {
            assert_eq!(
                rt_sim.qubit(QubitId(q)),
                0,
                "control-word parser roundtrip scratch q{q} not clean"
            );
        }
        println!("METRIC round218_b5_twos_zeta_control_word_parser_toffoli={toffoli}");
        println!("METRIC round218_b5_twos_zeta_control_word_parser_qubits={num_qubits}");
    }

    #[test]
    fn source_stream_forward_block_from_bits_matches_window_reference() {
        let zeta_bits = 5usize;
        let window_bits = ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f);
        let g = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g);
        let branch = b.alloc_bits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_bit_register(&branch);
        let old_g0 = b.alloc_bits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_bit_register(&old_g0);

        emit_round218_b5_source_stream_forward_block_from_bits(
            &mut b, &zeta, &f, &g, 0, &branch, &old_g0,
        );
        let ops = b.ops;
        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(ops.iter().copied());
        let toffoli = ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 5);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-source-stream-forward-block-from-bits-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            (-4i128, 31u16, 1u16),
            (-2i128, 31u16 | 512, 1u16),
            (-1i128, 31u16, 1u16 | 512),
            (0i128, 341u16, 682u16),
            (2i128, 513u16, 511u16),
            (3i128, 777u16, 1001u16),
            (6i128, 1023u16, 1023u16),
            (-7i128, 1u16, 0u16),
        ];
        for (shot, &(zeta_start, f_window, g_window)) in cases.iter().enumerate() {
            let (_, _, _, branch_word, old_g0_word) =
                expected_rotated_window_endpoint(zeta_start, f_window, g_window);
            sim.set_register(&regs[0], encode_twos_zeta(zeta_start, zeta_bits), shot);
            sim.set_register(
                &regs[1],
                alloy_primitives::U256::from(f_window as u64),
                shot,
            );
            sim.set_register(
                &regs[2],
                alloy_primitives::U256::from(g_window as u64),
                shot,
            );
            sim.set_register(
                &regs[3],
                alloy_primitives::U256::from(branch_word as u64),
                shot,
            );
            sim.set_register(
                &regs[4],
                alloy_primitives::U256::from(old_g0_word as u64),
                shot,
            );
        }

        sim.apply(&ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "measured-control source update left phase garbage"
        );
        for (shot, &(zeta_start, f_window, g_window)) in cases.iter().enumerate() {
            let (end_zeta, full_f, full_g, branch_word, old_g0_word) =
                expected_rotated_window_endpoint(zeta_start, f_window, g_window);
            assert_eq!(
                sim.get_register(&regs[0], shot),
                encode_twos_zeta(end_zeta, zeta_bits),
                "end zeta shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[1], shot),
                alloy_primitives::U256::from(full_f as u64),
                "full f shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[2], shot),
                alloy_primitives::U256::from(full_g as u64),
                "full g shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[3], shot),
                alloy_primitives::U256::from(branch_word as u64),
                "branch bits shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[4], shot),
                alloy_primitives::U256::from(old_g0_word as u64),
                "old_g0 bits shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for q in 0..num_qubits {
            assert_eq!(
                sim.qubit(QubitId(q)),
                0,
                "measured-control source update scratch q{q} not clean"
            );
        }
        println!("METRIC round218_b5_source_stream_forward_from_bits_toffoli={toffoli}");
        println!("METRIC round218_b5_source_stream_forward_from_bits_qubits={num_qubits}");
    }

    #[test]
    fn source_stream_forward_block_component_matches_window_reference() {
        let zeta_bits = 5usize;
        let window_bits = ROUND218_B5_LOW_WINDOW_BITS;
        let ops = build_round218_b5_source_stream_forward_block_component(zeta_bits, window_bits);
        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(ops.iter().copied());
        let toffoli = ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX | OperationType::CCZ))
            .count();
        assert_eq!(regs.len(), 5);
        assert_eq!(regs[0].len(), zeta_bits);
        assert_eq!(regs[1].len(), window_bits);
        assert_eq!(regs[2].len(), window_bits);
        assert_eq!(regs[3].len(), ROUND218_B5_LOW_STATE_BITS);
        assert_eq!(regs[4].len(), ROUND218_B5_LOW_STATE_BITS);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-source-stream-forward-block-component-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        let cases = [
            (-4i128, 31u16, 1u16),
            (-2i128, 31u16 | 512, 1u16),
            (-1i128, 31u16, 1u16 | 512),
            (0i128, 341u16, 682u16),
            (2i128, 513u16, 511u16),
            (3i128, 777u16, 1001u16),
            (6i128, 1023u16, 1023u16),
            (-7i128, 1u16, 0u16),
        ];
        for (shot, &(zeta_start, f_window, g_window)) in cases.iter().enumerate() {
            sim.set_register(&regs[0], encode_twos_zeta(zeta_start, zeta_bits), shot);
            sim.set_register(
                &regs[1],
                alloy_primitives::U256::from(f_window as u64),
                shot,
            );
            sim.set_register(
                &regs[2],
                alloy_primitives::U256::from(g_window as u64),
                shot,
            );
        }

        sim.apply(&ops);
        assert_eq!(
            sim.global_phase(),
            0,
            "source forward block left phase garbage"
        );
        for (shot, &(zeta_start, f_window, g_window)) in cases.iter().enumerate() {
            let (end_zeta, full_f, full_g, branch_word, old_g0_word) =
                expected_rotated_window_endpoint(zeta_start, f_window, g_window);
            assert_eq!(
                sim.get_register(&regs[0], shot),
                encode_twos_zeta(end_zeta, zeta_bits),
                "end zeta shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[1], shot),
                alloy_primitives::U256::from(full_f as u64),
                "full f shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[2], shot),
                alloy_primitives::U256::from(full_g as u64),
                "full g shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[3], shot),
                alloy_primitives::U256::from(branch_word as u64),
                "branch shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[4], shot),
                alloy_primitives::U256::from(old_g0_word as u64),
                "old_g0 shot {shot}"
            );
        }

        for reg in &regs {
            zero_qubit_register(&mut sim, reg);
        }
        for q in 0..num_qubits {
            assert_eq!(
                sim.qubit(QubitId(q)),
                0,
                "source forward block scratch q{q} not clean"
            );
        }
        assert!(toffoli <= 655);
        println!("METRIC round218_b5_source_stream_forward_block_toffoli={toffoli}");
        println!("METRIC round218_b5_source_stream_forward_block_qubits={num_qubits}");
    }

    #[test]
    fn hmr_masks_are_not_selector_controls_for_source_advance() {
        let zeta_bits = 5usize;
        let window_bits = ROUND218_B5_LOW_WINDOW_BITS;
        let mut b = B::new();
        let zeta = b.alloc_qubits(zeta_bits);
        b.declare_qubit_register(&zeta);
        let f = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&f);
        let g = b.alloc_qubits(window_bits);
        b.declare_qubit_register(&g);
        let branch_q = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&branch_q);
        let old_g0_q = b.alloc_qubits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_qubit_register(&old_g0_q);
        let branch_bits = b.alloc_bits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_bit_register(&branch_bits);
        let old_g0_bits = b.alloc_bits(ROUND218_B5_LOW_STATE_BITS);
        b.declare_bit_register(&old_g0_bits);

        emit_round218_b5_twos_zeta_control_word_parser(&mut b, &zeta, &f, &g, &branch_q, &old_g0_q);
        for i in 0..ROUND218_B5_LOW_STATE_BITS {
            b.hmr(branch_q[i], branch_bits[i]);
            b.hmr(old_g0_q[i], old_g0_bits[i]);
        }
        emit_round218_b5_source_stream_forward_block_from_bits(
            &mut b,
            &zeta,
            &f,
            &g,
            0,
            &branch_bits,
            &old_g0_bits,
        );

        let ops = b.ops;
        let (num_qubits, num_bits, _num_registers, regs) = analyze_ops(ops.iter().copied());
        assert_eq!(regs.len(), 7);

        let zeta_start = -4i128;
        let f_window = 31u16;
        let g_window = 1u16;
        let (end_zeta, full_f, full_g, branch_word, old_g0_word) =
            expected_rotated_window_endpoint(zeta_start, f_window, g_window);

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-hmr-masks-are-not-selector-controls-v1");
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);
        for shot in 0..64 {
            sim.set_register(&regs[0], encode_twos_zeta(zeta_start, zeta_bits), shot);
            sim.set_register(
                &regs[1],
                alloy_primitives::U256::from(f_window as u64),
                shot,
            );
            sim.set_register(
                &regs[2],
                alloy_primitives::U256::from(g_window as u64),
                shot,
            );
        }

        sim.apply(&ops);

        let mut endpoint_matches = 0usize;
        let mut measured_control_matches = 0usize;
        for shot in 0..64 {
            let got_endpoint = (
                sim.get_register(&regs[0], shot),
                sim.get_register(&regs[1], shot),
                sim.get_register(&regs[2], shot),
            );
            let want_endpoint = (
                encode_twos_zeta(end_zeta, zeta_bits),
                alloy_primitives::U256::from(full_f as u64),
                alloy_primitives::U256::from(full_g as u64),
            );
            if got_endpoint == want_endpoint {
                endpoint_matches += 1;
            }

            assert_eq!(
                sim.get_register(&regs[3], shot),
                alloy_primitives::U256::ZERO,
                "HMR did not reset branch control qubits on shot {shot}"
            );
            assert_eq!(
                sim.get_register(&regs[4], shot),
                alloy_primitives::U256::ZERO,
                "HMR did not reset old_g0 control qubits on shot {shot}"
            );

            let measured_controls = (
                sim.get_register(&regs[5], shot),
                sim.get_register(&regs[6], shot),
            );
            let true_controls = (
                alloy_primitives::U256::from(branch_word as u64),
                alloy_primitives::U256::from(old_g0_word as u64),
            );
            if measured_controls == true_controls {
                measured_control_matches += 1;
            }
        }

        assert!(
            measured_control_matches < 64,
            "HMR masks unexpectedly reproduced true selector controls on every shot"
        );
        assert!(
            endpoint_matches < 64,
            "source advance driven by HMR masks should not equal the selector-controlled endpoint"
        );
        println!("METRIC round218_b5_hmr_mask_control_matches={measured_control_matches}");
        println!("METRIC round218_b5_hmr_mask_endpoint_matches={endpoint_matches}");
    }

    #[test]
    fn dynamic_zeta_spec_has_exact_small_end_range_and_codes() {
        let spec = dynamic_zeta_test_spec();
        let mut end_values = Vec::new();
        for (zeta_start, f_low, g_low) in dynamic_zeta_exhaustive_cases(spec) {
            end_values.push(expected_row(zeta_start, f_low, g_low).end_zeta);
        }
        end_values.sort_unstable();
        let min = *end_values.first().unwrap();
        let max = *end_values.last().unwrap();
        assert_eq!((spec.end_zeta_min, spec.end_zeta_max), (min, max));
        let padded = Round218B5DynamicZetaTransducerSpec::with_end_range(
            spec.zeta_min,
            spec.zeta_max,
            min - 1,
            max + 1,
        );
        assert_eq!(
            (padded.end_zeta_min, padded.end_zeta_max),
            (min - 1, max + 1)
        );

        for zeta_start in spec.zeta_min..=spec.zeta_max {
            let code = spec.encode_start_zeta(zeta_start);
            assert_eq!(spec.decode_start_code(code), Some(zeta_start));
        }
        for code in spec.start_zeta_values()..(1usize << spec.start_zeta_bits()) {
            assert_eq!(spec.decode_start_code(code), None);
        }
        for end_zeta in spec.end_zeta_min..=spec.end_zeta_max {
            assert_eq!(
                spec.end_zeta_min + spec.encode_end_zeta(end_zeta) as i128,
                end_zeta
            );
        }
    }

    #[test]
    fn low_state_selector_gate_matches_block_row_for_all_low_states() {
        let cases = exhaustive_cases();
        for &zeta_start in zeta_cases() {
            let (b, wires) = build_selector_test_circuit(zeta_start, false);
            assert!(b.ops.iter().all(|op| matches!(
                op.kind,
                OperationType::Register
                    | OperationType::AppendToRegister
                    | OperationType::X
                    | OperationType::CX
                    | OperationType::CCX
            )));
            let (num_qubits, num_bits, _num_registers, _regs) = analyze_ops(b.ops.iter().copied());
            let toffoli = b
                .ops
                .iter()
                .filter(|op| matches!(op.kind, OperationType::CCX))
                .count();

            let mut seed = Shake128::default();
            seed.update(b"round218-b5-low-state-selector-gate-v1");
            seed.update(&zeta_start.to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);

            for chunk in cases.chunks(64) {
                sim.clear_for_shot();
                for (shot, &(f_low, g_low)) in chunk.iter().enumerate() {
                    set_word(&mut sim, &wires.f_low, f_low, shot);
                    set_word(&mut sim, &wires.g_low, g_low, shot);
                }

                sim.apply(&b.ops);
                assert_eq!(sim.global_phase(), 0, "selector left phase garbage");
                assert_scratch_zero(&sim, &wires.scratch);

                for (shot, &(f_low, g_low)) in chunk.iter().enumerate() {
                    let row = expected_row(zeta_start, f_low, g_low);
                    assert_eq!(get_word(&sim, &wires.f_low, shot), f_low, "f shot {shot}");
                    assert_eq!(get_word(&sim, &wires.g_low, shot), g_low, "g shot {shot}");
                    assert_eq!(
                        get_word(&sim, &wires.branch_word, shot),
                        row.branch_word,
                        "branch zeta={zeta_start} f={f_low} g={g_low}"
                    );
                    assert_eq!(
                        get_word(&sim, &wires.old_g0_word, shot),
                        row.old_g0_word,
                        "old_g0 zeta={zeta_start} f={f_low} g={g_low}"
                    );
                }
            }

            println!("METRIC round218_b5_selector_zeta_{zeta_start}_toffoli={toffoli}");
            println!("METRIC round218_b5_selector_zeta_{zeta_start}_qubits={num_qubits}");
            println!(
                "METRIC round218_b5_selector_zeta_{zeta_start}_terms={}",
                round218_b5_low_state_selector_term_counts(zeta_start)
                    .iter()
                    .sum::<usize>()
            );
        }
    }

    #[test]
    fn dynamic_zeta_transducer_gate_matches_block_row_for_all_small_states() {
        let spec = dynamic_zeta_test_spec();
        let cases = dynamic_zeta_exhaustive_cases(spec);
        let (b, wires) = build_dynamic_zeta_test_circuit(spec, false);
        assert!(b.ops.iter().all(|op| matches!(
            op.kind,
            OperationType::Register
                | OperationType::AppendToRegister
                | OperationType::X
                | OperationType::CX
                | OperationType::CCX
        )));
        let (num_qubits, num_bits, _num_registers, _regs) = analyze_ops(b.ops.iter().copied());
        let toffoli = b
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OperationType::CCX))
            .count();

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-dynamic-zeta-transducer-gate-v1");
        seed.update(&spec.zeta_min.to_le_bytes());
        seed.update(&spec.zeta_max.to_le_bytes());
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);

        for chunk in cases.chunks(64) {
            sim.clear_for_shot();
            for (shot, &(zeta_start, f_low, g_low)) in chunk.iter().enumerate() {
                set_usize_word(
                    &mut sim,
                    &wires.zeta_start,
                    spec.encode_start_zeta(zeta_start),
                    shot,
                );
                set_word(&mut sim, &wires.f_low, f_low, shot);
                set_word(&mut sim, &wires.g_low, g_low, shot);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "dynamic transducer left phase garbage"
            );
            assert_scratch_zero(&sim, &wires.scratch);

            for (shot, &(zeta_start, f_low, g_low)) in chunk.iter().enumerate() {
                let row = expected_row(zeta_start, f_low, g_low);
                assert_eq!(
                    get_usize_word(&sim, &wires.zeta_start, shot),
                    spec.encode_start_zeta(zeta_start),
                    "zeta input changed for shot {shot}"
                );
                assert_eq!(get_word(&sim, &wires.f_low, shot), f_low, "f shot {shot}");
                assert_eq!(get_word(&sim, &wires.g_low, shot), g_low, "g shot {shot}");
                assert_eq!(
                    get_word(&sim, &wires.branch_word, shot),
                    row.branch_word,
                    "branch zeta={zeta_start} f={f_low} g={g_low}"
                );
                assert_eq!(
                    get_word(&sim, &wires.old_g0_word, shot),
                    row.old_g0_word,
                    "old_g0 zeta={zeta_start} f={f_low} g={g_low}"
                );
                assert_eq!(
                    get_usize_word(&sim, &wires.end_zeta, shot),
                    spec.encode_end_zeta(row.end_zeta),
                    "end_zeta zeta={zeta_start} f={f_low} g={g_low}"
                );
            }
        }

        if spec.start_zeta_values() < (1usize << spec.start_zeta_bits()) {
            sim.clear_for_shot();
            for (shot, code) in
                (spec.start_zeta_values()..(1usize << spec.start_zeta_bits())).enumerate()
            {
                set_usize_word(&mut sim, &wires.zeta_start, code, shot);
                set_word(&mut sim, &wires.f_low, 31, shot);
                set_word(&mut sim, &wires.g_low, 31, shot);
            }
            sim.apply(&b.ops);
            assert_scratch_zero(&sim, &wires.scratch);
            for (shot, code) in
                (spec.start_zeta_values()..(1usize << spec.start_zeta_bits())).enumerate()
            {
                assert_eq!(get_usize_word(&sim, &wires.zeta_start, shot), code);
                assert_eq!(get_word(&sim, &wires.branch_word, shot), 0);
                assert_eq!(get_word(&sim, &wires.old_g0_word, shot), 0);
                assert_eq!(get_usize_word(&sim, &wires.end_zeta, shot), 0);
            }
        }

        let term_total = round218_b5_dynamic_zeta_transducer_term_counts(spec)
            .iter()
            .sum::<usize>();
        println!("METRIC round218_b5_dynamic_zeta_small_toffoli={toffoli}");
        println!("METRIC round218_b5_dynamic_zeta_small_qubits={num_qubits}");
        println!("METRIC round218_b5_dynamic_zeta_small_terms={term_total}");
        println!(
            "METRIC round218_b5_dynamic_zeta_small_start_bits={}",
            spec.start_zeta_bits()
        );
        println!(
            "METRIC round218_b5_dynamic_zeta_small_end_bits={}",
            spec.end_zeta_bits()
        );
    }

    #[test]
    fn low_state_selector_uncompute_roundtrips_and_cleans() {
        let cases = exhaustive_cases();
        for &zeta_start in zeta_cases() {
            let (b, wires) = build_selector_test_circuit(zeta_start, true);
            let (num_qubits, num_bits, _num_registers, _regs) = analyze_ops(b.ops.iter().copied());

            let mut seed = Shake128::default();
            seed.update(b"round218-b5-low-state-selector-uncompute-v1");
            seed.update(&zeta_start.to_le_bytes());
            let mut xof = seed.finalize_xof();
            let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);

            for chunk in cases.chunks(64) {
                sim.clear_for_shot();
                for (shot, &(f_low, g_low)) in chunk.iter().enumerate() {
                    set_word(&mut sim, &wires.f_low, f_low, shot);
                    set_word(&mut sim, &wires.g_low, g_low, shot);
                }

                sim.apply(&b.ops);
                assert_eq!(
                    sim.global_phase(),
                    0,
                    "selector uncompute left phase garbage"
                );
                assert_scratch_zero(&sim, &wires.scratch);

                for (shot, &(f_low, g_low)) in chunk.iter().enumerate() {
                    assert_eq!(
                        get_word(&sim, &wires.f_low, shot),
                        f_low,
                        "roundtrip f shot {shot}"
                    );
                    assert_eq!(
                        get_word(&sim, &wires.g_low, shot),
                        g_low,
                        "roundtrip g shot {shot}"
                    );
                    assert_eq!(
                        get_word(&sim, &wires.branch_word, shot),
                        0,
                        "branch was not uncomputed for zeta={zeta_start} shot={shot}"
                    );
                    assert_eq!(
                        get_word(&sim, &wires.old_g0_word, shot),
                        0,
                        "old_g0 was not uncomputed for zeta={zeta_start} shot={shot}"
                    );
                }
            }
        }
    }

    #[test]
    fn dynamic_zeta_transducer_uncompute_roundtrips_and_cleans() {
        let spec = dynamic_zeta_test_spec();
        let cases = dynamic_zeta_exhaustive_cases(spec);
        let (b, wires) = build_dynamic_zeta_test_circuit(spec, true);
        let (num_qubits, num_bits, _num_registers, _regs) = analyze_ops(b.ops.iter().copied());

        let mut seed = Shake128::default();
        seed.update(b"round218-b5-dynamic-zeta-transducer-uncompute-v1");
        seed.update(&spec.zeta_min.to_le_bytes());
        seed.update(&spec.zeta_max.to_le_bytes());
        let mut xof = seed.finalize_xof();
        let mut sim = Simulator::new(num_qubits as usize, num_bits as usize, &mut xof);

        for chunk in cases.chunks(64) {
            sim.clear_for_shot();
            for (shot, &(zeta_start, f_low, g_low)) in chunk.iter().enumerate() {
                set_usize_word(
                    &mut sim,
                    &wires.zeta_start,
                    spec.encode_start_zeta(zeta_start),
                    shot,
                );
                set_word(&mut sim, &wires.f_low, f_low, shot);
                set_word(&mut sim, &wires.g_low, g_low, shot);
            }

            sim.apply(&b.ops);
            assert_eq!(
                sim.global_phase(),
                0,
                "dynamic transducer uncompute left phase garbage"
            );
            assert_scratch_zero(&sim, &wires.scratch);

            for (shot, &(zeta_start, f_low, g_low)) in chunk.iter().enumerate() {
                assert_eq!(
                    get_usize_word(&sim, &wires.zeta_start, shot),
                    spec.encode_start_zeta(zeta_start),
                    "roundtrip zeta shot {shot}"
                );
                assert_eq!(
                    get_word(&sim, &wires.f_low, shot),
                    f_low,
                    "roundtrip f shot {shot}"
                );
                assert_eq!(
                    get_word(&sim, &wires.g_low, shot),
                    g_low,
                    "roundtrip g shot {shot}"
                );
                assert_eq!(
                    get_word(&sim, &wires.branch_word, shot),
                    0,
                    "branch was not uncomputed for zeta={zeta_start} shot={shot}"
                );
                assert_eq!(
                    get_word(&sim, &wires.old_g0_word, shot),
                    0,
                    "old_g0 was not uncomputed for zeta={zeta_start} shot={shot}"
                );
                assert_eq!(
                    get_usize_word(&sim, &wires.end_zeta, shot),
                    0,
                    "end_zeta was not uncomputed for zeta={zeta_start} shot={shot}"
                );
            }
        }
    }
}
