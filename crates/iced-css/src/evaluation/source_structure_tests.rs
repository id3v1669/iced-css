use crate as iced_css;
include!("../../tests/support/source_structure/fixtures.rs");

#[test]
fn hand_schedule_records_every_debit_and_close_reopen_visit() {
    with_whitespace!(separating("x"), |whitespace| {
        let mut observer = super::Observer::default();
        let output = super::build_observed(&whitespace, structure_limits(), &mut observer).unwrap();
        let r = output.receipt();
        assert_eq!(
            (
                r.sources,
                r.contents,
                r.events,
                r.continuations,
                r.anonymous_blocks,
                r.records,
                r.gap_events,
                r.gap_scalars,
                r.work
            ),
            (5, 1, 11, 2, 3, 11, 3, 1, 48)
        );
        assert_eq!(observer.trace[0].work_after, 17);
        let gaps = observer
            .trace
            .iter()
            .filter(|r| r.phase == "gap-debit")
            .map(|r| (r.requested, r.capacity, r.work_after - r.work_before))
            .collect::<Vec<_>>();
        assert_eq!(gaps, [(3, 1, 4)]);
        let emit = observer
            .trace
            .iter()
            .find(|r| r.phase == "emit-debit")
            .unwrap();
        assert_eq!(emit.work_after - emit.work_before, 27);
        let complete = observer.trace.last().unwrap();
        assert_eq!((complete.split_closes, complete.split_reopens), (2, 2));
        let c = complete.counters.unwrap();
        assert_eq!(
            (
                c.events,
                c.begins,
                c.contents,
                c.blocks,
                c.block_closes,
                c.inlines,
                c.inline_closes,
                c.payloads,
                c.anonymous
            ),
            (11, 5, 1, 3, 3, 4, 4, 1, 3)
        );
        assert_eq!(
            observer
                .trace
                .iter()
                .filter(|r| !matches!(r.phase, "continuation-increment" | "gap-debit-requested"))
                .map(|r| r.phase)
                .collect::<Vec<_>>(),
            [
                "scratch-combined-requested",
                "facts-requested",
                "facts-acquired",
                "frames-requested",
                "frames-acquired",
                "gap-debit",
                "emit-debit",
                "output-requested",
                "output-acquired",
                "complete"
            ]
        );
        for (phase, payload) in [
            ("facts-acquired", r.fact_capacity),
            ("frames-acquired", r.frame_capacity * r.frame_element_bytes),
            (
                "output-acquired",
                r.record_capacity * r.record_element_bytes,
            ),
        ] {
            let row = observer.trace.iter().find(|r| r.phase == phase).unwrap();
            assert_eq!(row.capacity * row.element, payload);
        }
        assert_eq!(
            observer
                .trace
                .iter()
                .find(|r| r.phase == "output-acquired")
                .unwrap()
                .overlap,
            r.peak_retained_bytes
        );
        println!("hand receipt={r:?}; trace={:?}", observer.trace);
    });
}

#[test]
fn requested_and_actual_capacity_gates_use_the_same_invocation() {
    // Prior vectors remain owned by this invocation until after its final check.
    for phase in [
        super::Phase::Facts,
        super::Phase::Frames,
        super::Phase::Output,
    ] {
        let mut facts = Vec::<u8>::new();
        let mut frames = Vec::<super::Frame>::new();
        let mut records = Vec::<super::Record>::new();
        let mut observer = super::Observer::default();
        let generous = 20_000_000;
        let (count, element, prior) = match phase {
            super::Phase::Facts => (5, 1, 0),
            super::Phase::Frames => {
                let prior = super::reserve(
                    &mut facts,
                    5,
                    0,
                    generous,
                    super::Phase::Facts,
                    &mut observer,
                )
                .unwrap();
                (5, std::mem::size_of::<super::Frame>(), prior)
            }
            super::Phase::Output => {
                let f = super::reserve(
                    &mut facts,
                    5,
                    0,
                    generous,
                    super::Phase::Facts,
                    &mut observer,
                )
                .unwrap();
                let prior = super::reserve(
                    &mut frames,
                    5,
                    f,
                    generous,
                    super::Phase::Frames,
                    &mut observer,
                )
                .unwrap();
                (11, std::mem::size_of::<super::Record>(), prior)
            }
        };
        let allowance = prior + count * element;
        let admission = match phase {
            super::Phase::Facts => {
                super::reserve(&mut facts, count, prior, allowance, phase, &mut observer)
            }
            super::Phase::Frames => {
                super::reserve(&mut frames, count, prior, allowance, phase, &mut observer)
            }
            super::Phase::Output => {
                super::reserve(&mut records, count, prior, allowance, phase, &mut observer)
            }
        };
        let acquired = observer.trace.last().unwrap();
        assert!(acquired.capacity >= count);
        assert_eq!(acquired.overlap, prior + acquired.capacity * element);
        if acquired.overlap > allowance {
            assert_eq!(admission, Err(SourceStructureError::Bytes));
        } else {
            assert_eq!(admission, Ok(acquired.overlap));
        }
        assert_eq!(
            facts.capacity()
                + frames.capacity() * std::mem::size_of::<super::Frame>()
                + records.capacity() * std::mem::size_of::<super::Record>(),
            acquired.overlap
        );
        println!(
            "capacity phase={phase:?}; allowance={allowance}; admission={admission:?}; trace={:?}",
            observer.trace
        );
        drop((facts, frames, records));
    }
}

