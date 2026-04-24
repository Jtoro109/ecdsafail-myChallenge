//! Classical numeric validation of the single-Kaliski point-add formula.
//!
//! Goal: verify (at pure U256 / mul_mod / inv_mod level) that the planned
//! single-inversion recipe in `single_inv_plan.md` produces the correct
//! `(Rx, Ry)` matching the reference `WeierstrassEllipticCurve::add`.
//!
//! This module is classical-only and compiled only under `#[cfg(test)]`.
//! It does not affect the quantum circuit.

#![cfg(test)]

use alloy_primitives::U256;

use super::SECP256K1_P;

fn sub_mod(a: U256, b: U256, p: U256) -> U256 {
    if a >= b {
        (a - b) % p
    } else {
        p - ((b - a) % p)
    }
}

/// Single-Kaliski affine point-add formula (classical).
/// Inputs: P = (px, py) live, Q = (qx, qy) classical, P != ±Q, P not zero,
/// Q not zero. Returns (Rx, Ry).
///
/// Same result as the textbook
///     λ  = (Py - Qy) / (Px - Qx)
///     Rx = λ² - Px - Qx
///     Ry = λ*(Qx - Rx) - Qy
/// but staged so only ONE inversion is needed (via Montgomery-style bundling).
pub fn single_inv_add(px: U256, py: U256, qx: U256, qy: U256) -> (U256, U256) {
    let p = SECP256K1_P;

    // Stage 1: dx, dy (the two subtractions are already free / cheap).
    let dx = sub_mod(px, qx, p);
    let dy = sub_mod(py, qy, p);

    // Stage 2: single inversion.
    // Compute a = dx * dy, invert once.
    let a = dx.mul_mod(dy, p);
    let a_inv = a.inv_mod(p).expect("dx*dy must be invertible");

    // Stage 3: split back using Montgomery's identity:
    //   1/dx = dy * a_inv
    //   1/dy = dx * a_inv   (we actually don't need this for plain add,
    //                        but it's symmetric proof that the inverse splits.)
    let inv_dx = dy.mul_mod(a_inv, p);
    // sanity check:
    debug_assert_eq!(dx.mul_mod(inv_dx, p), U256::from(1));

    // Stage 4: λ = dy * (1/dx).
    let lam = dy.mul_mod(inv_dx, p);

    // Stage 5: Rx = λ² - Px - Qx.
    let lam2 = lam.mul_mod(lam, p);
    let rx = sub_mod(sub_mod(lam2, px, p), qx, p);

    // Stage 6: Ry = λ * (Qx - Rx) - Qy.
    let qx_sub_rx = sub_mod(qx, rx, p);
    let ry = sub_mod(lam.mul_mod(qx_sub_rx, p), qy, p);

    (rx, ry)
}

/// Alternative formulation: instead of going through inv_dx, use the
/// Montgomery trick in the "dx cancels" direction, computing
///   λ = dy² * a_inv   (since λ = dy/dx = dy²/(dx*dy) = dy²*a_inv).
/// Should give the same answer; useful because it skips inv_dx and uses
/// only 2 quantum muls after the Kaliski instead of 3.
pub fn single_inv_add_skip_inv_dx(px: U256, py: U256, qx: U256, qy: U256) -> (U256, U256) {
    let p = SECP256K1_P;
    let dx = sub_mod(px, qx, p);
    let dy = sub_mod(py, qy, p);

    let a = dx.mul_mod(dy, p);
    let a_inv = a.inv_mod(p).expect("dx*dy must be invertible");

    // λ = dy * dy * a_inv
    let dy2 = dy.mul_mod(dy, p);
    let lam = dy2.mul_mod(a_inv, p);

    let lam2 = lam.mul_mod(lam, p);
    let rx = sub_mod(sub_mod(lam2, px, p), qx, p);
    let qx_sub_rx = sub_mod(qx, rx, p);
    let ry = sub_mod(lam.mul_mod(qx_sub_rx, p), qy, p);

    (rx, ry)
}

/// Yet another variant: compute ry directly from a_inv + dy + (Qx-Rx),
/// skipping the dedicated `lam` register. Sequence:
///   rx = (dy^2 - dx^2 * px - dx^2 * qx) / dx^2   (NOT cheaper, don't use)
/// vs the cleaner one below.
///
/// "λ folded": Rx uses λ²; λ² = dy² * a_inv²; and a_inv² is expensive.
/// This variant is recorded only so we remember it's dead.
#[allow(dead_code)]
pub fn single_inv_add_fold_lam(px: U256, py: U256, qx: U256, qy: U256) -> (U256, U256) {
    // Noop wrapper for now — we don't actually believe this saves anything.
    single_inv_add_skip_inv_dx(px, py, qx, qy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weierstrass_elliptic_curve::WeierstrassEllipticCurve;

    fn curve() -> WeierstrassEllipticCurve {
        WeierstrassEllipticCurve {
            modulus: SECP256K1_P,
            a: U256::from(0),
            b: U256::from(7),
            gx: U256::from_str_radix(
                "79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798",
                16,
            )
            .unwrap(),
            gy: U256::from_str_radix(
                "483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8",
                16,
            )
            .unwrap(),
            order: U256::from_str_radix(
                "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141",
                16,
            )
            .unwrap(),
        }
    }

    fn rand_u256(rng: &mut u64) -> U256 {
        let mut limbs = [0u64; 4];
        for l in &mut limbs {
            *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            *l = *rng;
        }
        U256::from_limbs(limbs) % SECP256K1_P
    }

    #[test]
    fn single_inv_matches_reference() {
        let c = curve();
        let mut rng = 0xdead_beef_cafe_f00du64;
        for trial in 0..200 {
            // pick two random scalars and form P = k1*G, Q = k2*G.
            let k1 = rand_u256(&mut rng);
            let k2 = rand_u256(&mut rng);
            let (px, py) = c.mul(c.gx, c.gy, k1);
            let (qx, qy) = c.mul(c.gx, c.gy, k2);
            if (px.is_zero() && py.is_zero())
                || (qx.is_zero() && qy.is_zero())
                || px == qx
            {
                continue;
            }
            let (rx_ref, ry_ref) = c.add(px, py, qx, qy);
            let (rx_new, ry_new) = single_inv_add(px, py, qx, qy);
            assert_eq!(rx_new, rx_ref, "rx mismatch, trial {trial}");
            assert_eq!(ry_new, ry_ref, "ry mismatch, trial {trial}");

            let (rx_alt, ry_alt) = single_inv_add_skip_inv_dx(px, py, qx, qy);
            assert_eq!(rx_alt, rx_ref, "rx_alt mismatch, trial {trial}");
            assert_eq!(ry_alt, ry_ref, "ry_alt mismatch, trial {trial}");
        }
    }
}
