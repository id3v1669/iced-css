mod constrained;

pub use constrained::Constrained;
pub use iced_css_core::{Length, MarginValue, Margins, Resolved};
pub use iced_css_macro::style;

pub fn apply<'a, Message, Theme, Renderer>(
    element: impl Into<iced::Element<'a, Message, Theme, Renderer>>,
    resolved: Resolved,
) -> iced::Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: iced::widget::container::Catalog + 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    let mut element = element.into();

    if resolved.width.is_some()
        || resolved.height.is_some()
        || resolved.min_width.is_some()
        || resolved.max_width.is_some()
    {
        element = Constrained::new(
            element,
            resolved.width,
            resolved.height,
            resolved.min_width,
            resolved.max_width,
        )
        .into();
    }

    if let Some(margin) = resolved.margin {
        let px = |value: MarginValue| match value {
            MarginValue::Px(v) => v,
            MarginValue::Auto => 0.0,
        };
        let mut wrapper = iced::widget::container(element).padding(iced::Padding {
            top: px(margin.top),
            right: px(margin.right),
            bottom: px(margin.bottom),
            left: px(margin.left),
        });

        wrapper = match (margin.left, margin.right) {
            (MarginValue::Auto, MarginValue::Auto) => wrapper.center_x(iced::Length::Fill),
            (MarginValue::Auto, _) => wrapper
                .width(iced::Length::Fill)
                .align_x(iced::alignment::Horizontal::Right),
            (_, MarginValue::Auto) => wrapper
                .width(iced::Length::Fill)
                .align_x(iced::alignment::Horizontal::Left),
            _ => wrapper,
        };

        element = wrapper.into();
    }

    element
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    Compile,
    OnDemand,
    Auto,
}

#[derive(Debug, Clone)]
pub enum Event {
    Reload,
    Reloaded(Result<(), Error>),
    //to be used for internal events like hover
    #[doc(hidden)]
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "iced-css error: {}", self.0)
    }
}

impl std::error::Error for Error {}

/// OnDemand/Auto only
pub fn update(event: Event) {
    let _ = event;
    unimplemented!("iced-css: runtime update not implemented yet")
}

/// for auto
pub fn subscription<Message: From<Event> + Send + 'static>() -> iced::Subscription<Message> {
    unimplemented!("iced-css: file subscription not implemented yet")
}

pub trait StyleClasses<'a, Message> {
    fn style_classes<const N: usize>(
        self,
        classes: [&'static str; N],
    ) -> iced::Element<'a, Message>;

    fn style_id(self, id: &'static str) -> iced::Element<'a, Message>;
}

impl<'a, Message, T> StyleClasses<'a, Message> for T
where
    T: Into<iced::Element<'a, Message>>,
{
    fn style_classes<const N: usize>(
        self,
        classes: [&'static str; N],
    ) -> iced::Element<'a, Message> {
        let _ = classes;
        self.into()
    }

    fn style_id(self, id: &'static str) -> iced::Element<'a, Message> {
        let _ = id;
        self.into()
    }
}
