include!("adapter.rs");

fn managed(label:&str,family:InputFamily,display:&str,children:Vec<InputNode>)->InputNode {
    InputNode { label:label.into(),family,display:display.into(),white_space:"normal".into(),
        position:"static".into(),direction:"ltr".into(),unicode_bidi:"normal".into(),
        operation:"none".into(),profile:"default".into(),text:String::new(),runs:None,children }
}
fn payload(label:&str,text:&str,mode:&str)->InputNode {
    InputNode { label:label.into(),family:InputFamily::Text,display:"inline".into(),white_space:mode.into(),
        position:"static".into(),direction:"ltr".into(),unicode_bidi:"normal".into(),operation:"none".into(),
        profile:"default".into(),text:text.into(),runs:None,children:vec![] }
}
fn rich(label:&str,text:&str,runs:Vec<Range<usize>>,mode:&str)->InputNode {
    InputNode { label:label.into(),family:InputFamily::RichText,display:"inline".into(),white_space:mode.into(),
        position:"static".into(),direction:"ltr".into(),unicode_bidi:"normal".into(),operation:"none".into(),
        profile:"default".into(),text:text.into(),runs:Some(runs),children:vec![] }
}
macro_rules! with_whitespace {
    ($root:expr,|$whitespace:ident|$body:block) => {{
        let (construction,css)=actual_tree(&$root); let host=host(construction,None,None);
        let ua=catalog(); let graph=graph(&css,ua.next_context()); let sheets=controls(&[1],true,100);
        let bindings=[binding(&graph,GraphOrigin::Author(sheet(1)))];
        let bundle=DisplayRuleBundle::new(&ua,&bindings,&sheets,bundle_limits()).unwrap();
        let display=evaluate_source_display(host.snapshot().unwrap(),&bundle,limits()).unwrap();
        let eligibility=evaluate_source_eligibility(&display,eligibility_limits()).unwrap();
        let $whitespace=preprocess_source_whitespace(&SourceParticipants::new(&eligibility),whitespace_limits()).unwrap();
        $body
    }};
}
fn nested_empty()->InputNode {
    managed("s",InputFamily::Column,"block",vec![
        managed("s_0",InputFamily::Container,"inline",vec![
            managed("s_0_0",InputFamily::Container,"inline",vec![
                managed("s_0_0_0",InputFamily::Row,"block",vec![])])])])
}
fn separating(text:&str)->InputNode {
    managed("s",InputFamily::Column,"block",vec![
        managed("s_0",InputFamily::Row,"inline",vec![
            managed("s_0_0",InputFamily::Row,"block",vec![]),
            payload("s_0_1",text,"normal"),
            managed("s_0_2",InputFamily::Row,"block",vec![])])])
}

#[test]
fn nested_empty_sides_retain_four_sources_two_wrappers_and_distinct_equal_cut_edges() {
    with_whitespace!(nested_empty(),|whitespace|{
        let output=build_source_structure(&whitespace,structure_limits()).unwrap();
        assert_eq!(output.source_count(),4); assert_eq!(output.record_count(),8);
        for source in 0..4 { assert!(output.fragments(source).unwrap().next().is_some()); }
        assert_eq!(output.records().filter(|r|r.kind()==SourceStructureKind::AnonymousBlock).count(),2);
        assert_eq!(output.fragments(1).unwrap().map(|r|r.index()).collect::<Vec<_>>(),vec![3,7]);
        assert_eq!(output.fragments(2).unwrap().map(|r|r.index()).collect::<Vec<_>>(),vec![2,6]);
        let outer_end=output.record(2).unwrap().end_edge().unwrap();
        let inner_end=output.record(3).unwrap().end_edge().unwrap();
        assert_eq!(outer_end.cut(),inner_end.cut()); assert!(!outer_end.is_same_edge(&inner_end));
        let outer_start=output.record(6).unwrap().start_edge().unwrap();
        let inner_start=output.record(7).unwrap().start_edge().unwrap();
        assert_eq!(outer_start.cut(),inner_start.cut()); assert!(!outer_start.is_same_edge(&inner_start));
    });
}

#[test]
fn adjacent_group_closes_and_reopens_once_around_three_direct_blocks() {
    let root=managed("s",InputFamily::Column,"block",vec![managed("s_0",InputFamily::Row,"inline",vec![
        managed("s_0_0",InputFamily::Row,"block",vec![]),managed("s_0_1",InputFamily::Row,"block",vec![]),
        managed("s_0_2",InputFamily::Row,"block",vec![])])]);
    with_whitespace!(root,|whitespace|{
        let output=build_source_structure(&whitespace,structure_limits()).unwrap(); let receipt=output.receipt();
        assert_eq!((receipt.sources,receipt.events,receipt.blocks,receipt.inline_sources,receipt.continuations,receipt.anonymous_blocks,receipt.records),(5,10,4,1,1,2,8));
        assert_eq!(output.root().unwrap().children().map(|r|r.kind()).collect::<Vec<_>>(),vec![SourceStructureKind::AnonymousBlock,SourceStructureKind::SourceBlock,SourceStructureKind::SourceBlock,SourceStructureKind::SourceBlock,SourceStructureKind::AnonymousBlock]);
    });
}

