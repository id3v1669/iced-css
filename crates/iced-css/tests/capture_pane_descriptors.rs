use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
use iced_css_core::source::AttributeLimits;
use std::num::NonZeroUsize;
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(100000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
fn attrs() -> AttributeLimits {
    let n = limits().nodes;
    AttributeLimits {
        entries: n,
        retained_bytes: n,
        work: n,
    }
}
fn text(value: &str) -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::text(value.to_owned(), limits()).unwrap().erase()
}
#[test]
fn optional_roles_replace_in_source_order_and_preserve_descriptor_keys() {
    let title = adapters::pane_title(text("title"), limits())
        .unwrap()
        .source_key(b"title-key", limits())
        .unwrap()
        .style_classes(&["heading"], attrs())
        .unwrap();
    assert_eq!(title.construction().nodes().len(), 2);
    let title = title
        .controls(
            adapters::pane_controls_dynamic(text("old-full"), text("old-compact"), limits())
                .unwrap(),
            limits(),
        )
        .unwrap()
        .controls(
            adapters::pane_controls(text("new-full"), limits()).unwrap(),
            limits(),
        )
        .unwrap();
    let nodes = title.construction().nodes();
    assert_eq!(
        nodes.iter().map(|n| n.canonical_type()).collect::<Vec<_>>(),
        ["text", "text", "pane_controls", "pane_title"]
    );
    assert_eq!(
        nodes.iter().map(|n| n.role()).collect::<Vec<_>>(),
        [Some("content"), Some("full"), Some("controls"), None]
    );
    assert_eq!(nodes[3].source_key(), Some(b"title-key".as_slice()));
    assert!(nodes[3].metadata().has_class("heading"));
    let content = adapters::pane_content(text("body"), limits())
        .unwrap()
        .source_key(b"pane-key", limits())
        .unwrap();
    assert_eq!(content.construction().nodes()[0].role(), Some("body"));
    let content = content.title_bar(title, limits()).unwrap();
    let nodes = content.construction().nodes();
    assert_eq!(
        nodes.iter().map(|n| n.parent()).collect::<Vec<_>>(),
        [Some(3), Some(2), Some(3), Some(5), Some(5), None]
    );
    assert_eq!(nodes[3].role(), Some("title"));
    assert_eq!(nodes[4].role(), Some("body"));
    assert_eq!(nodes[5].source_key(), Some(b"pane-key".as_slice()));
    let content = content
        .title_bar(
            adapters::pane_title(text("replacement"), limits()).unwrap(),
            limits(),
        )
        .unwrap();
    assert_eq!(
        content
            .construction()
            .nodes()
            .iter()
            .map(|n| n.text())
            .collect::<Vec<_>>(),
        [Some("replacement"), None, Some("body"), None]
    );
    assert_eq!(content.construction().depth(), 3);
    assert_eq!(
        content.construction().nodes()[3].source_key(),
        Some(b"pane-key".as_slice())
    );
}
#[test]
fn obsolete_subtree_storage_and_depth_do_not_count_against_replacement() {
    let fresh = || {
        adapters::pane_title(text("title"), limits())
            .unwrap()
            .controls(
                adapters::pane_controls(text("new"), limits()).unwrap(),
                limits(),
            )
            .unwrap()
    };
    let bytes = fresh().construction().retained_bytes();
    let mut old = text(&"x".repeat(4000))
        .source_key(&[42; 2048], limits())
        .unwrap()
        .style_attribute("data-old", &"y".repeat(2048), attrs())
        .unwrap();
    for _ in 0..6 {
        old = adapters::container(old, limits()).unwrap().erase();
    }
    let title = adapters::pane_title(text("title"), limits())
        .unwrap()
        .controls(adapters::pane_controls(old, limits()).unwrap(), limits())
        .unwrap();
    assert!(title.construction().retained_bytes() > bytes);
    assert!(title.construction().depth() > 3);
    let mut cap = limits();
    cap.retained_bytes = NonZeroUsize::new(bytes).unwrap();
    cap.depth = NonZeroUsize::new(3).unwrap();
    cap.nodes = NonZeroUsize::new(4).unwrap();
    let title = title
        .controls(adapters::pane_controls(text("new"), limits()).unwrap(), cap)
        .unwrap();
    assert_eq!(title.construction().retained_bytes(), bytes);
    assert_eq!(title.construction().depth(), 3);
}
#[test]
fn optional_assembly_and_constructors_observe_each_limit() {
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
        assert!(matches!(adapters::pane_title(text("title"),cap),Err(e) if e==expected));
        assert!(matches!(adapters::pane_content(text("body"),cap),Err(e) if e==expected));
        assert!(
            matches!(adapters::pane_title(text("title"),limits()).unwrap().controls(adapters::pane_controls(text("full"),limits()).unwrap(),cap),Err(e) if e==expected)
        );
        assert!(
            matches!(adapters::pane_content(text("body"),limits()).unwrap().title_bar(adapters::pane_title(text("title"),limits()).unwrap(),cap),Err(e) if e==expected)
        );
    }
}

