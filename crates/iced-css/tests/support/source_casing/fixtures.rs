include!("../source_capitalize/adapter.rs");
include!("casing_observer.rs");
fn leaf(id: usize, text: &str, operation: &str, lang: Option<&str>) -> InputNode {
    InputNode::Payload {
        id,
        text: text.to_owned(),
        mode: "normal".to_owned(),
        suppressed: false,
        runs: None,
        operation: operation.to_owned(),
        language: lang.map(str::to_owned),
    }
}
macro_rules! with_whitespace {
    ($children:expr,$root_lang:expr,$doc_lang:expr,$extra:expr,|$w:ident|$body:block) => {{
        let (construction, mut css) = actual_tree_with_language(&$children, $root_lang);
        css.push_str($extra);
        let h = host(construction, None, $doc_lang);
        let ua = catalog();
        let g = graph(&css, ua.next_context());
        let sheets = controls(&[1], true, 100);
        let bindings = [binding(&g, GraphOrigin::Author(sheet(1)))];
        let bundle = DisplayRuleBundle::new(&ua, &bindings, &sheets, bundle_limits()).unwrap();
        let display = evaluate_source_display(h.snapshot().unwrap(), &bundle, limits()).unwrap();
        let eligibility = evaluate_source_eligibility(&display, eligibility_limits()).unwrap();
        let $w = preprocess_source_whitespace(
            &SourceParticipants::new(&eligibility),
            whitespace_limits(),
        )
        .unwrap();
        $body
    }};
}
#[test]
fn language_inheritance_clear_malformed_and_nul() {
    with_whitespace!(
        [
            leaf(0, "I", "lower", None),
            leaf(1, "I", "lower", Some("")),
            leaf(2, "I", "lower", Some("tr\0")),
            InputNode::Empty { id: 3 }
        ],
        None,
        Some("tr"),
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            let by_id = |id: &str| {
                out.sources()
                    .find(|s| s.participant().node().metadata().css_id() == Some(id))
                    .unwrap()
            };
            for id in ["scope", "p0", "i3"] {
                let s = by_id(id);
                assert_eq!(s.profile(), SourceCasingProfile::Turkish);
                assert_eq!(s.language_status(), SourceCasingLanguageStatus::WellFormed);
            }
            assert_eq!(by_id("scope").operation(), None);
            assert_eq!(by_id("i3").operation(), None);
            assert_eq!(
                by_id("p1").language_status(),
                SourceCasingLanguageStatus::Unknown
            );
            assert_eq!(
                by_id("p2").language_status(),
                SourceCasingLanguageStatus::Malformed
            );
            assert_eq!(
                by_id("p2")
                    .participant()
                    .node()
                    .metadata()
                    .attribute("lang"),
                Some("tr\0")
            );
            assert_eq!(
                out.scalars()
                    .flat_map(|s| s.presentation().characters())
                    .collect::<String>(),
                "ıii"
            );
            assert_eq!(out.language_bytes(), 9);
            assert_eq!(out.known_language_count(), 4);
        }
    );
}
#[test]
fn managed_empty_language_and_empty_content_diagnostics() {
    with_whitespace!(
        [leaf(0, "", "none", None), InputNode::Empty { id: 1 }],
        Some(""),
        Some("lt"),
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(out.scalar_count(), 0);
            assert_eq!(out.source_count(), 3);
            assert_eq!(out.active_context_count(), 0);
            assert_eq!(out.selection_pass_count(), 0);
            assert!(
                out.sources()
                    .all(|s| s.language_status() == SourceCasingLanguageStatus::Unknown)
            );
            let payload = out
                .sources()
                .find(|s| s.participant().node().metadata().css_id() == Some("p0"))
                .unwrap();
            assert_eq!(payload.operation(), Some(SourceCasingOperation::None));
        }
    );
}
#[test]
fn original_context_includes_none_neighbors_and_whitespace_suppression() {
    with_whitespace!(
        [
            leaf(0, "A", "none", None),
            leaf(1, "Σ", "lower", None),
            leaf(2, "A", "none", None)
        ],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            assert_eq!(
                out.scalars()
                    .flat_map(|s| s.presentation().characters())
                    .collect::<String>(),
                "AσA"
            );
            assert_eq!(out.selection_pass_count(), 1);
            assert_eq!(out.context_bytes(), 4);
        }
    );
}
#[test]
fn exact_work_one_short_and_recovery_with_generous_bytes() {
    with_whitespace!(
        [
            leaf(0, "é\r\n漢Σ", "lower", Some("tr")),
            leaf(1, "", "upper", Some("lt"))
        ],
        None,
        None,
        "",
        |w| {
            let out = evaluate_source_casing(&w, casing_limits()).unwrap();
            let work = out.work();
            assert_eq!(out.language_view_work(), 65);
            assert_eq!(work, 575);
            let mut lim = casing_limits();
            lim.work = nz(work);
            assert_eq!(evaluate_source_casing(&w, lim).unwrap().work(), work);
            lim.work = nz(work - 1);
            assert!(matches!(
                evaluate_source_casing(&w, lim),
                Err(SourceCasingError::Work)
            ));
            assert_eq!(
                evaluate_source_casing(&w, casing_limits())
                    .unwrap()
                    .scalar_count(),
                w.scalar_count()
            );
            assert!(w.input_scalar_count() > w.scalar_count());
            let p0 = out
                .sources()
                .find(|s| s.participant().node().metadata().css_id() == Some("p0"))
                .unwrap()
                .source_index();
            let mut lim = casing_limits();
            lim.language_bytes = nz(1);
            lim.work = nz(69);
            assert!(
                matches!(evaluate_source_casing(&w,lim),Err(SourceCasingError::LanguageBytes{source}) if source==p0)
            );
            let mut lim = casing_limits();
            lim.context_bytes = nz(1);
            lim.work = nz(528);
            assert!(matches!(
                evaluate_source_casing(&w, lim),
                Err(SourceCasingError::ContextBytes { start: 0 })
            ));
        }
    );
}
#[test]
fn captured_source_limit_and_language_precedence() {
    with_whitespace!(
        [
            leaf(0, "X", "capitalize", Some("tr\0")),
            leaf(1, "Y", "none", None)
        ],
        None,
        None,
        "#p1 { display:none; }",
        |w| {
            let mut lim = casing_limits();
            lim.sources = nz(2);
            lim.work = nz(1);
            assert!(matches!(
                evaluate_source_casing(&w, lim),
                Err(SourceCasingError::Sources)
            ));
            let mut lim = casing_limits();
            lim.work = nz(1);
            assert!(matches!(
                evaluate_source_casing(&w, lim),
                Err(SourceCasingError::Work)
            ));
            let mut lim = casing_limits();
            lim.language_bytes = nz(1);
            assert!(matches!(
                evaluate_source_casing(&w, lim),
                Err(SourceCasingError::LanguageBytes { source: 0 })
            ));
        }
    );
}
#[test]
fn empty_capitalize_and_managed_override_are_admitted() {
    with_whitespace!([leaf(0, "", "capitalize", None)], None, None, "", |w| {
        let out = evaluate_source_casing(&w, casing_limits()).unwrap();
        assert_eq!(
            out.source(0).unwrap().operation(),
            Some(SourceCasingOperation::Capitalize)
        );
        assert_eq!(out.capitalize_scalar_count(), 0);
        assert_eq!(out.capitalize_context_count(), 0);
        assert_eq!(out.capitalize_unit_count(), 0);
    });
    with_whitespace!(
        [leaf(0, "X", "none", None)],
        None,
        None,
        "#scope { text-transform:capitalize; }",
        |w| {
            assert!(evaluate_source_casing(&w, casing_limits()).is_ok());
        }
    );
}
#[test]
fn all_hidden_shortcut_and_scalar_limit_before_work() {
    with_whitespace!(
        [leaf(0, "Σ", "lower", Some("tr"))],
        None,
        None,
        "#scope { display:none; }",
        |w| {
            let mut lim = casing_limits();
            lim.work = nz(1);
            lim.retained_bytes = nz(1);
            let out = evaluate_source_casing(&w, lim).unwrap();
            assert_eq!(out.work(), 1);
            assert_eq!(out.language_view_work(), 0);
            assert_eq!(out.retained_bytes(), 0);
            assert_eq!(out.source_count(), 0);
            assert_eq!(out.captured_source_count(), 2);
        }
    );
    with_whitespace!([leaf(0, "AA", "none", None)], None, None, "", |w| {
        let mut lim = casing_limits();
        lim.work = nz(1);
        lim.scalars = nz(1);
        assert!(matches!(
            evaluate_source_casing(&w, lim),
            Err(SourceCasingError::Scalars)
        ));
    });
}

#[test]
fn nested_rich_owner_queries_and_iterators() {
    let rich = InputNode::Payload {
        id: 0,
        text: "é\r\n漢Σ".into(),
        mode: "pre-line".into(),
        suppressed: false,
        runs: Some(vec![0..3, 3..9]),
        operation: "lower".into(),
        language: Some("tr".into()),
    };
    let nodes = [InputNode::Block {
        id: 0,
        suppressed: false,
        children: vec![InputNode::Empty { id: 1 }, rich, leaf(2, "A", "none", None)],
    }];
    // Verifies captured family/run/metadata identity before preprocessing, then
    // every public source/scalar/iterator join and all boundary interval queries.
    observe(0, &nodes);
}
