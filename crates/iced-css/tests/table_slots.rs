use iced_css::{
    capture::{CaptureLimits, Construction, adapters},
    host::{
        TableRow, TableSlotError, TableSlots,
        reconcile::{SlotError, SlotLimits},
    },
};
use std::num::NonZeroUsize;
fn n() -> NonZeroUsize {
    NonZeroUsize::new(100000).unwrap()
}
fn limits() -> CaptureLimits {
    CaptureLimits {
        nodes: n(),
        depth: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn slots() -> SlotLimits {
    SlotLimits {
        entries: n(),
        retained_bytes: n(),
        work: n(),
    }
}
fn leaf() -> Construction {
    adapters::space(limits()).unwrap().into_parts().1
}
#[test]
fn projection_uses_content_roots_in_native_order_and_excludes_nested_tables() {
    let inner = Construction::table(1, 1, vec![leaf(), leaf()], limits()).unwrap();
    let outer = Construction::table(2, 1, vec![leaf(), leaf(), inner, leaf()], limits()).unwrap();
    let inner_root = outer
        .nodes()
        .iter()
        .enumerate()
        .find(|(_, n)| n.canonical_type() == "table")
        .unwrap()
        .0;
    let projection = TableSlots::from_construction(&outer, outer.root(), slots()).unwrap();
    assert_eq!((projection.columns(), projection.rows()), (2, 1));
    assert_eq!(projection.cells().len(), 4);
    assert_eq!(
        projection
            .cells()
            .iter()
            .map(|s| (s.native_index, s.row, s.column))
            .collect::<Vec<_>>(),
        [
            (0, TableRow::Header, 0),
            (1, TableRow::Header, 1),
            (2, TableRow::Body(0), 0),
            (3, TableRow::Body(0), 1)
        ]
    );
    assert_eq!(projection.cells()[2].content_root, inner_root);
    for cell in projection.cells() {
        assert_eq!(
            outer.nodes()[cell.content_root].parent(),
            Some(cell.cell_root)
        );
        assert_eq!(outer.nodes()[cell.content_root].role(), Some("content"));
    }
    let nested = TableSlots::from_construction(&outer, inner_root, slots()).unwrap();
    assert_eq!(
        (nested.columns(), nested.rows(), nested.cells().len()),
        (1, 1, 2)
    );
    assert!(nested.cells().iter().all(|s| s.content_root < inner_root));
}
#[test]
fn empty_tables_keep_row_count_without_native_slots() {
    let c = Construction::table(0, 3, vec![], limits()).unwrap();
    let slots = TableSlots::from_construction(&c, c.root(), slots()).unwrap();
    assert_eq!((slots.columns(), slots.rows()), (0, 3));
    assert!(slots.cells().is_empty());
    assert_eq!(slots.retained_bytes(), 0);
}
#[test]
fn projection_rejects_wrong_roots_and_exhausted_bounds() {
    let c = Construction::table(2, 0, vec![leaf(), leaf()], limits()).unwrap();
    for root in [0, usize::MAX] {
        assert!(matches!(
            TableSlots::from_construction(&c, root, slots()),
            Err(TableSlotError::Root)
        ));
    }
    for (l, want) in [
        (
            SlotLimits {
                entries: NonZeroUsize::MIN,
                ..slots()
            },
            SlotError::Entries,
        ),
        (
            SlotLimits {
                retained_bytes: NonZeroUsize::MIN,
                ..slots()
            },
            SlotError::Bytes,
        ),
        (
            SlotLimits {
                work: NonZeroUsize::MIN,
                ..slots()
            },
            SlotError::Work,
        ),
    ] {
        assert!(
            matches!(TableSlots::from_construction(&c,c.root(),l),Err(TableSlotError::Slots(e)) if e==want)
        );
    }
}
#[test]
fn native_child_count_agrees_with_captured_projection() {
    let col = adapters::table_column(adapters::space(limits()).unwrap(), |()| {
        adapters::space(limits()).unwrap()
    });
    let capture =
        adapters::table_from_vecs::<(), (), iced::Theme, ()>(vec![col], vec![(), ()], limits())
            .unwrap();
    let projection = TableSlots::from_construction(
        capture.construction(),
        capture.construction().root(),
        slots(),
    )
    .unwrap();
    assert_eq!(projection.cells().len(), 3);
    let expected = projection.cells().len();
    let native = capture.erase().into_parts().0;
    assert_eq!(
        iced::advanced::widget::Tree::new(&native).children.len(),
        expected
    );
}

#[test]
fn exact_projection_work_and_retained_capacity_are_admitted() {
    let c = Construction::table(1, 0, vec![leaf()], limits()).unwrap();
    assert_eq!(c.nodes().len(), 6);
    assert!(matches!(
        TableSlots::from_construction(
            &c,
            c.root(),
            SlotLimits {
                work: NonZeroUsize::new(12).unwrap(),
                ..slots()
            }
        ),
        Err(TableSlotError::Slots(SlotError::Work))
    ));
    let p = TableSlots::from_construction(
        &c,
        c.root(),
        SlotLimits {
            work: NonZeroUsize::new(13).unwrap(),
            ..slots()
        },
    )
    .unwrap();
    let bytes = p.retained_bytes();
    assert!(bytes >= std::mem::size_of::<iced_css::host::TableCellSlot>());
    assert!(matches!(
        TableSlots::from_construction(
            &c,
            c.root(),
            SlotLimits {
                retained_bytes: NonZeroUsize::new(bytes - 1).unwrap(),
                ..slots()
            }
        ),
        Err(TableSlotError::Slots(SlotError::Bytes))
    ));
    assert_eq!(
        TableSlots::from_construction(
            &c,
            c.root(),
            SlotLimits {
                retained_bytes: NonZeroUsize::new(bytes).unwrap(),
                ..slots()
            }
        )
        .unwrap()
        .cells()
        .len(),
        1
    );
}
