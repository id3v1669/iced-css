use super::SafeTable;
use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    widget::table::Catalog,
};
impl<M, Theme: Catalog, R: Renderer> Widget<M, Theme, R> for SafeTable<'_, M, Theme, R> {
    fn size(&self) -> Size<Length> {
        self.inner.size()
    }
    fn size_hint(&self) -> Size<Length> {
        self.inner.size_hint()
    }
    fn tag(&self) -> tree::Tag {
        self.inner.tag()
    }
    fn state(&self) -> tree::State {
        self.inner.state()
    }
    fn children(&self) -> Vec<Tree> {
        self.inner.children()
    }
    fn diff(&self, tree: &mut Tree) {
        self.inner.diff(tree);
    }
    fn layout(&mut self, tree: &mut Tree, renderer: &R, limits: &layout::Limits) -> layout::Node {
        if self.empty {
            // The native Metrics type is private; its public state constructor
            // resets prior nonempty separator geometry without vendoring iced.
            tree.state = self.inner.state();
            let size = self.inner.size();
            let limits = limits.width(size.width).height(size.height);
            layout::Node::new(limits.resolve(size.width, size.height, Size::ZERO))
        } else {
            self.inner.layout(tree, renderer, limits)
        }
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut R,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.inner
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
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
        self.inner.update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &R,
        operation: &mut dyn Operation,
    ) {
        self.inner.operate(tree, layout, renderer, operation);
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &R,
    ) -> mouse::Interaction {
        self.inner
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &R,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, Theme, R>> {
        self.inner
            .overlay(tree, layout, renderer, viewport, translation)
    }
}
impl<'a, M: 'a, Theme: Catalog + 'a, R: Renderer + 'a> From<SafeTable<'a, M, Theme, R>>
    for Element<'a, M, Theme, R>
{
    fn from(table: SafeTable<'a, M, Theme, R>) -> Self {
        Element::new(table)
    }
}
