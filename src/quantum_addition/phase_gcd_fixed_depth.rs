//! Round185 fixed-depth64 half-GCD PA build target.
//!
//! This module is intentionally a build hook, not a certificate.  It prevents
//! the attractive Round145/Round185 resource row from being silently promoted
//! through the old qtail or standalone-D1 machinery.  The first missing code
//! object is the source-live fixed-depth64 prefix/tail splice: it must generate
//! the half-GCD prefix quotients from the live denominator, consume the quotient
//! witness while `h,n` still exist, and return only the four Google ABI
//! registers.

use super::{B, N, SECP256K1_P};
use crate::circuit::{BitId, Op, QubitId};
use alloy_primitives::U256;

pub const ROUND185_FIXED_DEPTH64_HALFGCD_PA_ENV: &str = "ROUND185_FIXED_DEPTH64_HALFGCD_PA";
pub const ROUND185_FIRST_MISSING_SYMBOL: &str =
    "emit_round185_fixed_depth64_source_live_prefix_tail_splice";

pub(super) fn round185_fixed_depth64_halfgcd_pa_enabled() -> bool {
    std::env::var(ROUND185_FIXED_DEPTH64_HALFGCD_PA_ENV)
        .ok()
        .as_deref()
        == Some("1")
}

pub(super) fn emit_round185_fixed_depth64_halfgcd_pa_or_fail(
    b: &mut B,
    tx: &[QubitId],
    ty: &[QubitId],
    ox: &[BitId],
    oy: &[BitId],
    p: U256,
) -> ! {
    assert_eq!(tx.len(), N, "round185 tx must be 256 qubits");
    assert_eq!(ty.len(), N, "round185 ty must be 256 qubits");
    assert_eq!(ox.len(), N, "round185 ox must be 256 classical bits");
    assert_eq!(oy.len(), N, "round185 oy must be 256 classical bits");
    assert_eq!(p, SECP256K1_P, "round185 is secp256k1-only");

    b.set_phase("round185_fixed_depth64_halfgcd_pa_fail_closed");
    eprintln!("{ROUND185_FIRST_MISSING_SYMBOL}");
    panic!("{ROUND185_FIRST_MISSING_SYMBOL}");
}

pub fn build_round185_halfgcd_fixed_depth64_google_abi_pa() -> Vec<Op> {
    let mut b = B::new();
    let tx = b.alloc_qubits(N);
    b.declare_qubit_register(&tx);
    let ty = b.alloc_qubits(N);
    b.declare_qubit_register(&ty);
    let ox = b.alloc_bits(N);
    b.declare_bit_register(&ox);
    let oy = b.alloc_bits(N);
    b.declare_bit_register(&oy);

    emit_round185_fixed_depth64_halfgcd_pa_or_fail(&mut b, &tx, &ty, &ox, &oy, SECP256K1_P);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round185_reports_first_missing_materialization_symbol() {
        assert_eq!(
            ROUND185_FIRST_MISSING_SYMBOL,
            "emit_round185_fixed_depth64_source_live_prefix_tail_splice"
        );
    }

    #[test]
    #[should_panic(expected = "emit_round185_fixed_depth64_source_live_prefix_tail_splice")]
    fn round185_build_target_fails_closed_before_kmx_emission() {
        let _ = build_round185_halfgcd_fixed_depth64_google_abi_pa();
    }
}
