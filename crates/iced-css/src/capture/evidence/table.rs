use super::{CaptureError, CaptureLimits, Construction, ConstructionNode, Kind, Role, check_bytes};

impl Construction {
    /// Assemble table sources from header-first, row-major cell content evidence.
    /// `rows` excludes the header row. Both groups and all empty rows are retained.
    /// Positions are construction-local, not identities or native state slots.
    ///
    /// Bounds cover final evidence storage and one scan plus output assembly/parent linking;
    /// consumed input storage overlaps the result during assembly. No native
    /// widgets or application callbacks are evaluated by this operation.
    pub fn table(
        columns: usize,
        rows: usize,
        cells: Vec<Self>,
        limits: CaptureLimits,
    ) -> Result<Self, CaptureError> {
        let all_rows = rows.checked_add(1).ok_or(CaptureError::Dimensions)?;
        let expected = columns
            .checked_mul(all_rows)
            .ok_or(CaptureError::Dimensions)?;
        if cells.len() != expected {
            return Err(CaptureError::Dimensions);
        }
        // Three outer nodes, one per row, and one wrapper per supplied cell.
        let mut count = all_rows
            .checked_add(3)
            .and_then(|n| n.checked_add(expected))
            .ok_or(CaptureError::Nodes)?;
        let mut work = limits.work.get();
        super::charge(
            &mut work,
            cells.len().checked_add(1).ok_or(CaptureError::Work)?,
        )?;
        let mut owned_bytes = 0usize;
        let mut depth = 3usize;
        let mut header_size = 2usize; // row and group
        for (index, cell) in cells.iter().enumerate() {
            count = count
                .checked_add(cell.nodes.len())
                .ok_or(CaptureError::Nodes)?;
            owned_bytes = owned_bytes
                .checked_add(cell.owned_bytes)
                .ok_or(CaptureError::Bytes)?;
            depth = depth.max(cell.depth.checked_add(4).ok_or(CaptureError::Depth)?);
            if index < columns {
                header_size = header_size
                    .checked_add(cell.nodes.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(CaptureError::Nodes)?;
            }
        }
        if count > limits.nodes.get() {
            return Err(CaptureError::Nodes);
        }
        if depth > limits.depth.get() {
            return Err(CaptureError::Depth);
        }
        super::charge(&mut work, count.checked_mul(2).ok_or(CaptureError::Work)?)?;
        check_bytes(count, owned_bytes, limits)?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(count)
            .map_err(|_| CaptureError::Allocation)?;
        check_bytes(nodes.capacity(), owned_bytes, limits)?;
        let mut cells = cells.into_iter();
        let root = count - 1;
        for row in 0..all_rows {
            let start = nodes.len();
            // Cell roots receive their row parent once the row endpoint is known.
            for column in 0..columns {
                let cell = cells.next().ok_or(CaptureError::Dimensions)?;
                let offset = nodes.len();
                let wrapper = offset + cell.nodes.len();
                for mut node in cell.nodes {
                    if let Some(parent) = node.parent {
                        node.parent = Some(offset + parent);
                    } else {
                        node.parent = Some(wrapper);
                        node.role = Some(Role::Content);
                    }
                    nodes.push(node);
                }
                nodes.push(record(
                    if row == 0 {
                        Kind::TableHeaderCell
                    } else {
                        Kind::TableCell
                    },
                    None,
                    Some(Role::Ordered(column)),
                ));
            }
            let row_root = nodes.len();
            for node in &mut nodes[start..row_root] {
                if node.parent.is_none() {
                    node.parent = Some(row_root);
                }
            }
            nodes.push(record(
                Kind::TableRow,
                Some(if row == 0 { header_size - 1 } else { count - 2 }),
                Some(Role::Ordered(row.saturating_sub(1))),
            ));
            if row == 0 {
                nodes.push(record(
                    Kind::TableHeaderGroup,
                    Some(root),
                    Some(Role::Ordered(0)),
                ));
            }
        }
        nodes.push(record(
            Kind::TableRowGroup,
            Some(root),
            Some(Role::Ordered(1)),
        ));
        nodes.push(record(Kind::Table, None, None));
        Ok(Self {
            nodes,
            owned_bytes,
            depth,
        })
    }
}
fn record(kind: Kind, parent: Option<usize>, role: Option<Role>) -> ConstructionNode {
    ConstructionNode {
        kind,
        parent,
        role,
        metadata: Default::default(),
        source_key: None,
    }
}