fn native_observation<'a>(
    state: &'a iced::widget::pane_grid::State<()>,
    content: iced::widget::pane_grid::Content<'a, (), iced::Theme, ()>,
) -> Vec<iced::Rectangle> {
    use iced::{
        Element, Size,
        advanced::{layout, widget::Tree},
        widget::pane_grid,
    };
    let content = std::cell::RefCell::new(Some(content));
    let mut e: Element<'_, (), iced::Theme, ()> =
        pane_grid::PaneGrid::new(state, |_, _, _| content.borrow_mut().take().unwrap()).into();
    let mut tree = Tree::new(&e);
    let node = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(150.0, 80.0)),
    );
    fn flatten(node: &layout::Node, out: &mut Vec<iced::Rectangle>) {
        out.push(node.bounds());
        for child in node.children() {
            flatten(child, out);
        }
    }
    let mut rects = Vec::new();
    flatten(&node, &mut rects);
    e.as_widget().draw(
        &tree,
        &mut (),
        &iced::Theme::Dark,
        &iced::advanced::renderer::Style::default(),
        iced::advanced::Layout::new(&node),
        iced::mouse::Cursor::Unavailable,
        &iced::Rectangle::with_size(Size::new(150.0, 80.0)),
    );
    rects
}
#[test]
fn builders_and_replaced_roles_match_native_layout_and_deferred_drawing() {
    use iced::widget::{Space, container, pane_grid};
    use std::cell::{Cell, RefCell};
    struct Pad<'a>(&'a Cell<usize>);
    impl From<Pad<'_>> for iced::Padding {
        fn from(v: Pad<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            iced::Padding::new(3.0)
        }
    }
    for class in [false, true] {
        for title_present in [false, true] {
            let mut observations = Vec::new();
            for capture in [false, true] {
                let (state, _) = pane_grid::State::new(());
                let log = RefCell::new(Vec::new());
                let count = Cell::new(0);
                let style = |name| -> container::StyleFn<'_, iced::Theme> {
                    let log = &log;
                    Box::new(move |_| {
                        log.borrow_mut().push(name);
                        container::Style::default()
                    })
                };
                let space = |w| {
                    adapters::space(limits())
                        .unwrap()
                        .width(w)
                        .height(10.0)
                        .erase::<(), iced::Theme, ()>()
                };
                let content = if capture {
                    let body = adapters::container(space(20.0), limits())
                        .unwrap()
                        .style(style("body-child"));
                    let content = adapters::pane_content(body, limits()).unwrap();
                    let content = if class {
                        content.class(style("content"))
                    } else {
                        content.style(style("content"))
                    };
                    let content = if title_present {
                        let title = adapters::pane_title(space(25.0), limits())
                            .unwrap()
                            .padding(Pad(&count))
                            .always_show_controls();
                        let title = if class {
                            title.class(style("title"))
                        } else {
                            title.style(style("title"))
                        };
                        let old = adapters::pane_controls(
                            adapters::container(space(60.0), limits())
                                .unwrap()
                                .style(style("obsolete-controls")),
                            limits(),
                        )
                        .unwrap();
                        let new = adapters::pane_controls(
                            adapters::container(space(5.0), limits())
                                .unwrap()
                                .style(style("controls")),
                            limits(),
                        )
                        .unwrap();
                        let title = title
                            .controls(old, limits())
                            .unwrap()
                            .controls(new, limits())
                            .unwrap();
                        content
                            .title_bar(
                                adapters::pane_title(space(80.0), limits())
                                    .unwrap()
                                    .style(style("obsolete-title")),
                                limits(),
                            )
                            .unwrap()
                            .title_bar(title, limits())
                            .unwrap()
                    } else {
                        content
                    };
                    content.into_parts().0
                } else {
                    let body =
                        container(Space::new().width(20.0).height(10.0)).style(style("body-child"));
                    let content = pane_grid::Content::new(body);
                    let content = if class {
                        content.class(style("content"))
                    } else {
                        content.style(style("content"))
                    };
                    if title_present {
                        let title = pane_grid::TitleBar::new(Space::new().width(25.0).height(10.0))
                            .padding(Pad(&count))
                            .always_show_controls();
                        let title = if class {
                            title.class(style("title"))
                        } else {
                            title.style(style("title"))
                        };
                        let old = pane_grid::Controls::new(
                            container(Space::new().width(60.0).height(10.0))
                                .style(style("obsolete-controls")),
                        );
                        let new = pane_grid::Controls::new(
                            container(Space::new().width(5.0).height(10.0))
                                .style(style("controls")),
                        );
                        content
                            .title_bar(
                                pane_grid::TitleBar::new(Space::new().width(80.0).height(10.0))
                                    .style(style("obsolete-title")),
                            )
                            .title_bar(title.controls(old).controls(new))
                    } else {
                        content
                    }
                };
                assert_eq!(count.get(), usize::from(title_present));
                assert!(log.borrow().is_empty());
                let geometry = native_observation(&state, content);
                let log = log.into_inner();
                assert!(log.contains(&"content") && log.contains(&"body-child"));
                assert_eq!(log.contains(&"title"), title_present);
                assert_eq!(log.contains(&"controls"), title_present);
                assert!(!log.contains(&"obsolete-title") && !log.contains(&"obsolete-controls"));
                observations.push((geometry, log));
            }
            assert_eq!(observations[0], observations[1]);
        }
    }
}

#[test]
fn mandatory_control_labels_remain_charged_when_a_title_is_added() {
    for kind in 0..4 {
        let label = "x".repeat(1000);
        let body: CapturedElement<'_, (), iced::Theme, ()> = match kind {
            0 => adapters::checkbox(false, limits())
                .unwrap()
                .label(label.clone(), limits())
                .unwrap()
                .erase(),
            1 => adapters::toggler(false, limits())
                .unwrap()
                .label(label.clone(), limits())
                .unwrap()
                .erase(),
            2 => adapters::radio(label.clone(), 0u8, None, |_| (), limits())
                .unwrap()
                .erase(),
            _ => adapters::text(label.clone(), limits()).unwrap().erase(),
        };
        let pane = adapters::pane_content(body, limits()).unwrap();
        let title = adapters::pane_title(adapters::space(limits()).unwrap(), limits()).unwrap();
        let mut cap = limits();
        // Four source records plus one byte less than the mandatory label itself.
        cap.retained_bytes =
            NonZeroUsize::new(4 * std::mem::size_of::<iced_css::capture::ConstructionNode>() + 999)
                .unwrap();
        assert!(matches!(
            pane.title_bar(title, cap),
            Err(CaptureError::Bytes)
        ));
    }
}
