use iced::{
    Element, Length, Size,
    advanced::{layout, widget::Tree},
    widget::{Space, keyed},
};
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
use std::{cell::Cell, num::NonZeroUsize};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
fn space(width: Length, height: Length) -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::space(limits())
        .unwrap()
        .width(width)
        .height(height)
        .erase()
}
fn observe(mut e: Element<'_, (), iced::Theme, ()>) -> (Size<Length>, Vec<iced::Rectangle>, usize) {
    let size = e.as_widget().size_hint();
    let mut tree = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
    );
    (
        size,
        std::iter::once(node.bounds())
            .chain(node.children().iter().map(|n| n.bounds()))
            .collect(),
        tree.children.len(),
    )
}
#[test]
fn from_vecs_and_push_preserve_native_hints_and_zero_size_members() {
    let captured = adapters::keyed_column_from_vecs(
        vec![1u8],
        vec![space(Length::Fill, Length::Fixed(20.0))],
        limits(),
    )
    .unwrap();
    let native = keyed::Column::from_vecs(
        vec![1u8],
        vec![Space::new().width(Length::Fill).height(20.0).into()],
    );
    assert_eq!(
        observe(captured.erase().into_parts().0),
        observe(native.into())
    );
    let captured = adapters::keyed_column_from_vecs(
        vec![1u8],
        vec![space(Length::Fill, Length::Fixed(20.0))],
        limits(),
    )
    .unwrap()
    .push(2, space(Length::Fixed(0.0), Length::Fixed(0.0)), limits())
    .unwrap()
    .push(3, space(Length::Fixed(30.0), Length::Fill), limits())
    .unwrap();
    let n = captured.construction().nodes();
    assert_eq!(n.len(), 4);
    assert_eq!(n[3].canonical_type(), "keyed_column");
    for (i, node) in n[..3].iter().enumerate() {
        assert_eq!(node.parent(), Some(3));
        assert_eq!(node.role_position(), Some(i));
    }
    let native = keyed::Column::from_vecs(
        vec![1u8],
        vec![Space::new().width(Length::Fill).height(20.0).into()],
    )
    .push(2, Space::new().width(0.0).height(0.0))
    .push(3, Space::new().width(30.0).height(Length::Fill));
    assert_eq!(
        observe(captured.erase().into_parts().0),
        observe(native.into())
    );
}
#[test]
fn native_keys_are_not_compared_during_capture_and_source_keys_remain_explicit() {
    #[derive(Clone, Copy)]
    struct Key<'a>(&'a Cell<usize>, u8);
    impl PartialEq for Key<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.0.set(self.0.get() + 1);
            self.1 == other.1
        }
    }
    let count = Cell::new(0);
    let child = space(Length::Shrink, Length::Shrink)
        .source_key(b"application-key", limits())
        .unwrap();
    let column =
        adapters::keyed_column_from_vecs(vec![Key(&count, 1)], vec![child], limits()).unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(
        column.construction().nodes()[0].source_key(),
        Some(b"application-key".as_slice())
    );
    let column = column
        .push(
            Key(&count, 2),
            space(Length::Shrink, Length::Shrink),
            limits(),
        )
        .unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(column.construction().nodes()[1].source_key(), None);
}
#[test]
fn insertion_and_parallel_vector_admission_are_bounded() {
    assert!(matches!(
        adapters::keyed_column_from_vecs(
            vec![1u8],
            Vec::<CapturedElement<'_, (), iced::Theme, ()>>::new(),
            limits()
        ),
        Err(CaptureError::Dimensions)
    ));
    let empty = adapters::keyed_column_new::<u8, (), iced::Theme, ()>(limits()).unwrap();
    let pointer = empty.construction().nodes().as_ptr();
    let empty = empty
        .push_maybe(
            1,
            None::<CapturedElement<'_, (), iced::Theme, ()>>,
            limits(),
        )
        .unwrap();
    assert_eq!(empty.construction().nodes().as_ptr(), pointer);
    assert_eq!(empty.construction().nodes().len(), 1);
    for (field, expected) in [
        (0, CaptureError::Nodes),
        (1, CaptureError::Depth),
        (2, CaptureError::Bytes),
        (3, CaptureError::Work),
    ] {
        let mut cap = limits();
        match field {
            0 => cap.nodes = NonZeroUsize::MIN,
            1 => cap.depth = NonZeroUsize::MIN,
            2 => cap.retained_bytes = NonZeroUsize::MIN,
            _ => cap.work = NonZeroUsize::MIN,
        }
        let empty = adapters::keyed_column_new::<u8, (), iced::Theme, ()>(limits()).unwrap();
        assert!(
            matches!(empty.push(1,space(Length::Shrink,Length::Shrink),cap),Err(e) if e==expected)
        );
    }
}

