use iced::{
    Element, Event, Point, Rectangle, Size, Vector,
    advanced::{Layout, Shell, layout, widget::Tree},
    mouse,
};
use iced_css::capture::KeyedColumnRecipe;
use iced_css::capture::{CaptureLimits, CapturedElement, ColumnRecipe, RowRecipe, adapters};
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
enum Recipe {
    Row(RowRecipe<'static, u8, iced::Theme, ()>),
    Column(ColumnRecipe<'static, u8, iced::Theme, ()>),
    Keyed(KeyedColumnRecipe<'static, u8, u8, iced::Theme, ()>),
}
impl Recipe {
    fn children_mut(&mut self) -> &mut [Element<'static, u8, iced::Theme, ()>] {
        match self {
            Self::Row(r) => r.native_children_mut(),
            Self::Column(r) => r.native_children_mut(),
            Self::Keyed(r) => r.native_children_mut(),
        }
    }
    fn lower(self) -> Element<'static, u8, iced::Theme, ()> {
        match self {
            Self::Row(r) => r.into_native().into(),
            Self::Column(r) => r.into_native().into(),
            Self::Keyed(r) => r.into_native().into(),
        }
    }
}
fn recipe(family: u8) -> Recipe {
    let child: CapturedElement<'static, u8, iced::Theme, ()> =
        adapters::pick_list([1u8, 2], Some(1u8), |v| v, limits())
            .unwrap()
            .width(70)
            .on_open(100)
            .on_close(101)
            .erase();
    if family == 2 {
        Recipe::Keyed(
            adapters::keyed_column_from_vecs(vec![0u8], vec![child], limits())
                .unwrap()
                .into_parts()
                .0,
        )
    } else if family == 1 {
        Recipe::Column(
            adapters::column_from_vec(vec![child], limits())
                .unwrap()
                .into_parts()
                .0,
        )
    } else {
        Recipe::Row(
            adapters::row_from_vec(vec![child], limits())
                .unwrap()
                .into_parts()
                .0,
        )
    }
}
#[test]
fn raw_recipe_borrows_live_overlay_then_lowers_without_recreating_child_state() {
    for family in [0, 1, 2] {
        let mut recipe = recipe(family);
        let viewport = Rectangle::with_size(Size::new(200.0, 200.0));
        let child = &mut recipe.children_mut()[0];
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
        let mut parent = recipe.lower();
        let mut parent_tree = Tree {
            tag: parent.as_widget().tag(),
            state: parent.as_widget().state(),
            children: vec![tree],
        };
        parent.as_widget().diff(&mut parent_tree);
        let parent_layout = parent.as_widget_mut().layout(
            &mut parent_tree,
            &(),
            &layout::Limits::new(Size::ZERO, viewport.size()),
        );
        let preserved = match &parent_tree.children[0].state {
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
    }
}
