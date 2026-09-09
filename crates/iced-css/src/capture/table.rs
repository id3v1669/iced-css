use super::{CaptureError, CaptureLimits, Captured, CapturedElement, Construction};
use iced::{Element, Length, Pixels, advanced::Renderer, alignment, widget::table};
use std::{cell::RefCell, num::NonZeroUsize};

/// A column recipe, not an independently erasable widget or source element.
pub struct CapturedTableColumn<'a, 'b, T, Message, Theme = iced::Theme, R = iced::Renderer> {
    header: Element<'a, Message, Theme, R>,
    construction: Construction,
    view: Box<dyn Fn(T) -> CapturedElement<'a, Message, Theme, R> + 'b>,
    width: Length,
    align_x: alignment::Horizontal,
    align_y: alignment::Vertical,
}
/// Convert the header now; defer each captured view and its native conversion
/// until Table construction, as native table::column does.
pub fn table_column<'a, 'b, T: 'a, Message, Theme, R, H, W>(
    header: Captured<H>,
    view: impl Fn(T) -> Captured<W> + 'b,
) -> CapturedTableColumn<'a, 'b, T, Message, Theme, R>
where
    H: Into<Element<'a, Message, Theme, R>>,
    W: Into<Element<'a, Message, Theme, R>>,
{
    CapturedTableColumn {
        header: header.payload.into(),
        construction: header.construction,
        view: Box::new(move |data| {
            let Captured {
                payload,
                construction,
            } = view(data);
            Captured {
                payload: payload.into(),
                construction,
            }
        }),
        width: Length::Shrink,
        align_x: alignment::Horizontal::Left,
        align_y: alignment::Vertical::Top,
    }
}
impl<T, M, Theme, R> CapturedTableColumn<'_, '_, T, M, Theme, R> {
    #[must_use]
    pub fn width(mut self, value: impl Into<Length>) -> Self {
        self.width = value.into();
        self
    }
    #[must_use]
    pub fn align_x(mut self, value: impl Into<alignment::Horizontal>) -> Self {
        self.align_x = value.into();
        self
    }
    #[must_use]
    pub fn align_y(mut self, value: impl Into<alignment::Vertical>) -> Self {
        self.align_y = value.into();
        self
    }
}

pub type CapturedTable<'a, Message, Theme = iced::Theme, R = iced::Renderer> =
    Captured<SafeTable<'a, Message, Theme, R>>;

/// Capture known column and row vectors with native eager callback/Clone order.
/// Evidence admission excludes application callback work and native allocations.
/// Final evidence rejection discards the constructed table without replaying views.
pub fn table_from_vecs<'a, 'b, T: Clone + 'a, Message, Theme: table::Catalog, R: Renderer>(
    columns: Vec<CapturedTableColumn<'a, 'b, T, Message, Theme, R>>,
    rows: Vec<T>,
    mut limits: CaptureLimits,
) -> Result<CapturedTable<'a, Message, Theme, R>, CaptureError> {
    let column_count = columns.len();
    let row_count = rows.len();
    let count = row_count
        .checked_add(1)
        .and_then(|n| n.checked_mul(column_count))
        .ok_or(CaptureError::Dimensions)?;
    let minimum = count
        .checked_mul(2)
        .and_then(|n| n.checked_add(row_count))
        .and_then(|n| n.checked_add(4))
        .ok_or(CaptureError::Nodes)?;
    if minimum > limits.nodes.get() {
        return Err(CaptureError::Nodes);
    }
    if limits.depth.get() < if count == 0 { 3 } else { 5 } {
        return Err(CaptureError::Depth);
    }
    let work = count
        .checked_mul(2)
        .and_then(|n| n.checked_add(column_count))
        .and_then(|n| n.checked_add(row_count))
        .and_then(|n| n.checked_add(1))
        .ok_or(CaptureError::Work)?;
    limits.work = limits
        .work
        .get()
        .checked_sub(work)
        .and_then(NonZeroUsize::new)
        .ok_or(CaptureError::Work)?;
    let mut cells = Vec::<Construction>::new();
    let bytes = |capacity: usize| {
        capacity
            .checked_mul(size_of::<Construction>())
            .filter(|n| *n <= limits.retained_bytes.get())
            .ok_or(CaptureError::Bytes)
    };
    bytes(count)?;
    cells
        .try_reserve_exact(count)
        .map_err(|_| CaptureError::Allocation)?;
    bytes(cells.capacity())?;
    let collected = RefCell::new((cells, None));
    let record = |construction| {
        let mut target = collected.borrow_mut();
        if target.0.len() < count {
            target.0.push(construction);
        } else {
            target.1 = Some(CaptureError::Dimensions);
        }
    };
    let native_columns = columns.into_iter().map(|column| {
        record(column.construction);
        let record = &record;
        table::column(column.header, move |row| {
            let Captured {
                payload,
                construction,
            } = (column.view)(row);
            record(construction);
            payload
        })
        .width(column.width)
        .align_x(column.align_x)
        .align_y(column.align_y)
    });
    let inner = table::Table::new(native_columns, rows);
    let (cells, error) = collected.into_inner();
    if let Some(error) = error {
        return Err(error);
    }
    let construction = Construction::table(column_count, row_count, cells, limits)?;
    Ok(Captured {
        payload: SafeTable {
            inner,
            empty: column_count == 0,
        },
        construction,
    })
}