#[test]
fn separated_block_hand_schedule_and_failure_precedence_are_literal() {
    with_whitespace!(separating("x"),|whitespace|{
        let output=build_source_structure(&whitespace,structure_limits()).unwrap(); let r=output.receipt();
        assert_eq!((r.sources,r.contents,r.events,r.continuations,r.anonymous_blocks,r.records,r.gap_events,r.gap_scalars,r.work),(5,1,11,2,3,11,3,1,48));
        assert_eq!(2+r.sources+2*r.events+r.gap_events+r.gap_scalars+2*r.continuations+r.records,48);
        let mut exact=structure_limits();exact.work=nz(48);assert_eq!(build_source_structure(&whitespace,exact).unwrap().work(),48);
        let mut short=structure_limits();short.work=nz(47);assert!(matches!(build_source_structure(&whitespace,short),Err(SourceStructureError::Work)));
    });
    with_whitespace!(separating(""),|whitespace|{
        let mut gap_exact=structure_limits();gap_exact.work=nz(20);
        assert!(matches!(build_source_structure(&whitespace,gap_exact),Err(SourceStructureError::Gap{events,scalars,witness:SourceStructureGapWitness::UnresolvedEmptyInlineGap{source:1,event:4},..}) if events==(4..7) && scalars==(0..0)));
        let mut gap_short=structure_limits();gap_short.work=nz(19);assert!(matches!(build_source_structure(&whitespace,gap_short),Err(SourceStructureError::Work)));
        let mut first_continuation=structure_limits();first_continuation.records=nz(6);
        assert!(matches!(build_source_structure(&whitespace,first_continuation),Err(SourceStructureError::Records)));
    });
}

#[test]
fn all_inline_empty_payloads_and_suppressed_scope_remain_distinct() {
    let inline=managed("s",InputFamily::Column,"block",vec![
        managed("s_0",InputFamily::Row,"inline",vec![]),payload("s_1","","normal"),
        rich("s_2","",vec![],"normal"),rich("s_3","",std::iter::once(0..0).collect(),"normal")]);
    with_whitespace!(inline,|whitespace|{
        let output=build_source_structure(&whitespace,structure_limits()).unwrap();
        assert_eq!((output.source_count(),output.record_count()),(5,8));
        assert_eq!(output.records().filter(|r|r.kind()==SourceStructureKind::Payload).count(),3);
        assert!(output.records().filter(|r|r.kind()==SourceStructureKind::Payload).all(|r|r.content_event().unwrap().scalar_range()==(0..0)));
    });
    let hidden=managed("s",InputFamily::Column,"none",vec![payload("s_0","hidden","normal")]);
    with_whitespace!(hidden,|whitespace|{
        let limits=SourceStructureLimits{sources:nz(1),records:nz(1),retained_bytes:nz(1),work:nz(1)};
        let output=build_source_structure(&whitespace,limits).unwrap();
        assert_eq!((output.source_count(),output.record_count(),output.work(),output.retained_bytes(),output.peak_retained_bytes()),(0,0,1,0,0));
        assert!(output.root().is_none()); assert!(output.fragments(0).unwrap().next().is_none());
        assert!(output.fragments(1).unwrap().next().is_none()); assert!(output.fragments(2).is_none());
    });
}

#[test]
fn identities_require_the_bound_owner_for_equal_coordinates_and_equal_inputs() {
    with_whitespace!(managed("s",InputFamily::Column,"block",vec![]),|whitespace|{
        let a=build_source_structure(&whitespace,structure_limits()).unwrap();
        let b=build_source_structure(&whitespace,structure_limits()).unwrap();
        assert!(!a.record(0).unwrap().is_same_record(&b.record(0).unwrap()));
        with_whitespace!(managed("s",InputFamily::Column,"block",vec![]),|equal_whitespace|{
            let c=build_source_structure(&equal_whitespace,structure_limits()).unwrap();
            assert!(!a.record(0).unwrap().is_same_record(&c.record(0).unwrap()));
            assert!(!std::ptr::eq(a.whitespace(),c.whitespace()));
        });
    });
}

#[test]
fn expected_free_observer_smoke_covers_real_container_row_column_text_and_rich_text() {
    observe("public-smoke",&managed("s",InputFamily::Column,"block",vec![
        managed("s_0",InputFamily::Container,"inline",vec![managed("s_0_0",InputFamily::Row,"inline",vec![
            payload("s_0_0_0","x","normal"),rich("s_0_0_1","é\r\n漢",vec![0..4,4..7],"pre")])]) ]));
}
