use serde_json::json;
#[test]
fn contract_p5_2() {
    iced_css_conformance::assert_case("P5.2");
}
/// CSS 2.2 §10.3.5 shrink-to-fit: min(max(preferred minimum, available), preferred).
#[test]
fn the_float_category_width_follows_the_shrink_to_fit_arithmetic() {
    let run = iced_css_conformance::registry::lookup("p5_2").unwrap();
    for (minimum, available, preferred, expected) in [
        (40.0, 70.0, 100.0, 70.0),
        (40.0, 120.0, 100.0, 100.0),
        (80.0, 70.0, 100.0, 80.0),
        (40.0, 70.0, 40.0, 40.0),
    ] {
        let input = json!({
            "scenario": "shrink_to_fit",
            "preferred_minimum": minimum,
            "available": available,
            "preferred": preferred,
        });
        assert_eq!(run(&input).unwrap(), json!({ "used_width": expected }));
    }
}
fn float(side: &str, left: f32, right: f32, top: f32, bottom: f32) -> serde_json::Value {
    json!({"side": side, "left": left, "right": right, "top": top, "bottom": bottom})
}
fn placement(floats: serde_json::Value, side: &str, width: f32, top: f32) -> serde_json::Value {
    json!({
        "scenario": "place_float",
        "containing_left": 0.0,
        "containing_right": 100.0,
        "containing_top": 0.0,
        "floats": floats,
        "side": side,
        "width": width,
        "height": 10.0,
        "hypothetical_top": top,
        "work": 16,
    })
}
/// §9.5.1 rules 9 and 10: as high as possible, then as far to its own side as possible.
#[test]
fn a_float_takes_the_highest_then_outermost_position_available() {
    let run = iced_css_conformance::registry::lookup("p5_2").unwrap();
    for (side, left, right) in [("left", 0.0, 30.0), ("right", 70.0, 100.0)] {
        let result = run(&placement(json!([]), side, 30.0, 0.0)).unwrap();
        assert_eq!(
            result,
            json!({"left": left, "right": right, "top": 0.0, "bottom": 10.0,
                   "bands": 1, "overflows": false})
        );
    }
}
/// §9.5.1 rule 2: beside an earlier same-side float, or below it.
#[test]
fn a_float_sits_beside_an_earlier_one_and_otherwise_drops_below_it() {
    let run = iced_css_conformance::registry::lookup("p5_2").unwrap();
    let earlier = json!([float("left", 0.0, 60.0, 0.0, 10.0)]);
    let beside = run(&placement(earlier.clone(), "left", 40.0, 0.0)).unwrap();
    assert_eq!(beside["top"], json!(0.0));
    assert_eq!(beside["left"], json!(60.0));
    assert_eq!(beside["bands"], json!(1));
    let below = run(&placement(earlier, "left", 50.0, 0.0)).unwrap();
    assert_eq!(
        below["top"],
        json!(10.0),
        "does not fit beside, so it drops"
    );
    assert_eq!(below["left"], json!(0.0));
    assert_eq!(below["bands"], json!(2), "the search advanced one band");
}
/// §9.5.1 rule 3: a left float's right edge may not pass a right float's left edge.
#[test]
fn opposing_floats_narrow_the_same_band_from_both_sides() {
    let run = iced_css_conformance::registry::lookup("p5_2").unwrap();
    let both = json!([
        float("left", 0.0, 30.0, 0.0, 10.0),
        float("right", 80.0, 100.0, 0.0, 20.0),
    ]);
    let band = run(&json!({
        "scenario": "exclusion_band",
        "containing_left": 0.0,
        "containing_right": 100.0,
        "floats": both.clone(),
        "y": 0.0,
    }))
    .unwrap();
    assert_eq!(band, json!({"left": 30.0, "right": 80.0, "width": 50.0}));
    // The left float ends first, so the band below it is wider on the left only.
    let lower = run(&json!({
        "scenario": "exclusion_band",
        "containing_left": 0.0,
        "containing_right": 100.0,
        "floats": both.clone(),
        "y": 10.0,
    }))
    .unwrap();
    assert_eq!(lower, json!({"left": 0.0, "right": 80.0, "width": 80.0}));
    // A 50-wide left float fits between them; a 60-wide one must drop past the left float.
    assert_eq!(
        run(&placement(both.clone(), "left", 50.0, 0.0)).unwrap()["top"],
        json!(0.0)
    );
    assert_eq!(
        run(&placement(both, "left", 60.0, 0.0)).unwrap()["top"],
        json!(10.0)
    );
}
/// §9.5.1 rule 8: an oversized float keeps its width and sticks out rather than
/// being reduced, once it is already as far to its own side as possible.
#[test]
fn an_oversized_float_overflows_instead_of_being_reduced() {
    let run = iced_css_conformance::registry::lookup("p5_2").unwrap();
    let result = run(&placement(json!([]), "left", 140.0, 0.0)).unwrap();
    assert_eq!(
        result,
        json!({"left": 0.0, "right": 140.0, "top": 0.0, "bottom": 10.0,
               "bands": 1, "overflows": true})
    );
    // It still descends past every earlier float before giving up.
    let after = run(&placement(
        json!([float("left", 0.0, 20.0, 0.0, 25.0)]),
        "left",
        140.0,
        0.0,
    ))
    .unwrap();
    assert_eq!(after["top"], json!(25.0));
    assert_eq!(after["bands"], json!(2));
    assert_eq!(after["overflows"], json!(true));
}
/// §9.5.1 rules 4 and 6: no higher than the containing block, nor than an earlier float.
#[test]
fn a_float_is_never_higher_than_its_containing_block_or_an_earlier_float() {
    let run = iced_css_conformance::registry::lookup("p5_2").unwrap();
    let mut raised = placement(json!([]), "left", 10.0, -50.0);
    raised["containing_top"] = json!(15.0);
    assert_eq!(run(&raised).unwrap()["top"], json!(15.0));
    let later = json!([float("left", 0.0, 10.0, 40.0, 50.0)]);
    assert_eq!(
        run(&placement(later, "right", 10.0, 0.0)).unwrap()["top"],
        json!(40.0)
    );
}
#[test]
fn the_band_budget_and_invalid_inputs_are_reported_rather_than_normalised() {
    let run = iced_css_conformance::registry::lookup("p5_2").unwrap();
    let stack = json!([
        float("left", 0.0, 90.0, 0.0, 10.0),
        float("left", 0.0, 80.0, 0.0, 20.0),
        float("left", 0.0, 40.0, 0.0, 30.0),
    ]);
    assert_eq!(
        run(&placement(stack.clone(), "left", 50.0, 0.0)).unwrap()["bands"],
        json!(3)
    );
    let mut starved = placement(stack, "left", 50.0, 0.0);
    starved["work"] = json!(2);
    assert!(run(&starved).is_err(), "an exhausted budget is an error");
    for (field, value) in [
        ("scenario", json!("unknown")),
        ("width", json!(-1.0)),
        ("unexpected", json!(true)),
        ("work", json!(0)),
    ] {
        let mut changed = placement(json!([]), "left", 10.0, 0.0);
        changed[field] = value;
        assert!(run(&changed).is_err(), "{field} must be rejected");
    }
    let inverted = placement(
        json!([float("left", 0.0, 10.0, 20.0, 5.0)]),
        "left",
        10.0,
        0.0,
    );
    assert!(run(&inverted).is_err(), "an inverted float box is rejected");
}
/// The band a line box sees must not be narrowed by a float that begins below it, and
/// opposing floats that overlap admit nothing at all, not even a zero-width box.
#[test]
fn a_band_reflects_only_the_floats_that_actually_occupy_it() {
    let run = iced_css_conformance::registry::lookup("p5_2").unwrap();
    let band = |floats: serde_json::Value, y: f32| {
        run(&json!({
            "scenario": "exclusion_band",
            "containing_left": 0.0,
            "containing_right": 100.0,
            "floats": floats,
            "y": y,
        }))
        .unwrap()
    };
    let later = json!([float("left", 0.0, 40.0, 50.0, 60.0)]);
    assert_eq!(
        band(later.clone(), 0.0),
        json!({"left": 0.0, "right": 100.0, "width": 100.0}),
        "a float starting lower does not narrow a band above it"
    );
    assert_eq!(
        band(later, 50.0),
        json!({"left": 40.0, "right": 100.0, "width": 60.0})
    );
    let overlapping = json!([
        float("left", 0.0, 70.0, 0.0, 10.0),
        float("right", 40.0, 100.0, 0.0, 10.0),
    ]);
    let zero_width = run(&placement(overlapping, "left", 0.0, 0.0)).unwrap();
    assert_eq!(
        zero_width["top"],
        json!(10.0),
        "an inverted band admits nothing, so even a zero-width float descends"
    );
    assert_eq!(zero_width["left"], json!(0.0));
}
