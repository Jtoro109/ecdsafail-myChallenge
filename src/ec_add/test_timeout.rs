//! Tiny in-tree timeout helpers for long-running classical tests.
//!
//! We can't use external crates, so we do cooperative deadline checks from
//! inside the long loops. Each test gets a 2 minute wall-clock budget.

use std::time::{Duration, Instant};

pub(crate) fn two_min_deadline() -> Instant {
    Instant::now() + Duration::from_secs(120)
}

#[inline(always)]
pub(crate) fn check_deadline(deadline: Instant, label: &str) {
    if Instant::now() > deadline {
        panic!("test timeout exceeded 120s in {}", label);
    }
}
