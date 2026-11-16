use iced_css_core::values::registry::{AnimationRule as A, Inheritance as I, PropertyId, Scope};
use serde_json::Value;
use std::collections::BTreeSet;

fn property(name: &str) -> PropertyId {
    PropertyId::lookup(name).unwrap_or_else(|| panic!("missing {name}"))
}
fn targets(name: &str) -> BTreeSet<&'static str> {
    property(name)
        .longhands()
        .iter()
        .map(|id| id.descriptor().name)
        .collect()
}

// Catches absent/extra properties and corrupted definition fields independently of
// the Rust generator, against the pre-existing retained chapter-table extraction.
#[test]
fn admitted_inventory_matches_retained_definition_fields() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/retained/tmp/css22");
    let retained: Vec<Value> =
        serde_json::from_slice(&std::fs::read(root.join("property-definitions.json")).unwrap())
            .unwrap();
    let mut expected_names = BTreeSet::new();
    for entry in retained {
        let source = entry["source"].as_str().unwrap();
        if source.starts_with("about.") || source.starts_with("aural.") {
            continue;
        }
        let name = entry["name"].as_str().unwrap();
        expected_names.insert(name.to_owned());
        let d = property(name).descriptor();
        let fields = &entry["fields"];
        assert_eq!(d.source, source);
        for (actual, field) in [
            (d.grammar, "Value"),
            (d.initial, "Initial"),
            (d.applies_to, "Applies to"),
            (d.percentages, "Percentages"),
            (d.media, "Media"),
            (d.computed, "Computed value"),
        ] {
            assert_eq!(actual, fields[field].as_str().unwrap(), "{name}: {field}");
        }
        assert_eq!(
            d.inheritance,
            match fields["Inherited"].as_str().unwrap() {
                "yes" => I::Yes,
                "no" => I::No,
                "no (see prose)" => I::NoSeeProse,
                other => panic!("unadmitted inheritance: {other}"),
            }
        );
        assert_eq!(
            d.scope,
            if source.starts_with("page.") {
                Scope::Paged
            } else {
                Scope::Screen
            }
        );
    }
    let actual: BTreeSet<_> = PropertyId::all()
        .map(|id| id.descriptor().name.to_owned())
        .collect();
    assert_eq!(actual, expected_names);
    assert_eq!(
        PropertyId::all()
            .filter(|p| p.descriptor().scope == Scope::Screen)
            .count(),
        90
    );
    assert_eq!(
        PropertyId::all()
            .filter(|p| p.descriptor().scope == Scope::Paged)
            .count(),
        5
    );
}

// Catches Unicode case-folding, whitespace repair, accidental newer CSS admission,
// and confusing the illustrative/aural definition tables with screen properties.
#[test]
fn decoded_lookup_is_ascii_case_insensitive_and_baseline_bounded() {
    assert_eq!(PropertyId::lookup("CoLoR"), Some(property("color")));
    for name in [
        " color",
        "color ",
        "co\\6cor",
        "COLOR:",
        "opacity",
        "transform",
        "animation",
        "--x",
        "property-name",
        "speak",
        "wİdth",
        "",
    ] {
        assert_eq!(PropertyId::lookup(name), None, "{name}");
    }
}

