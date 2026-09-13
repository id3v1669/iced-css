use super::*;
use std::cell::RefCell;
#[derive(Default)]
struct ReservationTrace {
    slots: Vec<Slot>,
    fail: Option<Slot>,
    extra: Option<Slot>,
    acquired_frames: Option<usize>,
    final_frames: Option<usize>,
}
thread_local! { static RESERVATIONS: RefCell<Option<ReservationTrace>> = const { RefCell::new(None) }; }
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
