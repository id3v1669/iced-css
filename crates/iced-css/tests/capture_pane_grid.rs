use iced::widget::{Space, pane_grid};
use iced_css::capture::{CaptureError, CaptureLimits, adapters};
use std::{cell::RefCell, num::NonZeroUsize};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(100000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
#[test]
fn eager_view_and_native_pane_identity_survive_maximization_and_reordering() {
    let (mut state, first) = pane_grid::State::new("first");
    let (second, _) = state
        .split(pane_grid::Axis::Vertical, first, "second")
        .unwrap();
    for maximized in [false, true] {
        if maximized {
            state.maximize(second);
        } else {
            state.restore();
        }
        for swap in [false, true] {
            if swap {
                state.swap(first, second);
            }
            let captured_log = RefCell::new(Vec::new());
            let grid = adapters::pane_grid(
                &state,
                |pane, data, max| {
                    captured_log.borrow_mut().push((pane, *data, max));
                    adapters::pane_content(adapters::space(limits()).unwrap(), limits()).unwrap()
                },
                limits(),
            )
            .unwrap();
            let _: &iced_css::capture::CapturedPaneGrid<'_, (), iced::Theme, ()> = &grid;
            let native_log = RefCell::new(Vec::new());
            let _: pane_grid::PaneGrid<'_, (), iced::Theme, ()> =
                pane_grid::PaneGrid::new(&state, |pane, data, max| {
                    native_log.borrow_mut().push((pane, *data, max));
                    pane_grid::Content::new(Space::new())
                });
            assert_eq!(*captured_log.borrow(), *native_log.borrow());
            assert_eq!(captured_log.borrow().len(), 2);
            let roots = grid
                .construction()
                .nodes()
                .iter()
                .filter(|n| n.canonical_type() == "pane_content")
                .collect::<Vec<_>>();
            assert_eq!(roots.len(), 2);
            for (i, node) in roots.iter().enumerate() {
                assert_eq!(node.pane_identity(), Some(captured_log.borrow()[i].0));
                assert_eq!(node.role(), Some("panes"));
                assert_eq!(node.role_position(), Some(i));
                assert_eq!(node.source_key(), None);
            }
            assert_eq!(
                grid.construction().nodes().last().unwrap().canonical_type(),
                "pane_grid"
            );
        }
    }
    state.restore();
    state.close(first).unwrap();
    let grid = adapters::pane_grid(
        &state,
        |_, _, _| adapters::pane_content(adapters::space(limits()).unwrap(), limits()).unwrap(),
        limits(),
    )
    .unwrap();
    let _: &iced_css::capture::CapturedPaneGrid<'_, (), iced::Theme, ()> = &grid;
    let node = &grid.construction().nodes()[1];
    assert_eq!(node.pane_identity(), Some(second));
    assert_eq!(node.role_position(), Some(0));
}
#[test]
fn preflight_rejects_before_view_and_aggregate_errors_do_not_replay_it() {
    let (state, _) = pane_grid::State::new(());
    let calls = std::cell::Cell::new(0);
    let view = |_, _, _| {
        calls.set(calls.get() + 1);
        adapters::pane_content(
            adapters::text("x".repeat(8000), limits()).unwrap(),
            limits(),
        )
        .unwrap()
    };
    let mut cap = limits();
    cap.nodes = NonZeroUsize::MIN;
    assert!(matches!(
        adapters::pane_grid::<_, (), iced::Theme, ()>(&state, view, cap),
        Err(CaptureError::Nodes)
    ));
    assert_eq!(calls.get(), 0);
    cap = limits();
    cap.retained_bytes = NonZeroUsize::new(4000).unwrap();
    assert!(matches!(
        adapters::pane_grid::<_, (), iced::Theme, ()>(&state, view, cap),
        Err(CaptureError::Bytes)
    ));
    assert_eq!(calls.get(), 1);
}

#[derive(Debug, PartialEq)]
enum Message {
    Click(pane_grid::Pane),
    Other,
}
fn observe(
    mut e: iced::Element<'_, Message, iced::Theme, ()>,
) -> (Vec<iced::Rectangle>, Vec<Message>, bool) {
    use iced::{
        Event, Point, Rectangle, Size,
        advanced::{Layout, Shell, layout, widget::Tree},
        mouse,
    };
    let mut tree = Tree::new(&e);
    e.as_widget().diff(&mut tree);
    let node = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
    );
    fn flatten(node: &layout::Node, out: &mut Vec<Rectangle>) {
        out.push(node.bounds());
        for c in node.children() {
            flatten(c, out);
        }
    }
    let mut geometry = Vec::new();
    flatten(&node, &mut geometry);
    let viewport = Rectangle::with_size(Size::new(200.0, 100.0));
    let cursor = mouse::Cursor::Available(Point::new(10.0, 10.0));
    e.as_widget().draw(
        &tree,
        &mut (),
        &iced::Theme::Dark,
        &iced::advanced::renderer::Style::default(),
        Layout::new(&node),
        cursor,
        &viewport,
    );
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    e.as_widget_mut().update(
        &mut tree,
        &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Layout::new(&node),
        cursor,
        &(),
        &mut iced_test::core::clipboard::Null,
        &mut shell,
        &viewport,
    );
    let captured = shell.is_event_captured();
    (geometry, messages, captured)
}
#[test]
fn builders_preserve_native_conditional_conversion_drawing_and_click_messages() {
    use std::cell::Cell;
    struct Len<'a>(&'a Cell<usize>, f32);
    impl From<Len<'_>> for iced::Length {
        fn from(v: Len<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            iced::Length::Fixed(v.1)
        }
    }
    struct Px<'a>(&'a Cell<usize>, f32);
    impl From<Px<'_>> for iced::Pixels {
        fn from(v: Px<'_>) -> Self {
            v.0.set(v.0.get() + 1);
            iced::Pixels(v.1)
        }
    }
    let (mut state, first) = pane_grid::State::new(String::from("borrowed first"));
    let (second, _) = state
        .split(
            pane_grid::Axis::Vertical,
            first,
            String::from("borrowed second"),
        )
        .unwrap();
    for maximized in [false, true] {
        if maximized {
            state.maximize(second);
        } else {
            state.restore();
        }
        for class in [false, true] {
            let mut observations = Vec::new();
            for capture in [false, true] {
                let conversions = Cell::new(0);
                let styles = Cell::new(0);
                let callbacks = Cell::new(0);
                let click = |pane| {
                    callbacks.set(callbacks.get() + 1);
                    Message::Click(pane)
                };
                let drag = |_| {
                    callbacks.set(callbacks.get() + 1);
                    Message::Other
                };
                let resize = |_| {
                    callbacks.set(callbacks.get() + 1);
                    Message::Other
                };
                let style: pane_grid::StyleFn<'_, iced::Theme> = Box::new(|theme| {
                    styles.set(styles.get() + 1);
                    pane_grid::default(theme)
                });
                let e = if capture {
                    let grid = adapters::pane_grid(
                        &state,
                        |_, data, _| {
                            adapters::pane_content(
                                adapters::text(data.as_str(), limits()).unwrap(),
                                limits(),
                            )
                            .unwrap()
                        },
                        limits(),
                    )
                    .unwrap()
                    .width(Len(&conversions, 180.0))
                    .height(Len(&conversions, 80.0))
                    .spacing(Px(&conversions, 4.0))
                    .min_size(Px(&conversions, 20.0))
                    .on_click(click)
                    .on_drag(drag)
                    .on_resize(Px(&conversions, 8.0), resize);
                    let grid = if class {
                        grid.class(style)
                    } else {
                        grid.style(style)
                    };
                    grid.erase().into_parts().0
                } else {
                    let grid = pane_grid::PaneGrid::new(&state, |_, data, _| {
                        pane_grid::Content::new(iced::widget::text(data.as_str()))
                    })
                    .width(Len(&conversions, 180.0))
                    .height(Len(&conversions, 80.0))
                    .spacing(Px(&conversions, 4.0))
                    .min_size(Px(&conversions, 20.0))
                    .on_click(click)
                    .on_drag(drag)
                    .on_resize(Px(&conversions, 8.0), resize);
                    let grid = if class {
                        grid.class(style)
                    } else {
                        grid.style(style)
                    };
                    grid.into()
                };
                assert_eq!(conversions.get(), if maximized { 4 } else { 5 });
                assert_eq!(styles.get(), 0);
                assert_eq!(callbacks.get(), 0);
                let result = observe(e);
                assert_eq!(
                    result.1,
                    [Message::Click(if maximized { second } else { first })]
                );
                assert_eq!(callbacks.get(), 1);
                assert!(styles.get() > 0);
                observations.push((result, styles.get()));
            }
            assert_eq!(observations[0], observations[1]);
        }
    }
}
#[test]
fn depth_and_work_limits_reject_without_a_partial_grid() {
    let (state, _) = pane_grid::State::new(());
    for depth in [false, true] {
        let mut cap = limits();
        if depth {
            cap.depth = NonZeroUsize::new(2).unwrap();
        } else {
            cap.work = NonZeroUsize::MIN;
        }
        let result = adapters::pane_grid::<_, (), iced::Theme, ()>(
            &state,
            |_, _, _| adapters::pane_content(adapters::space(limits()).unwrap(), limits()).unwrap(),
            cap,
        );
        assert!(
            matches!(result,Err(e) if e==if depth{CaptureError::Depth}else{CaptureError::Work})
        );
    }
}
