use iced_css_core::text::{
    bidi_bracket_pairs::*, bidi_data::BidiClass, bidi_explicit::*, bidi_initial::*,
    bidi_isolates::*, bidi_n0::*, bidi_runs::*, bidi_weak::*,
};
use std::num::NonZeroUsize;
fn max() -> NonZeroUsize {
    NonZeroUsize::new(usize::MAX).unwrap()
}
fn limits() -> BidiN0Limits {
    let n = max();
    BidiN0Limits {
        scalars: n,
        active_scalars: n,
        runs: n,
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
        let $r = resolve_bidi_n0(&pairs, $limits);
        $body
    }};
}
#[test]
fn literal_context_and_raw_nsm_deferred_order() {
    use BidiClass::*;
    for (source, expected) in [
        ("(a)", vec![L, L, L]),
        ("א(א)", vec![R, R, R, R]),
        ("(1)", vec![L, L, L]),
        ("(١)", vec![L, An, L]),
        ("()", vec![On, On]),
        ("(\u{2067}א\u{2069})", vec![On, Rli, R, Pdi, On]),
    ] {
        fixture!(source,limits(),result=>{let r=result.unwrap();let actual:Vec<_>=(0..r.scalar_count()).map(|i|r.scalar(i).unwrap().current_type().unwrap()).collect();assert_eq!(actual,expected);});
    }
    // At level2, a raw NSM overridden to current L must propagate the changed R bracket.
    fixture!("\u{202a}א(א)\u{202c}\u{202d}\u{0300}",limits(),result=>{let r=result.unwrap();let mark=r.scalar(7).unwrap();assert_eq!(mark.weak_scalar().current_type(),Some(L));assert_eq!(mark.weak_scalar().run_scalar().explicit().original().class(),Nsm);assert_eq!(mark.current_type(),Some(R));});
    // Deferred propagation keeps that L visible while deciding the later bracket pair.
    fixture!("\u{202a}א(א)\u{202c}\u{202d}\u{0300}\u{202c}\u{202a}[א]",limits(),result=>{let r=result.unwrap();let indices=[1,2,3,4,7,10,11,12];let types:Vec<_>=indices.map(|i|r.scalar(i).unwrap().current_type().unwrap()).into_iter().collect();assert_eq!(types,[R,R,R,R,R,L,R,L]);});
    fixture!("\u{202b}a(a)\u{202c}\u{202e}\u{0300}",limits(),result=>{let r=result.unwrap();let mark=r.scalar(7).unwrap();assert_eq!(mark.weak_scalar().current_type(),Some(R));assert_eq!(mark.current_type(),Some(L));});
}
#[test]
fn immutable_views_and_sequence_iterators() {
    fixture!("😀(a)\u{301}\r\n(\u{2067}א\u{2069})",limits(),result=>{let r=result.unwrap();assert!(std::ptr::eq(r.pairs().weak().runs().explicit().isolates().initial().source().as_ptr(),r.scalar(0).unwrap().weak_scalar().run_scalar().explicit().original().text().as_ptr()));
 for si in 0..r.sequence_count(){let seq=r.sequence(si).unwrap();let copy=seq;assert_eq!(copy.index(),si);assert_eq!(seq.bracket_sequence().index(),si);let parent=seq.bracket_sequence();let weak=parent.weak_sequence();assert_eq!(seq.level(),weak.level());assert_eq!(seq.sos(),weak.sos());assert_eq!(seq.eos(),weak.eos());assert_eq!(seq.run_count(),weak.run_count());let expected:Vec<_>=weak.scalars().map(|v|v.index()).collect();let mut iter=seq.scalars();let mut held=Vec::new();for (j,index) in expected.iter().enumerate(){assert_eq!(iter.len(),expected.len()-j);assert_eq!(iter.size_hint(),(expected.len()-j,Some(expected.len()-j)));let v=iter.next().unwrap();assert_eq!(v.index(),*index);held.push(v);}assert!(iter.next().is_none());assert!(iter.next().is_none());assert_eq!(iter.len(),0);for v in held{assert_eq!(v.current_type(),r.scalar(v.index()).unwrap().current_type());}}
 assert!(r.scalar(r.scalar_count()).is_none());assert!(r.scalar(usize::MAX).is_none());assert!(r.sequence(r.sequence_count()).is_none());assert!(r.sequence(usize::MAX).is_none());});
}
#[test]
fn reserved_resources_and_count_precedence() {
    for source in ["", "\u{ad}", "(a)", "\n\n"] {
        fixture!(source,limits(),result=>{let r=result.unwrap();assert_eq!(r.work(),1+r.run_count()+66*r.active_scalar_count()+2*r.sequence_count()+3*r.pair_count());assert_eq!(r.retained_bytes(),r.peak_retained_bytes());let mut l=limits();l.work=NonZeroUsize::new(r.work()).unwrap();l.retained_bytes=NonZeroUsize::new(r.retained_bytes().max(1)).unwrap();assert!(resolve_bidi_n0(r.pairs(),l).is_ok());if r.work()>1{l.work=NonZeroUsize::new(r.work()-1).unwrap();assert_eq!(resolve_bidi_n0(r.pairs(),l).unwrap_err(),BidiN0Error::Work);}
 if r.retained_bytes()>1{let l=BidiN0Limits{retained_bytes:NonZeroUsize::new(r.retained_bytes()-1).unwrap(),..limits()};assert_eq!(resolve_bidi_n0(r.pairs(),l).unwrap_err(),BidiN0Error::Bytes);}});
    }
    let one = NonZeroUsize::new(1).unwrap();
    let mut l = BidiN0Limits {
        scalars: one,
        active_scalars: one,
        runs: one,
        sequences: one,
        pairs: one,
        retained_bytes: one,
        work: one,
    };
    for source in ["", "\u{ad}"] {
        fixture!(source,l,result=>{let r=result.unwrap();assert_eq!(r.work(),1);assert_eq!(r.retained_bytes(),0);for i in 0..r.scalar_count(){assert_eq!(r.scalar(i).unwrap().current_type(),None);}});
    }
    for error in [
        BidiN0Error::Scalars,
        BidiN0Error::ActiveScalars,
        BidiN0Error::Runs,
        BidiN0Error::Sequences,
        BidiN0Error::Pairs,
        BidiN0Error::Work,
    ] {
        fixture!("()\n()",l,result=>{assert_eq!(result.unwrap_err(),error);});
        match error {
            BidiN0Error::Scalars => l.scalars = max(),
            BidiN0Error::ActiveScalars => l.active_scalars = max(),
            BidiN0Error::Runs => l.runs = max(),
            BidiN0Error::Sequences => l.sequences = max(),
            BidiN0Error::Pairs => l.pairs = max(),
            _ => {}
        }
    }
}
