use iced_css_core::text::{
    bidi_bracket_pairs::*, bidi_explicit::*, bidi_implicit::*, bidi_initial::*, bidi_isolates::*,
    bidi_lines::*, bidi_n0::*, bidi_runs::*, bidi_weak::*,
};
use std::num::NonZeroUsize;
fn max() -> NonZeroUsize {
    NonZeroUsize::new(usize::MAX).unwrap()
}
fn limits() -> BidiLineLimits {
    let n = max();
    BidiLineLimits {
        scalars: n,
        active_scalars: n,
        paragraphs: n,
        lines: n,
        retained_bytes: n,
        work: n,
    }
}
macro_rules! fixture {
    ($source:expr,$cuts:expr,$limits:expr,$r:ident=>$body:block) => {{
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
        let implicit = resolve_bidi_implicit(
            &n0,
            BidiImplicitLimits {
                scalars: n,
                active_scalars: n,
                runs: n,
                sequences: n,
                retained_bytes: n,
                work: n,
            },
        )
        .unwrap();
        let $r = reorder_bidi_lines(&implicit, $cuts, $limits);
        $body
    }};
}
#[test]
fn raw_ranges_l1_levels_and_l2_order() {
    fixture!("אא ", &[3], limits(), result=>{let r=result.unwrap();let line=r.line(0).unwrap();assert_eq!(line.scalar_range(),0..3);assert_eq!(line.source_range(),0..5);assert_eq!(line.scalars().map(|v|v.level().unwrap().number()).collect::<Vec<_>>(),[1,1,0]);assert_eq!(line.visual_scalars().map(|v|v.index()).collect::<Vec<_>>(),[1,0,2]);assert_eq!(r.implicit().scalar(2).unwrap().state().unwrap().level().number(),0);});
    fixture!("\r\n", &[2], limits(), result=>{let r=result.unwrap();let line=r.line(0).unwrap();assert_eq!(line.scalar_count(),2);assert_eq!(line.source_range(),0..2);assert_eq!(line.scalars().map(|v|v.level().unwrap().number()).collect::<Vec<_>>(),[0,0]);});
    fixture!("\r\n", &[1,2], limits(), result=>{assert_eq!(result.unwrap_err(),BidiLineError::InvalidLineEnds);});
    fixture!("\u{ad}", &[1], limits(), result=>{let r=result.unwrap();let line=r.line(0).unwrap();assert_eq!(line.source_range(),0..2);assert_eq!(line.scalar_count(),1);assert_eq!(line.active_scalar_count(),0);assert_eq!(line.scalar(0).unwrap().level(),None);assert!(line.visual_scalars().next().is_none());assert!(r.retained_bytes()>0);});
}
#[test]
fn partition_domain_and_resource_precedence() {
    for (source, cuts) in [
        ("", vec![0]),
        ("a", vec![]),
        ("a", vec![0, 1]),
        ("ab", vec![1, 1, 2]),
        ("ab", vec![2, 1]),
        ("ab", vec![1]),
        ("ab", vec![3]),
        ("a\nb", vec![3]),
        ("a\nb", vec![1, 3]),
        ("\r\n", vec![1, 2]),
    ] {
        fixture!(source,&cuts,limits(),result=>{assert_eq!(result.unwrap_err(),BidiLineError::InvalidLineEnds);});
    }
    for (source, cuts) in [
        ("", vec![]),
        ("\u{ad}", vec![1]),
        ("a\nb", vec![2, 3]),
        ("\r\n\r\n", vec![2, 4]),
        ("😀ab", vec![1, 3]),
    ] {
        fixture!(source,&cuts,limits(),result=>{let r=result.unwrap();let implicit=r.implicit();assert_eq!(r.work(),1+2*r.paragraph_count()+r.scalar_count()+130*r.line_count()+192*r.active_scalar_count());assert_eq!(r.retained_bytes(),r.peak_retained_bytes());let exact=BidiLineLimits{work:NonZeroUsize::new(r.work()).unwrap(),retained_bytes:NonZeroUsize::new(r.retained_bytes().max(1)).unwrap(),..limits()};assert!(reorder_bidi_lines(implicit,&cuts,exact).is_ok());if r.work()>1{assert_eq!(reorder_bidi_lines(implicit,&cuts,BidiLineLimits{work:NonZeroUsize::new(r.work()-1).unwrap(),..exact}).unwrap_err(),BidiLineError::Work);}
 if r.retained_bytes()>1{assert_eq!(reorder_bidi_lines(implicit,&cuts,BidiLineLimits{retained_bytes:NonZeroUsize::new(r.retained_bytes()-1).unwrap(),..exact}).unwrap_err(),BidiLineError::Bytes);}});
    }
    let one = NonZeroUsize::new(1).unwrap();
    let mut l = BidiLineLimits {
        scalars: one,
        active_scalars: one,
        paragraphs: one,
        lines: one,
        retained_bytes: one,
        work: one,
    };
    for error in [
        BidiLineError::Scalars,
        BidiLineError::ActiveScalars,
        BidiLineError::Paragraphs,
        BidiLineError::Lines,
        BidiLineError::Work,
        BidiLineError::InvalidLineEnds,
    ] {
        fixture!("a\nb\n",&[0,4],l,result=>{assert_eq!(result.unwrap_err(),error);});
        match error {
            BidiLineError::Scalars => l.scalars = max(),
            BidiLineError::ActiveScalars => l.active_scalars = max(),
            BidiLineError::Paragraphs => l.paragraphs = max(),
            BidiLineError::Lines => l.lines = max(),
            BidiLineError::Work => l.work = max(),
            _ => {}
        }
    }
    fixture!("",&[],BidiLineLimits{scalars:one,active_scalars:one,paragraphs:one,lines:one,retained_bytes:one,work:one},result=>{let r=result.unwrap();assert_eq!(r.work(),1);assert_eq!(r.retained_bytes(),0);assert_eq!(r.line_count(),0);assert!(r.scalar(0).is_none());});
}
#[test]
fn alternate_cuts_reuse_implicit_owner_and_original_l1_classes() {
    fixture!("\u{202b}א  א",&[5],limits(),result=>{let r=result.unwrap();let original=r.implicit();assert_eq!(original.scalar(2).unwrap().state().unwrap().level().number(),1);assert_eq!(r.scalar(2).unwrap().level().unwrap().number(),1);let mut cuts=vec![3,5];let wrapped=reorder_bidi_lines(original,&cuts,limits()).unwrap();cuts.clear();drop(cuts);assert_eq!(wrapped.scalar(2).unwrap().level().unwrap().number(),0);assert_eq!(original.scalar(2).unwrap().state().unwrap().level().number(),1);assert_eq!(wrapped.scalar(0).unwrap().level(),None);assert_eq!(wrapped.line(0).unwrap().scalar_count(),3);});
    // Original WS is reset even after an override makes its current type strong.
    fixture!("\u{202e}א ",&[3],limits(),result=>{let r=result.unwrap();let view=r.scalar(2).unwrap();assert_eq!(view.implicit_scalar().state().unwrap().current_type(),iced_css_core::text::bidi_data::BidiClass::R);assert_eq!(view.level().unwrap().number(),0);});
    // Original NSM at the end blocks resetting the preceding original WS.
    fixture!("\u{202b}א \u{301}",&[4],limits(),result=>{let r=result.unwrap();assert_eq!(r.scalar(2).unwrap().level().unwrap().number(),1);assert_eq!(r.scalar(3).unwrap().level().unwrap().number(),1);});
    // X9 gaps do not interrupt retained adjacency before a separator.
    fixture!("\u{202b}א \u{ad}\tא",&[6],limits(),result=>{let r=result.unwrap();assert_eq!(r.scalar(2).unwrap().level().unwrap().number(),0);assert_eq!(r.scalar(3).unwrap().level(),None);assert_eq!(r.scalar(4).unwrap().level().unwrap().number(),0);});
    fixture!("\u{202b}א\u{202b}אב",&[5],limits(),result=>{let r=result.unwrap();assert_eq!(r.line(0).unwrap().visual_scalars().map(|v|v.index()).collect::<Vec<_>>(),[4,3,1]);});
}
#[test]
fn checked_local_global_and_live_iterator_views() {
    fixture!("😀\u{ad}אב\r\na",&[2,6,7],limits(),result=>{let r=result.unwrap();for li in 0..r.line_count(){let line=r.line(li).unwrap();let copy=line;assert_eq!(copy.index(),li);let range=line.scalar_range();let mut iterator=line.scalars();let mut held=Vec::new();for (local,index) in range.clone().enumerate(){assert_eq!(iterator.len(),range.end-index);assert_eq!(iterator.size_hint(),(range.end-index,Some(range.end-index)));let view=iterator.next().unwrap();assert_eq!(view.index(),index);assert_eq!(view.line_index(),li);assert_eq!(view.line_scalar_index(),local);assert_eq!(line.scalar(local).unwrap().index(),index);assert_eq!(r.scalar(index).unwrap().line_index(),li);held.push(view);}assert!(iterator.next().is_none());assert!(iterator.next().is_none());for view in held{assert_eq!(view.level(),r.scalar(view.index()).unwrap().level());}let mut iter=line.visual_scalars();let mut visual=Vec::new();for local in 0..line.active_scalar_count(){assert_eq!(iter.len(),line.active_scalar_count()-local);let view=iter.next().unwrap();assert!(view.level().is_some());assert_eq!(view.index(),line.visual_scalar(local).unwrap().index());visual.push(view);}assert!(iter.next().is_none());assert!(iter.next().is_none());for view in visual{assert_eq!(view.level(),r.scalar(view.index()).unwrap().level());}assert!(line.scalar(line.scalar_count()).is_none());assert!(line.scalar(usize::MAX).is_none());assert!(line.visual_scalar(line.active_scalar_count()).is_none());assert!(line.visual_scalar(usize::MAX).is_none());}assert_eq!(r.line(0).unwrap().source_range(),0..6);assert!(r.line(r.line_count()).is_none());assert!(r.line(usize::MAX).is_none());assert!(r.scalar(r.scalar_count()).is_none());assert!(r.scalar(usize::MAX).is_none());});
}
