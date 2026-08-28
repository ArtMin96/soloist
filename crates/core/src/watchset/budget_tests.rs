//! Behavioural tests for [`Budget`], kept out of the implementation file. Pure arithmetic — no
//! fakes, no clock.

use super::*;

#[test]
fn an_unknown_capacity_assumes_the_kernel_default() {
    let budget = Budget::new(None);
    assert_eq!(budget.remaining(), ASSUMED_CAPACITY / BUDGET_FRACTION);
}

#[test]
fn a_second_open_project_halves_the_share() {
    let budget = Budget::new(Some(1_000));
    assert_eq!(budget.share(1), 500);
    assert_eq!(budget.share(2), 250);
}

#[test]
fn spend_and_refund_round_trip_to_the_same_remaining() {
    let mut budget = Budget::new(Some(1_000));
    let before = budget.remaining();
    budget.spend(37);
    budget.refund(37);
    assert_eq!(budget.remaining(), before);
}

#[test]
fn remaining_saturates_at_zero_rather_than_underflowing() {
    let mut budget = Budget::new(Some(100));
    budget.spend(1_000);
    assert_eq!(budget.remaining(), 0);
}
