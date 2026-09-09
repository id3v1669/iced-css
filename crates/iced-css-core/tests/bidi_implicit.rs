use iced_css_core::text::{
    bidi_bracket_pairs::*, bidi_data::BidiClass, bidi_explicit::*, bidi_implicit::*,
    bidi_initial::*, bidi_isolates::*, bidi_n0::*, bidi_runs::*, bidi_weak::*,
};
use std::num::NonZeroUsize;
fn max() -> NonZeroUsize {
    NonZeroUsize::new(usize::MAX).unwrap()
}
fn limits() -> BidiImplicitLimits {
    let n = max();
    BidiImplicitLimits {
        scalars: n,
        active_scalars: n,
        runs: n,
        sequences: n,
        retained_bytes: n,
        work: n,
    }
}
macro_rules! fixture {
    ($source:expr,$limits:expr,$r:ident=>$body:block) => {{
        let source = ($source).to_string();
        let n = max();
        let initial = evaluate_bidi_initial(
            &source,
            BidiParagraphMode::LeftToRight,
            BidiInitialLimits {
                input_bytes: n,
                scalars: n,
                paragraphs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let isolates = prepare_bidi_isolates(
            &initial,
            BidiIsolateLimits {
                scalars: n,
                paragraphs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let explicit = resolve_bidi_explicit(
            &isolates,
            BidiExplicitLimits {
                scalars: n,
                paragraphs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let runs = prepare_bidi_run_sequences(
            &explicit,
            BidiRunLimits {
                scalars: n,
                paragraphs: n,
                active_scalars: n,
                runs: n,
                sequences: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let weak = resolve_bidi_weak(
            &runs,
            BidiWeakLimits {
                scalars: n,
                active_scalars: n,
                runs: n,
                sequences: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let pairs = identify_bidi_bracket_pairs(
            &weak,
            BidiBracketPairLimits {
                scalars: n,
                active_scalars: n,
                sequences: n,
                pairs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let n0 = resolve_bidi_n0(
            &pairs,
            BidiN0Limits {
                scalars: n,
                active_scalars: n,
                runs: n,
                sequences: n,
                pairs: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let $r = resolve_bidi_implicit(&n0, $limits);
        $body
    }};
}
#[test]
fn neutral_types_and_implicit_levels_are_separate() {
    use BidiClass::*;
    for (source, types, levels) in [
        ("a a", vec![L, L, L], vec![0, 0, 0]),
        ("א א", vec![R, R, R], vec![1, 1, 1]),
        ("a א", vec![L, L, R], vec![0, 0, 1]),
        ("א ١", vec![R, R, An], vec![1, 1, 2]),
        ("()", vec![L, L], vec![0, 0]),
        (" א ", vec![L, R, L], vec![0, 1, 0]),
    ] {
        fixture!(source,limits(),result=>{let r=result.unwrap();let states:Vec<_>=(0..r.scalar_count()).map(|j|r.scalar(j).unwrap().state().unwrap()).collect();assert_eq!(states.iter().map(|v|v.current_type()).collect::<Vec<_>>(),types);assert_eq!(states.iter().map(|v|v.level().number()).collect::<Vec<_>>(),levels);});
    }
}
#[test]
fn reserved_resources_and_count_precedence() {
    for source in ["", "\u{ad}", "(a)", "\n\n"] {
        fixture!(source,limits(),result=>{let r=result.unwrap();assert_eq!(r.work(),1+2*r.run_count()+4*r.active_scalar_count()+r.sequence_count());assert_eq!(r.retained_bytes(),r.peak_retained_bytes());let mut l=limits();l.work=NonZeroUsize::new(r.work()).unwrap();l.retained_bytes=NonZeroUsize::new(r.retained_bytes().max(1)).unwrap();assert!(resolve_bidi_implicit(r.n0(),l).is_ok());if r.work()>1{l.work=NonZeroUsize::new(r.work()-1).unwrap();assert_eq!(resolve_bidi_implicit(r.n0(),l).unwrap_err(),BidiImplicitError::Work);}
 if r.retained_bytes()>1{let l=BidiImplicitLimits{retained_bytes:NonZeroUsize::new(r.retained_bytes()-1).unwrap(),..limits()};assert_eq!(resolve_bidi_implicit(r.n0(),l).unwrap_err(),BidiImplicitError::Bytes);}});
    }
    let one = NonZeroUsize::new(1).unwrap();
    let mut l = BidiImplicitLimits {
        scalars: one,
        active_scalars: one,
        runs: one,
        sequences: one,
        retained_bytes: one,
        work: one,
    };
    for source in ["", "\u{ad}"] {
        fixture!(source,l,result=>{let r=result.unwrap();assert_eq!(r.work(),1);assert_eq!(r.retained_bytes(),0);for i in 0..r.scalar_count(){assert_eq!(r.scalar(i).unwrap().state(),None);}});
    }
    for error in [
        BidiImplicitError::Scalars,
        BidiImplicitError::ActiveScalars,
        BidiImplicitError::Runs,
        BidiImplicitError::Sequences,
        BidiImplicitError::Work,
    ] {
        fixture!("()\n()",l,result=>{assert_eq!(result.unwrap_err(),error);});
        match error {
            BidiImplicitError::Scalars => l.scalars = max(),
            BidiImplicitError::ActiveScalars => l.active_scalars = max(),
            BidiImplicitError::Runs => l.runs = max(),
            BidiImplicitError::Sequences => l.sequences = max(),
            _ => {}
        }
    }
}
#[test]
fn linked_spans_keep_foreign_content_out_and_preserve_identity() {
    let source = format!(
        "a{}\u{2067}א\u{2069}{}a\r\n😀",
        " ".repeat(256),
        " ".repeat(256)
    );
    fixture!(source,limits(),result=>{let r=result.unwrap();let parent=r.n0();let runs=parent.pairs().weak().runs();assert_eq!(r.sequence_count(),3);let outer=r.sequence(0).unwrap();assert!(outer.run_count()>1);let expected:Vec<_>=runs.sequence(0).unwrap().scalars().map(|v|v.index()).collect();let mut iter=outer.scalars();let mut held=Vec::new();for (j,&index) in expected.iter().enumerate(){assert_eq!(iter.len(),expected.len()-j);assert_eq!(iter.size_hint(),(expected.len()-j,Some(expected.len()-j)));let view=iter.next().unwrap();assert_eq!(view.index(),index);assert_eq!(view.state().unwrap().current_type(),BidiClass::L);held.push(view);}assert!(iter.next().is_none());assert!(iter.next().is_none());for view in held{assert_eq!(view.state(),r.scalar(view.index()).unwrap().state());}
 for j in 0..r.scalar_count(){let view=r.scalar(j).unwrap();let prior=view.n0_scalar();assert_eq!(prior.current_type(),parent.scalar(j).unwrap().current_type());let weak_view=prior.weak_scalar();let run_view=weak_view.run_scalar();let old=run_view.explicit();assert_eq!(old.state(),runs.scalar(j).unwrap().explicit().state());assert_eq!(old.original().source_range(),runs.scalar(j).unwrap().explicit().original().source_range());}
 assert!(r.scalar(r.scalar_count()).is_none());assert!(r.scalar(usize::MAX).is_none());assert!(r.sequence(r.sequence_count()).is_none());assert!(r.sequence(usize::MAX).is_none());});
}
#[test]
fn real_embedding_boundaries_and_n0_schedule() {
    use BidiClass::*;
    for (control, count, content, last_type) in [
        ('\u{202a}', 62, "١", An),
        ('\u{202a}', 62, "א1", En),
        ('\u{202b}', 63, "a", L),
        ('\u{202b}', 63, "١", An),
        ('\u{202b}', 63, "א1", En),
    ] {
        let source = format!("{}{content}", control.to_string().repeat(count));
        fixture!(source,limits(),result=>{let r=result.unwrap();let view=r.scalar(r.scalar_count()-1).unwrap();let state=view.state().unwrap();assert_eq!(state.current_type(),last_type);assert_eq!(state.level().number(),126);let BidiExplicitState::Retained{level,..}=view.n0_scalar().weak_scalar().run_scalar().explicit().state()else{panic!()};assert_eq!(level.number(),if control=='\u{202a}'{124}else{125});});
    }
    fixture!("\u{202a}א(א)\u{202c}\u{202d}\u{0300}\u{202c}\u{202a}[א]",limits(),result=>{let r=result.unwrap();let states:Vec<_>=[1,2,3,4,7,10,11,12].map(|i|r.scalar(i).unwrap().state().unwrap()).into_iter().collect();assert_eq!(states.iter().map(|v|v.current_type()).collect::<Vec<_>>(),[R,R,R,R,R,L,R,L]);assert_eq!(states.iter().map(|v|v.level().number()).collect::<Vec<_>>(),[3,3,3,3,3,2,3,2]);assert_eq!(r.scalar(0).unwrap().state(),None);});
}
