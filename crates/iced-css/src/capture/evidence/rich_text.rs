use super::{
    CaptureError, CaptureLimits, Construction, ConstructionNode, Kind, charge, check_bytes,
};
use iced::advanced::text::Span;
use std::ops::Range;
impl Construction {
    pub(in crate::capture) fn rich_text<Link, Font>(
        spans: &[Span<'_, Link, Font>],
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let mut work = limits.work.get();
        charge(&mut work, 1)?;
        charge(
            &mut work,
            spans.len().checked_mul(2).ok_or(CaptureError::Work)?,
        )?;
        let mut len = 0usize;
        for span in spans {
            len = len
                .checked_add(span.text.len())
                .ok_or(CaptureError::Bytes)?;
        }
        charge(&mut work, len)?;
        let run_bytes = spans
            .len()
            .checked_mul(size_of::<Range<usize>>())
            .ok_or(CaptureError::Bytes)?;
        check_bytes(
            1,
            len.checked_add(run_bytes).ok_or(CaptureError::Bytes)?,
            limits,
        )?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(1)
            .map_err(|_| CaptureError::Allocation)?;
        let mut text = String::new();
        text.try_reserve_exact(len)
            .map_err(|_| CaptureError::Allocation)?;
        let mut runs = Vec::new();
        runs.try_reserve_exact(spans.len())
            .map_err(|_| CaptureError::Allocation)?;
        let owned_bytes = runs
            .capacity()
            .checked_mul(size_of::<Range<usize>>())
            .and_then(|n| n.checked_add(text.capacity()))
            .ok_or(CaptureError::Bytes)?;
        check_bytes(nodes.capacity(), owned_bytes, limits)?;
        for span in spans {
            let start = text.len();
            text.push_str(&span.text);
            runs.push(start..text.len());
        }
        nodes.push(ConstructionNode {
            kind: Kind::RichText { text, runs },
            parent: None,
            role: None,
            metadata: Default::default(),
            source_key: None,
        });
        Ok(Self {
            nodes,
            owned_bytes,
            depth: 1,
        })
    }
}
