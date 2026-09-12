// UNCOMPILED against the reviewed proposed API; no expected casing algorithm.
use iced_css::evaluation::source_casing::*;
use iced_css_core::text::casing_data::{CaseMapping, conditional_mappings};
fn casing_limits() -> SourceCasingLimits {
    SourceCasingLimits {
        sources: bound(),
        scalars: bound(),
        language_bytes: nz(20_000_000),
        context_bytes: nz(20_000_000),
        retained_bytes: nz(20_000_000),
        work: nz(20_000_000),
    }
}
fn encoded(value: &str) -> String {
    if value.is_empty() {
        "-".to_owned()
    } else {
        value
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}
fn char_list(value: impl Iterator<Item = char>) -> String {
    let s = value
        .map(|c| (c as u32).to_string())
        .collect::<Vec<_>>()
        .join(",");
    if s.is_empty() { "-".to_owned() } else { s }
}
fn observe_casing(case: usize, whitespace: &SourceWhitespace<'_, '_, '_, '_, '_>) {
    let out = evaluate_source_casing(whitespace, casing_limits()).unwrap();
    assert!(std::ptr::eq(out.whitespace(), whitespace));
    assert_eq!(out.scalar_count(), whitespace.scalar_count());
    assert_eq!(out.source_count(), whitespace.source_count());
    assert_eq!(
        out.captured_source_count(),
        whitespace
            .eligibility()
            .display()
            .snapshot()
            .construction()
            .nodes()
            .len()
    );
    assert!(out.peak_retained_bytes() >= out.retained_bytes());
    println!(
        "Q\t{case}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        out.captured_source_count(),
        out.source_count(),
        out.scalar_count(),
        out.language_bytes(),
        out.known_language_count(),
        out.context_bytes(),
        out.active_context_count(),
        out.selection_pass_count(),
        out.language_view_work(),
        out.work()
    );
    println!(
        "CQ\t{case}\t{}\t{}\t{}",
        out.capitalize_scalar_count(),
        out.capitalize_context_count(),
        out.capitalize_unit_count()
    );
    let mut sources = out.sources();
    iterator_contract(&sources);
    assert_eq!(sources.len(), out.source_count());
    for (ordinal, source) in sources.by_ref().enumerate() {
        let p = source.participant();
        assert_eq!(source.ordinal(), ordinal);
        assert_eq!(
            source.ordinal(),
            whitespace
                .eligibility()
                .ordinal(source.source_index())
                .unwrap()
        );
        assert_eq!(p.source_index(), source.source_index());
        assert!(std::ptr::eq(
            p.token(),
            whitespace
                .eligibility()
                .token(source.source_index())
                .unwrap()
        ));
        assert_eq!(
            out.source(source.source_index()).unwrap().ordinal(),
            ordinal
        );
        println!(
            "D\t{case}\t{}\t{}\t{:?}\t{:?}\t{:?}",
            p.node().metadata().css_id().unwrap(),
            source.source_index(),
            source.language_status(),
            source.profile(),
            source.operation()
        );
        println!(
            "DO\t{case}\t{}\t{}",
            p.node().metadata().css_id().unwrap(),
            source.ordinal()
        );
    }
    assert_eq!(sources.len(), 0);
    assert!(sources.next().is_none() && sources.next().is_none());
    for (i, _) in whitespace
        .eligibility()
        .display()
        .snapshot()
        .construction()
        .nodes()
        .iter()
        .enumerate()
    {
        assert_eq!(
            out.source(i).is_some(),
            whitespace.eligibility().ordinal(i).is_some()
        );
    }
    assert!(out.source(usize::MAX).is_none());
    assert!(out.scalar(out.scalar_count()).is_none());
    assert!(out.scalar(usize::MAX).is_none());
    assert!(out.capitalize_unit(out.capitalize_unit_count()).is_none());
    assert!(out.capitalize_unit(usize::MAX).is_none());
    let mut units = out.capitalize_units();
    iterator_contract(&units);
    assert_eq!(units.len(), out.capitalize_unit_count());
    for (ordinal, unit) in units.by_ref().enumerate() {
        assert_eq!(unit.index(), ordinal);
        let range = unit.scalar_range();
        let word = unit.word_range();
        assert!(range.start < range.end && range.start == word.start && range.end <= word.end);
        assert!(word.end <= out.scalar_count());
        let direct = out.capitalize_unit(ordinal).unwrap();
        assert_eq!(direct.scalar_range(), range);
        assert_eq!(direct.word_range(), word);
        let mut members = unit.scalars();
        iterator_contract(&members);
        assert_eq!(members.len(), range.len());
        let ids = members
            .by_ref()
            .map(|member| {
                assert!(range.contains(&member.index()));
                assert_eq!(member.capitalize_unit().unwrap().index(), ordinal);
                assert!(std::ptr::eq(
                    member.whitespace_scalar().participant().token(),
                    whitespace
                        .scalar(member.index())
                        .unwrap()
                        .participant()
                        .token()
                ));
                member.index().to_string()
            })
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(members.len(), 0);
        assert!(members.next().is_none() && members.next().is_none());
        println!(
            "U\t{case}\t{ordinal}\t{}\t{}\t{}\t{}\t{ids}",
            range.start, range.end, word.start, word.end
        );
    }
    assert_eq!(units.len(), 0);
    assert!(units.next().is_none() && units.next().is_none());
    let mut scalars = out.scalars();
    iterator_contract(&scalars);
    assert_eq!(scalars.len(), out.scalar_count());
    for (i, s) in scalars.by_ref().enumerate() {
        assert_eq!(s.index(), i);
        let membership = s.capitalize_unit().map(|u| u.index());
        println!(
            "T\t{case}\t{i}\t{}\t{}",
            membership.map_or_else(|| "-".to_owned(), |v| v.to_string()),
            usize::from(s.is_capitalize_target())
        );
        assert_eq!(
            s.is_capitalize_target(),
            membership.is_some()
                && s.source().operation() == Some(SourceCasingOperation::Capitalize)
        );
        let w = s.whitespace_scalar();
        let original = whitespace.scalar(i).unwrap();
        assert_eq!(w.source_range(), original.source_range());
        assert_eq!(w.normalized(), original.normalized());
        assert!(std::ptr::eq(
            w.participant().token(),
            original.participant().token()
        ));
        assert_eq!(s.source().source_index(), w.participant().source_index());
        let joined = out.source(w.participant().source_index()).unwrap();
        assert_eq!(
            s.source().ordinal(),
            whitespace
                .eligibility()
                .ordinal(w.participant().source_index())
                .unwrap()
        );
        assert_eq!(s.source().profile(), joined.profile());
        assert_eq!(s.source().language_status(), joined.language_status());
        assert_eq!(s.source().operation(), joined.operation());
        let (mapping, characters) = match s.mapping() {
            CaseMapping::Identity => ("Identity", "-".to_owned()),
            CaseMapping::Replacement(v) => ("Replacement", char_list(v.iter().copied())),
        };
        let (origin, expr, lower, upper) = match s.origin() {
            SourceCasingOrigin::Untransformed => (
                "Untransformed",
                "-".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
            ),
            SourceCasingOrigin::Base => ("Base", "-".to_owned(), "-".to_owned(), "-".to_owned()),
            SourceCasingOrigin::Conditional(row) => {
                assert_eq!(row.scalar(), w.normalized());
                assert!(
                    conditional_mappings(w.normalized())
                        .iter()
                        .any(|r| std::ptr::eq(r, row))
                );
                (
                    "Conditional",
                    encoded(row.expression()),
                    char_list(row.lower().iter().copied()),
                    char_list(row.upper().iter().copied()),
                )
            }
        };
        let presentation = s.presentation();
        let kind = match presentation {
            SourceCasingPresentation::SuppressedByWhitespace => "SuppressedByWhitespace",
            SourceCasingPresentation::Scalar(_) => "Scalar",
            SourceCasingPresentation::Replacement(_) => "Replacement",
        };
        let mut values = presentation.characters();
        iterator_contract(&values);
        let len = values.len();
        let rendered = char_list(values.by_ref());
        assert_eq!(values.len(), 0);
        assert!(values.next().is_none() && values.next().is_none());
        let count = presentation.characters().count();
        assert_eq!(len, count);
        println!(
            "K\t{case}\t{i}\t{}\t{mapping}\t{characters}\t{origin}\t{expr}\t{lower}\t{upper}\t{kind}\t{rendered}",
            w.participant().node().metadata().css_id().unwrap()
        );
    }
    assert_eq!(scalars.len(), 0);
    assert!(scalars.next().is_none() && scalars.next().is_none());
}