// Hand-reviewed constituent sets from CSS2.2 chapter prose. This catches missing
// resets, CSS3 additions, confusing border with table properties, and self cycles.
#[test]
fn shorthands_expand_only_to_their_css22_longhands() {
    for (name, expected) in [
        (
            "background",
            vec![
                "background-color",
                "background-image",
                "background-repeat",
                "background-attachment",
                "background-position",
            ],
        ),
        (
            "font",
            vec![
                "font-style",
                "font-variant",
                "font-weight",
                "font-size",
                "line-height",
                "font-family",
            ],
        ),
        (
            "list-style",
            vec!["list-style-type", "list-style-position", "list-style-image"],
        ),
        (
            "outline",
            vec!["outline-color", "outline-style", "outline-width"],
        ),
        (
            "margin",
            vec!["margin-top", "margin-right", "margin-bottom", "margin-left"],
        ),
        (
            "padding",
            vec![
                "padding-top",
                "padding-right",
                "padding-bottom",
                "padding-left",
            ],
        ),
        (
            "border-width",
            vec![
                "border-top-width",
                "border-right-width",
                "border-bottom-width",
                "border-left-width",
            ],
        ),
        (
            "border-color",
            vec![
                "border-top-color",
                "border-right-color",
                "border-bottom-color",
                "border-left-color",
            ],
        ),
        (
            "border-style",
            vec![
                "border-top-style",
                "border-right-style",
                "border-bottom-style",
                "border-left-style",
            ],
        ),
        (
            "border-top",
            vec!["border-top-width", "border-top-style", "border-top-color"],
        ),
        (
            "border-right",
            vec![
                "border-right-width",
                "border-right-style",
                "border-right-color",
            ],
        ),
        (
            "border-bottom",
            vec![
                "border-bottom-width",
                "border-bottom-style",
                "border-bottom-color",
            ],
        ),
        (
            "border-left",
            vec![
                "border-left-width",
                "border-left-style",
                "border-left-color",
            ],
        ),
        (
            "border",
            vec![
                "border-top-width",
                "border-top-style",
                "border-top-color",
                "border-right-width",
                "border-right-style",
                "border-right-color",
                "border-bottom-width",
                "border-bottom-style",
                "border-bottom-color",
                "border-left-width",
                "border-left-style",
                "border-left-color",
            ],
        ),
    ] {
        assert!(property(name).descriptor().shorthand);
        assert_eq!(targets(name), expected.into_iter().collect(), "{name}");
    }
    assert_eq!(
        PropertyId::all()
            .filter(|p| p.descriptor().shorthand)
            .count(),
        14
    );
    for id in PropertyId::all() {
        if !id.descriptor().shorthand {
            assert_eq!(id.longhands(), &[id]);
        }
        assert_eq!(id.longhands().len(), targets(id.descriptor().name).len());
        for longhand in id.longhands() {
            assert!(!longhand.descriptor().shorthand);
        }
    }
}

// Catches substituting discrete for ineligible display, dropping special rules,
// or treating omitted page animation assessment as ineligibility.
#[test]
fn animation_dispatch_preserves_selected_exceptions_and_scope() {
    for (name, rule) in [
        ("display", A::NotAnimatable),
        ("font-family", A::Discrete),
        ("font-variant", A::Discrete),
        ("font-style", A::FontStyle),
        ("font-weight", A::FontWeight),
        ("visibility", A::Visibility),
        ("width", A::ByComputedValue),
        ("background", A::Shorthand),
    ] {
        assert_eq!(property(name).descriptor().animation, Some(rule));
    }
    for id in PropertyId::all() {
        let d = id.descriptor();
        assert!(!d.owner.is_empty());
        assert_eq!(d.animation.is_some(), d.scope == Scope::Screen);
        assert_eq!(d.animation == Some(A::Shorthand), d.shorthand);
    }
    assert_eq!(
        property("text-decoration").descriptor().inheritance,
        I::NoSeeProse
    );
    assert_eq!(property("orphans").descriptor().animation, None);
}

// A single wrong default on any of the 90 properties changes dispatch. Check the
// complete approved table, independently of the Python generator's table reader.
#[test]
fn all_screen_dispatch_and_owner_assignments_match_retained_architecture() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/retained/arch");
    for (file, animation) in [
        ("architecture-67-animation-registry.md", true),
        ("property-responsibilities.md", false),
    ] {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        let mut seen = BTreeSet::new();
        for line in text.lines().filter(|l| l.starts_with("| [")) {
            let cells: Vec<_> = line.split('|').map(str::trim).collect();
            let link = cells[1];
            let (name, rest) = link.strip_prefix('[').unwrap().split_once("](").unwrap();
            let source = rest
                .strip_prefix("../tmp/css22/spec/")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            assert!(seen.insert(name));
            let descriptor = property(name).descriptor();
            assert_eq!(descriptor.scope, Scope::Screen);
            assert_eq!(descriptor.source, source);
            if animation {
                let rule = match cells[2] {
                    "B" => A::ByComputedValue,
                    "D" => A::Discrete,
                    "N" => A::NotAnimatable,
                    "S" => A::Shorthand,
                    "V" => A::Visibility,
                    "FW" => A::FontWeight,
                    "FS" => A::FontStyle,
                    unexpected => panic!("unadmitted rule {unexpected}"),
                };
                assert_eq!(descriptor.animation, Some(rule), "{name}");
            } else {
                assert_eq!(descriptor.owner, cells[2], "{name}");
            }
        }
        let screen_names = PropertyId::all()
            .filter(|id| id.descriptor().scope == Scope::Screen)
            .map(|id| id.descriptor().name)
            .collect();
        assert_eq!(seen, screen_names);
    }
}