/// Stock Table behavior with a repository-owned zero-column layout path.
/// Construction is private: the empty flag always matches the captured columns.
pub struct SafeTable<'a, Message, Theme: table::Catalog = iced::Theme, R = iced::Renderer> {
    inner: table::Table<'a, Message, Theme, R>,
    empty: bool,
}
impl<'a, M, Theme: table::Catalog, R: Renderer> Captured<SafeTable<'a, M, Theme, R>> {
    #[must_use]
    pub fn width(mut self, v: impl Into<Length>) -> Self {
        self.payload.inner = self.payload.inner.width(v);
        self
    }
    #[must_use]
    pub fn padding(mut self, v: impl Into<Pixels>) -> Self {
        self.payload.inner = self.payload.inner.padding(v);
        self
    }
    #[must_use]
    pub fn padding_x(mut self, v: impl Into<Pixels>) -> Self {
        self.payload.inner = self.payload.inner.padding_x(v);
        self
    }
    #[must_use]
    pub fn padding_y(mut self, v: impl Into<Pixels>) -> Self {
        self.payload.inner = self.payload.inner.padding_y(v);
        self
    }
    #[must_use]
    pub fn separator(mut self, v: impl Into<Pixels>) -> Self {
        self.payload.inner = self.payload.inner.separator(v);
        self
    }
    #[must_use]
    pub fn separator_x(mut self, v: impl Into<Pixels>) -> Self {
        self.payload.inner = self.payload.inner.separator_x(v);
        self
    }
    #[must_use]
    pub fn separator_y(mut self, v: impl Into<Pixels>) -> Self {
        self.payload.inner = self.payload.inner.separator_y(v);
        self
    }
}
mod widget;

#[cfg(test)]
mod tests {
    use super::*;
    struct Convert<'a>(&'a RefCell<Vec<&'static str>>, &'static str);
    impl<'a> From<Convert<'a>> for Element<'a, (), iced::Theme, ()> {
        fn from(c: Convert<'a>) -> Self {
            c.0.borrow_mut().push(c.1);
            iced::widget::Space::new().into()
        }
    }
    struct Scalar<'a>(&'a RefCell<Vec<&'static str>>, &'static str);
    impl From<Scalar<'_>> for Length {
        fn from(v: Scalar<'_>) -> Self {
            v.0.borrow_mut().push(v.1);
            Length::Fixed(50.0)
        }
    }
    impl From<Scalar<'_>> for Pixels {
        fn from(v: Scalar<'_>) -> Self {
            v.0.borrow_mut().push(v.1);
            Pixels(2.0)
        }
    }
    impl From<Scalar<'_>> for alignment::Horizontal {
        fn from(v: Scalar<'_>) -> Self {
            v.0.borrow_mut().push(v.1);
            Self::Right
        }
    }
    impl From<Scalar<'_>> for alignment::Vertical {
        fn from(v: Scalar<'_>) -> Self {
            v.0.borrow_mut().push(v.1);
            Self::Bottom
        }
    }
    #[test]
    fn header_and_builder_conversions_are_eager_and_cell_conversion_is_deferred() {
        let n = NonZeroUsize::new(100000).unwrap();
        let limits = CaptureLimits {
            nodes: n,
            depth: n,
            retained_bytes: n,
            work: n,
        };
        let log = RefCell::new(Vec::new());
        let child = |name| Captured {
            payload: Convert(&log, name),
            construction: Construction::space(limits).unwrap(),
        };
        let column = table_column(child("header"), |()| {
            log.borrow_mut().push("view");
            child("cell")
        })
        .width(Scalar(&log, "column-width"))
        .align_x(Scalar(&log, "x"))
        .align_y(Scalar(&log, "y"));
        assert_eq!(*log.borrow(), ["header", "column-width", "x", "y"]);
        let _table = table_from_vecs(vec![column], vec![()], limits)
            .unwrap()
            .width(Scalar(&log, "width"))
            .padding(Scalar(&log, "padding"))
            .padding_x(Scalar(&log, "padding-x"))
            .padding_y(Scalar(&log, "padding-y"))
            .separator(Scalar(&log, "separator"))
            .separator_x(Scalar(&log, "separator-x"))
            .separator_y(Scalar(&log, "separator-y"));
        assert_eq!(
            *log.borrow(),
            [
                "header",
                "column-width",
                "x",
                "y",
                "view",
                "cell",
                "width",
                "padding",
                "padding-x",
                "padding-y",
                "separator",
                "separator-x",
                "separator-y"
            ]
        );
    }
}
