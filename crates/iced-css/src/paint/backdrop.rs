//! Q287's opaque native canvas, independent of CSS background transparency.
use iced::{Color, window::Settings};
use std::num::NonZeroU64;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpaqueBackdrop {
    rgb: [f32; 3],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackdropError {
    InvalidColor,
    Transparency,
    Blur,
    RevisionExhausted,
}
impl std::fmt::Display for BackdropError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidColor => "native canvas channels must be finite and within 0..=1",
            Self::Transparency => "managed native canvas does not support desktop transparency",
            Self::Blur => "managed native canvas does not support desktop blur",
            Self::RevisionExhausted => "native canvas backdrop revision exhausted",
        })
    }
}
impl std::error::Error for BackdropError {}
fn valid(channel: f32) -> bool {
    channel.is_finite() && (0.0..=1.0).contains(&channel)
}
impl OpaqueBackdrop {
    pub fn new(red: f32, green: f32, blue: f32) -> Result<Self, BackdropError> {
        let rgb = [red, green, blue];
        if !rgb.iter().all(|channel| valid(*channel)) {
            return Err(BackdropError::InvalidColor);
        }
        Ok(Self {
            rgb: rgb.map(|channel| if channel == 0.0 { 0.0 } else { channel }),
        })
    }
    pub fn color(self) -> Color {
        Color::from_rgb(self.rgb[0], self.rgb[1], self.rgb[2])
    }
}
/// An explicit validated override takes precedence. Otherwise a valid opaque
/// theme background is used, with opaque white for valid transparent themes.
pub fn select_backdrop(
    explicit: Option<OpaqueBackdrop>,
    theme: Color,
) -> Result<OpaqueBackdrop, BackdropError> {
    if let Some(explicit) = explicit {
        return Ok(explicit);
    }
    if ![theme.r, theme.g, theme.b, theme.a].into_iter().all(valid) {
        return Err(BackdropError::InvalidColor);
    }
    if theme.a == 1.0 {
        OpaqueBackdrop::new(theme.r, theme.g, theme.b)
    } else {
        OpaqueBackdrop::new(1.0, 1.0, 1.0)
    }
}
pub fn validate_window(settings: &Settings) -> Result<(), BackdropError> {
    if settings.transparent {
        return Err(BackdropError::Transparency);
    }
    if settings.blur {
        return Err(BackdropError::Blur);
    }
    Ok(())
}
/// Revision is scoped to the owning window state. It is not a global cache key;
/// caches must also carry their window/backend identity and geometry generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackdropSnapshot {
    backdrop: OpaqueBackdrop,
    revision: NonZeroU64,
}
impl BackdropSnapshot {
    pub fn backdrop(self) -> OpaqueBackdrop {
        self.backdrop
    }
    pub fn revision(self) -> NonZeroU64 {
        self.revision
    }
}
pub struct BackdropState {
    accepted: BackdropSnapshot,
    limit: NonZeroU64,
}
impl BackdropState {
    pub fn new(backdrop: OpaqueBackdrop, revision_limit: NonZeroU64) -> Self {
        Self {
            accepted: BackdropSnapshot {
                backdrop,
                revision: NonZeroU64::MIN,
            },
            limit: revision_limit,
        }
    }
    pub fn snapshot(&self) -> BackdropSnapshot {
        self.accepted
    }
    /// A changed result requires owner-side prefix invalidation before painting.
    /// Exhaustion leaves the accepted snapshot unchanged; equal RGB is a no-op.
    pub fn replace(&mut self, backdrop: OpaqueBackdrop) -> Result<bool, BackdropError> {
        if backdrop == self.accepted.backdrop {
            return Ok(false);
        }
        let revision = self
            .accepted
            .revision
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .filter(|revision| *revision <= self.limit)
            .ok_or(BackdropError::RevisionExhausted)?;
        self.accepted = BackdropSnapshot { backdrop, revision };
        Ok(true)
    }
}
