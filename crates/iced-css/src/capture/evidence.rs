use std::num::NonZeroUsize;

/// Per-operation bounds for the resulting evidence, excluding native payloads.
/// Image path capacity is included. Shared image byte backing and SVG `Arc`
/// backing are excluded, although retained handles can keep that backing alive.
#[derive(Debug, Clone, Copy)]
pub struct CaptureLimits {
    pub nodes: NonZeroUsize,
    pub depth: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
    pub work: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureError {
    /// Captured dimensions disagree or their arithmetic overflows.
    Dimensions,
    Nodes,
    Depth,
    Bytes,
    Work,
    Allocation,
    /// An internal metadata operation did not match the source role.
    Role,
}
impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "construction evidence: {self:?}")
    }
}
impl std::error::Error for CaptureError {}

#[derive(Debug)]
enum Kind {
    #[cfg(feature = "image-without-codecs")]
    Image(ImageSource),
    #[cfg(feature = "svg")]
    Svg(SvgSource),
    Text(String),
    RichText {
        text: String,
        runs: Vec<std::ops::Range<usize>>,
    },
    Checkbox(Option<String>),
    Toggler(Option<String>),
    Radio(String),
    PickList,
    Table,
    TableHeaderGroup,
    TableRowGroup,
    TableRow,
    TableHeaderCell,
    TableCell,
    PaneGrid,
    PaneControls,
    PaneTitle,
    PaneContent,
    Container,
    Row,
    Column,
    KeyedColumn,
    Stack,
    Grid,
    Button,
    Scrollable,
    Tooltip,
    Hover,
    Pin,
    MouseArea,
    Float,
    Themer,
    Sensor,
    Rule,
    ProgressBar,
    Slider,
    VerticalSlider,
    Opaque,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Base,
    Top,
    Content,
    Tooltip,
    Full,
    Compact,
    Title,
    Body,
    Controls,
    Ordered(usize),
    Pane {
        identity: iced::widget::pane_grid::Pane,
        position: usize,
    },
}

/// A source construction record, independent of native state and formatting boxes.
#[derive(Debug)]
pub struct ConstructionNode {
    kind: Kind,
    parent: Option<usize>,
    role: Option<Role>,
    metadata: iced_css_core::source::Metadata,
    source_key: Option<Vec<u8>>,
}
impl ConstructionNode {
    /// Exact application key bytes, independent of CSS IDs and native IDs.
    /// None needs a separately supplied static identity; it is not an index key.
    pub fn source_key(&self) -> Option<&[u8]> {
        self.source_key.as_deref()
    }

