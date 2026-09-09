use iced_css_core::text::casing_data::*;

#[test]
fn base_full_mappings_expand_without_applying_conditions() {
    assert_eq!(DATA_VERSION, (17, 0, 0));
    assert_eq!(
        base_upper_mapping('ß'),
        CaseMapping::Replacement(&['S', 'S'])
    );
    assert_eq!(
        base_upper_mapping('ﬃ'),
        CaseMapping::Replacement(&['F', 'F', 'I'])
    );
    assert_eq!(
        base_lower_mapping('İ'),
        CaseMapping::Replacement(&['i', '\u{307}'])
    );
    assert_eq!(base_lower_mapping('Σ'), CaseMapping::Replacement(&['σ']));
    assert_eq!(
        base_lower_mapping('\u{10400}'),
        CaseMapping::Replacement(&['\u{10428}'])
    );
    assert_eq!(
        base_upper_mapping('\u{10428}'),
        CaseMapping::Replacement(&['\u{10400}'])
    );
    for c in ['\0', '漢', '\u{1f600}', '\u{10ffff}'] {
        assert_eq!(base_upper_mapping(c), CaseMapping::Identity);
        assert_eq!(base_lower_mapping(c), CaseMapping::Identity);
    }
    // A Unicode titlecase mapping would be Ss, not the CSS-required uppercase SS.
    assert_ne!(
        base_upper_mapping('ß'),
        CaseMapping::Replacement(&['S', 's'])
    );
    assert_ne!(CaseMapping::Identity, CaseMapping::Replacement(&[]));
}
#[test]
fn conditional_rows_preserve_deletion_identity_order_and_negation() {
    let sigma = conditional_mappings('Σ');
    assert_eq!(sigma.len(), 1);
    assert_eq!(sigma[0].scalar(), 'Σ');
    assert_eq!(sigma[0].expression(), "Final_Sigma");
    assert_eq!(
        sigma[0].conditions(),
        &[CasingCondition::Context {
            context: CasingContext::FinalSigma,
            negated: false
        }]
    );
    assert_eq!(sigma[0].lower(), &['ς']);
    assert_eq!(sigma[0].upper(), &['Σ']);
    let dot = conditional_mappings('\u{307}');
    assert_eq!(dot.len(), 3);
    assert_eq!(
        dot.iter().map(|r| r.expression()).collect::<Vec<_>>(),
        ["lt After_Soft_Dotted", "tr After_I", "az After_I"]
    );
    assert_eq!(dot[0].lower(), &['\u{307}']);
    assert_eq!(dot[0].upper(), &[]);
    assert_eq!(dot[1].lower(), &[]);
    assert_eq!(dot[2].lower(), &[]);
    let capital_i = conditional_mappings('I');
    assert_eq!(capital_i.len(), 3);
    assert_eq!(capital_i[0].lower(), &['i', '\u{307}']);
    assert_eq!(
        capital_i[1].conditions(),
        &[
            CasingCondition::Locale(CasingLocale::Turkish),
            CasingCondition::Context {
                context: CasingContext::BeforeDot,
                negated: true
            }
        ]
    );
    assert_eq!(
        capital_i[2].conditions(),
        &[
            CasingCondition::Locale(CasingLocale::Azeri),
            CasingCondition::Context {
                context: CasingContext::BeforeDot,
                negated: true
            }
        ]
    );
    for c in ['a', '漢', '\u{10ffff}'] {
        assert!(conditional_mappings(c).is_empty());
    }
}
#[test]
fn independent_properties_keep_overlap_and_exact_combining_classes() {
    assert!(is_cased('A'));
    assert!(!is_case_ignorable('A'));
    assert!(is_cased('\u{345}'));
    assert!(is_case_ignorable('\u{345}'));
    assert!(is_soft_dotted('i'));
    assert!(is_soft_dotted('j'));
    assert!(!is_soft_dotted('I'));
    assert_eq!(canonical_combining_class('A'), 0);
    assert_eq!(canonical_combining_class('\u{300}'), 230);
    assert_eq!(canonical_combining_class('\u{327}'), 202);
    assert_eq!(canonical_combining_class('\u{345}'), 240);
    assert_eq!(canonical_combining_class('\u{1dce}'), 214);
    for c in ['\0', '漢', '\u{10ffff}'] {
        assert!(!is_cased(c));
        assert!(!is_case_ignorable(c));
        assert!(!is_soft_dotted(c));
        assert_eq!(canonical_combining_class(c), 0);
    }
}
