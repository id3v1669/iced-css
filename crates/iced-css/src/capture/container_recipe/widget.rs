//! Keep content state transparent while delegating geometry/paint primitives to iced.
use super::ContainerRecipe;
use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{
            Operation, Tree,
            tree::{State, Tag},
        },
    },
    widget::container::{self, Catalog},
};
impl<M, T: Catalog, R: Renderer> Widget<M, T, R> for ContainerRecipe<'_, M, T, R> {
    fn tag(&self) -> Tag {
        self.content.as_widget().tag()
    }
    fn state(&self) -> State {
        self.content.as_widget().state()
    }
    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }
    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }
    fn size(&self) -> Size<Length> {
        self.size
    }
    fn layout(&mut self, tree: &mut Tree, renderer: &R, limits: &layout::Limits) -> layout::Node {
        container::layout(
            limits,
            self.size.width,
            self.size.height,
            self.maximum.width,
            self.maximum.height,
            self.padding,
            self.horizontal,
            self.vertical,
            |child_limits| {
                self.content
                    .as_widget_mut()
                    .layout(tree, renderer, child_limits)
            },
        )
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &R,
        operation: &mut dyn Operation,
    ) {
        operation.container(self.id.as_ref(), layout.bounds());
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                tree,
                layout.children().next().unwrap(),
                renderer,
                operation,
            )
        });
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &R,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            tree,
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &R,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            tree,
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut R,
        theme: &T,
        inherited: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let appearance = self.presentation.apply(theme.style(&self.class));
        if let Some(intersection) = bounds.intersection(viewport) {
            container::draw_background(renderer, &appearance, bounds);
            let inherited = renderer::Style {
                text_color: appearance.text_color.unwrap_or(inherited.text_color),
            };
            self.content.as_widget().draw(
                tree,
                renderer,
                theme,
                &inherited,
                layout.children().next().unwrap(),
                cursor,
                if self.clip { &intersection } else { viewport },
            );
        }
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &R,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, T, R>> {
        self.content.as_widget_mut().overlay(
            tree,
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
impl<'a, M: 'a, T: Catalog + 'a, R: Renderer + 'a> From<ContainerRecipe<'a, M, T, R>>
    for Element<'a, M, T, R>
{
    fn from(value: ContainerRecipe<'a, M, T, R>) -> Self {
        Element::new(value)
    }
}