#[test]
fn configuration_converts_once_and_matches_native_geometry() {
    struct Len<'a>(&'a Cell<usize>, f32);
    impl From<Len<'_>> for Length {
        fn from(v: Len<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            Length::Fixed(v.1)
        }
    }
    struct Px<'a>(&'a Cell<usize>, f32);
    impl From<Px<'_>> for iced::Pixels {
        fn from(v: Px<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            iced::Pixels(v.1)
        }
    }
    struct Pad<'a>(&'a Cell<usize>);
    impl From<Pad<'_>> for iced::Padding {
        fn from(v: Pad<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            iced::Padding::new(3.0)
        }
    }
    let count = Cell::new(0);
    let captured = adapters::keyed_column_new::<u8, (), iced::Theme, ()>(limits())
        .unwrap()
        .push(1, space(Length::Fixed(20.0), Length::Fixed(10.0)), limits())
        .unwrap()
        .spacing(Px(&count, 2.0))
        .padding(Pad(&count))
        .width(Len(&count, 100.0))
        .height(Len(&count, 80.0))
        .max_width(Px(&count, 90.0))
        .align_items(iced::Alignment::End);
    assert_eq!(count.get(), 5);
    let native = keyed::Column::new()
        .push(1u8, Space::new().width(20.0).height(10.0))
        .spacing(Px(&count, 2.0))
        .padding(Pad(&count))
        .width(Len(&count, 100.0))
        .height(Len(&count, 80.0))
        .max_width(Px(&count, 90.0))
        .align_items(iced::Alignment::End);
    assert_eq!(count.get(), 10);
    assert_eq!(
        observe(captured.erase().into_parts().0),
        observe(native.into())
    );
    let column = adapters::keyed_column_new::<u8, (), iced::Theme, ()>(limits()).unwrap();
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        column.push_maybe(1, None::<CapturedElement<'_, (), iced::Theme, ()>>, cap),
        Err(CaptureError::Bytes)
    ));
}

struct Input<'a, T> {
    log: &'a std::cell::RefCell<Vec<&'static str>>,
    items: Vec<T>,
}
struct Iter<'a, T> {
    log: &'a std::cell::RefCell<Vec<&'static str>>,
    items: std::vec::IntoIter<T>,
}
impl<'a, T> IntoIterator for Input<'a, T> {
    type Item = T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.log.borrow_mut().push("into");
        Iter {
            log: self.log,
            items: self.items.into_iter(),
        }
    }
}
impl<T> Iterator for Iter<'_, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.log.borrow_mut().push("next");
        self.items.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.log.borrow_mut().push("hint");
        self.items.size_hint()
    }
}
#[test]
fn iterator_acquisition_hint_and_polling_match_native_paths() {
    use std::cell::RefCell;
    let captured_log = RefCell::new(Vec::new());
    let native_log = RefCell::new(Vec::new());
    let captured = adapters::keyed_column_with_children(
        Input {
            log: &captured_log,
            items: vec![
                (1u8, space(Length::Fill, Length::Fixed(10.0))),
                (2, space(Length::Fixed(0.0), Length::Fixed(0.0))),
            ],
        },
        limits(),
    )
    .unwrap();
    let native = keyed::Column::with_children(Input {
        log: &native_log,
        items: vec![
            (1u8, Space::new().width(Length::Fill).height(10.0).into()),
            (2, Space::new().width(0.0).height(0.0).into()),
        ],
    });
    assert_eq!(
        *captured_log.borrow(),
        ["into", "hint", "next", "next", "next"]
    );
    assert_eq!(*captured_log.borrow(), *native_log.borrow());
    assert_eq!(
        observe(captured.erase().into_parts().0),
        observe(native.into())
    );
    captured_log.borrow_mut().clear();
    native_log.borrow_mut().clear();
    let captured = adapters::keyed_column_from_vecs(
        vec![1u8],
        vec![space(Length::Fill, Length::Fixed(10.0))],
        limits(),
    )
    .unwrap()
    .extend(
        Input {
            log: &captured_log,
            items: vec![(2, space(Length::Fixed(0.0), Length::Fill))],
        },
        limits(),
    )
    .unwrap();
    let native = keyed::Column::from_vecs(
        vec![1u8],
        vec![Space::new().width(Length::Fill).height(10.0).into()],
    )
    .extend(Input {
        log: &native_log,
        items: vec![(2, Space::new().width(0.0).height(Length::Fill).into())],
    });
    assert_eq!(*captured_log.borrow(), ["into", "next", "next"]);
    assert_eq!(*captured_log.borrow(), *native_log.borrow());
    assert_eq!(
        observe(captured.erase().into_parts().0),
        observe(native.into())
    );
    captured_log.borrow_mut().clear();
    let parent =
        adapters::keyed_column_with_capacity::<u8, (), iced::Theme, ()>(8, limits()).unwrap();
    assert_eq!(parent.construction().nodes().len(), 1);
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::MIN;
    assert!(matches!(
        parent.extend(
            Input {
                log: &captured_log,
                items: vec![(1, space(Length::Shrink, Length::Shrink))]
            },
            cap
        ),
        Err(CaptureError::Bytes)
    ));
    assert!(captured_log.borrow().is_empty());
}

#[test]
fn with_children_keeps_one_budget_across_empty_construction_and_iteration() {
    let log = std::cell::RefCell::new(Vec::new());
    let mut cap = limits();
    cap.work = NonZeroUsize::new(2).unwrap();
    let input = || Input {
        log: &log,
        items: Vec::<(u8, CapturedElement<'_, (), iced::Theme, ()>)>::new(),
    };
    assert!(matches!(
        adapters::keyed_column_with_children(input(), cap),
        Err(CaptureError::Work)
    ));
    assert_eq!(*log.borrow(), ["into", "hint"]);
    log.borrow_mut().clear();
    cap.work = NonZeroUsize::new(3).unwrap();
    assert!(adapters::keyed_column_with_children(input(), cap).is_ok());
    assert_eq!(*log.borrow(), ["into", "hint", "next"]);
}
