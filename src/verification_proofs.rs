// SPDX-License-Identifier: MIT
//! Bounded verification proofs (issue #1243).
//!
//! Pure mirrors of the arithmetic used by `contract/mutations.rs`
//! (`distribute_payout`, `set_reputation`) and `storage.rs`
//! (`status_page_count`). They live here, not in the contract, so the
//! on-chain ABI and WASM are unchanged. Each helper is proved either
//! exhaustively over its small domain, or via Kani (`cargo kani`).
//!
//! Run: `cargo test verification_proofs` (exhaustive/property proofs) and
//! `cargo kani --harness proof_` (symbolic proofs, needs Kani installed).

pub const BP_DENOM: i128 = 10_000;
pub const PAGE_SIZE: u32 = 50;

/// Mirrors `reward_amount * share_bp / 10_000` without overflow panics.
pub fn payout(reward_amount: i128, share_bp: u32) -> Option<i128> {
    reward_amount.checked_mul(share_bp as i128)?.checked_div(BP_DENOM)
}

/// Mirrors `total.div_ceil(PAGE_SIZE)` with the zero special-case.
pub fn page_count(total: u32) -> u32 {
    if total == 0 {
        0
    } else {
        total.div_ceil(PAGE_SIZE)
    }
}

/// Mirrors `set_reputation`'s saturating update.
pub fn apply_reputation(old: u32, delta: i64) -> u32 {
    if delta >= 0 {
        old.saturating_add(delta as u32)
    } else {
        old.saturating_sub(delta.unsigned_abs() as u32)
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn proof_payout_never_exceeds_reward() {
        let r: i128 = kani::any();
        let bp: u32 = kani::any();
        kani::assume(r >= 0 && r <= i128::MAX / BP_DENOM && bp <= 10_000);
        let p = payout(r, bp).unwrap();
        assert!(p >= 0 && p <= r);
    }

    #[kani::proof]
    fn proof_page_count_covers_total() {
        let t: u32 = kani::any();
        let pages = page_count(t);
        assert!((pages as u64) * (PAGE_SIZE as u64) >= t as u64);
        if t > 0 {
            assert!(((pages - 1) as u64) * (PAGE_SIZE as u64) < t as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payout_exhaustive_over_all_basis_points() {
        let rewards = [0i128, 1, 99, 100, 9_999, 10_000, 10_001, 1_000_000_007, i128::MAX / BP_DENOM];
        for &r in &rewards {
            let mut prev = 0;
            for bp in 0..=10_000u32 {
                let p = payout(r, bp).expect("no overflow in safe domain");
                assert!(p >= 0 && p <= r, "payout bounded by reward");
                assert!(p >= prev, "payout monotonic in share_bp");
                prev = p;
            }
            assert_eq!(payout(r, 10_000), Some(r));
            assert_eq!(payout(r, 0), Some(0));
        }
    }

    #[test]
    fn payout_split_never_exceeds_reward() {
        for r in [1i128, 7, 101, 12_345, 1_000_003] {
            for a in (0..=10_000u32).step_by(37) {
                let b = 10_000 - a;
                let sum = payout(r, a).unwrap() + payout(r, b).unwrap();
                assert!(sum <= r && r - sum <= 1, "rounding dust is at most one unit");
            }
        }
    }

    #[test]
    fn payout_overflow_is_detected_not_wrapped() {
        assert_eq!(payout(i128::MAX, 10_000), None);
        assert!(payout(i128::MAX / BP_DENOM + 1, 10_000).is_none());
    }

    #[test]
    fn page_count_exhaustive_small_domain_and_edges() {
        for t in 0..=(PAGE_SIZE * 40) {
            let p = page_count(t);
            assert!(p * PAGE_SIZE >= t);
            if t > 0 {
                assert!((p - 1) * PAGE_SIZE < t);
            } else {
                assert_eq!(p, 0);
            }
            // div_ceil agrees with the multiple-based formulation.
            let alt = t / PAGE_SIZE + if t % PAGE_SIZE == 0 { 0 } else { 1 };
            assert_eq!(p, alt);
            assert_eq!(t % PAGE_SIZE == 0, t.is_multiple_of(PAGE_SIZE));
        }
        assert_eq!(page_count(u32::MAX), u32::MAX / PAGE_SIZE + 1);
    }

    #[test]
    fn reputation_saturates_at_both_ends() {
        for old in [0u32, 1, 10, u32::MAX - 1, u32::MAX] {
            for delta in [i64::MIN + 1, -(u32::MAX as i64), -10, -1, 0, 1, 10, u32::MAX as i64] {
                let new = apply_reputation(old, delta);
                if delta >= 0 {
                    assert!(new >= old);
                } else {
                    assert!(new <= old);
                }
            }
        }
        assert_eq!(apply_reputation(u32::MAX, 10), u32::MAX);
        assert_eq!(apply_reputation(5, -10), 0);
    }
}
