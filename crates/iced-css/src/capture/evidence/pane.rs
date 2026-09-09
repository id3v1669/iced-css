//! Replacement of the two structured pane descriptors' optional source roles.
use super::{
    CaptureError, CaptureLimits, Construction, ConstructionNode, Kind, Role, charge, check_bytes,
};
use std::ops::Range;

impl Construction {
    pub(in crate::capture) fn pane_grid(
        children: Vec<crate::capture::Captured<iced::widget::pane_grid::Pane>>,
        mut limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let mut count = 1usize;
        let mut work = limits.work.get();
        for child in &children {
            charge(&mut work, 1)?;
            if !matches!(
                child.construction.nodes[child.construction.root()].kind,
                Kind::PaneContent
            ) {
                return Err(CaptureError::Role);
            }
            count = count
                .checked_add(child.construction.nodes.len())
                .ok_or(CaptureError::Nodes)?;
        }
        charge(&mut work, count)?;
        limits.work = std::num::NonZeroUsize::new(work).ok_or(CaptureError::Work)?;
        let (panes, mut construction) = Self::ordered(children, Kind::PaneGrid, limits)?;
        if panes
            .capacity()
            .checked_mul(size_of::<iced::widget::pane_grid::Pane>())
            .is_none_or(|bytes| bytes > limits.retained_bytes.get())
        {
            return Err(CaptureError::Bytes);
        }
        let root = construction.root();
        for node in &mut construction.nodes {
            if node.parent == Some(root) {
                let Some(Role::Ordered(position)) = node.role else {
                    return Err(CaptureError::Role);
                };
                let identity = *panes.get(position).ok_or(CaptureError::Role)?;
                node.role = Some(Role::Pane { identity, position });
            }
        }
        Ok(construction)
    }

    pub(in crate::capture) fn pane_title(
        self,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        self.single_child_with_role(Kind::PaneTitle, Role::Content, limits)
    }
    pub(in crate::capture) fn pane_content(
        self,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        self.single_child_with_role(Kind::PaneContent, Role::Body, limits)
    }

