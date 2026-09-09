use iced::{
    Element, Length, Size,
    advanced::{layout, widget::Tree},
    widget::{table, text},
};
use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use std::{cell::RefCell, num::NonZeroUsize, rc::Rc};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(100000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
struct Message;
struct Row {
    id: u8,
    log: Rc<RefCell<Vec<String>>>,
}
impl Clone for Row {
    fn clone(&self) -> Self {
        self.log.borrow_mut().push(format!("clone:{}", self.id));
        Self {
            id: self.id,
            log: self.log.clone(),
        }
    }
}
fn geometry(mut e: Element<'_, Message, iced::Theme, ()>) -> (Size, Vec<iced::Rectangle>, usize) {
    let mut tree = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(300.0, 200.0)),
    );
    (
        node.size(),
        node.children().iter().map(|n| n.bounds()).collect(),
        tree.children.len(),
    )
}
#[test]
fn table_capture_preserves_eager_row_major_clones_and_native_geometry() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let rows = || {
        vec![
            Row {
                id: 1,
                log: log.clone(),
            },
            Row {
                id: 2,
                log: log.clone(),
            },
        ]
    };
    let columns = ["A", "B"].map(|name| {
        adapters::table_column(
            adapters::text::<iced::Theme, ()>(name, limits()).unwrap(),
            move |r: Row| {
                r.log.borrow_mut().push(format!("view:{}:{name}", r.id));
                adapters::text::<iced::Theme, ()>(format!("{name}{}", r.id), limits()).unwrap()
            },
        )
        .width(Length::Fixed(60.0))
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom)
    });
    assert!(log.borrow().is_empty());
    let captured = adapters::table_from_vecs::<_, Message, iced::Theme, ()>(
        Vec::from(columns),
        rows(),
        limits(),
    )
    .unwrap()
    .width(250)
    .padding(3)
    .padding_x(4)
    .padding_y(5)
    .separator(2)
    .separator_x(3)
    .separator_y(4);
    assert_eq!(
        *log.borrow(),
        [
            "clone:1", "view:1:A", "clone:1", "view:1:B", "clone:2", "view:2:A", "clone:2",
            "view:2:B"
        ]
    );
    assert_eq!(
        captured
            .construction()
            .nodes()
            .iter()
            .filter_map(|n| n.text())
            .collect::<Vec<_>>(),
        ["A", "B", "A1", "B1", "A2", "B2"]
    );
    let expected_log = log.borrow().clone();
    log.borrow_mut().clear();
    let columns = ["A", "B"].map(|name| {
        table::column(text(name), move |r: Row| {
            r.log.borrow_mut().push(format!("view:{}:{name}", r.id));
            text(format!("{name}{}", r.id))
        })
        .width(Length::Fixed(60.0))
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom)
    });
    let native = table::table(columns, rows())
        .width(250)
        .padding(3)
        .padding_x(4)
        .padding_y(5)
        .separator(2)
        .separator_x(3)
        .separator_y(4);
    assert_eq!(*log.borrow(), expected_log);
    assert_eq!(
        geometry(captured.erase().into_parts().0),
        geometry(native.into())
    );
}
#[test]
fn empty_native_table_has_safe_layout_and_preserves_empty_source_rows() {
    let capture =
        adapters::table_from_vecs::<(), Message, iced::Theme, ()>(vec![], vec![(), ()], limits())
            .unwrap()
            .width(75);
    assert_eq!(capture.construction().nodes().len(), 6);
    assert_eq!(
        geometry(capture.erase().into_parts().0),
        (Size::new(75.0, 0.0), vec![], 0)
    );
}
#[test]
fn node_preflight_skips_callbacks() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let column = adapters::table_column(
        adapters::text::<iced::Theme, ()>("H", limits()).unwrap(),
        |r: Row| {
            r.log.borrow_mut().push("view".into());
            adapters::text::<iced::Theme, ()>("body", limits()).unwrap()
        },
    );
    let mut restricted = limits();
    restricted.nodes = NonZeroUsize::MIN;
    let result = adapters::table_from_vecs::<_, Message, iced::Theme, ()>(
        vec![column],
        vec![Row {
            id: 1,
            log: log.clone(),
        }],
        restricted,
    );
    assert!(matches!(result, Err(CaptureError::Nodes)));
    assert!(log.borrow().is_empty());
}

