use kb_core::Debouncer;

#[test]
fn press_and_release_require_a_stable_interval() {
    let (state, update) = Debouncer::<8>::new(5).sample(1, 100);
    assert_eq!(update.stable, 0);
    let (state, update) = state.sample(1, 104);
    assert_eq!(update.pressed, 0);
    let (state, update) = state.sample(1, 105);
    assert_eq!((update.stable, update.pressed, update.released), (1, 1, 0));
    let (state, update) = state.sample(0, 200);
    assert_eq!(update.released, 0);
    let (_, update) = state.sample(0, 205);
    assert_eq!((update.stable, update.pressed, update.released), (0, 0, 1));
}

#[test]
fn bounce_restarts_only_the_affected_key_timer() {
    let (state, _) = Debouncer::<8>::new(5).sample(3, 0);
    let (state, _) = state.sample(2, 3);
    let (state, _) = state.sample(3, 4);
    let (state, update) = state.sample(3, 5);
    assert_eq!(update.pressed, 2);
    let (_, update) = state.sample(3, 9);
    assert_eq!(update.pressed, 1);
}

#[test]
fn a_reverted_candidate_never_emits_an_edge() {
    let (state, _) = Debouncer::<1>::new(5).sample(1, 0);
    let (state, _) = state.sample(0, 4);
    let (_, update) = state.sample(0, 50);
    assert_eq!(update.stable | update.pressed | update.released, 0);
}

#[test]
fn millisecond_clock_wrap_does_not_release_or_accept_early() {
    let (state, _) = Debouncer::<1>::new(5).sample(1, u32::MAX - 2);
    let (state, update) = state.sample(1, 1);
    assert_eq!(update.pressed, 0);
    let (_, update) = state.sample(1, 2);
    assert_eq!(update.pressed, 1);
}

#[test]
fn zero_delay_and_full_width_masks_are_supported() {
    let (_, update) = Debouncer::<64>::new(0).sample(u64::MAX, 0);
    assert_eq!(update.pressed, u64::MAX);
    let (_, update) = Debouncer::<0>::new(0).sample(u64::MAX, 0);
    assert_eq!(update.stable, 0);
}
