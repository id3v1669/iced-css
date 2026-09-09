use iced_css::capture::{CaptureError, CaptureLimits, Captured, CapturedElement, adapters};
use std::{cell::Cell, num::NonZeroUsize};
type Child = CapturedElement<'static, (), iced::Theme, ()>;
fn limits() -> CaptureLimits {
    let nz = |n| NonZeroUsize::new(n).unwrap();
    CaptureLimits {
        nodes: nz(100),
        depth: nz(100),
        retained_bytes: nz(100000),
        work: nz(100000),
    }
}
fn child() -> Child {
    adapters::space(limits())
        .unwrap()
        .width(0.0)
        .height(0.0)
        .erase()
}
fn grid() -> Captured<iced::widget::Grid<'static, (), iced::Theme, ()>> {
    adapters::grid_from_vec(Vec::<Child>::new(), limits()).unwrap()
}
struct Sequence<'a> {
    values: Vec<Child>,
    into: &'a Cell<usize>,
    next: &'a Cell<usize>,
    hints: &'a Cell<usize>,
}
struct Iter<'a> {
    values: std::vec::IntoIter<Child>,
    next: &'a Cell<usize>,
    hints: &'a Cell<usize>,
}
impl<'a> IntoIterator for Sequence<'a> {
    type Item = Child;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.into.set(self.into.get() + 1);
        Iter {
            values: self.values.into_iter(),
            next: self.next,
            hints: self.hints,
        }
    }
}
impl Iterator for Iter<'_> {
    type Item = Child;
    fn next(&mut self) -> Option<Child> {
        self.next.set(self.next.get() + 1);
        self.values.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.hints.set(self.hints.get() + 1);
        self.values.size_hint()
    }
}
#[test]
fn optional_none_retains_storage_and_some_supplies_exactly_one_child() {
    let mut cap = limits();
    cap.work = NonZeroUsize::MIN;
    let parent = grid();
    let pointer = parent.construction().nodes().as_ptr();
    let parent = parent.push_maybe(None::<Child>, cap).unwrap();
    assert_eq!(parent.construction().nodes().as_ptr(), pointer);
    assert_eq!(parent.construction().nodes().len(), 1);
    let parent = parent.push_maybe(Some(child()), limits()).unwrap();
    assert_eq!(parent.construction().nodes().len(), 2);
    assert_eq!(parent.construction().nodes()[0].role_position(), Some(0));
    cap = limits();
    cap.nodes = NonZeroUsize::MIN;
    assert!(matches!(
        parent.push_maybe(None::<Child>, cap),
        Err(CaptureError::Nodes)
    ));
}
#[test]
fn extend_converts_once_polls_through_one_none_and_retains_native_children() {
    let (into, next, hints) = (Cell::new(0), Cell::new(0), Cell::new(0));
    let sequence = Sequence {
        values: vec![child(), child()],
        into: &into,
        next: &next,
        hints: &hints,
    };
    let mut cap = limits();
    cap.work = NonZeroUsize::new(14).unwrap();
    let parent = grid().extend(sequence, cap).unwrap();
    assert_eq!((into.get(), next.get(), hints.get()), (1, 3, 0));
    assert_eq!(
        parent
            .construction()
            .nodes()
            .iter()
            .map(|n| n.role_position())
            .collect::<Vec<_>>(),
        [Some(0), Some(1), None]
    );
    let (native, _) = parent.into_parts();
    let native: iced::Element<'_, (), iced::Theme, ()> = native.into();
    assert_eq!(iced::advanced::widget::Tree::new(&native).children.len(), 2);
}
#[test]
fn a_single_work_budget_stops_a_yielded_prefix_without_publishing_it() {
    let (into, next, hints) = (Cell::new(0), Cell::new(0), Cell::new(0));
    let sequence = Sequence {
        values: vec![child(), child(), child()],
        into: &into,
        next: &next,
        hints: &hints,
    };
    let mut cap = limits();
    cap.work = NonZeroUsize::new(7).unwrap();
    assert!(matches!(
        grid().extend(sequence, cap),
        Err(CaptureError::Work)
    ));
    assert_eq!((into.get(), next.get(), hints.get()), (1, 2, 0));
}
#[test]
fn empty_extend_and_initial_admission_use_explicit_boundaries() {
    let mut cap = limits();
    cap.work = NonZeroUsize::MIN;
    assert!(matches!(
        grid().extend(Vec::<Child>::new(), cap),
        Err(CaptureError::Work)
    ));
    cap.work = NonZeroUsize::new(2).unwrap();
    assert_eq!(
        grid()
            .extend(Vec::<Child>::new(), cap)
            .unwrap()
            .construction()
            .nodes()
            .len(),
        1
    );
    let (into, next, hints) = (Cell::new(0), Cell::new(0), Cell::new(0));
    let sequence = Sequence {
        values: vec![child()],
        into: &into,
        next: &next,
        hints: &hints,
    };
    cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        grid().extend(sequence, cap),
        Err(CaptureError::Bytes)
    ));
    assert_eq!((into.get(), next.get(), hints.get()), (0, 0, 0));
}

#[test]
fn exact_total_budget_matches_the_sum_of_append_visits_and_final_none() {
    for count in 0usize..33 {
        // Empty parent has one node; each child adds one. Summing append costs
        // 2*(k+1)+1+1, initial validation and count+1 next calls gives this bound.
        let required = count * count + 4 * count + 2;
        for (budget, success) in [(required - 1, false), (required, true)] {
            let (into, next, hints) = (Cell::new(0), Cell::new(0), Cell::new(0));
            let sequence = Sequence {
                values: (0..count).map(|_| child()).collect(),
                into: &into,
                next: &next,
                hints: &hints,
            };
            let mut cap = limits();
            cap.work = NonZeroUsize::new(budget).unwrap();
            let result = grid().extend(sequence, cap);
            if success {
                assert!(result.is_ok());
                assert_eq!(next.get(), count + 1);
            } else {
                assert!(matches!(result, Err(CaptureError::Work)));
                assert_eq!(next.get(), count);
            }
            assert_eq!((into.get(), hints.get()), (1, 0));
        }
    }
}
#[test]
fn non_fused_iterators_are_not_polled_after_the_first_none() {
    let next = Cell::new(0);
    let sequence = std::iter::from_fn(|| {
        next.set(next.get() + 1);
        match next.get() {
            1 => Some(child()),
            2 => None,
            _ => panic!("polled after None"),
        }
    });
    let parent = grid().extend(sequence, limits()).unwrap();
    assert_eq!(next.get(), 2);
    assert_eq!(parent.construction().nodes().len(), 2);
}
