// An inline interrupted by N blocks. Writing Modes §2.4.4 closes the affected box
// before each paragraph boundary and reopens it after, so N boundaries apply the same
// adopted rule N times; CSS 2.2 §9.10 supplies the characters. `view_owner`, `block`
// and `origin_view` come from block_embed.rs / block_override.rs, included earlier.

/// Outer block > affected inline > [text, block, text, block, …, text] with `n` blocks
/// and `n + 1` nonempty, break-free text fragments.
fn interruption_chain(n: usize, direction: &str) -> InputNode {
    let letter = |i: usize| ((b'a' + u8::try_from(i).unwrap()) as char).to_string();
    let mut children = vec![text("T0", &letter(0), "pre", "none", "default", "normal")];
    for i in 0..n {
        children.push(block(
            &format!("K{i}"),
            "ltr",
            "normal",
            vec![text(
                &format!("X{i}"),
                "x",
                "pre",
                "none",
                "default",
                "normal",
            )],
        ));
        children.push(text(
            &format!("T{}", i + 1),
            &letter(i + 1),
            "pre",
            "none",
            "default",
            "normal",
        ));
    }
    block(
        "B",
        "ltr",
        "normal",
        vec![managed(
            "I",
            InputFamily::Row,
            "inline",
            direction,
            "embed",
            children,
        )],
    )
}
#[test]
fn multiple_interruptions_follow_the_derived_closed_forms() {
    for direction in ["ltr", "rtl"] {
        let open = if direction == "ltr" {
            '\u{202a}'
        } else {
            '\u{202b}'
        };
        for n in 1..=3usize {
            let root = interruption_chain(n, direction);
            with_predecessors!(root, |_w, casing, structure| {
                assert_eq!(structure.record_count(), 7 * n + 5);
                assert_eq!(structure.source_count(), 3 * n + 3);
                let owner =
                    build_source_control_input(&structure, &casing, control_limits())
                        .unwrap_or_else(|e| panic!("n={n} rejected: {e:?}"));
                let r = owner.receipt();
                // Closed forms derived from the observed seven-record period; at n = 1
                // they reproduce the accepted single-interruption witness exactly.
                assert_eq!(r.records, 7 * n + 5);
                assert_eq!(r.sources, 3 * n + 3);
                assert_eq!(r.payloads, 2 * n + 1);
                assert_eq!(r.frames_pushed, 5 * n + 4);
                assert_eq!(r.synthetic_scalars, 2 * n + 2);
                assert_eq!(r.literal_scalars, 2 * n + 1);
                assert_eq!(r.domains, 2 * n + 1);
                assert_eq!(r.scopes, 2 * n + 2);
                assert_eq!(r.frame_visits_at_breaks, 0);
                assert_eq!(r.work, 46 * n + 37, "work = 46N + 37");
                // Domains alternate: a wrapped fragment, then the block it was cut by.
                let texts: Vec<String> = (0..owner.scope_count())
                    .filter_map(|i| {
                        owner
                            .scope(i)
                            .unwrap()
                            .inline_sequence()
                            .map(|d| d.text().to_string())
                    })
                    .collect();
                let mut expected = Vec::new();
                for i in 0..=n {
                    let letter = (b'a' + u8::try_from(i).unwrap()) as char;
                    expected.push(format!("{open}{letter}\u{202c}"));
                    if i < n {
                        expected.push("x".to_string());
                    }
                }
                assert_eq!(texts, expected);
                // Every fragment domain is anonymous and inherits the outer block's
                // base; every interrupting block owns its own.
                let outer = owner.scope(0).unwrap();
                for index in 1..owner.scope_count() {
                    let scope = owner.scope(index).unwrap();
                    let anonymous = index % 2 == 1;
                    assert_eq!(
                        matches!(
                            scope.base_origin(),
                            SourceControlBaseOrigin::AnonymousInherited(_)
                        ),
                        anonymous,
                        "scope {index}"
                    );
                    if anonymous {
                        assert_eq!(
                            scope.base_origin().participant().source_index(),
                            outer.base_origin().participant().source_index()
                        );
                    }
                    assert_eq!(scope.base_origin().direction().as_str(), "ltr");
                }
            });
        }
    }
}
#[test]
fn every_interior_fragment_is_cut_on_both_sides_by_its_own_blocks() {
    // Each fragment's interruption edge shares the neighbouring block's event while its
    // cut differs by side, which is the anchor rule the single-interruption slice fixed.
    let n = 3usize;
    let root = interruption_chain(n, "rtl");
    with_predecessors!(root, |_w, casing, structure| {
        let owner = build_source_control_input(&structure, &casing, control_limits()).unwrap();
        let (_, scopes) = view_owner(&owner);
        let fragment_domains: Vec<&ScopeView> = scopes
            .iter()
            .filter(|s| s.text.as_deref().is_some_and(|t| t.starts_with('\u{202b}')))
            .collect();
        assert_eq!(fragment_domains.len(), n + 1);
        for (i, domain) in fragment_domains.iter().enumerate() {
            let opening = &domain.tokens[0].1;
            let closing = &domain.tokens[2].1;
            // Occurrences restart in every anonymous domain.
            assert!(opening.contains("occurrence=0"), "{opening}");
            assert!(closing.contains("occurrence=1"), "{closing}");
            // One affected element across every fragment: compare against fragment 0,
            // not against this fragment's own closing, which shares a frame trivially.
            let cause = |origin: &str| {
                origin
                    .split_once("cause=")
                    .unwrap()
                    .1
                    .split_once(' ')
                    .unwrap()
                    .0
                    .to_string()
            };
            let first = cause(&fragment_domains[0].tokens[0].1);
            assert_eq!(cause(opening), first, "fragment {i} opening cause");
            assert_eq!(cause(closing), first, "fragment {i} closing cause");
            let start_kind = if i == 0 {
                "kind=OriginalBegin"
            } else {
                "kind=AfterInterruption"
            };
            let end_kind = if i == n {
                "kind=OriginalEnd"
            } else {
                "kind=BeforeInterruption"
            };
            assert!(opening.contains(start_kind), "fragment {i}: {opening}");
            assert!(closing.contains(end_kind), "fragment {i}: {closing}");
            // A Start edge cuts one past its event; an End edge cuts at it.
            let value = |origin: &str, name: &str| -> usize {
                origin
                    .split_once(&format!("{name}="))
                    .unwrap()
                    .1
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            };
            assert_eq!(value(opening, "cut"), value(opening, "event") + 1);
            assert_eq!(value(closing, "cut"), value(closing, "event"));
        }
    });
}
#[test]
fn adjacent_blocks_without_a_fragment_between_them_reject() {
    // Two interrupting blocks with no content between them would need an empty
    // continuation, which no adopted authority resolves; it stays rejected.
    let root = block(
        "B",
        "ltr",
        "normal",
        vec![managed(
            "I",
            InputFamily::Row,
            "inline",
            "rtl",
            "embed",
            vec![
                text("A", "a", "pre", "none", "default", "normal"),
                block(
                    "K1",
                    "ltr",
                    "normal",
                    vec![text("X", "x", "pre", "none", "default", "normal")],
                ),
                block(
                    "K2",
                    "ltr",
                    "normal",
                    vec![text("Y", "y", "pre", "none", "default", "normal")],
                ),
                text("C", "c", "pre", "none", "default", "normal"),
            ],
        )],
    );
    with_predecessors!(root, |_w, casing, structure| {
        let error = build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
        assert!(matches!(
            error.kind,
            SourceControlInputErrorKind::Unsupported(SourceControlUnsupported::InterruptedInline(
                SourceStructureEdgeKind::BeforeInterruption
            ))
        ));
    });
}
#[test]
fn a_broken_middle_fragment_rejects() {
    // An *empty* middle fragment never reaches this producer: the structure builder
    // rejects that capture first with `UnresolvedEmptyInlineGap` (observed and retained
    // in docs/tmp/implementation-source-control-multiple-interruption). Only a fragment
    // carrying a presented forced break reaches the control producer.
    {
        let middle = "m\nm";

        let root = block(
            "B",
            "ltr",
            "normal",
            vec![managed(
                "I",
                InputFamily::Row,
                "inline",
                "rtl",
                "embed",
                vec![
                    text("A", "a", "pre", "none", "default", "normal"),
                    block(
                        "K1",
                        "ltr",
                        "normal",
                        vec![text("X", "x", "pre", "none", "default", "normal")],
                    ),
                    text("M", middle, "pre", "none", "default", "normal"),
                    block(
                        "K2",
                        "ltr",
                        "normal",
                        vec![text("Y", "y", "pre", "none", "default", "normal")],
                    ),
                    text("C", "c", "pre", "none", "default", "normal"),
                ],
            )],
        );
        with_predecessors!(root, |_w, casing, structure| {
            // The capture is a valid two-interruption shape, so a rejection here can
            // only come from the presented break, not from a shape mismatch.
            assert_eq!(structure.record_count(), 19);
            assert_eq!(structure.source_count(), 9);
            let error =
                build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
            assert!(
                matches!(
                    error.kind,
                    SourceControlInputErrorKind::Unsupported(
                        SourceControlUnsupported::InterruptedInline(
                            SourceStructureEdgeKind::BeforeInterruption
                        )
                    )
                ),
                "middle {middle:?}: {error:?}"
            );
        });
    }
}
#[test]
fn an_empty_terminal_fragment_is_not_admitted() {
    // Observed, correcting the prediction that an empty fragment would drop records: an
    // empty text still contributes its inline and payload records, so the capture keeps
    // the valid two-interruption shape and the rejection comes from the profile's
    // nonemptiness rule in the counted member loop, not from the shape gate.
    for empty_first in [true, false] {
        let mut children = vec![text(
            "A",
            if empty_first { "" } else { "a" },
            "pre",
            "none",
            "default",
            "normal",
        )];
        children.push(block(
            "K1",
            "ltr",
            "normal",
            vec![text("X", "x", "pre", "none", "default", "normal")],
        ));
        children.push(text("M", "m", "pre", "none", "default", "normal"));
        children.push(block(
            "K2",
            "ltr",
            "normal",
            vec![text("Y", "y", "pre", "none", "default", "normal")],
        ));
        children.push(text(
            "C",
            if empty_first { "c" } else { "" },
            "pre",
            "none",
            "default",
            "normal",
        ));
        let root = block(
            "B",
            "ltr",
            "normal",
            vec![managed(
                "I",
                InputFamily::Row,
                "inline",
                "rtl",
                "embed",
                children,
            )],
        );
        with_predecessors!(root, |_w, casing, structure| {
            assert_eq!(structure.record_count(), 19);
            assert_eq!(structure.source_count(), 9);
            let error =
                build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
            assert!(
                matches!(
                    error.kind,
                    SourceControlInputErrorKind::Unsupported(
                        SourceControlUnsupported::InterruptedInline(
                            SourceStructureEdgeKind::BeforeInterruption
                        )
                    )
                ),
                "empty_first={empty_first}: {error:?}"
            );
        });
    }
}
