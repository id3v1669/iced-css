//! One-parent leaf TextInput state fixture, not a complete CssHost implementation.
use iced::advanced::widget::Tree;
use iced_css::{
    animation::engine::{ProgressEngine, ProgressLimits},
    host::{
        compatibility::{CompatibilityLimits, NativeCompatibility},
        leaf_slots::{IncomingLeafSlots, LeafSlotLimits, PreviousLeafSlots, prepare_leaf_slots},
        reconcile::SlotLimits,
    },
};
use iced_css_core::{
    identity::{SourceChangeLimits, SourceRegistry},
    source::sibling_matches::{MatchLimits, SiblingKey, match_siblings},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    KeyedReorder {
        before: Vec<String>,
        after: Vec<String>,
        focused: String,
        #[serde(default)]
        reset: Vec<String>,
    },
}
fn n(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
fn input(focused: bool) -> Tree {
    let element: iced::Element<'_, (), iced::Theme, ()> =
        iced::widget::text_input("", "fixture value")
            .on_input(|_| ())
            .into();
    let mut tree = Tree::new(&element);
    if focused {
        tree.state
            .downcast_mut::<iced::widget::text_input::State<()>>()
            .focus();
    }
    tree
}
fn slot_limits() -> SlotLimits {
    SlotLimits {
        entries: n(1000),
        retained_bytes: n(1000000),
        work: n(1000000),
    }
}
pub fn run(value: &Value) -> crate::CaseResult {
    let Input::KeyedReorder {
        before,
        after,
        focused,
        reset,
    } = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    let entries = before
        .len()
        .checked_add(after.len())
        .and_then(|v| v.checked_add(reset.len()))
        .ok_or("fixture entry overflow")?;
    if entries > 128 {
        return Err("P3.3 fixture exceeds 128 entries".into());
    }
    let bytes = before
        .iter()
        .chain(&after)
        .chain(&reset)
        .chain(std::iter::once(&focused))
        .try_fold(0usize, |n, key| n.checked_add(key.len()))
        .ok_or("fixture byte overflow")?;
    if bytes > 4096 {
        return Err("P3.3 fixture exceeds 4096 key bytes".into());
    }
    if !before.contains(&focused) {
        return Err("focused key is absent from previous sources".into());
    }
    for (index, key) in reset.iter().enumerate() {
        if !after.contains(key) || reset[..index].contains(key) {
            return Err("reset keys must be unique incoming keys".into());
        }
    }
    let previous_keys: Vec<_> = before
        .iter()
        .map(|key| SiblingKey::Explicit(key.as_bytes()))
        .collect();
    let incoming_keys: Vec<_> = after
        .iter()
        .map(|key| SiblingKey::Explicit(key.as_bytes()))
        .collect();
    let matched = match_siblings(
        &previous_keys,
        &incoming_keys,
        MatchLimits {
            entries: n(1000),
            retained_bytes: n(1000000),
            work: n(1000000),
        },
    )
    .map_err(|e| e.to_string())?;
    let mut native: Vec<_> = before
        .iter()
        .map(|key| Some(input(*key == focused)))
        .collect();
    // Fixture declaration: one complete TextInput Tree per leaf source, no
    // managed source descendants. This is not a production adapter registration.
    let schema = NativeCompatibility {
        adapter_id: "fixture.text-input",
        schema_version: NonZeroU32::MIN,
        content_tag: native[0].as_ref().unwrap().tag,
        ordered_roles: b"fixture:text-input:empty-roles:v1",
        representation: "native",
    };
    let previous_schema = vec![schema; before.len()];
    let incoming_schema: Vec<_> = after
        .iter()
        .map(|key| NativeCompatibility {
            representation: if reset.contains(key) {
                "reset"
            } else {
                "native"
            },
            ..schema
        })
        .collect();
    let mut engine = ProgressEngine::new(
        SourceRegistry::new(NonZeroU64::new(256).unwrap()),
        ProgressLimits {
            addresses: NonZeroU64::new(256).unwrap(),
            bindings: n(256),
            tracks: n(256),
            retained_bytes: n(1000000),
            work: n(1000000),
        },
    );
    let mut sources = Vec::new();
    let mut old_ids = Vec::new();
    for _ in &before {
        let source = engine.register_source().map_err(|e| e.to_string())?;
        old_ids.push(engine.instance(&source).map_err(|e| e.to_string())?);
        sources.push(Some(source));
    }
    let prepared = prepare_leaf_slots(
        &mut engine,
        PreviousLeafSlots {
            sources: &mut sources,
            native: &mut native,
            schema: &previous_schema,
        },
        IncomingLeafSlots {
            schema: &incoming_schema,
            mapping: matched.previous_indices(),
        },
        LeafSlotLimits {
            compatibility: CompatibilityLimits {
                entries: n(1000),
                retained_bytes: n(1000000),
                work: n(1000000),
            },
            sources: slot_limits(),
            native: slot_limits(),
            identities: SourceChangeLimits {
                entries: n(1000),
                retained_bytes: n(1000000),
                work: n(1000000),
            },
        },
    )
    .map_err(|e| e.to_string())?;
    let incoming = prepared.commit();
    let mut incoming_native = incoming.native;
    let incoming_sources = incoming.sources;
    for tree in &mut incoming_native {
        if tree.is_none() {
            *tree = Some(input(false));
        }
    }
    let actual_focus = incoming_native
        .iter()
        .enumerate()
        .filter(|(_, tree)| {
            tree.as_ref()
                .unwrap()
                .state
                .downcast_ref::<iced::widget::text_input::State<()>>()
                .is_focused()
        })
        .map(|(index, _)| after[index].as_str())
        .collect::<Vec<_>>();
    if actual_focus.len() > 1 {
        return Err("multiple focused incoming leaf states".into());
    }
    let mut recreated = 0;
    for source in &incoming_sources {
        let id = engine.instance(source).map_err(|e| e.to_string())?;
        if !old_ids.contains(&id) {
            recreated += 1;
        }
    }
    Ok(json!({"focused":actual_focus.first().copied(),"recreated_sources":recreated}))
}
