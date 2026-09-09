use iced::{
    Element, Event, Point, Rectangle, Size, Vector,
    advanced::{Layout, Shell, layout, widget::Tree},
    mouse,
};
use iced_css::capture::{CaptureLimits, adapters};
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
#[test]
fn container_recipe_borrows_menu_then_erases_with_direct_content_state() {
    {
        let child = adapters::pick_list([1u8, 2], Some(1u8), |v| v, limits())
            .unwrap()
            .width(70)
            .on_open(100)
            .on_close(101);
        let (mut recipe, _) = adapters::container::<u8, iced::Theme, (), _>(child, limits())
            .unwrap()
            .padding(3)
            .into_parts();
        let viewport = Rectangle::with_size(Size::new(200.0, 200.0));
        let child = recipe.native_content_mut();
        let mut tree = Tree::new(&*child);
        let initial_state = match &tree.state {
            iced::advanced::widget::tree::State::Some(state) => {
                std::ptr::from_ref(&**state) as *const ()
            }
            _ => panic!("native pick-list state"),
        };
        let node = child.as_widget_mut().layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let mut messages = vec![];
        child.as_widget_mut().update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(1.0, 1.0)),
            &(),
            &mut iced_test::core::clipboard::Null,
            &mut Shell::new(&mut messages),
            &viewport,
        );
        assert_eq!(messages, [100]);
        {
            let mut overlay = child
                .as_widget_mut()
                .overlay(&mut tree, Layout::new(&node), &(), &viewport, Vector::ZERO)
                .expect("raw child overlay");
            let overlay_layout = overlay.as_overlay_mut().layout(&(), viewport.size());
            overlay.as_overlay().draw(
                &mut (),
                &iced::Theme::Dark,
                &iced::advanced::renderer::Style::default(),
                Layout::new(&overlay_layout),
                mouse::Cursor::Unavailable,
            );
        }
        let mut parent: Element<'_, u8, iced::Theme, ()> = recipe.into();
        let mut parent_tree = tree;
        parent.as_widget().diff(&mut parent_tree);
        let parent_layout = parent.as_widget_mut().layout(
            &mut parent_tree,
            &(),
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let preserved = match &parent_tree.state {
            iced::advanced::widget::tree::State::Some(state) => {
                std::ptr::from_ref(&**state) as *const ()
            }
            _ => panic!("native pick-list state"),
        };
        assert_eq!(preserved, initial_state);
        assert!(
            parent
                .as_widget_mut()
                .overlay(
                    &mut parent_tree,
                    Layout::new(&parent_layout),
                    &(),
                    &viewport,
                    Vector::ZERO
                )
                .is_some()
        );
        assert_eq!(messages, [100]);
        let position = parent_layout.children()[0].bounds().center();
        let mut shell = Shell::new(&mut messages);
        parent.as_widget_mut().update(
            &mut parent_tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&parent_layout),
            mouse::Cursor::Available(position),
            &(),
            &mut iced_test::core::clipboard::Null,
            &mut shell,
            &viewport,
        );
        assert!(shell.is_event_captured());
        assert_eq!(messages, [100, 101]);
        assert!(
            parent
                .as_widget_mut()
                .overlay(
                    &mut parent_tree,
                    Layout::new(&parent_layout),
                    &(),
                    &viewport,
                    Vector::ZERO
                )
                .is_none()
        );
    }
}
