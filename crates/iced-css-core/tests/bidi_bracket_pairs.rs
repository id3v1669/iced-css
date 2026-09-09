use iced_css_core::text::{
    bidi_bracket_pairs::*, bidi_explicit::*, bidi_initial::*, bidi_isolates::*, bidi_runs::*,
    bidi_weak::*,
};
use std::num::NonZeroUsize;
fn max() -> NonZeroUsize {
    NonZeroUsize::new(usize::MAX).unwrap()
}
fn limits() -> BidiBracketPairLimits {
    let n = max();
    BidiBracketPairLimits {
        scalars: n,
        active_scalars: n,
        sequences: n,
        pairs: n,
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
            BidiParagraphMode::AutoLeftToRight,
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
        let $r = identify_bidi_bracket_pairs(&weak, $limits);
        $body
    }};
}
#[test]
fn identities_order_and_checked_pair_views() {
    for (source, expected) in [
        ("", vec![]),
        ("(())", vec![(0, 3), (1, 2)]),
        ("([)]", vec![(0, 2)]),
        ("〈〉", vec![(0, 1)]),
        ("〈〉", vec![(0, 1)]),
        ("（)", vec![]),
        ("（ ）", vec![(0, 2)]),
        ("\u{202d}()\u{202c}", vec![]),
        ("(\u{301})", vec![(0, 2)]),
        ("(\u{2067}a\u{2069})", vec![(0, 4)]),
    ] {
        fixture!(source,limits(),result=>{
         let r=result.unwrap();let actual:Vec<_>=(0..r.sequence_count()).flat_map(|si|{let seq=r.sequence(si).unwrap();seq.pairs().map(|p|(p.opening_scalar_index(),p.closing_scalar_index())).collect::<Vec<_>>()}).collect();assert_eq!(actual,expected);assert_eq!(r.pair_count(),expected.len());
         for si in 0..r.sequence_count(){let seq=r.sequence(si).unwrap();assert_eq!(seq.index(),si);assert_eq!(seq.status(),BidiBracketSequenceStatus::Complete);assert_eq!(seq.weak_sequence().index(),si);let mut it=seq.pairs();let total=it.len();let mut held=Vec::new();for j in 0..total{assert_eq!(it.len(),total-j);assert_eq!(it.size_hint(),(total-j,Some(total-j)));let pair=it.next().unwrap();let copied=pair;assert_eq!(copied.pair_index(),j);assert_eq!(pair.sequence_index(),si);assert_eq!(seq.pair(j).unwrap().opening_scalar_index(),pair.opening_scalar_index());assert_eq!(pair.opening().index(),pair.opening_scalar_index());assert_eq!(pair.closing().index(),pair.closing_scalar_index());held.push(pair);}assert!(it.next().is_none());assert!(it.next().is_none());assert_eq!(it.len(),0);for p in held{assert!(p.opening_scalar_index()<p.closing_scalar_index());}assert!(seq.pair(total).is_none());assert!(seq.pair(usize::MAX).is_none());}
         assert!(r.sequence(r.sequence_count()).is_none());assert!(r.sequence(usize::MAX).is_none());
        });
    }
    fixture!("😀(\u{301})",limits(),result=>{let r=result.unwrap();let seq=r.sequence(0).unwrap();let pair=seq.pair(0).unwrap();assert_eq!(pair.opening().run_scalar().explicit().original().source_range(),4..5);assert_eq!(pair.closing().run_scalar().explicit().original().source_range(),7..8);assert_eq!(r.weak().scalar(2).unwrap().run_scalar().explicit().original().class(),iced_css_core::text::bidi_data::BidiClass::Nsm);});
}
#[test]
fn whole_sequence_overflow_and_final_pair_limit() {
    for depth in [63, 64] {
        let s = format!("()(){}{}", "(".repeat(depth), ")".repeat(depth));
        fixture!(s,limits(),result=>{let r=result.unwrap();let seq=r.sequence(0).unwrap();assert_eq!(seq.status(),if depth==63{BidiBracketSequenceStatus::Complete}else{BidiBracketSequenceStatus::Overflow});assert_eq!(seq.pair_count(),if depth==63{65}else{0});});
    }
    let one = NonZeroUsize::new(1).unwrap();
    let l = BidiBracketPairLimits {
        pairs: one,
        ..limits()
    };
    fixture!("()()",l,result=>{assert_eq!(result.unwrap_err(),BidiBracketPairError::Pairs);});
    let s = format!("()(){}\u{2067}[]\u{2069}{}", "(".repeat(64), ")".repeat(64));
    fixture!(s,l,result=>{let r=result.unwrap();assert_eq!(r.sequence(0).unwrap().status(),BidiBracketSequenceStatus::Overflow);assert_eq!(r.sequence(1).unwrap().status(),BidiBracketSequenceStatus::Complete);assert_eq!(r.pair_count(),1);});
    let s = format!("()(){}\r\n[]", "(".repeat(64));
    fixture!(s,l,result=>{let r=result.unwrap();assert_eq!(r.sequence(0).unwrap().status(),BidiBracketSequenceStatus::Overflow);assert_eq!(r.sequence(1).unwrap().pair_count(),1);});
}
#[test]
fn reserved_work_payload_and_limit_order() {
    for s in ["", "\u{ad}", "abc", "()", "\r\n\n"] {
        fixture!(s,limits(),result=>{let r=result.unwrap();let a=r.active_scalar_count();assert_eq!(r.work(),1+if a>0{63}else{0}+r.sequence_count()+65*a);assert_eq!(r.retained_bytes()+r.status_stack_bytes(),r.peak_retained_bytes());assert_eq!(r.status_stack_bytes()==0,a==0);let work=r.work();let bytes=r.peak_retained_bytes();let mut l=limits();l.work=NonZeroUsize::new(work).unwrap();l.retained_bytes=NonZeroUsize::new(bytes.max(1)).unwrap();assert!(identify_bidi_bracket_pairs(r.weak(),l).is_ok());if work>1{l.work=NonZeroUsize::new(work-1).unwrap();assert_eq!(identify_bidi_bracket_pairs(r.weak(),l).unwrap_err(),BidiBracketPairError::Work);}
if bytes>1{let l=BidiBracketPairLimits{retained_bytes:NonZeroUsize::new(bytes-1).unwrap(),..limits()};assert_eq!(identify_bidi_bracket_pairs(r.weak(),l).unwrap_err(),BidiBracketPairError::Bytes);}});
    }
    let one = NonZeroUsize::new(1).unwrap();
    let mut l = BidiBracketPairLimits {
        scalars: one,
        active_scalars: one,
        sequences: one,
        pairs: one,
        retained_bytes: one,
        work: one,
    };
    for s in ["", "\u{ad}"] {
        fixture!(s,l,result=>{let r=result.unwrap();assert_eq!(r.work(),1);assert_eq!(r.retained_bytes(),0);assert_eq!(r.status_stack_bytes(),0);});
    }
    for error in [
        BidiBracketPairError::Scalars,
        BidiBracketPairError::ActiveScalars,
        BidiBracketPairError::Sequences,
        BidiBracketPairError::Work,
    ] {
        fixture!("\n\n",l,result=>{assert_eq!(result.unwrap_err(),error);});
        match error {
            BidiBracketPairError::Scalars => l.scalars = max(),
            BidiBracketPairError::ActiveScalars => l.active_scalars = max(),
            BidiBracketPairError::Sequences => l.sequences = max(),
            _ => {}
        }
    }
}