    pub fn metadata(&self) -> &iced_css_core::source::Metadata {
        &self.metadata
    }
    pub fn canonical_type(&self) -> &'static str {
        match self.kind {
            #[cfg(feature = "image-without-codecs")]
            Kind::Image(_) => "image",
            #[cfg(feature = "svg")]
            Kind::Svg(_) => "svg",
            Kind::Text(_) => "text",
            Kind::RichText { .. } => "rich_text",
            Kind::Checkbox(_) => "checkbox",
            Kind::Toggler(_) => "toggler",
            Kind::Radio(_) => "radio",
            Kind::PickList => "pick_list",
            Kind::Table => "table",
            Kind::TableHeaderGroup => "table_header_group",
            Kind::TableRowGroup => "table_row_group",
            Kind::TableRow => "table_row",
            Kind::TableHeaderCell => "table_header_cell",
            Kind::TableCell => "table_cell",
            Kind::PaneGrid => "pane_grid",
            Kind::PaneControls => "pane_controls",
            Kind::PaneTitle => "pane_title",
            Kind::PaneContent => "pane_content",
            Kind::Container => "container",
            Kind::Row => "row",
            Kind::Column => "column",
            Kind::KeyedColumn => "keyed_column",
            Kind::Stack => "stack",
            Kind::Grid => "grid",
            Kind::Button => "button",
            Kind::Scrollable => "scrollable",
            Kind::Tooltip => "tooltip",
            Kind::Hover => "hover",
            Kind::Pin => "pin",
            Kind::MouseArea => "mouse_area",
            Kind::Float => "float",
            Kind::Themer => "themer",
            Kind::Sensor => "sensor",
            Kind::Rule => "rule",
            Kind::ProgressBar => "progress_bar",
            Kind::Slider => "slider",
            Kind::VerticalSlider => "vertical_slider",
            Kind::Opaque => "opaque",
            Kind::Space => "space",
        }
    }
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            #[cfg(feature = "image-without-codecs")]
            Kind::Image(_) => None,
            #[cfg(feature = "svg")]
            Kind::Svg(_) => None,
            Kind::Text(text) | Kind::Radio(text) | Kind::RichText { text, .. } => Some(text),
            Kind::Checkbox(label) | Kind::Toggler(label) => label.as_deref(),
            Kind::Container
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
            | Kind::Table
            | Kind::TableHeaderGroup
            | Kind::TableRowGroup
            | Kind::TableRow
            | Kind::TableHeaderCell
            | Kind::TableCell
            | Kind::PickList
            | Kind::PaneGrid
            | Kind::PaneControls
            | Kind::PaneTitle
            | Kind::PaneContent
            | Kind::Opaque
            | Kind::Space => None,
        }
    }
    /// Authored RichText span boundaries as UTF-8 byte ranges in text(). Empty
    /// spans retain empty ranges; runs are text content, not source elements.
    pub fn text_runs(&self) -> Option<&[std::ops::Range<usize>]> {
        match &self.kind {
            Kind::RichText { runs, .. } => Some(runs),
            _ => None,
        }
    }
    pub fn parent(&self) -> Option<usize> {
        self.parent
    }
    /// Declared source-child role, independent of native state slots.
    pub fn role(&self) -> Option<&'static str> {
        self.role.as_ref().map(|role| match role {
            Role::Base => "base",
            Role::Top => "top",
            Role::Content => "content",
            Role::Tooltip => "tooltip",
            Role::Full => "full",
            Role::Compact => "compact",
            Role::Title => "title",
            Role::Body => "body",
            Role::Controls => "controls",
            Role::Ordered(_) => "children",
            Role::Pane { .. } => "panes",
        })
    }
    /// Exact native Pane identity supplied by the registered grid constructor.
    /// Scoped to its native State; independent of source keys and CSS IDs.
    pub fn pane_identity(&self) -> Option<iced::widget::pane_grid::Pane> {
        match self.role {
            Some(Role::Pane { identity, .. }) => Some(identity),
            _ => None,
        }
    }

    /// Order within an ordered child role. This is not a repeated-data key.
    pub fn role_position(&self) -> Option<usize> {
        match self.role {
            Some(Role::Ordered(position) | Role::Pane { position, .. }) => Some(position),
            _ => None,
        }
    }
}

/// Owned flat postorder recipe. No live source identities have been allocated.
#[derive(Debug)]
pub struct Construction {
    nodes: Vec<ConstructionNode>,
    owned_bytes: usize,
    depth: usize,
}
impl Construction {
    pub fn nodes(&self) -> &[ConstructionNode] {
        &self.nodes
    }
    pub fn root(&self) -> usize {
        self.nodes.len() - 1
    }
    pub fn depth(&self) -> usize {
        self.depth
    }
    /// Actual evidence vector and owned payload capacities, including image paths.
    /// Excludes native payloads, shared image byte backing, and SVG `Arc` backing;
    /// retained handles can nevertheless keep shared backing alive.
    pub fn retained_bytes(&self) -> usize {
        // Constructors check this exact sum before publishing the value.
        self.nodes.capacity() * std::mem::size_of::<ConstructionNode>() + self.owned_bytes
    }

    pub(super) fn text(text: &str, limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::text_leaf(text, Kind::Text, limits)
    }

    pub(super) fn radio(text: &str, limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::text_leaf(text, Kind::Radio, limits)
    }