    /// Final evidence bounds exclude transient overlap with consumed old buffers.
    pub(in crate::capture) fn replace_pane_role(
        mut self,
        child: Self,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let old_root = self.root();
        let (mandatory, optional, incoming_first) =
            match (&self.nodes[old_root].kind, &child.nodes[child.root()].kind) {
                (Kind::PaneTitle, Kind::PaneControls) => (Role::Content, Role::Controls, false),
                (Kind::PaneContent, Kind::PaneTitle) => (Role::Body, Role::Title, true),
                _ => return Err(CaptureError::Role),
            };
        let mut work = limits.work.get();
        charge(&mut work, old_root)?;
        let mut start = 0;
        let mut kept = None;
        let mut seen_optional = false;
        for (index, node) in self.nodes[..old_root].iter().enumerate() {
            if node.parent == Some(old_root) {
                match node.role {
                    Some(role) if role == mandatory && kept.is_none() => {
                        kept = Some(start..index + 1)
                    }
                    Some(role) if role == optional && !seen_optional => seen_optional = true,
                    _ => return Err(CaptureError::Role),
                }
                start = index + 1;
            }
        }
        if start != old_root {
            return Err(CaptureError::Role);
        }
        let kept = kept.ok_or(CaptureError::Role)?;
        let count = kept
            .len()
            .checked_add(child.nodes.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(CaptureError::Nodes)?;
        if count > limits.nodes.get() {
            return Err(CaptureError::Nodes);
        }
        let kept_depth = self.pane_segment_depth(kept.clone(), old_root, &mut work)?;
        let depth = kept_depth
            .max(child.depth)
            .checked_add(1)
            .ok_or(CaptureError::Depth)?;
        if depth > limits.depth.get() {
            return Err(CaptureError::Depth);
        }
        let mut owned_bytes = child.owned_bytes;
        for node in self.nodes[kept.clone()]
            .iter()
            .chain(std::iter::once(&self.nodes[old_root]))
        {
            charge(
                &mut work,
                node.metadata
                    .attribute_count()
                    .checked_add(1)
                    .ok_or(CaptureError::Work)?,
            )?;
            owned_bytes = owned_bytes
                .checked_add(node_heap_bytes(node)?)
                .ok_or(CaptureError::Bytes)?;
        }
        check_bytes(count, owned_bytes, limits)?;
        // Charge consumption of the old vector, both retained subtree moves and root move.
        charge(
            &mut work,
            self.nodes
                .len()
                .checked_add(count)
                .ok_or(CaptureError::Work)?,
        )?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(count)
            .map_err(|_| CaptureError::Allocation)?;
        check_bytes(nodes.capacity(), owned_bytes, limits)?;
        let root = self.nodes.pop().ok_or(CaptureError::Role)?;
        let child_len = child.nodes.len();
        let kept_offset = if incoming_first { child_len } else { 0 };
        let child_offset = if incoming_first { 0 } else { kept.len() };
        // All parent indices are within admitted private postorder constructions.
        let retained = self
            .nodes
            .into_iter()
            .enumerate()
            .filter_map(|(index, mut node)| {
                if !kept.contains(&index) {
                    return None;
                }
                node.parent = node.parent.map(|p| {
                    if p == old_root {
                        count - 1
                    } else {
                        p - kept.start + kept_offset
                    }
                });
                Some(node)
            });
        let incoming = child.nodes.into_iter().map(|mut node| {
            node.parent = Some(match node.parent {
                Some(p) => p + child_offset,
                None => {
                    node.role = Some(optional);
                    count - 1
                }
            });
            node
        });
        if incoming_first {
            nodes.extend(incoming);
            nodes.extend(retained);
        } else {
            nodes.extend(retained);
            nodes.extend(incoming);
        }
        nodes.push(root);
        Ok(Self {
            nodes,
            owned_bytes,
            depth,
        })
    }

    fn pane_segment_depth(
        &self,
        range: Range<usize>,
        root: usize,
        work: &mut usize,
    ) -> Result<usize, CaptureError> {
        let mut maximum = 0;
        for index in range.clone() {
            let mut cursor = index;
            let mut depth = 0usize;
            loop {
                charge(work, 1)?;
                depth = depth.checked_add(1).ok_or(CaptureError::Depth)?;
                let parent = self.nodes[cursor].parent.ok_or(CaptureError::Role)?;
                if parent == root {
                    break;
                }
                if parent <= cursor || !range.contains(&parent) {
                    return Err(CaptureError::Role);
                }
                cursor = parent;
            }
            maximum = maximum.max(depth);
        }
        Ok(maximum)
    }
}

fn node_heap_bytes(node: &ConstructionNode) -> Result<usize, CaptureError> {
    let label = match &node.kind {
        Kind::Text(s) | Kind::Radio(s) => s.capacity(),
        Kind::Checkbox(s) | Kind::Toggler(s) => s.as_ref().map_or(0, String::capacity),
        Kind::PaneGrid
        | Kind::PickList
        | Kind::PaneControls
        | Kind::PaneTitle
        | Kind::PaneContent
        | Kind::Container
        | Kind::Row
        | Kind::Column
        | Kind::KeyedColumn
        | Kind::Stack
        | Kind::Grid
        | Kind::Button
        | Kind::Scrollable
        | Kind::Tooltip
        | Kind::Hover
        | Kind::Pin
        | Kind::MouseArea
        | Kind::Float
        | Kind::Themer
        | Kind::Sensor
        | Kind::Rule
        | Kind::ProgressBar
        | Kind::Slider
        | Kind::VerticalSlider
        | Kind::Opaque
        | Kind::Space => 0,
    };
    node.metadata
        .retained_bytes()
        .checked_add(label)
        .and_then(|n| n.checked_add(node.source_key.as_ref().map_or(0, Vec::capacity)))
        .ok_or(CaptureError::Bytes)
}
