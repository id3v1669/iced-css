use super::{CaptureError, CaptureLimits, Captured, CapturedElement, Construction};
use iced::{
    Alignment, Element, Length, Padding, Pixels, advanced::Renderer, widget::keyed::Column,
};

pub type CapturedKeyedColumn<'a, Key, Message, Theme = iced::Theme, R = iced::Renderer> =
    Captured<Column<'a, Key, Message, Theme, R>>;

/// Capture an empty native keyed column. Native keys do not encode source keys.
pub fn keyed_column_new<'a, Key, Message, Theme, R>(
    limits: CaptureLimits,
) -> Result<CapturedKeyedColumn<'a, Key, Message, Theme, R>, CaptureError>
where
    Key: Copy + PartialEq,
    R: Renderer,
{
    keyed_column_from_vecs(Vec::new(), Vec::new(), limits)
}
/// Retain aligned native keys and captured children without querying size hints.
/// Explicit child source_key metadata remains independent from native Key values.
pub fn keyed_column_from_vecs<'a, Key, Message, Theme, R>(
    keys: Vec<Key>,
    children: Vec<CapturedElement<'a, Message, Theme, R>>,
    limits: CaptureLimits,
) -> Result<CapturedKeyedColumn<'a, Key, Message, Theme, R>, CaptureError>
where
    Key: Copy + PartialEq,
    R: Renderer,
{
    if keys.len() != children.len() {
        return Err(CaptureError::Dimensions);
    }
    let (children, construction) = Construction::keyed_column(children, limits)?;
    Ok(Captured {
        payload: Column::from_vecs(keys, children),
        construction,
    })
}
/// Preserve an explicit native reservation; native allocation is outside evidence limits.
pub fn keyed_column_with_capacity<'a, Key, Message, Theme, R>(
    capacity: usize,
    limits: CaptureLimits,
) -> Result<CapturedKeyedColumn<'a, Key, Message, Theme, R>, CaptureError>
where
    Key: Copy + PartialEq,
    R: Renderer,
{
    let (_, construction) = Construction::keyed_column(Vec::<Captured<()>>::new(), limits)?;
    Ok(Captured {
        payload: Column::with_capacity(capacity),
        construction,
    })
}

/// Preserve native iterator acquisition and its single lower-bound capacity hint.
pub fn keyed_column_with_children<'a, Key, Message, Theme, R>(
    children: impl IntoIterator<Item = (Key, CapturedElement<'a, Message, Theme, R>)>,
    limits: CaptureLimits,
) -> Result<CapturedKeyedColumn<'a, Key, Message, Theme, R>, CaptureError>
where
    Key: Copy + PartialEq,
    R: Renderer,
{
    let iterator = children.into_iter();
    let parent = keyed_column_with_capacity(iterator.size_hint().0, limits)?;
    // Empty ordered construction consumes one evidence visit. Carry its balance
    // into iterator admission instead of resetting this constructor's budget.
    let iteration_limits = CaptureLimits {
        work: std::num::NonZeroUsize::new(limits.work.get() - 1).ok_or(CaptureError::Work)?,
        ..limits
    };
    parent.extend(iterator, iteration_limits)
}

// Acquire the application's iterator only when the shared insertion helper does.
struct Paired<I>(I);
fn pair<K, W>((key, child): (K, Captured<W>)) -> Captured<(K, W)> {
    let Captured {
        payload,
        construction,
    } = child;
    Captured {
        payload: (key, payload),
        construction,
    }
}
impl<I, K, W> IntoIterator for Paired<I>
where
    I: IntoIterator<Item = (K, Captured<W>)>,
{
    type Item = Captured<(K, W)>;
    type IntoIter = std::iter::Map<I::IntoIter, fn((K, Captured<W>)) -> Captured<(K, W)>>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().map(pair::<K, W>)
    }
}

impl<'a, Key: Copy + PartialEq, Message, Theme, R: Renderer>
    Captured<Column<'a, Key, Message, Theme, R>>
{
    /// Admit the source subtree before native conversion and hint inspection.
    pub fn push<W>(
        self,
        key: Key,
        child: Captured<W>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError>
    where
        W: Into<Element<'a, Message, Theme, R>>,
    {
        let Captured {
            payload,
            construction,
        } = self;
        let Captured {
            payload: child,
            construction: child_construction,
        } = child;
        let construction = construction.append_ordered(child_construction, limits)?;
        Ok(Self {
            payload: payload.push(key, child),
            construction,
        })
    }
    /// None omits the supplied role; validate existing evidence without native work.
    pub fn push_maybe<W>(
        self,
        key: Key,
        child: Option<Captured<W>>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError>
    where
        W: Into<Element<'a, Message, Theme, R>>,
    {
        match child {
            Some(child) => self.push(key, child, limits),
            None => {
                self.construction.validate_limits(limits)?;
                Ok(self)
            }
        }
    }
    /// Append paired children with one bounded evidence-visit budget. Native keys
    /// are moved without comparison; arbitrary iterator callback work is excluded.
    pub fn extend(
        self,
        children: impl IntoIterator<Item = (Key, CapturedElement<'a, Message, Theme, R>)>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        super::ordered_insertion::extend(self, Paired(children), limits, |parent, child, limits| {
            let Captured {
                payload: (key, payload),
                construction,
            } = child;
            parent.push(
                key,
                Captured {
                    payload,
                    construction,
                },
                limits,
            )
        })
    }

    #[must_use]
    pub fn spacing(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.spacing(value);
        self
    }
    #[must_use]
    pub fn padding(mut self, value: impl Into<Padding>) -> Self {
        self.payload = self.payload.padding(value);
        self
    }
    #[must_use]
    pub fn width(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.width(value);
        self
    }
    #[must_use]
    pub fn height(mut self, value: impl Into<Length>) -> Self {
        self.payload = self.payload.height(value);
        self
    }
    #[must_use]
    pub fn max_width(mut self, value: impl Into<Pixels>) -> Self {
        self.payload = self.payload.max_width(value);
        self
    }
    #[must_use]
    pub fn align_items(mut self, value: Alignment) -> Self {
        self.payload = self.payload.align_items(value);
        self
    }
}