    fn text_leaf(
        text: &str,
        kind: fn(String) -> Kind,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        if text.len().checked_add(1).ok_or(CaptureError::Work)? > limits.work.get() {
            return Err(CaptureError::Work);
        }
        let mut nodes = Vec::new();
        check_bytes(1, text.len(), limits)?;
        nodes
            .try_reserve_exact(1)
            .map_err(|_| CaptureError::Allocation)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(text.len())
            .map_err(|_| CaptureError::Allocation)?;
        check_bytes(nodes.capacity(), owned.capacity(), limits)?;
        owned.push_str(text);
        let owned_bytes = owned.capacity();
        nodes.push(ConstructionNode {
            kind: kind(owned),
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

    pub(super) fn container(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child(Kind::Container, limits)
    }

    pub(super) fn space(limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::leaf(Kind::Space, limits)
    }

    pub(super) fn rule(limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::leaf(Kind::Rule, limits)
    }

    pub(super) fn progress_bar(limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::leaf(Kind::ProgressBar, limits)
    }

    pub(super) fn checkbox(limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::leaf(Kind::Checkbox(None), limits)
    }

    pub(super) fn toggler(limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::leaf(Kind::Toggler(None), limits)
    }

    pub(super) fn pick_list(limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::leaf(Kind::PickList, limits)
    }

    pub(super) fn set_control_label(
        &mut self,
        text: &str,
        limits: CaptureLimits,
    ) -> Result<(), CaptureError> {
        if self.nodes.len() > limits.nodes.get() {
            return Err(CaptureError::Nodes);
        }
        if self.depth > limits.depth.get() {
            return Err(CaptureError::Depth);
        }
        if text.len().checked_add(1).ok_or(CaptureError::Work)? > limits.work.get() {
            return Err(CaptureError::Work);
        }
        let root = self.root();
        let (Kind::Checkbox(label) | Kind::Toggler(label)) = &self.nodes[root].kind else {
            return Err(CaptureError::Role);
        };
        let base = self
            .owned_bytes
            .checked_sub(label.as_ref().map_or(0, String::capacity))
            .ok_or(CaptureError::Bytes)?;
        check_bytes(
            self.nodes.capacity(),
            base.checked_add(text.len()).ok_or(CaptureError::Bytes)?,
            limits,
        )?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(text.len())
            .map_err(|_| CaptureError::Allocation)?;
        let total = base
            .checked_add(owned.capacity())
            .ok_or(CaptureError::Bytes)?;
        check_bytes(self.nodes.capacity(), total, limits)?;
        owned.push_str(text);
        match &mut self.nodes[root].kind {
            Kind::Checkbox(label) | Kind::Toggler(label) => *label = Some(owned),
            _ => return Err(CaptureError::Role),
        }
        self.owned_bytes = total;
        Ok(())
    }

    fn leaf(kind: Kind, limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::leaf_with_owned(kind, 0, limits)
    }

    fn leaf_with_owned(
        kind: Kind,
        owned_bytes: usize,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        check_bytes(1, owned_bytes, limits)?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(1)
            .map_err(|_| CaptureError::Allocation)?;
        check_bytes(nodes.capacity(), owned_bytes, limits)?;
        nodes.push(ConstructionNode {
            kind,
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

    pub(super) fn button(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child(Kind::Button, limits)
    }

    pub(super) fn scrollable(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child(Kind::Scrollable, limits)
    }

    pub(super) fn tooltip(
        self,
        tooltip: Self,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        Self::pair(
            self,
            tooltip,
            Kind::Tooltip,
            [Role::Content, Role::Tooltip],
            limits,
        )
    }

    pub(super) fn hover(self, top: Self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::pair(self, top, Kind::Hover, [Role::Base, Role::Top], limits)
    }

    pub(super) fn pane_controls(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child_with_role(Kind::PaneControls, Role::Full, limits)
    }

    pub(super) fn pane_controls_dynamic(
        self,
        compact: Self,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        Self::pair(
            self,
            compact,
            Kind::PaneControls,
            [Role::Full, Role::Compact],
            limits,
        )
    }

    /// Merge two fixed source roles; native payload conversion stays with the adapter.
    fn pair(
        left: Self,
        right: Self,
        kind: Kind,
        roles: [Role; 2],
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let child_count = left
            .nodes
            .len()
            .checked_add(right.nodes.len())
            .ok_or(CaptureError::Nodes)?;
        let count = child_count.checked_add(1).ok_or(CaptureError::Nodes)?;
        if count > limits.nodes.get() {
            return Err(CaptureError::Nodes);
        }
        let depth = left
            .depth
            .max(right.depth)
            .checked_add(1)
            .ok_or(CaptureError::Depth)?;
        if depth > limits.depth.get() {
            return Err(CaptureError::Depth);
        }
        let owned_bytes = left
            .owned_bytes
            .checked_add(right.owned_bytes)
            .ok_or(CaptureError::Bytes)?;
        check_bytes(count, owned_bytes, limits)?;
        let cost = child_count
            .checked_mul(2)
            .and_then(|n| n.checked_add(5))
            .ok_or(CaptureError::Work)?;
        let mut work = limits.work.get();
        charge(&mut work, cost)?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(count)
            .map_err(|_| CaptureError::Allocation)?;
        check_bytes(nodes.capacity(), owned_bytes, limits)?;
        for (child, role) in [left, right].into_iter().zip(roles) {
            let offset = nodes.len();
            for mut node in child.nodes {
                if let Some(parent) = &mut node.parent {
                    // Valid child-local indices; total count was checked above.
                    *parent += offset;
                } else {
                    node.parent = Some(count - 1);
                }
                nodes.push(node);
            }
            // Every construction is nonempty and root-last.
            nodes.last_mut().ok_or(CaptureError::Nodes)?.role = Some(role);
        }
        nodes.push(ConstructionNode {
            kind,
            parent: None,
            role: None,
            metadata: Default::default(),
            source_key: None,
        });
        Ok(Self {
            nodes,
            owned_bytes,
            depth,
        })
    }

    pub(super) fn pin(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child(Kind::Pin, limits)
    }

    pub(super) fn mouse_area(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child(Kind::MouseArea, limits)
    }

    pub(super) fn float(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child(Kind::Float, limits)
    }

    pub(super) fn themer(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child(Kind::Themer, limits)
    }

    pub(super) fn sensor(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child(Kind::Sensor, limits)
    }

    pub(super) fn opaque(self, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child(Kind::Opaque, limits)
    }

    pub(super) fn slider(limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::leaf(Kind::Slider, limits)
    }
    pub(super) fn vertical_slider(limits: CaptureLimits) -> Result<Self, CaptureError> {
        Self::leaf(Kind::VerticalSlider, limits)
    }

    fn single_child(self, kind: Kind, limits: CaptureLimits) -> Result<Self, CaptureError> {
        self.single_child_with_role(kind, Role::Content, limits)
    }

    fn single_child_with_role(
        mut self,
        kind: Kind,
        role: Role,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let count = self.nodes.len().checked_add(1).ok_or(CaptureError::Nodes)?;
        if count > limits.nodes.get() {
            return Err(CaptureError::Nodes);
        }
        let depth = self.depth.checked_add(1).ok_or(CaptureError::Depth)?;
        if depth > limits.depth.get() {
            return Err(CaptureError::Depth);
        }
        if count > limits.work.get() {
            return Err(CaptureError::Work);
        }
        check_bytes(count.max(self.nodes.capacity()), self.owned_bytes, limits)?;
        self.nodes
            .try_reserve_exact(1)
            .map_err(|_| CaptureError::Allocation)?;
        check_bytes(self.nodes.capacity(), self.owned_bytes, limits)?;
        let old_root = self.root();
        self.nodes[old_root].parent = Some(count - 1);
        self.nodes[old_root].role = Some(role);
        self.nodes.push(ConstructionNode {
            kind,
            parent: None,
            role: None,
            metadata: Default::default(),
            source_key: None,
        });
        self.depth = depth;
        Ok(self)
    }

    pub(super) fn row<W>(
        children: Vec<super::Captured<W>>,
        limits: CaptureLimits,
    ) -> Result<(Vec<W>, Self), CaptureError> {
        Self::ordered(children, Kind::Row, limits)
    }

    pub(super) fn keyed_column<W>(
        children: Vec<super::Captured<W>>,
        limits: CaptureLimits,
    ) -> Result<(Vec<W>, Self), CaptureError> {
        Self::ordered(children, Kind::KeyedColumn, limits)
    }

    pub(super) fn column<W>(
        children: Vec<super::Captured<W>>,
        limits: CaptureLimits,
    ) -> Result<(Vec<W>, Self), CaptureError> {
        Self::ordered(children, Kind::Column, limits)
    }

    pub(super) fn stack<W>(
        children: Vec<super::Captured<W>>,
        limits: CaptureLimits,
    ) -> Result<(Vec<W>, Self), CaptureError> {
        Self::ordered(children, Kind::Stack, limits)
    }

    pub(super) fn grid<W>(
        children: Vec<super::Captured<W>>,
        limits: CaptureLimits,
    ) -> Result<(Vec<W>, Self), CaptureError> {
        Self::ordered(children, Kind::Grid, limits)
    }

    /// Cached evidence bounds. Callers charge one constant validation step.
    pub(super) fn validate_limits(&self, limits: CaptureLimits) -> Result<(), CaptureError> {
        if self.nodes.len() > limits.nodes.get() {
            return Err(CaptureError::Nodes);
        }
        if self.depth > limits.depth.get() {
            return Err(CaptureError::Depth);
        }
        check_bytes(self.nodes.capacity(), self.owned_bytes, limits)
    }

    pub(super) fn append_cost(&self, child: &Self) -> Result<usize, CaptureError> {
        self.nodes
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(child.nodes.len()))
            .and_then(|n| n.checked_add(1))
            .ok_or(CaptureError::Work)
    }

    /// Merge an incoming subtree into an already ordered parent. Native owners
    /// may use this only when their insertion also retains the entire payload.
    pub(super) fn append_ordered(
        mut self,
        child: Self,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let count = self
            .nodes
            .len()
            .checked_add(child.nodes.len())
            .ok_or(CaptureError::Nodes)?;
        if count > limits.nodes.get() {
            return Err(CaptureError::Nodes);
        }
        let depth = self
            .depth
            .max(child.depth.checked_add(1).ok_or(CaptureError::Depth)?);
        if depth > limits.depth.get() {
            return Err(CaptureError::Depth);
        }
        let owned_bytes = self
            .owned_bytes
            .checked_add(child.owned_bytes)
            .ok_or(CaptureError::Bytes)?;
        check_bytes(count, owned_bytes, limits)?;
        let cost = self.append_cost(&child)?;
        let mut work = limits.work.get();
        charge(&mut work, cost)?;
        let old_root = self.root();
        let new_root = count - 1;
        let position = self
            .nodes
            .iter()
            .filter(|node| node.parent == Some(old_root))
            .count();
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(count)
            .map_err(|_| CaptureError::Allocation)?;
        check_bytes(nodes.capacity(), owned_bytes, limits)?;
        let root = self.nodes.pop().ok_or(CaptureError::Nodes)?;
        for mut node in self.nodes {
            if node.parent == Some(old_root) {
                node.parent = Some(new_root);
            }
            nodes.push(node);
        }
        for mut node in child.nodes {
            if let Some(parent) = &mut node.parent {
                // Child-local indices are below its node count; count was checked.
                *parent += old_root;
            } else {
                node.parent = Some(new_root);
                node.role = Some(Role::Ordered(position));
            }
            nodes.push(node);
        }
        nodes.push(root);
        Ok(Self {
            nodes,
            owned_bytes,
            depth,
        })
    }

    /// Prepend a complete subtree, preserving the old root and postorder indices.
    /// Only ordered-family adapters may call this with matching native insertion.
    pub(super) fn prepend_ordered(
        mut self,
        child: Self,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let child_count = child.nodes.len();
        let count = self
            .nodes
            .len()
            .checked_add(child_count)
            .ok_or(CaptureError::Nodes)?;
        if count > limits.nodes.get() {
            return Err(CaptureError::Nodes);
        }
        let depth = self
            .depth
            .max(child.depth.checked_add(1).ok_or(CaptureError::Depth)?);
        if depth > limits.depth.get() {
            return Err(CaptureError::Depth);
        }
        let owned_bytes = self
            .owned_bytes
            .checked_add(child.owned_bytes)
            .ok_or(CaptureError::Bytes)?;
        check_bytes(count, owned_bytes, limits)?;
        let cost = count.checked_add(1).ok_or(CaptureError::Work)?;
        let mut work = limits.work.get();
        charge(&mut work, cost)?;
        let old_root = self.root();
        let new_root = count - 1;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(count)
            .map_err(|_| CaptureError::Allocation)?;
        check_bytes(nodes.capacity(), owned_bytes, limits)?;
        for mut node in child.nodes {
            if node.parent.is_none() {
                node.parent = Some(new_root);
                node.role = Some(Role::Ordered(0));
            }
            nodes.push(node);
        }
        let root = self.nodes.pop().ok_or(CaptureError::Nodes)?;
        for mut node in self.nodes {
            if node.parent == Some(old_root)
                && let Some(Role::Ordered(position)) = &mut node.role
            {
                // The validated source position is below old parent node count.
                *position += 1;
            }
            if let Some(parent) = &mut node.parent {
                // Old-local indices are below old node count; total was checked.
                *parent += child_count;
            }
            nodes.push(node);
        }
        nodes.push(root);
        Ok(Self {
            nodes,
            owned_bytes,
            depth,
        })
    }

    fn ordered<W>(
        children: Vec<super::Captured<W>>,
        kind: Kind,
        limits: CaptureLimits,
    ) -> Result<(Vec<W>, Self), CaptureError> {
        let mut count = 1usize;
        let mut owned_bytes = 0usize;
        let mut depth = 1usize;
        let mut work = limits.work.get();
        charge(&mut work, 1)?;
        for child in &children {
            // One preflight and one movement visit, plus adjustment and copying
            // of every flat node. String allocations move without copying text.
            charge(&mut work, 2)?;
            charge(&mut work, child.construction.nodes.len())?;
            charge(&mut work, child.construction.nodes.len())?;
            count = count
                .checked_add(child.construction.nodes.len())
                .ok_or(CaptureError::Nodes)?;
            if count > limits.nodes.get() {
                return Err(CaptureError::Nodes);
            }
            owned_bytes = owned_bytes
                .checked_add(child.construction.owned_bytes)
                .ok_or(CaptureError::Bytes)?;
            depth = depth.max(
                child
                    .construction
                    .depth
                    .checked_add(1)
                    .ok_or(CaptureError::Depth)?,
            );
            if depth > limits.depth.get() {
                return Err(CaptureError::Depth);
            }
        }
        check_bytes(count, owned_bytes, limits)?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(count)
            .map_err(|_| CaptureError::Allocation)?;
        check_bytes(nodes.capacity(), owned_bytes, limits)?;
        let mut payloads = Vec::new();
        payloads
            .try_reserve_exact(children.len())
            .map_err(|_| CaptureError::Allocation)?;
        for (position, child) in children.into_iter().enumerate() {
            let super::Captured {
                payload,
                mut construction,
            } = child;
            let offset = nodes.len();
            for node in &mut construction.nodes {
                if let Some(parent) = &mut node.parent {
                    *parent += offset;
                }
            }
            let root = construction.root();
            construction.nodes[root].parent = Some(count - 1);
            construction.nodes[root].role = Some(Role::Ordered(position));
            nodes.append(&mut construction.nodes);
            payloads.push(payload);
        }
        nodes.push(ConstructionNode {
            kind,
            parent: None,
            role: None,
            metadata: Default::default(),
            source_key: None,
        });
        Ok((
            payloads,
            Self {
                nodes,
                owned_bytes,
                depth,
            },
        ))
    }

    pub(super) fn set_source_key(
        &mut self,
        key: &[u8],
        limits: CaptureLimits,
    ) -> Result<(), CaptureError> {
        if self.nodes.len() > limits.nodes.get() {
            return Err(CaptureError::Nodes);
        }
        if self.depth > limits.depth.get() {
            return Err(CaptureError::Depth);
        }
        if key.len().checked_add(1).ok_or(CaptureError::Work)? > limits.work.get() {
            return Err(CaptureError::Work);
        }
        let root = self.root();
        let old = self.nodes[root]
            .source_key
            .as_ref()
            .map_or(0, Vec::capacity);
        let base = self
            .owned_bytes
            .checked_sub(old)
            .ok_or(CaptureError::Bytes)?;
        check_bytes(
            self.nodes.capacity(),
            base.checked_add(key.len()).ok_or(CaptureError::Bytes)?,
            limits,
        )?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(key.len())
            .map_err(|_| CaptureError::Allocation)?;
        let total = base
            .checked_add(owned.capacity())
            .ok_or(CaptureError::Bytes)?;
        check_bytes(self.nodes.capacity(), total, limits)?;
        owned.extend_from_slice(key);
        self.nodes[root].source_key = Some(owned);
        self.owned_bytes = total;
        Ok(())
    }

    pub(super) fn edit_metadata(
        &mut self,
        mut limits: iced_css_core::source::AttributeLimits,
        edit: impl FnOnce(
            &mut iced_css_core::source::Metadata,
            iced_css_core::source::AttributeLimits,
        ) -> Result<bool, iced_css_core::source::AttributeError>,
    ) -> Result<(), iced_css_core::source::AttributeError> {
        use iced_css_core::source::AttributeError;
        let root = self.root();
        let metadata = &mut self.nodes[root].metadata;
        // Prepay old and new heap-capacity scans; an edit adds at most one slot.
        let scans = metadata
            .attribute_count()
            .checked_mul(2)
            .and_then(|n| n.checked_add(2))
            .ok_or(AttributeError::Work)?;
        limits.work = limits
            .work
            .get()
            .checked_sub(scans)
            .and_then(NonZeroUsize::new)
            .ok_or(AttributeError::Work)?;
        let before = metadata.retained_bytes();
        edit(metadata, limits)?;
        self.owned_bytes = self
            .owned_bytes
            .checked_sub(before)
            .and_then(|n| n.checked_add(metadata.retained_bytes()))
            .ok_or(AttributeError::Bytes)?;
        Ok(())
    }
}

fn charge(work: &mut usize, amount: usize) -> Result<(), CaptureError> {
    *work = work.checked_sub(amount).ok_or(CaptureError::Work)?;
    Ok(())
}

fn check_bytes(capacity: usize, text: usize, limits: CaptureLimits) -> Result<(), CaptureError> {
    let bytes = capacity
        .checked_mul(std::mem::size_of::<ConstructionNode>())
        .and_then(|n| n.checked_add(text))
        .ok_or(CaptureError::Bytes)?;
    if bytes > limits.retained_bytes.get() {
        return Err(CaptureError::Bytes);
    }
    Ok(())
}

mod pane;

mod rich_text;

mod table;

#[cfg(any(feature = "image-without-codecs", feature = "svg"))]
mod resource;
#[cfg(feature = "image-without-codecs")]
pub use resource::{ImageHints, ImageSource, RgbaBufferStatus, RgbaDeclaration};
#[cfg(feature = "svg")]
pub use resource::{SvgHints, SvgSource, SvgStyleSelection};
