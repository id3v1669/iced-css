use super::*;
use std::cell::RefCell;
#[derive(Default)]
struct ReservationTrace {
    slots: Vec<Slot>,
    fail: Option<Slot>,
    extra: Option<Slot>,
    acquired_frames: Option<usize>,
    final_frames: Option<usize>,
    inventory_attempts: usize,
    inventory_records: usize,
    inventory_before: Option<usize>,
    inventory_after: Option<usize>,
    inventory_count_uses: usize,
    inventory_replay_uses: usize,
}
thread_local! { static RESERVATIONS: RefCell<Option<ReservationTrace>> = const { RefCell::new(None) }; }
pub(super) fn inventory_before(work: usize) {
    RESERVATIONS.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            t.inventory_attempts += 1;
            assert!(t.inventory_before.replace(work).is_none());
        }
    });
}
pub(super) fn inventory_after(work: usize) {
    RESERVATIONS.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            assert!(t.inventory_after.replace(work).is_none());
        }
    });
}
pub(super) fn inventory_record() {
    RESERVATIONS.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            t.inventory_records += 1;
        }
    });
}
pub(super) fn inventory_use(count: bool) {
    RESERVATIONS.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            if count {
                t.inventory_count_uses += 1;
            } else {
                t.inventory_replay_uses += 1;
            }
        }
    });
}
fn impossible_reservation() -> TryReserveError {
    Vec::<u8>::new().try_reserve(usize::MAX).unwrap_err()
}
pub(super) fn reserve_request(slot: Slot, requested: usize) -> Result<usize, TryReserveError> {
    RESERVATIONS.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            t.slots.push(slot);
            if t.fail == Some(slot) {
                return Err(impossible_reservation());
            }
            if t.extra == Some(slot) {
                return requested.checked_add(1).ok_or_else(impossible_reservation);
            }
        }
        Ok(requested)
    })
}
pub(super) fn observe_frames(capacity: usize, final_observation: bool) {
    RESERVATIONS.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            let target = if final_observation {
                &mut t.final_frames
            } else {
                &mut t.acquired_frames
            };
            assert!(
                target.replace(capacity).is_none(),
                "one frame vector per construction"
            );
        }
    });
}
fn limits() -> SourceControlInputLimits {
    let n = NonZeroUsize::new(usize::MAX).unwrap();
    SourceControlInputLimits {
        sources: n,
        records: n,
        originals: n,
        scopes: n,
        depth: n,
        output_scalars: n,
        output_bytes: n,
        retained_bytes: n,
        scratch_bytes: n,
        peak_bytes: n,
        work: n,
    }
}
fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
fn small_counts() -> Counts {
    Counts {
        scopes: 1,
        payloads: 1,
        originals: 1,
        tokens: 1,
        bytes: 1,
        ..Counts::default()
    }
}
#[test]
fn retained_same_allocation_and_scratch_boundaries() {
    let l = limits();
    let mut frames = Vec::<Frame>::new();
    reserve_vec(&mut frames, 4, Slot::Frames).unwrap();
    let scratch = bytes::<Frame>(frames.capacity()).unwrap();
    let out = reserve_storage(&small_counts(), scratch, l).unwrap();
    let actual = out.scopes.capacity() * size_of::<ScopeRow>()
        + out.payloads.capacity() * size_of::<PayloadRow>()
        + out.originals.capacity() * size_of::<OriginalRow>()
        + out.tokens.capacity() * size_of::<TokenRow>()
        + out.transport.capacity();
    assert_eq!(retained_for(out.capacities()).unwrap(), actual);
    let mut exact = l;
    exact.retained_bytes = nz(actual);
    exact.scratch_bytes = nz(scratch);
    exact.peak_bytes = nz(actual + scratch);
    assert_eq!(
        admit_storage(actual, scratch, exact).unwrap(),
        actual + scratch
    );
    admit_scratch(scratch, exact).unwrap();
    exact.retained_bytes = nz(actual - 1);
    assert!(matches!(
        admit_storage(actual, scratch, exact).unwrap_err().kind,
        ErrorKind::Limit(Limit::RetainedBytes)
    ));
    exact.retained_bytes = nz(actual);
    exact.peak_bytes = nz(actual + scratch - 1);
    assert!(matches!(
        admit_storage(actual, scratch, exact).unwrap_err().kind,
        ErrorKind::Limit(Limit::PeakBytes)
    ));
    exact.peak_bytes = l.peak_bytes;
    exact.scratch_bytes = nz(scratch - 1);
    assert!(matches!(
        admit_scratch(scratch, exact).unwrap_err().kind,
        ErrorKind::Limit(Limit::ScratchBytes)
    ));
}
#[test]
fn checked_arithmetic_rejects_without_allocation() {
    assert!(matches!(
        bytes::<TokenRow>(usize::MAX).unwrap_err().kind,
        ErrorKind::Addressability
    ));
    assert!(matches!(
        bytes::<u8>(isize::MAX as usize + 1).unwrap_err().kind,
        ErrorKind::Addressability
    ));
    assert!(matches!(
        add(usize::MAX, 1, global()).unwrap_err().kind,
        ErrorKind::Addressability
    ));
    assert!(matches!(
        retained_for([usize::MAX, 0, 0, 0, 0]).unwrap_err().kind,
        ErrorKind::Addressability
    ));
    let mut ledger = Ledger {
        used: usize::MAX,
        limit: usize::MAX,
    };
    assert!(matches!(
        ledger.charge(1, global()).unwrap_err().kind,
        ErrorKind::Limit(Limit::Work)
    ));
}
#[test]
fn every_reservation_failure_preserves_slot_order_and_source() {
    let all = [
        Slot::Frames,
        Slot::Scopes,
        Slot::Payloads,
        Slot::Originals,
        Slot::Tokens,
        Slot::Transport,
    ];
    for (end, slot) in all.into_iter().enumerate() {
        RESERVATIONS.with_borrow_mut(|t| {
            *t = Some(ReservationTrace {
                slots: vec![],
                fail: Some(slot),
                ..ReservationTrace::default()
            })
        });
        let result = (|| {
            let mut frames = Vec::<Frame>::new();
            reserve_vec(&mut frames, 1, Slot::Frames)?;
            reserve_storage(
                &small_counts(),
                bytes::<Frame>(frames.capacity())?,
                limits(),
            )
        })();
        let trace = RESERVATIONS.with_borrow_mut(|t| t.take().unwrap());
        assert_eq!(trace.slots, &all[..=end]);
        let error = result.unwrap_err();
        let ErrorKind::Allocation { storage, source } = &error.kind else {
            panic!("expected controlled reserve failure")
        };
        assert_eq!(*storage, slot);
        let exposed = std::error::Error::source(&error).unwrap();
        let concrete = exposed.downcast_ref::<TryReserveError>().unwrap();
        assert!(std::ptr::eq(concrete, source));
        let kind_source = std::error::Error::source(&error.kind)
            .unwrap()
            .downcast_ref::<TryReserveError>()
            .unwrap();
        assert!(std::ptr::eq(kind_source, source));
    }
}
#[test]
fn requested_floor_rejects_before_reservations_and_zero_skips() {
    RESERVATIONS.with_borrow_mut(|t| *t = Some(ReservationTrace::default()));
    let mut l = limits();
    l.retained_bytes = nz(1);
    assert!(matches!(
        reserve_storage(&small_counts(), 0, l).unwrap_err().kind,
        ErrorKind::Limit(Limit::RetainedBytes)
    ));
    assert!(
        RESERVATIONS
            .with_borrow_mut(|t| t.take().unwrap())
            .slots
            .is_empty()
    );
    RESERVATIONS.with_borrow_mut(|t| *t = Some(ReservationTrace::default()));
    reserve_storage(&Counts::default(), 0, l).unwrap();
    assert!(
        RESERVATIONS
            .with_borrow_mut(|t| t.take().unwrap())
            .slots
            .is_empty()
    );
}
#[test]
fn simulated_capacity_excess_uses_real_admission() {
    let counts = small_counts();
    let floors = [
        counts.scopes,
        counts.payloads,
        counts.originals,
        counts.tokens,
        counts.bytes,
    ];
    let floor = retained_for(floors).unwrap();
    let mut l = limits();
    l.retained_bytes = nz(floor);
    let mut acquired = floors;
    acquired[0] += 1;
    let actual = retained_for(acquired).unwrap();
    assert!(matches!(
        admit_storage(actual, 0, l).unwrap_err().kind,
        ErrorKind::Limit(Limit::RetainedBytes)
    ));
}
#[test]
fn bounded_push_and_completion_reject_partial_output() {
    let mut out = Storage::default();
    assert!(matches!(
        push(
            &mut out.originals,
            OriginalRow {
                payload: 0,
                members: 0,
                tokens: 0..0
            },
            global()
        )
        .unwrap_err()
        .kind,
        ErrorKind::Invariant(Reason::EmissionCounts)
    ));
    let expected = small_counts();
    let frames = Vec::new();
    assert!(matches!(
        verify_emission(
            &out,
            &expected,
            &Counts::default(),
            out.capacities(),
            0,
            &frames
        )
        .unwrap_err()
        .kind,
        ErrorKind::Invariant(Reason::EmissionCounts)
    ));
    let mut ledger = Ledger {
        used: 0,
        limit: usize::MAX,
    };
    let mut walk = Walk {
        counts: Counts {
            tokens: 1,
            ..Counts::default()
        },
        ledger: &mut ledger,
        debit: false,
        limits: limits(),
        output: None,
        scope: Some(PendingScope {
            record: 0,
            context: global(),
            participation: Participation::Empty,
            originals: 0,
            payloads: 0,
            tokens: 0,
            bytes: 0,
        }),
        occurrence: 0,
    };
    assert!(matches!(
        walk.finalize(1).unwrap_err().kind,
        ErrorKind::Invariant(Reason::ScopeFinalization)
    ));
}
#[expect(
    dead_code,
    reason = "This unit test reuses the larger captured-tree integration fixture harness."
)]
mod actual_owner {
    use crate as iced_css;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/source_control_input/setup.rs"
    ));
    fn interruption_recipe(before: &str, middle: &str, after: &str, bidi: &str) -> InputNode {
        managed(
            "B",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![managed(
                "I",
                InputFamily::Row,
                "inline",
                "rtl",
                bidi,
                vec![
                    text("A", before, "pre", "none", "default", "normal"),
                    managed(
                        "K",
                        InputFamily::Column,
                        "block",
                        "ltr",
                        "normal",
                        vec![text("X", middle, "pre", "none", "default", "normal")],
                    ),
                    text("C", after, "pre", "none", "default", "normal"),
                ],
            )],
        )
    }
    #[test]
    fn mismatched_private_inventory_is_invariant_in_count_and_replay() {
        with_predecessors!(
            interruption_recipe("a", "x", "b", "embed"),
            |_whitespace, _casing, structure| {
                let before = structure
                    .records()
                    .find(|record| {
                        record.end_edge().is_some_and(|edge| {
                            edge.kind() == SourceStructureEdgeKind::BeforeInterruption
                        })
                    })
                    .unwrap();
                let context = super::Context {
                    source: before.source_index(),
                    record: Some(before.index()),
                    ..super::global()
                };
                let valid = super::inventory_shape(
                    &structure,
                    context,
                    SourceStructureEdgeKind::BeforeInterruption,
                )
                .unwrap();
                for debit in [true, false] {
                    let mut ledger = super::Ledger {
                        used: 22,
                        limit: usize::MAX,
                    };
                    let mut cached = Some(valid);
                    super::RESERVATIONS.with_borrow_mut(|trace| {
                        *trace = Some(super::ReservationTrace::default());
                    });
                    assert_eq!(
                        super::inline_open(&before, context, &mut ledger, debit, &mut cached)
                            .unwrap(),
                        Some('\u{202b}')
                    );
                    let mut mismatched = valid;
                    mismatched.fragments.swap(0, 1);
                    let mut cached = Some(mismatched);
                    let error =
                        super::inline_open(&before, context, &mut ledger, debit, &mut cached)
                            .unwrap_err();
                    let trace = super::RESERVATIONS.with_borrow_mut(|trace| trace.take().unwrap());
                    assert!(matches!(
                        error.kind,
                        super::ErrorKind::Invariant(super::Reason::EmissionCounts)
                    ));
                    assert_eq!(
                        (error.source, error.record, error.original, error.member),
                        (before.source_index(), Some(before.index()), None, None)
                    );
                    assert_eq!(ledger.used, 22);
                    assert_eq!((trace.inventory_attempts, trace.inventory_records), (0, 0));
                    assert!(trace.slots.is_empty());
                }
            }
        );
    }
    #[test]
    fn actual_lazy_inventory_once_and_no_scan_for_normal_fragments() {
        for (bidi, work, scans) in [("embed", 83, 1), ("normal", 63, 0)] {
            with_predecessors!(
                interruption_recipe("a", "x", "b", bidi),
                |_w, casing, structure| {
                    assert_eq!(
                        (
                            structure.source_count(),
                            structure.record_count(),
                            casing.scalar_count()
                        ),
                        (6, 12, 3)
                    );
                    super::RESERVATIONS
                        .with_borrow_mut(|t| *t = Some(super::ReservationTrace::default()));
                    let owner =
                        super::build_source_control_input(&structure, &casing, super::limits())
                            .unwrap();
                    let trace = super::RESERVATIONS.with_borrow_mut(|t| t.take().unwrap());
                    let r = owner.receipt();
                    assert_eq!(r.work, work);
                    assert_eq!(
                        (
                            r.frames_pushed,
                            r.max_depth,
                            r.payloads,
                            r.scopes,
                            r.domains
                        ),
                        (9, 4, 3, 4, 3)
                    );
                    assert_eq!(trace.inventory_attempts, scans);
                    assert_eq!(trace.inventory_records, scans * 12);
                    assert_eq!(trace.inventory_count_uses, scans * 2);
                    assert_eq!(trace.inventory_replay_uses, scans * 2);
                    assert_eq!(trace.inventory_before, (scans != 0).then_some(10));
                    assert_eq!(trace.inventory_after, (scans != 0).then_some(22));
                    assert_eq!(
                        trace.slots,
                        [
                            super::Slot::Frames,
                            super::Slot::Scopes,
                            super::Slot::Payloads,
                            super::Slot::Originals,
                            super::Slot::Tokens,
                            super::Slot::Transport
                        ]
                    );
                }
            );
        }
    }
    #[test]
    fn actual_lazy_inventory_work_boundaries_precede_retained_allocation() {
        with_predecessors!(
            interruption_recipe("a", "x", "b", "embed"),
            |_w, casing, structure| {
                let cause = structure
                    .records()
                    .find(|r| {
                        r.kind() == SourceStructureKind::SourceInline
                            && r.participant().unwrap().bidi_style().unicode_bidi()
                                != iced_css_core::values::bidi::ComputedUnicodeBidi::Normal
                    })
                    .unwrap();
                for allowance in [21, 22, 82] {
                    let mut limits = super::limits();
                    limits.work = super::nz(allowance);
                    super::RESERVATIONS
                        .with_borrow_mut(|t| *t = Some(super::ReservationTrace::default()));
                    let error =
                        super::build_source_control_input(&structure, &casing, limits).unwrap_err();
                    let trace = super::RESERVATIONS.with_borrow_mut(|t| t.take().unwrap());
                    assert!(matches!(
                        error.kind,
                        super::ErrorKind::Limit(super::Limit::Work)
                    ));
                    assert_eq!(
                        (error.source, error.record, error.original, error.member),
                        if allowance == 22 {
                            (cause.source_index(), Some(cause.index()), None, None)
                        } else {
                            (None, None, None, None)
                        }
                    );
                    assert_eq!(trace.inventory_attempts, 1);
                    assert_eq!(trace.inventory_before, Some(10));
                    assert_eq!(trace.inventory_after, (allowance != 21).then_some(22));
                    assert_eq!(
                        trace.inventory_records,
                        if allowance == 21 { 0 } else { 12 }
                    );
                    assert_eq!(trace.inventory_replay_uses, 0);
                    assert_eq!(trace.slots, [super::Slot::Frames]);
                }
                let mut exact = super::limits();
                exact.work = super::nz(83);
                assert_eq!(
                    super::build_source_control_input(&structure, &casing, exact)
                        .unwrap()
                        .receipt()
                        .work,
                    83
                );
                let mut scratch_short = super::limits();
                scratch_short.scratch_bytes = super::nz(1);
                scratch_short.work = super::nz(21);
                super::RESERVATIONS
                    .with_borrow_mut(|t| *t = Some(super::ReservationTrace::default()));
                let error = super::build_source_control_input(&structure, &casing, scratch_short)
                    .unwrap_err();
                let trace = super::RESERVATIONS.with_borrow_mut(|t| t.take().unwrap());
                assert!(matches!(
                    error.kind,
                    super::ErrorKind::Limit(super::Limit::ScratchBytes)
                ));
                assert_eq!(trace.inventory_attempts, 0);
                assert!(trace.slots.is_empty());
            }
        );
    }
    #[test]
    fn override_blocks_in_interrupted_graph_reject_without_inventory_or_reservation() {
        for (block, bidi) in [("B", "bidi-override"), ("K", "bidi-override")] {
            let mut root = interruption_recipe("a", "x", "b", "embed");
            let target = if block == "B" {
                &mut root
            } else {
                &mut root.children[0].children[1]
            };
            target.unicode_bidi = bidi.into();
            with_predecessors!(root, |_w, casing, structure| {
                super::RESERVATIONS
                    .with_borrow_mut(|t| *t = Some(super::ReservationTrace::default()));
                let error = super::build_source_control_input(&structure, &casing, super::limits())
                    .unwrap_err();
                let trace = super::RESERVATIONS.with_borrow_mut(|t| t.take().unwrap());
                assert!(matches!(
                    error.kind,
                    super::ErrorKind::Unsupported(super::Unsupported::BlockBidi(_))
                ));
                assert_eq!((trace.inventory_attempts, trace.inventory_records), (0, 0));
                assert_eq!(
                    (trace.inventory_before, trace.inventory_after),
                    (None, None)
                );
                assert!(trace.slots.is_empty());
            });
        }
    }
    #[test]
    fn actual_interrupted_member_rejections_keep_trigger_and_only_scratch() {
        for (before, middle, after) in [
            ("", "x", "b"),
            ("a", "", "b"),
            ("a", "x", ""),
            ("a\nb", "x", "b"),
            ("a", "x\ny", "b"),
            ("a", "x", "b\nc"),
        ] {
            with_predecessors!(
                interruption_recipe(before, middle, after, "embed"),
                |_w, casing, structure| {
                    let cause = structure
                        .records()
                        .find(|r| {
                            r.kind() == SourceStructureKind::SourceInline
                                && r.participant().unwrap().bidi_style().unicode_bidi()
                                    != iced_css_core::values::bidi::ComputedUnicodeBidi::Normal
                        })
                        .unwrap();
                    super::RESERVATIONS
                        .with_borrow_mut(|t| *t = Some(super::ReservationTrace::default()));
                    let error =
                        super::build_source_control_input(&structure, &casing, super::limits())
                            .unwrap_err();
                    let trace = super::RESERVATIONS.with_borrow_mut(|t| t.take().unwrap());
                    assert!(matches!(
                        error.kind,
                        super::ErrorKind::Unsupported(super::Unsupported::InterruptedInline(
                            SourceStructureEdgeKind::BeforeInterruption
                        ))
                    ));
                    assert_eq!(
                        (error.source, error.record, error.original, error.member),
                        (cause.source_index(), Some(cause.index()), None, None)
                    );
                    assert_eq!(trace.inventory_records, 12);
                    assert_eq!(trace.inventory_replay_uses, 0);
                    assert_eq!(trace.slots, [super::Slot::Frames]);
                }
            );
        }
    }
    #[test]
    fn actual_owner_capacity_receipt_and_budget_boundaries() {
        let root = managed(
            "B",
            InputFamily::Column,
            "block",
            "ltr",
            "normal",
            vec![managed(
                "I",
                InputFamily::Row,
                "inline",
                "rtl",
                "embed",
                vec![text("T", "a\nb", "pre", "none", "default", "normal")],
            )],
        );
        with_predecessors!(root, |_whitespace, casing, structure| {
            super::RESERVATIONS.with_borrow_mut(|t| *t = Some(super::ReservationTrace::default()));
            let owner =
                super::build_source_control_input(&structure, &casing, super::limits()).unwrap();
            let actual = owner.scopes.capacity() * std::mem::size_of::<super::ScopeRow>()
                + owner.payloads.capacity() * std::mem::size_of::<super::PayloadRow>()
                + owner.originals.capacity() * std::mem::size_of::<super::OriginalRow>()
                + owner.tokens.capacity() * std::mem::size_of::<super::TokenRow>()
                + owner.transport.capacity();
            let trace = super::RESERVATIONS.with_borrow_mut(|t| t.take().unwrap());
            assert_eq!(
                (
                    trace.inventory_attempts,
                    trace.inventory_records,
                    trace.inventory_count_uses,
                    trace.inventory_replay_uses
                ),
                (0, 0, 0, 0)
            );
            let acquired = trace
                .acquired_frames
                .expect("actual frame acquisition observed");
            let final_capacity = trace
                .final_frames
                .expect("actual frames observed before drop");
            assert_eq!(acquired, final_capacity);
            let scratch = acquired * std::mem::size_of::<super::Frame>();
            let r = owner.receipt();
            assert_eq!(r.frame_capacity, acquired);
            assert_eq!(r.scratch_bytes, scratch);
            assert_eq!(r.peak_retained_bytes, actual + scratch);
            assert_eq!(r.retained_bytes, actual);
            assert_eq!(r.work, 52);
            assert_eq!(r.scope_capacity, owner.scopes.capacity());
            assert_eq!(r.payload_capacity, owner.payloads.capacity());
            assert_eq!(r.original_capacity, owner.originals.capacity());
            assert_eq!(r.token_capacity, owner.tokens.capacity());
            assert_eq!(r.transport_capacity, owner.transport.capacity());
            let mut l = super::limits();
            l.retained_bytes = super::nz(actual);
            l.peak_bytes = super::nz(actual + scratch);
            l.scratch_bytes = super::nz(scratch);
            assert_eq!(
                super::admit_storage(actual, scratch, l).unwrap(),
                r.peak_retained_bytes
            );
            super::admit_scratch(scratch, l).unwrap();
            l.peak_bytes = super::nz(actual + scratch - 1);
            assert!(matches!(
                super::admit_storage(actual, scratch, l).unwrap_err().kind,
                super::ErrorKind::Limit(super::Limit::PeakBytes)
            ));
            l.peak_bytes = super::nz(actual + scratch);
            l.scratch_bytes = super::nz(scratch - 1);
            assert!(matches!(
                super::admit_scratch(scratch, l).unwrap_err().kind,
                super::ErrorKind::Limit(super::Limit::ScratchBytes)
            ));
            l.scratch_bytes = super::nz(scratch);
            l.retained_bytes = super::nz(actual - 1);
            assert!(matches!(
                super::admit_storage(actual, scratch, l).unwrap_err().kind,
                super::ErrorKind::Limit(super::Limit::RetainedBytes)
            ));
        });
    }
}

#[test]
fn acquired_excess_rejects_before_the_next_real_reservation() {
    let counts = small_counts();
    let floor = retained_for([
        counts.scopes,
        counts.payloads,
        counts.originals,
        counts.tokens,
        counts.bytes,
    ])
    .unwrap();
    let mut l = limits();
    l.retained_bytes = nz(floor);
    RESERVATIONS.with_borrow_mut(|t| {
        *t = Some(ReservationTrace {
            extra: Some(Slot::Scopes),
            ..ReservationTrace::default()
        })
    });
    let error = reserve_storage(&counts, 0, l).unwrap_err();
    let trace = RESERVATIONS.with_borrow_mut(|t| t.take().unwrap());
    assert!(matches!(error.kind, ErrorKind::Limit(Limit::RetainedBytes)));
    assert_eq!(trace.slots, [Slot::Scopes]);
}
