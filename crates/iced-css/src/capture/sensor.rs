//! Sensor source capture and native continuity-key transitions.
use super::{CaptureError, CaptureLimits, Captured};
use iced::{
    Element, Pixels, Size,
    time::Duration,
    widget::sensor::{Key as NativeKey, Sensor},
};
/// Preserve one Content source role before native Sensor construction.
///
/// ```compile_fail,E0308
/// # use iced_css::capture::{adapters, CaptureLimits};
/// fn bypass<'a>(child: iced::Element<'a, (), iced::Theme, ()>, limits: CaptureLimits) {
///     let _ = adapters::sensor(child, limits);
/// }
/// ```
///
/// # Errors
/// Rejects aggregate evidence bounds/allocation before native child conversion.
pub fn sensor<'a, Message, Theme, Renderer, W>(
    child: Captured<W>,
    limits: CaptureLimits,
) -> Result<Captured<Sensor<'a, (), Message, Theme, Renderer>>, CaptureError>
where
    Renderer: iced::advanced::Renderer,
    W: Into<Element<'a, Message, Theme, Renderer>>,
{
    let Captured {
        payload,
        construction,
    } = child;
    let construction = construction.sensor(limits)?;
    Ok(Captured {
        payload: Sensor::new(payload),
        construction,
    })
}
impl<'a, Key, Message, Theme, Renderer> Captured<Sensor<'a, Key, Message, Theme, Renderer>>
where
    Key: NativeKey,
    Renderer: iced::advanced::Renderer,
{
    /// Store the native visibility callback without evaluating it.
    #[must_use]
    pub fn on_show(mut self, callback: impl Fn(Size) -> Message + 'a) -> Self {
        self.payload = self.payload.on_show(callback);
        self
    }
    #[must_use]
    pub fn on_resize(mut self, callback: impl Fn(Size) -> Message + 'a) -> Self {
        self.payload = self.payload.on_resize(callback);
        self
    }
    #[must_use]
    pub fn on_hide(mut self, message: Message) -> Self {
        self.payload = self.payload.on_hide(message);
        self
    }
    /// Change the native continuity key without changing source metadata.
    #[must_use]
    pub fn key<K>(self, key: K) -> Captured<Sensor<'a, impl NativeKey, Message, Theme, Renderer>>
    where
        K: Clone + PartialEq + 'static,
    {
        Captured {
            payload: self.payload.key(key),
            construction: self.construction,
        }
    }
    /// Preserve the borrowed native key and the existing source allocation.
    #[must_use]
    pub fn key_ref<K>(self, key: &'a K) -> Captured<Sensor<'a, &'a K, Message, Theme, Renderer>>
    where
        K: ToOwned + PartialEq<K::Owned> + ?Sized,
        K::Owned: 'static,
    {
        Captured {
            payload: self.payload.key_ref(key),
            construction: self.construction,
        }
    }
    #[must_use]
    pub fn anticipate(mut self, distance: impl Into<Pixels>) -> Self {
        self.payload = self.payload.anticipate(distance);
        self
    }
    #[must_use]
    pub fn delay(mut self, delay: impl Into<Duration>) -> Self {
        self.payload = self.payload.delay(delay);
        self
    }
}
