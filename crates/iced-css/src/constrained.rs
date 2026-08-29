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
}

impl<'a, Message, Theme, Renderer> Constrained<'a, Message, Theme, Renderer> {
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        width: Option<Css>,
        height: Option<Css>,
        min_width: Option<Css>,
        max_width: Option<Css>,
    ) -> Self {
        Constrained {
            content: content.into(),
            width,
            height,
            min_width,
            max_width,
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
            Some(Css::Px(v)) => Length::Fixed(v),
            Some(Css::Percent(_) | Css::Auto) => Length::Fill,
            None => self.content.as_widget().size().width,
        };
        let height = match self.height {
            Some(Css::Px(v)) => Length::Fixed(v),
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

        let explicit_width = self.width.map(|w| resolve_x(w, available.width));
        let explicit_height = match self.height {
            Some(Css::Px(v)) => Some(v),
            Some(Css::Percent(f)) => Some(f * available.height),
            Some(Css::Auto) | None => None,
        };

        let clamp = |width: f32| -> f32 {
            let mut width = width;
            if let Some(max) = self.max_width {
                width = width.min(resolve_x(max, available.width));
            }
            if let Some(min) = self.min_width {
                width = width.max(resolve_x(min, available.width));
            }
            width
        };

        let child_max = Size::new(
            explicit_width.map(clamp).unwrap_or(available.width),
            explicit_height.unwrap_or(available.height),
        );
        let child_min = Size::new(
            explicit_width.map(clamp).unwrap_or(0.0),
            explicit_height.unwrap_or(0.0),
        );
        let child_limits = layout::Limits::new(child_min, child_max);
        let content = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &child_limits);

        let size = Size::new(
            clamp(explicit_width.unwrap_or(content.size().width)),
            explicit_height.unwrap_or(content.size().height),
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