#[test]
fn fixed_public_limit_checks_combined_requested_scratch_before_reservation() {
    with_whitespace!(separating("x"), |whitespace| {
        let requested = 5 * (1 + std::mem::size_of::<super::Frame>());
        let mut limits = structure_limits();
        limits.retained_bytes = nz(requested - 1);
        let mut observer = super::Observer::default();
        assert!(matches!(
            super::build_observed(&whitespace, limits, &mut observer),
            Err(SourceStructureError::Bytes)
        ));
        assert_eq!(observer.trace.len(), 1);
        assert_eq!(observer.trace[0].phase, "scratch-combined-requested");
        // Equality admits the combined requested phase, but cannot admit output.
        limits.retained_bytes = nz(requested);
        let mut observer = super::Observer::default();
        assert!(matches!(
            super::build_observed(&whitespace, limits, &mut observer),
            Err(SourceStructureError::Bytes)
        ));
        assert!(observer.trace.iter().any(|r| r.phase == "facts-requested"));
        println!(
            "fixed scratch requested={requested}; trace={:?}",
            observer.trace
        );
    });
}

#[test]
fn every_reservation_failure_is_safe_phase_local_and_precedes_later_work() {
    with_whitespace!(separating("x"), |whitespace| {
        for (fault, last) in [
            (super::Phase::Facts, "facts-requested"),
            (super::Phase::Frames, "frames-requested"),
            (super::Phase::Output, "output-requested"),
        ] {
            let mut observer = super::Observer {
                fault: Some(fault),
                ..Default::default()
            };
            assert!(matches!(
                super::build_observed(&whitespace, structure_limits(), &mut observer),
                Err(SourceStructureError::Allocation)
            ));
            assert_eq!(observer.trace.last().unwrap().phase, last);
            println!(
                "safe reservation failure={fault:?}; trace={:?}",
                observer.trace
            );
        }
    });
}

#[test]
fn empty_gap_and_first_continuation_preserve_failure_precedence() {
    with_whitespace!(separating(""), |whitespace| {
        for work in [19, 20] {
            let mut limits = structure_limits();
            limits.work = nz(work);
            let mut observer = super::Observer::default();
            let result = super::build_observed(&whitespace, limits, &mut observer);
            if work == 19 {
                assert!(matches!(result, Err(SourceStructureError::Work)));
                assert_eq!(observer.trace.last().unwrap().phase, "gap-debit-requested");
            } else {
                assert!(
                    matches!(result,Err(SourceStructureError::Gap{events,scalars,witness:SourceStructureGapWitness::UnresolvedEmptyInlineGap{source:1,event:4},..}) if events==(4..7)&&scalars==(0..0))
                );
                let row = observer.trace.last().unwrap();
                assert_eq!(
                    (
                        row.requested,
                        row.capacity,
                        row.work_after - row.work_before
                    ),
                    (3, 0, 3)
                );
            }
            println!("empty gap work={work}; trace={:?}", observer.trace);
        }
        let mut limits = structure_limits();
        limits.records = nz(6);
        let mut observer = super::Observer::default();
        assert!(matches!(
            super::build_observed(&whitespace, limits, &mut observer),
            Err(SourceStructureError::Records)
        ));
        assert_eq!(
            observer.trace.last().unwrap().phase,
            "continuation-increment"
        );
        assert!(!observer.trace.iter().any(|r| r.phase == "gap-debit"));
    });
}

