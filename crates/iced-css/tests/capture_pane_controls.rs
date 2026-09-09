use iced::{
    Element, Size,
    advanced::{layout, widget::Tree},
    widget::{Space, pane_grid},
};
use iced_css::capture::{CaptureError, CaptureLimits, CapturedElement, adapters};
use std::{cell::RefCell, num::NonZeroUsize};
fn limits() -> CaptureLimits {
    let n = NonZeroUsize::new(10000).unwrap();
    CaptureLimits {
        nodes: n,
        depth: n,
        retained_bytes: n,
        work: n,
    }
}
fn space(w: f32) -> CapturedElement<'static, (), iced::Theme, ()> {
    adapters::space(limits())
        .unwrap()
        .width(w)
        .height(10.0)
        .erase()
}
#[test]
fn full_and_compact_preserve_source_roles_and_nested_parents() {
    let single = adapters::pane_controls(space(30.0), limits()).unwrap();
    let n = single.construction().nodes();
    assert_eq!(n.len(), 2);
    assert_eq!(n[0].role(), Some("full"));
    assert_eq!(n[0].parent(), Some(1));
    assert_eq!(n[1].canonical_type(), "pane_controls");
    assert_eq!(n[1].text(), None);
    let dynamic = adapters::pane_controls_dynamic(
        space(30.0),
        adapters::row_from_vec(vec![space(0.0)], limits()).unwrap(),
        limits(),
    )
    .unwrap();
    let n = dynamic.construction().nodes();
    assert_eq!(
        n.iter().map(|n| n.canonical_type()).collect::<Vec<_>>(),
        ["space", "space", "row", "pane_controls"]
    );
    assert_eq!(
        n.iter().map(|n| n.parent()).collect::<Vec<_>>(),
        [Some(3), Some(2), Some(3), None]
    );
    assert_eq!(n[0].role(), Some("full"));
    assert_eq!(n[2].role(), Some("compact"));
}
#[test]
fn descriptor_assembly_checks_all_aggregate_limits() {
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
        assert!(matches!(adapters::pane_controls(space(30.0),cap),Err(e) if e==expected));
        assert!(
            matches!(adapters::pane_controls_dynamic(space(30.0),space(5.0),cap),Err(e) if e==expected)
        );
    }
}
fn native_observation(
    controls: pane_grid::Controls<'_, (), iced::Theme, ()>,
    width: f32,
) -> (String, Vec<iced::Rectangle>) {
    let (state, _) = pane_grid::State::new(());
    let controls = RefCell::new(Some(controls));
    let mut e: Element<'_, (), iced::Theme, ()> = pane_grid::PaneGrid::new(&state, |_, _, _| {
        pane_grid::Content::new(Space::new().width(20.0).height(20.0)).title_bar(
            pane_grid::TitleBar::new(Space::new().width(25.0).height(10.0))
                .controls(controls.borrow_mut().take().unwrap())
                .always_show_controls(),
        )
    })
    .into();
    let mut tree = Tree::new(&e);
    let slots = format!(
        "{:?}",
        tree.children
            .iter()
            .map(|pane| pane
                .children
                .iter()
                .map(|role| role.children.len())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    let layout = e.as_widget_mut().layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(width, 80.0)),
    );
    fn flatten(n: &layout::Node, out: &mut Vec<iced::Rectangle>) {
        out.push(n.bounds());
        for c in n.children() {
            flatten(c, out);
        }
    }
    let mut bounds = Vec::new();
    flatten(&layout, &mut bounds);
    e.as_widget().draw(
        &tree,
        &mut (),
        &iced::Theme::Dark,
        &iced::advanced::renderer::Style::default(),
        iced::advanced::Layout::new(&layout),
        iced::mouse::Cursor::Unavailable,
        &iced::Rectangle::with_size(Size::new(width, 80.0)),
    );
    (slots, bounds)
}
#[test]
fn native_descriptors_match_stock_pane_grid_at_both_available_widths() {
    for width in [40.0, 150.0] {
        for compact in [false, true] {
            let captured = if compact {
                adapters::pane_controls_dynamic(space(50.0), space(5.0), limits()).unwrap()
            } else {
                adapters::pane_controls(space(50.0), limits()).unwrap()
            };
            let expected = if compact {
                pane_grid::Controls::dynamic(
                    Space::new().width(50.0).height(10.0),
                    Space::new().width(5.0).height(10.0),
                )
            } else {
                pane_grid::Controls::new(Space::new().width(50.0).height(10.0))
            };
            assert_eq!(
                native_observation(captured.into_parts().0, width),
                native_observation(expected, width)
            );
        }
    }
}

#[test]
fn actual_drawing_selects_variant_without_changing_source_membership() {
    for (width, expected) in [(40.0, "compact"), (150.0, "full")] {
        let log = RefCell::new(Vec::new());
        let log_ref = &log;
        let variant = |width, name| {
            adapters::container(space(width), limits())
                .unwrap()
                .style(move |_| {
                    log_ref.borrow_mut().push(name);
                    iced::widget::container::Style::default()
                })
        };
        let controls = adapters::pane_controls_dynamic(
            variant(50.0, "full"),
            variant(5.0, "compact"),
            limits(),
        )
        .unwrap();
        assert!(log.borrow().is_empty());
        let (payload, evidence) = controls.into_parts();
        let before = evidence
            .nodes()
            .iter()
            .map(|n| (n.canonical_type(), n.parent(), n.role()))
            .collect::<Vec<_>>();
        native_observation(payload, width);
        assert_eq!(*log.borrow(), [expected]);
        assert_eq!(
            before,
            evidence
                .nodes()
                .iter()
                .map(|n| (n.canonical_type(), n.parent(), n.role()))
                .collect::<Vec<_>>()
        );
        assert_eq!(evidence.nodes().len(), 5);
    }
}
