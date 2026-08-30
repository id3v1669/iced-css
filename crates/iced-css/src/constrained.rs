use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{tree, Operation, Tree, Widget};
use iced::advanced::{overlay, renderer, Clipboard, Shell};
use iced::{Element, Event, Length, Rectangle, Size, Vector};
use iced_css_core::Length as Css;

pub struct Constrained<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    width: Option<Css>,
    height: Option<Css>,
    min_width: Option<Css>,
    max_width: Option<Css>,
    min_height: Option<Css>,
    max_height: Option<Css>,
    pad_x: f32,
    pad_y: f32,
}

impl<'a, Message, Theme, Renderer> Constrained<'a, Message, Theme, Renderer> {
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        resolved: &iced_css_core::Resolved,
    ) -> Self {
        let padding = resolved.padding.unwrap_or_default();
        Constrained {
            content: content.into(),
            width: resolved.width,
            height: resolved.height,
            min_width: resolved.min_width,
            max_width: resolved.max_width,
            min_height: resolved.min_height,
            max_height: resolved.max_height,
            pad_x: padding.left + padding.right,
            pad_y: padding.top + padding.bottom,
        }
    }
}

fn resolve_x(length: Css, available: f32) -> f32 {
    match length {
        Css::Px(v) => v,
        Css::Percent(f) => f * available,
        Css::Auto => available,
    }
}

fn constraint(length: Option<Css>, available: f32) -> Option<f32> {
    match length? {
        Css::Px(v) => Some(v),
        Css::Percent(f) => Some(f * available),
        Css::Auto => None,
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Constrained<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        let width = match self.width {
            Some(Css::Px(v)) => Length::Fixed(v + self.pad_x),
            Some(Css::Percent(_) | Css::Auto) | None => Length::Fill,
        };
        let height = match self.height {
            Some(Css::Px(v)) => Length::Fixed(v + self.pad_y),
            Some(Css::Percent(_)) => Length::Fill,
            Some(Css::Auto) | None => self.content.as_widget().size().height,
        };
        Size { width, height }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let available = limits.max();

        let clamp_x = |width: f32| -> f32 {
            let mut width = width;
            if let Some(max) = constraint(self.max_width, available.width) {
                width = width.min(max);
            }
            if let Some(min) = constraint(self.min_width, available.width) {
                width = width.max(min);
            }
            width
        };
        let clamp_y = |height: f32| -> f32 {
            let mut height = height;
            if let Some(max) = constraint(self.max_height, available.height) {
                height = height.min(max);
            }
            if let Some(min) = constraint(self.min_height, available.height) {
                height = height.max(min);
            }
            height
        };

        let content_width = match self.width.unwrap_or(Css::Auto) {
            Css::Auto => (available.width - self.pad_x).max(0.0),
            other => resolve_x(other, available.width),
        };
        let final_width = clamp_x(content_width) + self.pad_x;

        // CSS 10.5 "The percentage is calculated with respect to the height of the generated box’s containing block"
        // says a % height against an auto-height containing block computes
        // the containing block size is not visible with iced limits, so that case is tracked in unimplemented.rs
        let explicit_height = match self.height {
            Some(Css::Px(v)) => Some(v),
            Some(Css::Percent(f)) => Some(f * available.height),
            Some(Css::Auto) | None => None,
        };
        let final_height = explicit_height.map(|h| clamp_y(h) + self.pad_y);

        let child_min_h = final_height
            .or_else(|| constraint(self.min_height, available.height).map(|min| min + self.pad_y))
            .unwrap_or(0.0);
        let child_max_h = final_height
            .or_else(|| constraint(self.max_height, available.height).map(|max| max + self.pad_y))
            .unwrap_or(available.height)
            .max(child_min_h);

        let child_limits = layout::Limits::new(
            Size::new(0.0, child_min_h),
            Size::new(final_width, child_max_h),
        );
        let content = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &child_limits);

        let size = Size::new(
            final_width,
            final_height.unwrap_or_else(|| {
                clamp_y((content.size().height - self.pad_y).max(0.0)) + self.pad_y
            }),
        );

        layout::Node::with_children(size, vec![content])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                tree,
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: iced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
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
        cursor: iced::mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> iced::mouse::Interaction {
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
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            tree,
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            tree,
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Constrained<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(constrained: Constrained<'a, Message, Theme, Renderer>) -> Self {
        Element::new(constrained)
    }
}
