// Block `unicode-bidi: bidi-override` (CSS 2.2 §9.10) overrides inline-level
// descendants not within another block container. This module qualifies only the
// shapes that prose fully determines: no anonymous child block box, and no forced
// break inside the overridden domain. `view_owner`, `origin_view` and `block` come
// from block_embed.rs, which is included before this file.
use iced_css_core::values::bidi::ComputedUnicodeBidi;
type OwnerView = (SourceControlInputReceipt, Vec<ScopeView>);
/// Builds the Normal and the bidi-override variant of one recipe, with the number of
/// block participants that really compute `bidi-override` in the second structure.
fn override_pair(recipe: impl Fn(&str) -> InputNode) -> (OwnerView, OwnerView, usize) {
    let normal = recipe("normal");
    let overridden = recipe("bidi-override");
    let mut views = Vec::new();
    with_predecessors!(normal, |_nw, normal_casing, normal_structure| {
        views.push(view_owner(
            &build_source_control_input(&normal_structure, &normal_casing, control_limits())
                .unwrap(),
        ));
    });
    let mut overridden_blocks = Vec::new();
    with_predecessors!(overridden, |_ow, casing, structure| {
        // Guards every "emits nothing" case against passing because the value was
        // never applied to the block in the first place.
        overridden_blocks.push(
            structure
                .records()
                .filter(|r| r.kind() == SourceStructureKind::SourceBlock)
                .filter(|r| {
                    r.participant().unwrap().bidi_style().unicode_bidi()
                        == ComputedUnicodeBidi::BidiOverride
                })
                .count(),
        );
        views.push(view_owner(
            &build_source_control_input(&structure, &casing, control_limits())
                .unwrap_or_else(|e| panic!("override block rejected: {e:?}")),
        ));
    });
    let mut views = views.into_iter();
    let expected = views.next().expect("normal owner");
    let actual = views.next().expect("override owner");
    (
        expected,
        actual,
        overridden_blocks.pop().expect("override structure visited"),
    )
}
/// A block override that emits nothing must leave the whole owner untouched.
fn override_block_emits_nothing(recipe: impl Fn(&str) -> InputNode, overridden_blocks: usize) {
    let (expected, actual, observed) = override_pair(recipe);
    assert_eq!(observed, overridden_blocks, "the value must really be applied");
    assert_eq!(actual, expected);
}
/// The smallest work allowance at which the builder gets past every work debit and
/// reaches its semantic rejection; below it the Work limit wins. Pins the debit point.
macro_rules! rejection_work_threshold {
    ($structure:expr, $casing:expr) => {
        (1..=400usize)
            .find(|&work| {
                let mut limits = control_limits();
                limits.work = nz(work);
                !matches!(
                    build_source_control_input($structure, $casing, limits)
                        .unwrap_err()
                        .kind,
                    SourceControlInputErrorKind::Limit(SourceControlLimit::Work)
                )
            })
            .expect("a semantic rejection within the scanned work range")
    };
}
fn opening_for(direction: &str) -> char {
    match direction {
        "ltr" => '\u{202d}',
        _ => '\u{202e}',
    }
}
#[test]
fn block_override_wraps_its_own_inline_sequence() {
    for direction in ["ltr", "rtl"] {
        let (expected, actual, _) = override_pair(|bidi| {
            block(
                "B",
                direction,
                bidi,
                vec![text("T", "a", "pre", "none", "default", "normal")],
            )
        });
        let (expected_receipt, expected_scopes) = expected;
        let (receipt, scopes) = actual;
        // Exactly one opening and one PDF are added, and nothing else changes. Work
        // grows by four, not two: each synthetic costs one tick and the ledger charges
        // the walk's units in the count pass and again as replay prepayment.
        assert_eq!(receipt.work, expected_receipt.work + 4);
        assert_eq!(
            receipt.synthetic_scalars,
            expected_receipt.synthetic_scalars + 2
        );
        assert_eq!(receipt.output_scalars, expected_receipt.output_scalars + 2);
        assert_eq!(receipt.output_bytes, expected_receipt.output_bytes + 6);
        assert_eq!(receipt.literal_scalars, expected_receipt.literal_scalars);
        assert_eq!(receipt.original_scalars, expected_receipt.original_scalars);
        assert_eq!(receipt.payloads, expected_receipt.payloads);
        assert_eq!(receipt.scopes, expected_receipt.scopes);
        assert_eq!(receipt.domains, expected_receipt.domains);
        assert_eq!(receipt.frames_pushed, expected_receipt.frames_pushed);
        assert_eq!(
            receipt.frame_visits_at_breaks,
            expected_receipt.frame_visits_at_breaks
        );
        assert_eq!(scopes.len(), expected_scopes.len());
        let scope = &scopes[0];
        let expected_scope = &expected_scopes[0];
        // The base still comes from the block's own direction; the override raises
        // the explicit level inside the domain rather than replacing the base.
        assert_eq!(scope.base_direction, direction);
        assert_eq!(scope.base_direction, expected_scope.base_direction);
        assert_eq!(scope.base_source, expected_scope.base_source);
        assert_eq!(scope.participation, expected_scope.participation);
        let open = opening_for(direction);
        assert_eq!(scope.text.as_deref(), Some(&*format!("{open}a\u{202c}")));
        assert_eq!(expected_scope.text.as_deref(), Some("a"));
        // The cause is the block's own record and source, at its original edges.
        let record = scope.record;
        let source = scope.base_source;
        assert_eq!(
            scope.tokens[0],
            (
                open,
                format!(
                    "Synthetic occurrence=0 cause={source} fragment={record} \
                     direction={direction} bidi=bidi-override \
                     Edge record={record} side=Start kind=OriginalBegin \
                     event=0 event_source={source} cut=1"
                )
            )
        );
        assert_eq!(scope.tokens[1].0, 'a');
        assert_eq!(scope.tokens[1].1, expected_scope.tokens[0].1);
        assert_eq!(
            scope.tokens[2],
            (
                '\u{202c}',
                format!(
                    "Synthetic occurrence=1 cause={source} fragment={record} \
                     direction={direction} bidi=bidi-override \
                     Edge record={record} side=End kind=OriginalEnd \
                     event=4 event_source={source} cut=4"
                )
            )
        );
    }
}
#[test]
fn block_override_opens_outside_a_nested_inline_cause() {
    for direction in ["ltr", "rtl"] {
        let (_, actual, _) = override_pair(|bidi| {
            block(
                "B",
                direction,
                bidi,
                vec![managed(
                    "I",
                    InputFamily::Row,
                    "inline",
                    "rtl",
                    "embed",
                    vec![text("T", "a", "pre", "none", "default", "normal")],
                )],
            )
        });
        let (receipt, scopes) = actual;
        let open = opening_for(direction);
        // Outer opens first and closes last: nesting order, not presentation order.
        assert_eq!(
            scopes[0].text.as_deref(),
            Some(&*format!("{open}\u{202b}a\u{202c}\u{202c}"))
        );
        assert_eq!(receipt.synthetic_scalars, 4);
        let field = |origin: &str, name: &str| {
            origin
                .split_once(&format!("{name}="))
                .unwrap_or_else(|| panic!("{name} in {origin}"))
                .1
                .split_once(' ')
                .map_or_else(
                    || origin.rsplit('=').next().unwrap().to_string(),
                    |(value, _)| value.to_string(),
                )
        };
        let synthetics: Vec<&String> = scopes[0]
            .tokens
            .iter()
            .filter(|(_, origin)| origin.starts_with("Synthetic"))
            .map(|(_, origin)| origin)
            .collect();
        assert_eq!(synthetics.len(), 4);
        let causes: Vec<_> = synthetics.iter().map(|o| field(o, "cause")).collect();
        let fragments: Vec<_> = synthetics.iter().map(|o| field(o, "fragment")).collect();
        let occurrences: Vec<_> = synthetics.iter().map(|o| field(o, "occurrence")).collect();
        assert_eq!(occurrences, ["0", "1", "2", "3"]);
        // The outer pair belongs to the block and the inner pair to the inline: a
        // swapped attribution with the same characters must not pass.
        assert_eq!(causes[0], causes[3], "outer opening and closing share a cause");
        assert_eq!(fragments[0], fragments[3]);
        assert_eq!(causes[1], causes[2], "inner opening and closing share a cause");
        assert_eq!(fragments[1], fragments[2]);
        assert_ne!(causes[0], causes[1], "block and inline causes are distinct");
        assert!(synthetics[0].contains("bidi=bidi-override"));
        assert!(synthetics[0].contains("side=Start kind=OriginalBegin"));
        assert!(synthetics[1].contains("bidi=embed"));
        assert!(synthetics[3].contains("bidi=bidi-override"));
        assert!(synthetics[3].contains("side=End kind=OriginalEnd"));
    }
}
#[test]
fn block_override_with_only_block_children_emits_nothing() {
    for direction in ["ltr", "rtl"] {
        let recipe = |bidi: &str| {
            block(
                "B",
                direction,
                bidi,
                vec![
                    block(
                        "K1",
                        "ltr",
                        "normal",
                        vec![text("X", "x", "pre", "none", "default", "normal")],
                    ),
                    block(
                        "K2",
                        "rtl",
                        "normal",
                        vec![text("Y", "y", "pre", "none", "default", "normal")],
                    ),
                ],
            )
        };
        override_block_emits_nothing(recipe, 1);
    }
}
#[test]
fn nested_override_blocks_emit_nothing_for_the_excluded_outer_block() {
    // The inner block is another block container, so the outer override covers none
    // of its content; the inner one overrides only its own inline sequence.
    for direction in ["ltr", "rtl"] {
        let (expected, actual, observed) = override_pair(|bidi| {
            block(
                "B",
                direction,
                bidi,
                vec![block(
                    "K",
                    "rtl",
                    "normal",
                    vec![text("X", "x", "pre", "none", "default", "normal")],
                )],
            )
        });
        assert_eq!(observed, 1);
        assert_eq!(actual, expected, "the outer override covers nothing");
    }
}
#[test]
fn empty_block_override_emits_nothing() {
    for direction in ["ltr", "rtl"] {
        override_block_emits_nothing(|bidi| block("B", direction, bidi, vec![]), 1);
    }
}
#[test]
fn block_override_is_admitted_as_the_interruption_middle_block() {
    for direction in ["ltr", "rtl"] {
        let recipe = |bidi: &str| {
            block(
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
                            "K",
                            direction,
                            bidi,
                            vec![text("X", "x", "pre", "none", "default", "normal")],
                        ),
                        text("C", "b", "pre", "none", "default", "normal"),
                    ],
                )],
            )
        };
        let (expected, actual, _) = override_pair(recipe);
        assert_eq!(expected.0.work, 83);
        assert_eq!(actual.0.work, 87, "the middle domain gains one pair, charged twice");
        let open = opening_for(direction);
        // Only K's own domain is overridden; the outer anonymous domains are unchanged.
        assert_eq!(
            actual
                .1
                .iter()
                .filter_map(|s| s.text.clone())
                .collect::<Vec<_>>(),
            [
                "\u{202b}a\u{202c}".to_string(),
                format!("{open}x\u{202c}"),
                "\u{202b}b\u{202c}".to_string(),
            ]
        );
        assert_eq!(
            expected
                .1
                .iter()
                .filter_map(|s| s.text.clone())
                .collect::<Vec<_>>(),
            ["\u{202b}a\u{202c}", "x", "\u{202b}b\u{202c}"]
        );
    }
}
#[test]
fn block_override_with_anonymous_children_rejects_at_the_anonymous_record() {
    // The outer block of the interruption graph owns anonymous child block boxes;
    // CSS 2.2 §9.10 places an opening at each but enumerates no closing schedule.
    for direction in ["ltr", "rtl"] {
        let root = block(
            "B",
            direction,
            "bidi-override",
            vec![managed(
                "I",
                InputFamily::Row,
                "inline",
                "rtl",
                "embed",
                vec![
                    text("A", "a", "pre", "none", "default", "normal"),
                    block(
                        "K",
                        "ltr",
                        "normal",
                        vec![text("X", "x", "pre", "none", "default", "normal")],
                    ),
                    text("C", "b", "pre", "none", "default", "normal"),
                ],
            )],
        );
        with_predecessors!(root, |_w, casing, structure| {
            let anonymous = structure
                .records()
                .find(|r| r.kind() == SourceStructureKind::AnonymousBlock)
                .unwrap();
            let enclosing = anonymous.anonymous_origin().unwrap().enclosing_block();
            let error =
                build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
            assert!(matches!(
                error.kind,
                SourceControlInputErrorKind::Unsupported(SourceControlUnsupported::BlockBidi(
                    ComputedUnicodeBidi::BidiOverride
                ))
            ));
            assert_eq!(
                (error.source, error.record, error.original, error.member),
                (
                    enclosing.source_index(),
                    Some(anonymous.index()),
                    None,
                    None
                )
            );
            // Debit point: 1 setup + 6 sources + one tick each for the outer block
            // and for its first anonymous child, which is where the rejection fires.
            assert_eq!(rejection_work_threshold!(&structure, &casing), 9);
        });
    }
}
#[test]
fn block_override_across_a_forced_break_rejects_at_the_break() {
    // Writing Modes §2.4.4's close/reopen rule is scoped to inline boxes; no adopted
    // authority states whether a block container's override restarts at a break.
    for direction in ["ltr", "rtl"] {
        let root = block(
            "B",
            direction,
            "bidi-override",
            vec![text("T", "a\nb", "pre", "none", "default", "normal")],
        );
        with_predecessors!(root, |_w, casing, structure| {
            let error =
                build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
            assert!(matches!(
                error.kind,
                SourceControlInputErrorKind::Unsupported(SourceControlUnsupported::BlockBidi(
                    ComputedUnicodeBidi::BidiOverride
                ))
            ));
            let block_record = structure
                .records()
                .find(|r| r.kind() == SourceStructureKind::SourceBlock)
                .unwrap();
            // Reported against the overridden block itself, at the presented break
            // member inside its domain.
            assert_eq!(
                (error.source, error.record, error.original, error.member),
                (
                    block_record.source_index(),
                    Some(block_record.index()),
                    Some(1),
                    Some(0)
                )
            );
            // Debit point: 1 setup + 2 sources + block record + its opening + inline
            // record + payload record + originals 0 and 1 with their first members.
            assert_eq!(rejection_work_threshold!(&structure, &casing), 11);
        });
    }
}
#[test]
fn a_leading_break_in_an_override_block_keeps_the_existing_boundary_rejection() {
    // The pre-existing break-boundary rejections precede the block-override guard, so
    // they keep their precedence; no renewal escapes on this path either.
    for direction in ["ltr", "rtl"] {
        let root = block(
            direction,
            direction,
            "bidi-override",
            vec![text("T", "\na", "pre", "none", "default", "normal")],
        );
        with_predecessors!(root, |_w, casing, structure| {
            let error =
                build_source_control_input(&structure, &casing, control_limits()).unwrap_err();
            assert!(matches!(
                error.kind,
                SourceControlInputErrorKind::Unsupported(SourceControlUnsupported::BreakBoundary(
                    SourceControlBreakSide::Before
                ))
            ));
        });
    }
}
