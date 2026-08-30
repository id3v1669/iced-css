#[macro_use]
mod common;

css_todo_test!(policy_ondemand_reload, "OnDemand: Event::Reload");
css_todo_test!(policy_ondemand_error_recovery, "OnDemand: Reloaded(Err)");
css_todo_test!(policy_auto_watcher, "Auto: file watcher");
css_todo_test!(policy_auto_file_replaced, "Auto: file delete+recreate");
css_todo_test!(policy_runtime_structure_change, "runtime CSS edit changing structure");
css_todo_test!(policy_equivalence, "engine tests under OnDemand");

css_todo_test!(macro_dynamic_classes_rejected_under_compile, "dynamic classes under Compile");
css_todo_test!(macro_bad_policy_arg, "bad policy arg");
css_todo_test!(macro_duplicate_attr, "duplicate attribute");
css_todo_test!(macro_warning_mechanism, "build-time warnings");

css_todo_test!(selector_type, "type selectors");
css_todo_test!(selector_universal, "*");
css_todo_test!(selector_id, "#id");
css_todo_test!(selector_descendant, "descendant combinator");
css_todo_test!(selector_child, "> combinator");
css_todo_test!(selector_siblings, "+ and ~ combinators");
css_todo_test!(selector_grouping, ".a, .b grouping");
css_todo_test!(selector_compound, "button.primary compound");
css_todo_test!(selector_attribute, "[attr=val]");
css_todo_test!(selector_not_is_where_has, ":not() / :is() / :where() / :has()");
css_todo_test!(selector_structural, ":first-child / :last-child / :nth-child / :only-child / :empty");
css_todo_test!(specificity_and_important, "ordering, !important");
css_todo_test!(inheritance, "inheritance");
css_todo_test!(cascade_keywords, "inherit / initial / unset / revert");

css_todo_test!(box_sizing, "box-sizing");
css_todo_test!(aspect_ratio, "aspect-ratio");
css_todo_test!(margin_negative, "negative margins");
css_todo_test!(margin_collapsing, "margin collapsing");

css_todo_test!(display_none, "display: none");
css_todo_test!(display_inline_flow, "display: inline / inline-block");
css_todo_test!(visibility, "visibility");
css_todo_test!(overflow, "overflow");

css_todo_test!(flex_direction, "flex-direction");
css_todo_test!(flex_justify_content, "justify-content");
css_todo_test!(flex_align, "align-items / align-self / align-content");
css_todo_test!(flex_gap, "gap / row-gap / column-gap");
css_todo_test!(flex_grow_shrink_basis, "flex-grow / flex-shrink / flex-basis / flex");
css_todo_test!(flex_wrap_and_order, "flex-wrap / order");

css_todo_test!(grid_templates, "grid-template-columns/rows");
css_todo_test!(grid_placement, "grid-column / grid-row / grid-area");
css_todo_test!(grid_gaps, "grid gaps");
css_todo_test!(grid_auto, "grid-auto-flow / minmax() / auto-fill");

css_todo_test!(position_relative, "position: relative");
css_todo_test!(position_absolute, "position: absolute");
css_todo_test!(position_fixed, "position: fixed");
css_todo_test!(position_sticky, "position: sticky");
css_todo_test!(z_index, "z-index");
css_todo_test!(float_and_clear, "float / clear");

css_todo_test!(color_formats, "hex / rgb() / hsl() / named colors");
css_todo_test!(background_color, "background-color");
css_todo_test!(gradients, "linear-gradient / radial-gradient");
css_todo_test!(background_images, "background-image url()");
css_todo_test!(borders, "border");
css_todo_test!(border_radius, "border-radius");
css_todo_test!(box_shadows, "box-shadow");
css_todo_test!(outline, "outline");
css_todo_test!(opacity, "opacity");

css_todo_test!(text_color, "color");
css_todo_test!(font_size_units, "font-size");
css_todo_test!(font_family, "font-family");
css_todo_test!(font_weight_style, "font-weight / font-style / font");
css_todo_test!(line_and_letter_spacing, "line-height / letter-spacing / word-spacing");
css_todo_test!(text_align, "text-align");
css_todo_test!(text_decoration_transform, "text-decoration / text-transform");
css_todo_test!(text_wrapping, "white-space / text-overflow / overflow-wrap");

css_todo_test!(units_relative, "em / rem / vw / vh / vmin / vmax");
css_todo_test!(percent_of_window, "% on a root widget");
css_todo_test!(percent_height_against_auto_parent, "height: % against an auto-height parent computes to auto");
css_todo_test!(percent_margin_padding, "% margin / padding");
css_todo_test!(unimplemented_property_under_compile, "unimplemented property under Compile");
css_todo_test!(calc_and_math_fns, "calc() / min() / max() / clamp()");
css_todo_test!(custom_properties, "--x / var() / @property");

css_todo_test!(pseudo_hover_active, ":hover / :active");
css_todo_test!(pseudo_focus, ":focus / :focus-visible");
css_todo_test!(pseudo_state, ":disabled / :enabled / :checked");
css_todo_test!(pseudo_hover_structural, ":hover changing structure");
css_todo_test!(pseudo_elements, "::before / ::after / ::placeholder");

css_todo_test!(transform_translate, "transform: translate");
css_todo_test!(transform_scale_rotate_skew, "transform: scale / rotate / skew");
css_todo_test!(transitions, "transition");
css_todo_test!(keyframes_animations, "@keyframes / animation");

css_todo_test!(media_queries_size, "@media width/height/orientation");
css_todo_test!(media_prefers_color_scheme, "@media prefers-color-scheme");
css_todo_test!(media_never_matching, "@media print/speech");
css_todo_test!(at_import, "@import");
css_todo_test!(at_font_face, "@font-face");
css_todo_test!(at_supports, "@supports");
css_todo_test!(at_layer, "@layer");
css_todo_test!(css_nesting, "nesting with &");
css_todo_test!(at_charset_namespace, "@charset / @namespace");

css_todo_test!(unknown_property_warn_skip, "unknown property");
css_todo_test!(malformed_value_recovery, "malformed value");
css_todo_test!(edge_case_sheets, "empty / comments-only / BOM / huge sheets");