#[test]
fn checked_arithmetic_and_addressability_defenses() {
    assert_eq!(
        super::bytes(usize::MAX, 2),
        Err(SourceStructureError::Bytes)
    );
    assert_eq!(
        super::bytes(isize::MAX as usize + 1, 1),
        Err(SourceStructureError::Bytes)
    );
    assert_eq!(
        super::overlap(usize::MAX, 1, usize::MAX),
        Err(SourceStructureError::Bytes)
    );
    assert_eq!(
        super::overlap(isize::MAX as usize, isize::MAX as usize, usize::MAX),
        Ok(usize::MAX - 1)
    );
    let mut work = usize::MAX;
    assert_eq!(
        super::debit(&mut work, 1, usize::MAX),
        Err(SourceStructureError::Work)
    );
    let mut records = usize::MAX;
    assert_eq!(
        super::increment(&mut records, 1),
        Err(SourceStructureError::Records)
    );
    with_whitespace!(separating("x"), |whitespace| {
        let builder = super::Builder {
            whitespace: &whitespace,
            records: Vec::new(),
            pending_ends: 0,
            counters: Default::default(),
        };
        assert_eq!(
            builder.check_start(super::EventIndex(usize::MAX), 0),
            Err(SourceStructureError::Work)
        );
    });
}

#[test]
fn duplicate_close_suspension_and_completion_never_publish() {
    with_whitespace!(separating("x"), |whitespace| {
        let mut builder = super::Builder {
            whitespace: &whitespace,
            records: Vec::with_capacity(20),
            pending_ends: 0,
            counters: Default::default(),
        };
        let root = builder
            .append(
                None,
                super::RecordKind::SourceBlock {
                    source: super::SourceIndex(4),
                    begin: super::EventIndex(0),
                    end: None,
                },
                0,
            )
            .unwrap();
        let inline = builder
            .append(
                Some(root),
                super::RecordKind::SourceInline {
                    source: super::SourceIndex(3),
                    start: super::InlineStart::OriginalBegin(super::EventIndex(1)),
                    end: None,
                },
                1,
            )
            .unwrap();
        assert_eq!(builder.pending_ends, 2);
        builder
            .close_inline(
                inline,
                super::SourceIndex(3),
                super::InlineEnd::BeforeInterruption(super::EventIndex(2)),
            )
            .unwrap();
        assert_eq!(builder.pending_ends, 1);
        assert!(matches!(
            builder.close_inline(
                inline,
                super::SourceIndex(3),
                super::InlineEnd::BeforeInterruption(super::EventIndex(2))
            ),
            Err(SourceStructureError::Invariant {
                reason: SourceStructureInvariantReason::RecordState,
                ..
            })
        ));
        assert_eq!(builder.pending_ends, 1);
        let suspended = super::Frame {
            ordinal: super::EligibleOrdinal(1),
            state: super::FrameState::EmitInline { current: None },
        };
        assert!(matches!(
            super::Builder::inline_current(&suspended, 9),
            Err(SourceStructureError::Invariant {
                reason: SourceStructureInvariantReason::RecordState,
                ..
            })
        ));
        builder
            .close_block(root, super::SourceIndex(4), super::EventIndex(10))
            .unwrap();
        assert!(matches!(
            builder.close_block(root, super::SourceIndex(4), super::EventIndex(10)),
            Err(SourceStructureError::Invariant {
                reason: SourceStructureInvariantReason::RecordState,
                ..
            })
        ));
        assert_eq!(builder.pending_ends, 0);
        let mut observer = super::Observer::default();
        let output = super::build_observed(&whitespace, structure_limits(), &mut observer).unwrap();
        assert!(matches!(
            builder.complete(&output.receipt(), &[]),
            Err(SourceStructureError::Invariant {
                reason: SourceStructureInvariantReason::Completion,
                ..
            })
        ));
        // Even otherwise successful counters cannot publish a deliberately pending end.
        let counters = super::Counters {
            events: 11,
            begins: 5,
            contents: 1,
            blocks: 3,
            block_closes: 3,
            inlines: 4,
            inline_closes: 4,
            payloads: 1,
            anonymous: 3,
            split_closes: 2,
            split_reopens: 2,
        };
        let mut completed = super::Builder {
            whitespace: &whitespace,
            records: output.records,
            pending_ends: 1,
            counters,
        };
        assert!(matches!(
            completed.complete(&output.receipt, &[]),
            Err(SourceStructureError::Invariant {
                reason: SourceStructureInvariantReason::Completion,
                ..
            })
        ));
        completed.pending_ends = 0;
        assert!(completed.complete(&output.receipt, &[]).is_ok());
    });
}
