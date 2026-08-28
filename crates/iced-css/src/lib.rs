pub use iced_css_macro::style;

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
