// Block `unicode-bidi: embed` has no inline embedding effect (CSS 2.2 §9.10); it
// is admitted exactly like a Normal block while its computed value stays visible.
#[derive(Debug, PartialEq, Eq)]
struct ScopeView {
    record: usize,
    participation: SourceAnalysisParticipation,
    base_direction: &'static str,
    base_source: usize,
    original_range: std::ops::Range<usize>,
    payload_range: std::ops::Range<usize>,
    text: Option<String>,
    tokens: Vec<(char, String)>,
}
/// Explicit projection of every origin accessor; the Debug form of a synthetic
/// origin carries only its owner and index, so it is not a comparison oracle.
fn origin_view(token: SourceControlToken<'_, '_, '_, '_, '_, '_, '_, '_, '_, '_>) -> String {
    match token.origin() {
        SourceControlOrigin::Literal(literal) => {
            let scalar = literal.contribution().whitespace_scalar();
            format!(
                "Literal original={} member={} payload={} source={} whitespace={} bytes={:?}",
                literal.original_index(),
                literal.member_index(),
                literal.payload().record().index(),
                scalar.participant().source_index(),
                scalar.index(),
                scalar.source_range()
            )
        }
        SourceControlOrigin::Synthetic(synthetic) => {
            let cause = synthetic.cause();
            let style = cause.bidi_style();
            let placement = match synthetic.placement() {
                SourceControlPlacement::FragmentEdge(edge) => format!(
                    "Edge record={} side={:?} kind={:?} event={} event_source={} cut={}",
                    edge.record().index(),
                    edge.side(),
                    edge.kind(),
                    edge.event().index(),
                    edge.event().participant().source_index(),
                    edge.cut()
                ),
                SourceControlPlacement::Break { anchor, side } => format!(
                    "Break payload={} original={} member={} side={:?}",
                    anchor.payload().record().index(),
                    anchor.original_index(),
                    anchor.member_index(),
                    side
                ),
            };
            format!(
                "Synthetic occurrence={} cause={} fragment={} direction={} bidi={} {placement}",
                synthetic.occurrence_index(),
                cause.source_index(),
                synthetic.cause_fragment().index(),
                style.direction().as_str(),
                style.unicode_bidi().as_str()
            )
        }
    }
}
fn view_owner(
    owner: &SourceControlInput<'_, '_, '_, '_, '_, '_, '_, '_, '_>,
) -> (SourceControlInputReceipt, Vec<ScopeView>) {
    let scopes = (0..owner.scope_count())
        .map(|index| {
            let scope = owner.scope(index).unwrap();
            let base = scope.base_origin();
            let domain = scope.inline_sequence();
            ScopeView {
                record: scope.record().index(),
                participation: scope.participation(),
                base_direction: base.direction().as_str(),
                base_source: base.participant().source_index(),
                original_range: scope.original_range(),
                payload_range: scope.payload_range(),
                text: domain.as_ref().map(|d| d.text().to_string()),
                tokens: domain
                    .as_ref()
                    .map(|d| {
                        (0..d.scalar_count())
                            .map(|i| {
                                let token = d.token(i).unwrap();
                                (token.character(), origin_view(token))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect();
    (owner.receipt(), scopes)
}
/// Builds the Normal and Embed variants of one recipe and requires identical
/// owners; `embedded_blocks` is how many block participants must still read `embed`.
fn embed_block_matches_normal(
    recipe: impl Fn(&str) -> InputNode,
    embedded_blocks: usize,
    expected_work: Option<usize>,
) {
    let normal = recipe("normal");
    let embed = recipe("embed");
    with_predecessors!(normal, |_nw, normal_casing, normal_structure| {
        let expected = view_owner(
            &build_source_control_input(&normal_structure, &normal_casing, control_limits())
                .unwrap(),
        );
        with_predecessors!(embed, |_ew, casing, structure| {
            let owner = build_source_control_input(&structure, &casing, control_limits())
                .unwrap_or_else(|e| panic!("embed block rejected: {e:?}"));
            let actual = view_owner(&owner);
            assert_eq!(actual, expected);
            if let Some(work) = expected_work {
                assert_eq!(owner.receipt().work, work);
            }
            // The real computed value is retained in projections, not rewritten.
            let embedded: Vec<_> = structure
                .records()
                .filter(|r| r.kind() == SourceStructureKind::SourceBlock)
                .filter(|r| r.participant().unwrap().bidi_style().unicode_bidi().as_str() == "embed")
                .collect();
            assert_eq!(embedded.len(), embedded_blocks);
            for scope in (0..owner.scope_count()).map(|i| owner.scope(i).unwrap()) {
                if let SourceControlBaseOrigin::OwnBlock(_) = scope.base_origin()
                    && embedded.iter().any(|r| r.index() == scope.record().index())
                {
                    assert_eq!(
                        scope.base_origin().participant().bidi_style().unicode_bidi().as_str(),
                        "embed"
                    );
                }
            }
        });
    });
}
fn block(label: &str, direction: &str, bidi: &str, children: Vec<InputNode>) -> InputNode {
    managed(label, InputFamily::Column, "block", direction, bidi, children)
}
#[test]
fn block_embed_root_inline_sequence_matches_normal_in_both_directions() {
    for direction in ["ltr", "rtl"] {
        for value in ["ab", "a\nb"] {
            embed_block_matches_normal(
                |bidi| block("B", direction, bidi, vec![text("T", value, "pre", "none", "default", "normal")]),
                1,
                None,
            );
        }
    }
}
#[test]
fn block_embed_keeps_inline_embed_controls_distinct() {
    for direction in ["ltr", "rtl"] {
        for (inline_direction, inline_bidi) in [("rtl", "embed"), ("ltr", "bidi-override")] {
            embed_block_matches_normal(
                |bidi| {
                    block(
                        "B",
                        direction,
                        bidi,
                        vec![managed(
                            "I",
                            InputFamily::Row,
                            "inline",
                            inline_direction,
                            inline_bidi,
                            vec![text("T", "a", "pre", "none", "default", "normal")],
                        )],
                    )
                },
                1,
                None,
            );
        }
    }
}
#[test]
fn block_embed_block_children_and_nested_blocks_match_normal() {
    for direction in ["ltr", "rtl"] {
        // BlockChildren root with two Normal children.
        embed_block_matches_normal(
            |bidi| {
                block(
                    "B",
                    direction,
                    bidi,
                    vec![
                        block("K1", "ltr", "normal", vec![text("X", "x", "pre", "none", "default", "normal")]),
                        block("K2", "rtl", "normal", vec![text("Y", "y", "pre", "none", "default", "normal")]),
                    ],
                )
            },
            1,
            None,
        );
        // Nested Embed blocks: neither level emits anything.
        embed_block_matches_normal(
            |bidi| {
                block(
                    "B",
                    direction,
                    bidi,
                    vec![block("K", "rtl", bidi, vec![text("X", "x", "pre", "none", "default", "normal")])],
                )
            },
            2,
            None,
        );
    }
}
#[test]
fn block_embed_in_single_interruption_graph_keeps_work_83() {
    for direction in ["ltr", "rtl"] {
        for (embed_b, embed_k) in [(true, false), (false, true), (true, true)] {
            let expected_embedded = usize::from(embed_b) + usize::from(embed_k);
            embed_block_matches_normal(
                |bidi| {
                    let pick = |on: bool| if on { bidi } else { "normal" };
                    block(
                        "B",
                        direction,
                        pick(embed_b),
                        vec![managed(
                            "I",
                            InputFamily::Row,
                            "inline",
                            "rtl",
                            "embed",
                            vec![
                                text("A", "a", "pre", "none", "default", "normal"),
                                block("K", "ltr", pick(embed_k), vec![text("X", "x", "pre", "none", "default", "normal")]),
                                text("C", "b", "pre", "none", "default", "normal"),
                            ],
                        )],
                    )
                },
                expected_embedded,
                Some(83),
            );
        }
    }
}
#[test]
fn empty_block_embed_matches_empty_normal_block() {
    for direction in ["ltr", "rtl"] {
        embed_block_matches_normal(|bidi| block("B", direction, bidi, vec![]), 1, None);
    }
}
