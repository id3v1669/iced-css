// Qualification observers are attached to actual production operations and compiled
// only into the test library. Their storage is outside the producer receipt.
#[derive(Debug, Default)]
struct QualificationTrace {
    phases: Vec<String>,
    work: usize,
    reservations: [usize; 3],
}
std::thread_local! {
    static TRACE: std::cell::RefCell<Option<QualificationTrace>> = const { std::cell::RefCell::new(None) };
    static QUERIES: std::cell::Cell<QueryInstrumentation> = const { std::cell::Cell::new(QueryInstrumentation { scalar_partition_comparisons:0, scope_partition_comparisons:0, scalar_binary_search_ceiling:0, scope_binary_search_ceiling:0 }) };
}
fn trace_phase(name: &str, amount: usize) {
    TRACE.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            t.phases.push(format!("{name}:{amount}"));
        }
    });
}
fn trace_complete() {
    TRACE.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            t.phases.push("first-pass-complete".into());
        }
    });
}
fn trace_reservation(phase: usize) {
    TRACE.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            t.reservations[phase] += 1;
        }
    });
}
fn trace_work(work: usize) {
    TRACE.with_borrow_mut(|trace| {
        if let Some(t) = trace {
            t.work = work;
        }
    });
}
#[derive(Clone, Copy, Debug)]
struct QueryInstrumentation {
    scalar_partition_comparisons: usize,
    scope_partition_comparisons: usize,
    scalar_binary_search_ceiling: usize,
    scope_binary_search_ceiling: usize,
}
fn query_instrumentation_reset() {
    QUERIES.set(QueryInstrumentation {
        scalar_partition_comparisons: 0,
        scope_partition_comparisons: 0,
        scalar_binary_search_ceiling: 0,
        scope_binary_search_ceiling: 0,
    });
}
fn query_instrumentation_read() -> QueryInstrumentation {
    QUERIES.get()
}
fn binary_ceiling(size: usize) -> usize {
    size.checked_ilog2().map_or(0, |n| n as usize + 2)
}
pub(super) fn query_scalar_size(size: usize) {
    let mut q = QUERIES.get();
    q.scalar_binary_search_ceiling = binary_ceiling(size);
    QUERIES.set(q);
}
pub(super) fn query_scope_size(size: usize) {
    let mut q = QUERIES.get();
    q.scope_binary_search_ceiling = binary_ceiling(size);
    QUERIES.set(q);
}
pub(super) fn query_scalar_comparison() {
    let mut q = QUERIES.get();
    q.scalar_partition_comparisons += 1;
    QUERIES.set(q);
}
pub(super) fn query_scope_comparison() {
    let mut q = QUERIES.get();
    q.scope_partition_comparisons += 1;
    QUERIES.set(q);
}
impl Ledger {
    fn qualification(allowance: usize) -> Self {
        Self { used: 0, allowance }
    }
}
impl ScopeGeneration {
    fn qualification(q: usize, s: usize, u: usize, t: usize) -> Self {
        Self::new(
            reserve_three(q, s, t, usize::MAX).expect("qualification allocation"),
            q,
            s,
            u,
            t,
        )
    }
}
#[derive(Debug)]
struct QualificationReservationReceipt {
    scope_calls: usize,
    scalar_calls: usize,
    transport_calls: usize,
    scope_capacity: usize,
    scalar_capacity: usize,
    transport_capacity: usize,
    retained_bytes: usize,
}
fn qualification_reserve_three(
    q: usize,
    s: usize,
    t: usize,
    allowance: usize,
) -> Result<QualificationReservationReceipt, Error> {
    TRACE.with_borrow_mut(|v| *v = Some(QualificationTrace::default()));
    let result = reserve_three(q, s, t, allowance);
    let trace = TRACE.with_borrow_mut(Option::take).expect("active trace");
    let output = result?;
    let scope_capacity = output.scopes.capacity();
    let scalar_capacity = output.scalars.capacity();
    let transport_capacity = output.transport.capacity();
    let retained_bytes = admit_retained_bytes(
        bytes_for_capacity::<ScopeRecord>(scope_capacity)?,
        bytes_for_capacity::<ScalarRecord>(scalar_capacity)?,
        bytes_for_capacity::<u8>(transport_capacity)?,
        allowance,
    )?;
    Ok(QualificationReservationReceipt {
        scope_calls: trace.reservations[0],
        scalar_calls: trace.reservations[1],
        transport_calls: trace.reservations[2],
        scope_capacity,
        scalar_capacity,
        transport_capacity,
        retained_bytes,
    })
}
#[derive(Debug)]
struct QualificationWorkFailure {
    error: Error,
    phases: Vec<String>,
    work_before_failed_phase: usize,
    reservation_calls: (usize, usize, usize),
}
fn qualification_work_trace<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>(
    structure: &'t SourceStructure<'wt, 'e, 'd, 's, 'b, 'r>,
    casing: &'c SourceCasing<'wc, 'e, 'd, 's, 'b, 'r>,
    work_allowance: usize,
) -> Result<SourceAnalysisInput<'t, 'c, 'wt, 'wc, 'e, 'd, 's, 'b, 'r>, QualificationWorkFailure> {
    let n = NonZeroUsize::new(usize::MAX).expect("nonzero");
    let limits = SourceAnalysisInputLimits {
        sources: n,
        scopes: n,
        output_scalars: n,
        output_bytes: n,
        retained_bytes: n,
        work: NonZeroUsize::new(work_allowance).expect("nonzero qualification work"),
    };
    TRACE.with_borrow_mut(|v| *v = Some(QualificationTrace::default()));
    let result = build_source_analysis_input(structure, casing, limits);
    let trace = TRACE.with_borrow_mut(Option::take).expect("active trace");
    result.map_err(|error| QualificationWorkFailure {
        error,
        phases: trace.phases,
        work_before_failed_phase: trace.work,
        reservation_calls: (
            trace.reservations[0],
            trace.reservations[1],
            trace.reservations[2],
        ),
    })
}
mod tests {
    // Reviewed private tests, with qualification additions below. Every named item is
    // bound by private-helper-bindings.json to the production helper; these tests define no model.
    // Include inside a cfg(test) child module of the production source_analysis_input module.
    // The four public fixture files are installed first. Reuse their exact construction and
    // smoke tests here; the local alias makes their external crate paths name this crate.
    use super::{
        Ledger, ScalarRecord, ScopeGeneration, ScopeRecord, admit_retained_bytes,
        bytes_for_capacity, count_presentation_members, qualification_reserve_three,
        qualification_work_trace, query_instrumentation_read, query_instrumentation_reset,
        reserve_string_with, reserve_vec_with,
    };
    use crate as iced_css;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/source_analysis_input/fixtures.rs"
    ));

    // Macro bodies run while every captured/style/eligibility/whitespace/predecessor owner
    // from with_predecessors remains live. No helper returns a locally borrowing owner.
    macro_rules! with_large_query_fixture {
        (|$input:ident, $inline_scope:ident, $payload:ident| $body:block) => {{
            let children = (0..4)
                .map(|block| {
                    let children = (0..128)
                        .map(|scalar| {
                            text(
                                &format!("q_{block}_{scalar}"),
                                "x",
                                "normal",
                                "none",
                                "default",
                                "normal",
                            )
                        })
                        .collect();
                    managed(
                        &format!("q_{block}"),
                        InputFamily::Column,
                        "block",
                        "ltr",
                        "normal",
                        children,
                    )
                })
                .collect();
            let recipe = managed("q", InputFamily::Column, "block", "ltr", "normal", children);
            with_predecessors!(recipe, |_whitespace, casing, structure| {
                assert_eq!(
                    (
                        structure.source_count(),
                        structure.record_count(),
                        casing.scalar_count()
                    ),
                    (517, 1029, 512)
                );
                let $input =
                    build_source_analysis_input(&structure, &casing, analysis_limits()).unwrap();
                let receipt = $input.receipt();
                assert_eq!(
                    (
                        receipt.sources,
                        receipt.records,
                        receipt.payloads,
                        receipt.original_scalars,
                        receipt.scopes,
                        receipt.domains,
                        receipt.output_scalars,
                        receipt.output_bytes
                    ),
                    (517, 1029, 512, 512, 5, 4, 512, 512)
                );
                let $inline_scope = 1;
                let $payload = 3;
                assert_eq!(
                    $input.scope($inline_scope).unwrap().participation(),
                    SourceAnalysisParticipation::InlineSequence
                );
                assert_eq!(
                    $input
                        .scope($inline_scope)
                        .unwrap()
                        .inline_sequence()
                        .unwrap()
                        .scalar_count(),
                    128
                );
                assert_eq!(
                    structure.record($payload).unwrap().kind(),
                    SourceStructureKind::Payload
                );
                $body
            })
        }};
    }

    macro_rules! with_separating_text_predecessors {
        (|$structure:ident, $casing:ident| $body:block) => {{
            let recipe = managed(
                "s",
                InputFamily::Column,
                "block",
                "ltr",
                "normal",
                vec![managed(
                    "s_0",
                    InputFamily::Row,
                    "inline",
                    "ltr",
                    "normal",
                    vec![
                        managed("s_0_0", InputFamily::Row, "block", "ltr", "normal", vec![]),
                        text("s_0_1", "x", "normal", "none", "default", "normal"),
                        managed("s_0_2", InputFamily::Row, "block", "ltr", "normal", vec![]),
                    ],
                )],
            );
            with_predecessors!(recipe, |_whitespace, $casing, $structure| {
                assert_eq!(
                    (
                        $structure.source_count(),
                        $structure.record_count(),
                        $casing.scalar_count()
                    ),
                    (5, 11, 1)
                );
                $body
            })
        }};
    }
    use std::mem::size_of;

    #[test]
    fn real_size_and_live_capacity_admission_at_each_phase() {
        let q = 6;
        let s = 1;
        let t = 1;
        let requested_scope = bytes_for_capacity::<ScopeRecord>(q).unwrap();
        let requested_scalar = bytes_for_capacity::<ScalarRecord>(s).unwrap();
        admit_retained_bytes(requested_scope, requested_scalar, t, usize::MAX).unwrap();
        let mut scopes = Vec::<ScopeRecord>::new();
        reserve_vec_with(&mut scopes, q, |v, n| {
            v.try_reserve_exact(n).map_err(|_| ())
        })
        .unwrap();
        let actual_scope = bytes_for_capacity::<ScopeRecord>(scopes.capacity()).unwrap();
        let scope_floor = actual_scope
            .checked_add(requested_scalar)
            .unwrap()
            .checked_add(t)
            .unwrap();
        assert_eq!(
            admit_retained_bytes(actual_scope, requested_scalar, t, scope_floor).unwrap(),
            scope_floor
        );
        assert!(matches!(
            admit_retained_bytes(actual_scope, requested_scalar, t, scope_floor - 1),
            Err(SourceAnalysisInputError::Bytes)
        ));
        let mut scalars = Vec::<ScalarRecord>::new();
        reserve_vec_with(&mut scalars, s, |v, n| {
            v.try_reserve_exact(n).map_err(|_| ())
        })
        .unwrap();
        let actual_scalar = bytes_for_capacity::<ScalarRecord>(scalars.capacity()).unwrap();
        let scalar_floor = actual_scope
            .checked_add(actual_scalar)
            .unwrap()
            .checked_add(t)
            .unwrap();
        assert_eq!(
            admit_retained_bytes(actual_scope, actual_scalar, t, scalar_floor).unwrap(),
            scalar_floor
        );
        assert!(matches!(
            admit_retained_bytes(actual_scope, actual_scalar, t, scalar_floor - 1),
            Err(SourceAnalysisInputError::Bytes)
        ));
        let mut transport = String::new();
        reserve_string_with(&mut transport, t, |v, n| {
            v.try_reserve_exact(n).map_err(|_| ())
        })
        .unwrap();
        let final_bytes = admit_retained_bytes(
            actual_scope,
            actual_scalar,
            transport.capacity(),
            usize::MAX,
        )
        .unwrap();
        assert_eq!(
            admit_retained_bytes(
                actual_scope,
                actual_scalar,
                transport.capacity(),
                final_bytes
            )
            .unwrap(),
            final_bytes
        );
        assert!(matches!(
            admit_retained_bytes(
                actual_scope,
                actual_scalar,
                transport.capacity(),
                final_bytes - 1
            ),
            Err(SourceAnalysisInputError::Bytes)
        ));
        eprintln!(
            "actual live capacities: scopes={} scalars={} transport={} row sizes={}/{} phase floors={}/{}/{}",
            scopes.capacity(),
            scalars.capacity(),
            transport.capacity(),
            size_of::<ScopeRecord>(),
            size_of::<ScalarRecord>(),
            scope_floor,
            scalar_floor,
            final_bytes
        );
        assert_eq!(
            final_bytes,
            scopes.capacity() * size_of::<ScopeRecord>()
                + scalars.capacity() * size_of::<ScalarRecord>()
                + transport.capacity()
        );
        assert!(scopes.capacity() >= q && scalars.capacity() >= s && transport.capacity() >= t);
        std::hint::black_box((&scopes, &scalars, &transport));
    }

    #[test]
    fn three_zero_request_patterns_skip_only_the_zero_reservations() {
        let all = qualification_reserve_three(0, 0, 0, 1).unwrap();
        assert_eq!(
            (
                all.scope_calls,
                all.scalar_calls,
                all.transport_calls,
                all.scope_capacity,
                all.scalar_capacity,
                all.transport_capacity,
                all.retained_bytes
            ),
            (0, 0, 0, 0, 0, 0, 0)
        );
        let one = qualification_reserve_three(1, 0, 0, usize::MAX).unwrap();
        assert_eq!(
            (one.scope_calls, one.scalar_calls, one.transport_calls),
            (1, 0, 0)
        );
        assert!(one.scope_capacity >= 1);
        assert_eq!((one.scalar_capacity, one.transport_capacity), (0, 0));
        assert_eq!(
            one.retained_bytes,
            one.scope_capacity * size_of::<ScopeRecord>()
        );
        let two = qualification_reserve_three(1, 1, 0, usize::MAX).unwrap();
        assert_eq!(
            (two.scope_calls, two.scalar_calls, two.transport_calls),
            (1, 1, 0)
        );
        assert!(two.scope_capacity >= 1 && two.scalar_capacity >= 1);
        assert_eq!(two.transport_capacity, 0);
        assert_eq!(
            two.retained_bytes,
            two.scope_capacity * size_of::<ScopeRecord>()
                + two.scalar_capacity * size_of::<ScalarRecord>()
        );
    }

    #[test]
    fn deterministic_reservation_failure_uses_actual_mapping_helper_without_oom() {
        let mut values = Vec::<ScopeRecord>::new();
        let error = reserve_vec_with(&mut values, 1, |_, _| Err(())).unwrap_err();
        assert_eq!(error, SourceAnalysisInputError::Allocation);
        assert_eq!(values.capacity(), 0);
        let mut text = String::new();
        let error = reserve_string_with(&mut text, 1, |_, _| Err(())).unwrap_err();
        assert_eq!(error, SourceAnalysisInputError::Allocation);
        assert_eq!(text.capacity(), 0);
    }

    #[test]
    fn dynamic_member_count_is_prepaid_before_actual_presentation_iteration() {
        let mut ledger = Ledger::qualification(100);
        let mut visits = 0;
        let count = count_presentation_members(
            &mut ledger,
            0,
            SourceCasingPresentation::Replacement(&['S', 'S']),
            |prepaid, c| {
                assert_eq!(prepaid, 2);
                visits += 1;
                assert_eq!(c, 'S');
                Ok(())
            },
        )
        .unwrap();
        assert_eq!((count, visits, ledger.work()), (2, 2, 2));
        let mut short = Ledger::qualification(1);
        let mut visits = 0;
        assert!(matches!(
            count_presentation_members(
                &mut short,
                0,
                SourceCasingPresentation::Replacement(&['S', 'S']),
                |_, _| {
                    visits += 1;
                    Ok(())
                }
            ),
            Err(SourceAnalysisInputError::Work)
        ));
        assert_eq!(visits, 0);
    }

    #[test]
    fn exclusive_scope_finalization_and_nonconsuming_validation() {
        let mut g = ScopeGeneration::qualification(3, 0, 0, 0);
        g.append_scope(0, SourceAnalysisParticipation::InlineSequence, 0, 0, 0)
            .unwrap();
        assert!(matches!(
            g.append_scope(1, SourceAnalysisParticipation::Empty, 0, 0, 0),
            Err(SourceAnalysisInputInvariantReason::ScopeFinalization)
        ));
        g.finalize_scope(0, 0, 0, 0).unwrap();
        assert!(matches!(
            g.finalize_scope(0, 0, 0, 0),
            Err(SourceAnalysisInputInvariantReason::ScopeFinalization)
        ));
        g.append_scope(1, SourceAnalysisParticipation::Empty, 0, 0, 0)
            .unwrap();
        assert!(matches!(
            g.finalize_scope(0, 0, 0, 0),
            Err(SourceAnalysisInputInvariantReason::ScopeFinalization)
        ));
        g.finalize_scope(1, 0, 0, 0).unwrap();
        g.append_scope(2, SourceAnalysisParticipation::InlineSequence, 0, 0, 0)
            .unwrap();
        assert!(matches!(
            g.validate_completion(),
            Err(SourceAnalysisInputInvariantReason::ScopeFinalization)
        ));
        g.finalize_scope(2, 0, 0, 0).unwrap();
        assert!(g.validate_completion().is_ok());
        let completed = g.finish().unwrap();
        assert_eq!(completed.scopes.len(), 3);
    }

    #[test]
    fn completion_and_emission_bounds_use_actual_generation_state() {
        let mut g = ScopeGeneration::qualification(1, 0, 0, 0);
        g.append_scope(0, SourceAnalysisParticipation::InlineSequence, 0, 0, 0)
            .unwrap();
        assert!(matches!(
            g.append_character('x'),
            Err(SourceAnalysisInputInvariantReason::EmissionCounts)
        ));
        g.finalize_scope(0, 0, 0, 0).unwrap();
        assert!(g.validate_completion().is_ok());
        let mut wrong = ScopeGeneration::qualification(1, 1, 1, 1);
        wrong
            .append_scope(0, SourceAnalysisParticipation::InlineSequence, 0, 0, 0)
            .unwrap();
        wrong.finalize_scope(0, 0, 0, 0).unwrap();
        assert!(matches!(
            wrong.validate_completion(),
            Err(SourceAnalysisInputInvariantReason::Completion)
        ));
    }

    #[test]
    fn query_counter_wraps_actual_public_query_helpers() {
        with_large_query_fixture!(|input, inline_scope, payload| {
            assert_eq!(
                (
                    input.scope_count(),
                    input.domain_count(),
                    input.contribution_count()
                ),
                (5, 4, 512)
            );
            query_instrumentation_reset();
            let scope = input.scope(inline_scope).unwrap();
            assert_eq!(inline_scope, 1);
            let domain = scope.inline_sequence().unwrap();
            assert_eq!(domain.scalar_count(), 128);
            let _ = domain.origin(127).unwrap();
            let contribution = input.contribution(511).unwrap();
            let _ = contribution.scope();
            assert_eq!(payload, 3);
            let _ = input.payload_projection(payload).unwrap();
            let q = query_instrumentation_read();
            eprintln!("actual query comparisons: {q:?}");
            assert!(q.scalar_partition_comparisons > 0 && q.scope_partition_comparisons > 0);
            assert!(
                q.scalar_partition_comparisons <= q.scalar_binary_search_ceiling
                    && q.scope_partition_comparisons <= 2 * q.scope_binary_search_ceiling
            );
        });
    }

    #[test]
    fn actual_work_phase_trace_precedes_reservation() {
        with_separating_text_predecessors!(|structure, casing| {
            let trace = qualification_work_trace(&structure, &casing, 32).unwrap_err();
            assert_eq!(trace.error, SourceAnalysisInputError::Work);
            assert_eq!(
                trace.phases,
                [
                    "setup:1",
                    "eligible-normal-prepaid:5",
                    "records-prepaid:11",
                    "original-visit:1",
                    "members-prepaid:1",
                    "first-pass-complete",
                    "generation-prepaid:14"
                ]
            );
            eprintln!("actual work failure: {trace:?}");
            assert_eq!(trace.work_before_failed_phase, 19);
            assert_eq!(trace.reservation_calls, (0, 0, 0));
        });
    }

    #[test]
    fn original_rows_include_zero_output_and_group_expansion_members() {
        let mut g = ScopeGeneration::qualification(1, 2, 2, 2);
        g.append_scope(0, SourceAnalysisParticipation::InlineSequence, 0, 0, 0)
            .unwrap();
        g.append_original(3, 0).unwrap();
        g.append_character('S').unwrap();
        g.append_character('S').unwrap();
        g.append_original(3, 1).unwrap();
        g.finalize_scope(0, 2, 2, 2).unwrap();
        let completed = g.finish().unwrap();
        assert_eq!(completed.scalars.len(), 2);
        assert_eq!(completed.transport, "SS");
    }

    #[test]
    fn clearing_pending_cannot_forge_a_finalized_scope() {
        let mut g = ScopeGeneration::qualification(1, 0, 0, 0);
        g.append_scope(0, SourceAnalysisParticipation::InlineSequence, 0, 0, 0)
            .unwrap();
        g.pending_scope = None; // cfg(test) child-module corruption of the real private field.
        assert!(matches!(
            g.validate_completion(),
            Err(SourceAnalysisInputInvariantReason::ScopeFinalization)
        ));
    }
    #[test]
    fn corrupted_finalization_counter_rejects_close_and_publication() {
        let mut g = ScopeGeneration::qualification(1, 0, 0, 0);
        g.append_scope(0, SourceAnalysisParticipation::Empty, 0, 0, 0)
            .unwrap();
        g.finalized_scopes = 1;
        assert_eq!(
            g.finalize_scope(0, 0, 0, 0),
            Err(SourceAnalysisInputInvariantReason::ScopeFinalization)
        );
        assert_eq!(
            g.validate_completion(),
            Err(SourceAnalysisInputInvariantReason::ScopeFinalization)
        );
        let mut g = ScopeGeneration::qualification(1, 0, 0, 0);
        g.append_scope(0, SourceAnalysisParticipation::Empty, 0, 0, 0)
            .unwrap();
        g.finalize_scope(0, 0, 0, 0).unwrap();
        g.finalized_scopes = 0;
        assert_eq!(
            g.validate_completion(),
            Err(SourceAnalysisInputInvariantReason::ScopeFinalization)
        );
    }
}