#[test]
fn evidence_failure_after_construction_never_replays_views() {
    for (work, want_ok, want_calls) in [(7, false, 0), (27, false, 2), (28, true, 2)] {
        let log = Rc::new(RefCell::new(Vec::new()));
        let column = adapters::table_column(adapters::space(limits()).unwrap(), |r: Row| {
            r.log.borrow_mut().push("view".into());
            adapters::space(limits()).unwrap()
        });
        let mut restricted = limits();
        restricted.work = NonZeroUsize::new(work).unwrap();
        let result = adapters::table_from_vecs::<_, Message, iced::Theme, ()>(
            vec![column],
            vec![Row {
                id: 1,
                log: log.clone(),
            }],
            restricted,
        );
        assert_eq!(result.is_ok(), want_ok, "work={work}");
        if !want_ok {
            assert!(matches!(result, Err(CaptureError::Work)));
        }
        assert_eq!(log.borrow().len(), want_calls);
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let column = adapters::table_column(adapters::space(limits()).unwrap(), |r: Row| {
        r.log.borrow_mut().push("view".into());
        adapters::container::<Message, iced::Theme, (), _>(
            adapters::space(limits()).unwrap(),
            limits(),
        )
        .unwrap()
    });
    let mut restricted = limits();
    restricted.nodes = NonZeroUsize::new(9).unwrap();
    assert!(matches!(
        adapters::table_from_vecs(
            vec![column],
            vec![Row {
                id: 1,
                log: log.clone()
            }],
            restricted
        ),
        Err(CaptureError::Nodes)
    ));
    assert_eq!(*log.borrow(), ["clone:1", "view"]);
}

#[derive(Default)]
struct Paint(Vec<(iced::advanced::renderer::Quad, iced::Background)>);
impl iced::advanced::Renderer for Paint {
    fn start_layer(&mut self, _: iced::Rectangle) {}
    fn end_layer(&mut self) {}
    fn start_transformation(&mut self, _: iced::Transformation) {}
    fn end_transformation(&mut self) {}
    fn fill_quad(&mut self, q: iced::advanced::renderer::Quad, b: impl Into<iced::Background>) {
        self.0.push((q, b.into()));
    }
    fn reset(&mut self, _: iced::Rectangle) {
        self.0.clear();
    }
    fn allocate_image(
        &mut self,
        h: &iced_test::core::image::Handle,
        f: impl FnOnce(Result<iced_test::core::image::Allocation, iced_test::core::image::Error>)
        + Send
        + 'static,
    ) {
        iced::advanced::Renderer::allocate_image(&mut (), h, f);
    }
}
fn populated() -> Element<'static, Message, iced::Theme, Paint> {
    let columns = (0..2)
        .map(|_| {
            adapters::table_column(adapters::space(limits()).unwrap(), |()| {
                adapters::space(limits()).unwrap()
            })
            .width(40)
        })
        .collect();
    adapters::table_from_vecs(columns, vec![(), ()], limits())
        .unwrap()
        .erase()
        .into_parts()
        .0
}
fn draw(
    e: &mut Element<'_, Message, iced::Theme, Paint>,
    tree: &mut Tree,
    paint: &mut Paint,
) -> layout::Node {
    use iced::advanced::{Layout, mouse, renderer};
    paint.0.clear();
    let node = e.as_widget_mut().layout(
        tree,
        paint,
        &layout::Limits::new(Size::ZERO, Size::new(300.0, 200.0)),
    );
    e.as_widget().draw(
        tree,
        paint,
        &iced::Theme::Dark,
        &renderer::Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &iced::Rectangle::with_size(Size::new(300.0, 200.0)),
    );
    node
}
#[test]
fn populated_empty_populated_transition_clears_stale_separator_geometry() {
    let mut table = populated();
    let mut tree = Tree::new(&table);
    let mut paint = Paint::default();
    let first = draw(&mut table, &mut tree, &mut paint);
    assert_eq!(tree.children.len(), 6);
    assert_eq!(paint.0.len(), 3);
    let expected = paint.0.clone();
    let columns = (0..2).map(|_| {
        table::column(iced::widget::Space::new(), |()| iced::widget::Space::new()).width(40)
    });
    let mut native: Element<'_, Message, iced::Theme, Paint> =
        table::table(columns, vec![(), ()]).into();
    let mut native_tree = Tree::new(&native);
    let native_layout = draw(&mut native, &mut native_tree, &mut paint);
    assert_eq!(native_layout.size(), first.size());
    assert_eq!(paint.0, expected);

    let mut empty = adapters::table_from_vecs::<(), Message, iced::Theme, Paint>(
        vec![],
        vec![(), ()],
        limits(),
    )
    .unwrap()
    .width(75)
    .erase()
    .into_parts()
    .0;
    tree.diff(&empty);
    let node = draw(&mut empty, &mut tree, &mut paint);
    assert_eq!(node.size(), Size::new(75.0, 0.0));
    assert!(tree.children.is_empty());
    assert!(paint.0.is_empty());
    let mut restored = populated();
    tree.diff(&restored);
    let last = draw(&mut restored, &mut tree, &mut paint);
    assert_eq!(last.size(), first.size());
    assert_eq!(paint.0, expected);
    assert_eq!(tree.children.len(), 6);
}

#[test]
fn byte_depth_and_work_preflight_skip_views_and_application_panics_propagate() {
    for restricted in [
        CaptureLimits {
            retained_bytes: NonZeroUsize::MIN,
            ..limits()
        },
        CaptureLimits {
            depth: NonZeroUsize::MIN,
            ..limits()
        },
        CaptureLimits {
            work: NonZeroUsize::MIN,
            ..limits()
        },
    ] {
        let calls = std::cell::Cell::new(0);
        let col = adapters::table_column(adapters::space(limits()).unwrap(), |()| {
            calls.set(calls.get() + 1);
            adapters::space(limits()).unwrap()
        });
        let result = adapters::table_from_vecs::<_, Message, iced::Theme, ()>(
            vec![col],
            vec![()],
            restricted,
        );
        assert!(matches!(
            result,
            Err(CaptureError::Bytes | CaptureError::Depth | CaptureError::Work)
        ));
        assert_eq!(calls.get(), 0);
    }
    let calls = std::cell::Cell::new(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let col = adapters::table_column(
            adapters::space(limits()).unwrap(),
            |()| -> iced_css::capture::Captured<iced::widget::Space> {
                calls.set(calls.get() + 1);
                std::panic::panic_any(83u32)
            },
        );
        let _ = adapters::table_from_vecs::<_, Message, iced::Theme, ()>(
            vec![col],
            vec![(), ()],
            limits(),
        );
    }));
    assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 83);
    assert_eq!(calls.get(), 1);
}
