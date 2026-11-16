//! A stylesheet-driven iced widget for captured box trees.
//!
//! ```
//! use iced_css::{
//!     box_tree::{BoxTree, attribute_limits, capture_limits},
//!     capture::adapters,
//! };
//!
//! # fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! #[derive(Debug, Clone)]
//! enum Message {
//!     Save,
//!     OpenCard,
//! }
//!
//! // Capture a tree. `style_id` names each box for the stylesheet.
//! let leaf = |id: &str| -> Result<_, Box<dyn std::error::Error>> {
//!     Ok(adapters::column_from_vec::<Message, iced::Theme, iced::Renderer>(vec![], capture_limits())?
//!         .style_id(id, attribute_limits())?
//!         .erase())
//! };
//! let children = vec![leaf("action")?, leaf("card")?];
//! let root = adapters::column_from_vec(children, capture_limits())?
//!     .style_id("page", attribute_limits())?
//!     .erase();
//!
//! // Cascade once; the tree lays out again at any size.
//! let tree = BoxTree::new(
//!     root,
//!     "#page { padding: 16px; background-color: #f4f1ea; } #action, #card { height: 40px; }",
//! )?;
//! let boxes = tree.layout(800.0, None)?;
//! assert_eq!(boxes[1].as_ref().map(|action| action.border.top), Some(16.0));
//!
//! // In a view: host native iced widgets in leaf boxes and handle presses.
//! let element: iced::Element<'_, Message> = tree
//!     .clone()
//!     .interactive()
//!     .widget("action", iced::widget::button("Save").on_press(Message::Save))
//!     .on_press("card", Message::OpenCard)
//!     .into();
//! # drop(element);
//! # Ok(())
//! # }
//! # demo().unwrap();
//! ```
//!
//! What it does: real cascade, general block
//! flow, background colors and images and border paint, and inline text: consecutive inline siblings share
//! one wrapped paragraph of styled runs. Runs carry size, color, weight, style, family,
//! underline, strikethrough and background. One line height and alignment apply per
//! paragraph. Inline boxes take their sides through stand-in spaces. Native iced widgets can be placed
//! in leaf boxes by CSS id, where they receive input; boxes can publish a message on press;
//! `:hover`, `:active` and `:focus` restyle the tree. Boxes with a press message or a
//! `tabindex` attribute take focus by click and Tab; Enter and Space press the focused box. `overflow` other than visible clips, and
//! `auto` and `scroll` boxes scroll on both axes with the wheel; there is no draggable bar. Captured payloads themselves are not drawn.
//!
//! Its text takes its own path: white space collapses and `text-transform` applies here,
//! and lines are built by `iced_css_core::formatting::block_flow`. The standalone
//! pipelines in `iced_css_core::text` (`preprocess_whitespace`, `trim_line_edges`),
//! `iced_css_core::formatting::line_geometry` (`layout_line_geometry`,
//! `layout_inline_tree`) and the `evaluation::source_whitespace` and `source_casing`
//! chain are building blocks this widget does not use yet, so their tests do not cover
//! what it draws.
use crate::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, Construction, source::ConstructionState},
    evaluation::{
        clearance_box_facts::ClearanceBoxFactsLimits,
        display_rules::{
            BundleLimits, DisplayRuleBundle, GraphBinding, GraphOrigin, resolve_variables,
        },
        source_display::{SourceDisplayLimits, evaluate_source_display},
        tree_layout::{
            BackgroundImage, BackgroundOffset, BackgroundSize, BoxArea, BoxPaint, ElementAnimation,
            ElementTransform, ElementTransition, GeneratedFrame, GeneratedRun, GradientAim,
            GradientPaint, InlineBoxSides, PreparedTree, RunSource, TextLineHeight, TextStyle,
            TransformStep, TreeLayoutError, TreePaint, evaluate_tree_paint, prepare_captured_tree,
            prepare_tree_source_projection,
        },
    },
    host::{
        source_publication::{InitialSourceHost, PublicationLimits},
        stylesheet::graph_recovery::report_graph_recovery,
    },
};
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, image, layout, overlay,
    renderer::{self, Quad},
    text::{self, Paragraph as _},
    widget::{Operation, Tree, tree},
};
use iced::{
    Color, Element, Event, Length, Pixels, Point, Rectangle, Size, Vector, keyboard, mouse, touch,
    window,
};
/// Core types named in this module's signatures, so callers need no second dependency.
pub use iced_css_core::formatting::block_flow::{FlowGeometry, FlowRect};
pub use iced_css_core::source::{AttributeError, AttributeLimits};
pub use iced_css_core::syntax::MediaEnvironment;
use iced_css_core::values::bidi::{ComputedDirection, ComputedUnicodeBidi};
use iced_css_core::values::registry::PropertyId;
use iced_css_core::{
    cascade::{
        rule::{RuleBatchLimits, RuleCascadeLimits, RuleContext},
        sheets::{AuthorSheet, AuthorSheets, Membership, SheetId, SheetLimits},
        ua_display::{UaDisplayCatalog, UaDisplayLimits},
    },
    formatting::{
        block::Direction,
        block_flow::{Dim, FlowContent, FlowLimits, FlowViewport, RowAlign, TableRole},
        display::Position,
    },
    identity::SourceChangeLimits,
    selectors::{MatchLimits, SelectorLimits},
    source::selector_tree::SelectorTreeLimits,
    stylesheet::{
        RuleAssemblyLimits, StylesheetIntakeLimits, StylesheetLimits,
        imports::{LoadedGraphLimits, UnavailableImportPolicy},
        owned_graph::{OwnedGraphLimits, OwnedLoadedSheet, OwnedStylesheetGraph},
        parse_separate_unicode_stylesheet,
    },
    syntax::{
        RecoveryLimits, StringLimits,
        decode::DecodeLimits,
        encoding_selection::{EncodingMetadata, SelectionLimits},
        imports::{ImportLimits, ResourceId},
        unicode_intake::IntakeLimits,
        with_media_environment,
    },
    values::{NumericLimits, color::ColorLimits},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU64, NonZeroUsize},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

mod animation;
pub use animation::{BoxTreeAnimations, Transition};

/// What building a box tree can refuse. Each cause keeps its own message.
#[derive(Debug)]
pub struct BoxTreeError(String);
impl std::fmt::Display for BoxTreeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "box tree: {}", self.0)
    }
}
impl std::error::Error for BoxTreeError {}
fn failed(error: impl std::fmt::Debug) -> BoxTreeError {
    BoxTreeError(format!("{error:?}"))
}

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("limit constants are positive")
}
const COUNT: usize = 100_000;
const BYTES: usize = 64_000_000;
const WORK: usize = 200_000_000;
/// For limits that multiply worst cases ahead of time (every number at its longest in every
/// box), which would refuse a page of a few hundred boxes. `COUNT` and `BYTES` bound the input.
const WORST_CASE: usize = usize::MAX >> 4;
/// The one budget that counts real work: three rule tests per box and rule, so 100 million
/// box and rule pairs. Simple selectors take about 33 s there (see `tests/box_tree_scale.rs`).
/// It bounds the number of tests, not what each costs: `MATCH_WORK` bounds that.
const RULE_VISITS: usize = 300_000_000;
/// One selector against one box. Each compound walks the ancestors once, so a selector spends
/// a few units and its names' lengths per compound and ancestor: ordinary deep pages fit, and
/// only names thousands of bytes long do not. Nothing meters the total, so sheets are trusted
/// input.
const MATCH_WORK: usize = 1_000_000;
/// Boxes laid out in one flow layout, counting each again when its subtree is laid out again:
/// a box with `overflow` beside floats is, once per place tried, and nesting multiplies those.
/// A hundred passes over the largest tree; one built to double per level stops here.
const LAYOUT_WORK: usize = 100 * COUNT;

fn big() -> NonZeroUsize {
    nz(COUNT)
}
fn numeric() -> NumericLimits {
    NumericLimits {
        work: big(),
        digits: big(),
    }
}
fn colors() -> ColorLimits {
    ColorLimits {
        syntax: StringLimits {
            work: big(),
            retained_bytes: big(),
        },
        numeric: numeric(),
    }
}
fn selectors() -> SelectorTreeLimits {
    SelectorTreeLimits {
        entries: big(),
        retained_bytes: nz(BYTES),
        work: nz(WORST_CASE),
    }
}
fn per_target() -> RuleBatchLimits {
    RuleBatchLimits {
        rules: big(),
        per_rule: RuleCascadeLimits {
            declarations: big(),
            matching: MatchLimits {
                work: nz(MATCH_WORK),
                retained_bytes: big(),
            },
        },
    }
}
fn facts() -> ClearanceBoxFactsLimits {
    ClearanceBoxFactsLimits {
        work: nz(WORST_CASE),
        rule_visits: nz(RULE_VISITS),
        matching_work: nz(WORST_CASE),
        declaration_offers: nz(WORST_CASE),
        membership_work: nz(WORST_CASE),
        numeric_work: nz(WORST_CASE),
        selectors: selectors(),
        per_target: per_target(),
        numeric: numeric(),
    }
}
/// The cascade order space each author sheet gets, which also clears the user agent's.
const SHEET_ORDER: u64 = 100_000_000;

/// Parses one author sheet. `order` places its rules in the cascade, later over earlier.
fn parse(css: &str, order: u64) -> Result<OwnedStylesheetGraph, BoxTreeError> {
    let context = RuleContext {
        first_order: 0,
        first_declaration: NonZeroU64::MIN,
    };
    let candidate = parse_separate_unicode_stylesheet(
        css.as_bytes(),
        EncodingMetadata {
            transport: None,
            linking: None,
            referring: None,
        },
        context,
        StylesheetIntakeLimits {
            intake: IntakeLimits {
                selection: SelectionLimits {
                    registry_work: big(),
                    prefix_work: big(),
                },
                decode: DecodeLimits {
                    entries: nz(BYTES),
                    retained_bytes: nz(BYTES),
                    work: nz(WORK),
                },
                charset_work: nz(BYTES),
            },
            sheet: StylesheetLimits {
                rules: RuleAssemblyLimits {
                    recovery: RecoveryLimits {
                        work: nz(WORK),
                        retained_bytes: nz(BYTES),
                        nesting: big(),
                    },
                    selectors: SelectorLimits {
                        work: nz(WORK),
                        retained_bytes: nz(BYTES),
                        entries: big(),
                    },
                    values: colors(),
                    entries: big(),
                    record_bytes: nz(BYTES),
                    work: nz(WORK),
                    name_bytes: big(),
                },
                statements: big(),
                record_bytes: nz(BYTES),
                work: nz(WORK),
            },
        },
    )
    .map_err(failed)?;
    let graph = OwnedStylesheetGraph::new(
        vec![OwnedLoadedSheet {
            // Numbered from one by place in the sheets, as the log names it.
            resource: ResourceId::new(NonZeroU64::MIN.saturating_add(order)),
            candidate,
            imports: vec![],
        }],
        0,
        // Author rules order after every user agent rule.
        RuleContext {
            first_order: SHEET_ORDER * (order + 1),
            first_declaration: NonZeroU64::new(SHEET_ORDER * (order + 1)).expect("positive"),
        },
        OwnedGraphLimits {
            record_bytes: nz(BYTES),
            work: nz(WORK),
            expansion: LoadedGraphLimits {
                unavailable: UnavailableImportPolicy::Record,
                entries: big(),
                record_bytes: nz(BYTES),
                work: nz(WORK),
                tracking: ImportLimits {
                    entries: big(),
                    retained_bytes: nz(BYTES),
                    work: nz(WORK),
                    occurrences: NonZeroU64::new(100).expect("positive"),
                },
            },
        },
    )
    .map_err(failed)?;
    // What the parser dropped and went on without, logged as host stylesheets log it.
    if let Err(error) = report_graph_recovery(&graph, None, nz(WORK)) {
        log::error!(target: "iced_css::stylesheet", "stylesheet resource {}: {error}", order + 1);
    }
    Ok(graph)
}

/// Default capture limits for assembling a tree with the capture adapters.
#[must_use]
pub fn capture_limits() -> CaptureLimits {
    CaptureLimits {
        nodes: nz(COUNT),
        depth: nz(2_000),
        retained_bytes: nz(BYTES),
        work: nz(WORK),
    }
}

/// Limits for naming a box with `style_id` or `style_attribute`, ample for hand-written trees.
#[must_use]
pub fn attribute_limits() -> AttributeLimits {
    AttributeLimits {
        entries: big(),
        retained_bytes: nz(BYTES),
        work: nz(WORK),
    }
}

/// One styled run of inline text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub size: f32,
    /// The size its line height comes from, when it is drawn smaller than its computed one:
    /// the small letters of `small-caps`.
    pub line_size: Option<f32>,
    /// The inline boxes it sits in, outermost first, as painted above and below the line.
    pub inline_boxes: Vec<InlineDecoration>,
    /// Spaces standing in for an indent or an inline box's side: they stay with the next or
    /// previous word when measuring its minimum width.
    pub spacer: bool,
    /// The `border-*-style` of that side border.
    pub edge_style: &'static str,
    /// A side border in this run's room: at its start (`true`) or end, its width and color.
    pub edge: Option<(bool, f32, [f32; 3])>,
    pub color: Option<[f32; 3]>,
    /// The opacity `color` draws at: its element's `opacity` through its ancestors.
    pub alpha: f32,
    pub background: Option<[f32; 3]>,
    /// The source node of the inline element whose background `background` is, for
    /// animating it; `None` for a run's own or no background.
    pub background_from: Option<usize>,
    /// Every inline background around it, nearest first: the color, the box's source node
    /// (a `:before` or `:after` box's `generated_owner`), and whether the box shows.
    /// `background` is the first that shows; an animated `visibility` picks again.
    pub backgrounds: Vec<([f32; 3], usize, bool)>,
    /// A side's stand-in spaces sized at layout, at the run's font size: fixed pixels, a
    /// percentage of the containing block's width, and whether they round up to keep a
    /// thin border's bar.
    pub percent_room: Option<(f32, f32, bool)>,
    /// For a side's spaces: the box (its source node, or a `:before` or `:after` box's
    /// `generated_owner`) and side, 3 for the start and 1 for the end. Its margin's spaces
    /// do not round (`percent_room`), its border and padding's do.
    pub side_of: Option<(usize, usize)>,
    /// Inline backgrounds on their way to or from transparent, over `background` in its
    /// content area, outermost first: each box's source node, color and opacity.
    pub fades: Vec<(usize, [f32; 3], f32)>,
    /// The opacity of `edge`'s color, 1 but while it fades.
    pub edge_alpha: f32,
    /// For a side's border and padding spaces under a negative margin: how far its box
    /// draws past them, outward over what comes before (start) or after (end) it.
    pub overhang: f32,
    /// What a negative margin pulls past its box's own border and padding, in pixels: the
    /// space beside it narrows by up to that much.
    pub pull: f32,
    /// A negative margin's pixels beside a percentage padding, taken out of the room it has
    /// at layout, where `overhang` is set (`percent_overhang`).
    pub pulled: f32,
    /// The `:before` or `:after` whose content this is, by its `generated_owner` key; its own
    /// side spaces carry none.
    pub generated: Option<usize>,
    /// An inside list marker's image: once it loads, the run is spaces as wide as it.
    pub marker_image: Option<MarkerImage>,
    pub weight: u16,
    pub italic: bool,
    pub family: Option<String>,
    pub underline: bool,
    pub line_through: bool,
    /// 16.3.1: the source node whose `text-decoration` draws the underline, with its color
    /// and font size, when an ancestor's; the run's own otherwise.
    pub underline_from: Option<(usize, [f32; 3], f32)>,
    /// As `underline_from`, for the line through.
    pub line_through_from: Option<(usize, [f32; 3], f32)>,
    pub overline: bool,
    /// As `underline_from`, for the overline.
    pub overline_from: Option<(usize, [f32; 3], f32)>,
    /// 11.2: hidden text still takes up its room.
    pub visible: bool,
    /// The source node this run belongs to, for hit testing.
    pub owner: usize,
}

/// The image of an inside list marker, drawn over its run's spaces.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerImage {
    handle: image::Handle,
    /// The marker's text, shown while the image does not load.
    fallback: Option<String>,
    /// The image's size, once loaded.
    shown: Option<Size>,
}

/// The inline content of one block: its runs, after white space and text-transform.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLeaf {
    pub runs: Vec<TextRun>,
    pub align: &'static str,
    pub wraps: bool,
    /// CSS Text 3: where a wrapping leaf's lines may break.
    pub breaks: TextBreaks,
    /// CSS Overflow 3 `text-overflow: ellipsis` in a box that clips: a line that does not
    /// wrap ends in "…" where it would overflow.
    pub ellipsis: bool,
    pub line_height: TextLineHeight,
    /// A percentage `text-indent` this leaf's first line takes once its width is known.
    pub percent_indent: Option<PercentIndent>,
    /// How far a negative `text-indent` pulls the first line out past the left edge, in
    /// pixels: that line lays out this much wider. Left-to-right blocks only.
    pub hang: f32,
}

/// Where a leaf's lines may break: `word-break` and `overflow-wrap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextBreaks {
    /// Between words only.
    #[default]
    Words,
    /// `overflow-wrap: break-word`: a word too long for a line breaks anywhere, though
    /// the narrowest the leaf can be stays its widest word.
    LongWords,
    /// `overflow-wrap: anywhere` or `word-break: break-word`: likewise, and its narrowest
    /// is its widest character.
    Anywhere,
    /// `word-break: break-all`: between any two characters.
    Everywhere,
}

/// A percentage `text-indent`, resolved against the width the leaf lays out in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PercentIndent {
    pub percent: f32,
    /// The source node of the block the indent belongs to.
    pub owner: usize,
    /// Whether the leaf's first run is the spacer this indent last made.
    pub inserted: bool,
}

impl TextLeaf {
    /// The runs' text, joined.
    #[must_use]
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
    fn is_empty(&self) -> bool {
        self.runs.iter().all(|run| run.text.is_empty())
    }
}

/// Where `capitalize` words start in a leaf's joined source text: UAX#29 word
/// boundaries, so an apostrophe or a combining mark inside a word starts none,
/// and neither does the seam between two runs.
fn word_starts(text: &str) -> Vec<usize> {
    use iced_css_core::text::word_boundaries::{WordBoundaryLimits, evaluate_word_boundaries};
    use std::num::NonZeroUsize;
    let scalars = text.chars().count();
    let (Some(input_bytes), Some(count)) =
        (NonZeroUsize::new(text.len()), NonZeroUsize::new(scalars))
    else {
        return Vec::new();
    };
    let work =
        NonZeroUsize::new(scalars.saturating_mul(4).saturating_add(1)).unwrap_or(NonZeroUsize::MAX);
    evaluate_word_boundaries(
        text,
        WordBoundaryLimits {
            input_bytes,
            scalars: count,
            retained_bytes: NonZeroUsize::MAX,
            work,
        },
    )
    .map(|words| words.offsets().to_vec())
    .unwrap_or_default()
}

/// `starts` are the leaf's word starts and `base` this run's offset among them;
/// only `capitalize` reads either.
fn transformed(text: &str, transform: &str, starts: &[usize], base: usize) -> String {
    match transform {
        "uppercase" => text.to_uppercase(),
        "lowercase" => text.to_lowercase(),
        "capitalize" => {
            let mut out = String::with_capacity(text.len());
            for (at, character) in text.char_indices() {
                if character.is_alphanumeric() && starts.binary_search(&(base + at)).is_ok() {
                    out.extend(character.to_uppercase());
                } else {
                    out.push(character);
                }
            }
            out
        }
        _ => text.to_owned(),
    }
}

/// 16.6.1: only space, tab and line feed collapse (a carriage return is a line feed by
/// then); U+000C, U+00A0 and U+3000 are ordinary characters.
fn is_css_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r')
}

/// The white-space mode that marks an empty inline's zero-width placeholder run.
const PLACEHOLDER: &str = "placeholder";

/// 16.6.1: collapse white space across the runs of one block, by each run's own mode.
fn collapse(texts: &mut [(String, &'static str)]) {
    // 16.6: with no newline rules from a document language, CR and CRLF are one line feed.
    for (text, _) in texts.iter_mut() {
        if text.contains('\r') {
            *text = text.replace("\r\n", "\n").replace('\r', "\n");
        }
    }
    let mut after_space = true;
    let mut last_collapsing = None;
    for index in 0..texts.len() {
        let (before, rest) = texts.split_at_mut(index);
        let (text, mode) = &mut rest[0];
        // An empty inline's placeholder is no text: spaces collapse across it.
        if *mode == PLACEHOLDER {
            continue;
        }
        if matches!(*mode, "pre" | "pre-wrap") {
            // A preserved space is not collapsible: a collapsible one after it stays.
            if !text.is_empty() {
                after_space = false;
                last_collapsing = None;
            }
            continue;
        }
        let keep_newlines = *mode == "pre-line";
        let mut out = String::with_capacity(text.len());
        for character in text.chars() {
            if keep_newlines && character == '\n' {
                while out.ends_with(' ') {
                    out.pop();
                }
                // A collapsible space ending an earlier run ends this line too.
                if out.is_empty() {
                    trim_before_newline(before);
                }
                out.push('\n');
                after_space = true;
            } else if is_css_space(character) {
                if !after_space {
                    out.push(' ');
                }
                after_space = true;
            } else {
                out.push(character);
                after_space = false;
            }
        }
        *text = out;
        if !text.is_empty() {
            last_collapsing = Some(index);
        }
    }
    if let Some(index) = last_collapsing
        && texts[index].0.ends_with(' ')
    {
        texts[index].0.pop();
    }
}

/// Drops the collapsible spaces that end `texts`, back to a preserved run or other text.
fn trim_before_newline(texts: &mut [(String, &'static str)]) {
    for (text, mode) in texts.iter_mut().rev() {
        if *mode == PLACEHOLDER {
            continue;
        }
        if matches!(*mode, "pre" | "pre-wrap") && !text.is_empty() {
            return;
        }
        while text.ends_with(' ') {
            text.pop();
        }
        if !text.is_empty() {
            return;
        }
    }
}

/// The hovered, active and focused source nodes a tree is styled for.
type InteractionKey = (Option<usize>, Option<usize>, Option<usize>);

/// Which boxes a rule can select with one interaction pseudo-class, read from the text of
/// the sheets. It may say too much, never too little: anything it cannot read means all.
#[derive(Debug, Default)]
struct PseudoReach {
    any: bool,
    ids: BTreeSet<String>,
    classes: BTreeSet<String>,
}

impl PseudoReach {
    fn of(sheets: &[String], pseudo: &str) -> Self {
        let mut reach = Self::default();
        for sheet in sheets {
            // ASCII only: full lowercasing depends on context (a final sigma), so one name
            // could fold differently here and on the node, and the matcher does not fold it.
            let text = sheet.to_ascii_lowercase();
            // An escape can spell the pseudo-class or a name in a way this does not read, and
            // a comment can sit between the colon and the name.
            // Nor does it read what decoding changes: a NUL becomes a replacement character in
            // a name, and NULs mean the bytes are UTF-16 or UTF-32 under an `@charset`.
            if text.contains(['\\', '\0']) || text.contains(":/*") {
                reach.any = true;
                continue;
            }
            for (at, _) in text.match_indices(pseudo) {
                let before = &text[..at];
                // Past the whole delimiter: whitespace can be more than one byte long.
                let start = before
                    .char_indices()
                    .rev()
                    .find(|&(_, character)| {
                        character.is_whitespace() || ",>+~{};/".contains(character)
                    })
                    .map_or(0, |(found, character)| found + character.len_utf8());
                let compound = &before[start..];
                let name = |marker: char| {
                    compound.find(marker).map(|found| {
                        compound[found + 1..]
                            .split(['.', '#', ':'])
                            .next()
                            .unwrap_or_default()
                            .to_owned()
                    })
                };
                if compound.contains(['[', ']', '"', '\'', '(', ')']) {
                    reach.any = true;
                } else if let Some(id) = name('#') {
                    reach.ids.insert(id);
                } else if let Some(class) = name('.') {
                    reach.classes.insert(class);
                } else {
                    reach.any = true;
                }
            }
        }
        reach
    }

    fn selects(&self, metadata: &iced_css_core::source::Metadata) -> bool {
        self.any
            || metadata
                .css_id()
                .is_some_and(|id| self.ids.contains(&id.to_ascii_lowercase()))
            || metadata.attribute("class").is_some_and(|classes| {
                classes
                    .split_whitespace()
                    .any(|class| self.classes.contains(&class.to_ascii_lowercase()))
            })
    }
}

#[derive(Debug)]
struct Source {
    /// What `:hover`, `:active` and `:focus` can select in these sheets.
    reach: [PseudoReach; 3],
    /// Which capture this is; restyles share it, so it tells one element from a reused id.
    capture: u64,
    construction: Construction,
    sheets: Vec<String>,
    /// What relative `url()`s resolve against.
    base: PathBuf,
    /// What the sheets' media queries are answered against.
    media: MediaEnvironment,
    /// Restyles already cascaded. Pointerless, so nothing here keeps the source alive.
    variants: Mutex<Variants>,
    /// The sheets parsed, and the rules every page starts from: the same for every restyle.
    parsed: OnceLock<Parsed>,
    /// Keys whose restyle was refused: asking again would only cascade again, per frame.
    refused: Mutex<BTreeSet<InteractionKey>>,
    /// Why the widget's last layout was refused, said once until a layout succeeds.
    layout_refusal: Mutex<Option<String>>,
}

/// One keyframe rule of a `@keyframes`, as the animations read it.
#[derive(Debug, Clone)]
pub(crate) struct KeyframeSummary {
    pub(crate) offsets: Vec<f32>,
    pub(crate) longhands: Vec<PropertyId>,
    /// Its `animation-timing-function`, for the stretch after it.
    pub(crate) easing: Option<crate::animation::easing::CssEasing>,
}

/// The `@keyframes` rules of `graphs` by name; of two with one name, the later wins.
fn keyframe_rules(
    graphs: &[OwnedStylesheetGraph],
) -> std::collections::HashMap<&str, &iced_css_core::stylesheet::ParsedKeyframes> {
    graphs
        .iter()
        .flat_map(OwnedStylesheetGraph::keyframes)
        .map(|rule| (rule.name.as_str(), rule))
        .collect()
}

/// A source's sheets as rule graphs, after the rules every page starts from.
struct Parsed(UaDisplayCatalog, Vec<OwnedStylesheetGraph>);

impl std::fmt::Debug for Parsed {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Parsed").finish_non_exhaustive()
    }
}

static NEXT_CAPTURE: AtomicU64 = AtomicU64::new(0);

impl Source {
    fn reach_of(sheets: &[String]) -> [PseudoReach; 3] {
        [":hover", ":active", ":focus"].map(|pseudo| PseudoReach::of(sheets, pseudo))
    }

    /// Whether an interaction state could restyle `node`: some `:hover`, `:active` or
    /// `:focus` rule can select it, an ancestor, or an earlier sibling of either, which
    /// descendant and sibling combinators reach it from.
    pub(super) fn interaction_reaches(&self, node: usize) -> bool {
        let nodes = self.construction.nodes();
        let parent = |at: usize| nodes.get(at).and_then(|value| value.parent());
        // The node and its ancestors, and for each parent on the way the latest of its
        // children there: its earlier children come before it (siblings in order).
        let mut chain = BTreeSet::new();
        let mut latest: BTreeMap<usize, usize> = BTreeMap::new();
        let mut at = Some(node);
        while let Some(here) = at {
            chain.insert(here);
            at = parent(here);
            if let Some(up) = at {
                latest.insert(up, here);
            }
        }
        // One pass, testing only what could match.
        (0..nodes.len()).any(|at| {
            let candidate = chain.contains(&at)
                || parent(at).is_some_and(|up| latest.get(&up).is_some_and(|last| at < *last));
            candidate
                && self
                    .reach
                    .iter()
                    .any(|reach| reach.selects(nodes[at].metadata()))
        })
    }

    /// The same styling under fewer keys: an interaction on a box no rule can select counts
    /// as being on the nearest ancestor one can, for the states ancestors share (5.11.3),
    /// and otherwise as none. So hovering a box no rule cares about cascades nothing.
    fn normal_key(&self, key: InteractionKey) -> InteractionKey {
        let nodes = self.construction.nodes();
        let reach = |start: Option<usize>, reach: &PseudoReach, inherits: bool| {
            let mut at = start;
            while let Some(here) = at {
                let node = nodes.get(here)?;
                if reach.selects(node.metadata()) {
                    return Some(here);
                }
                at = if inherits { node.parent() } else { None };
            }
            None
        };
        (
            reach(key.0, &self.reach[0], true),
            reach(key.1, &self.reach[1], true),
            reach(key.2, &self.reach[2], false),
        )
    }
}

/// Interaction states the cache holds before dropping the one unused longest: each is a
/// whole style.
const VARIANT_LIMIT: usize = 64;

/// Cascaded stylings by interaction state, the least recently used dropped first.
#[derive(Debug, Default)]
struct Variants {
    /// Counts uses; each styling carries the count of its latest one.
    uses: u64,
    styles: BTreeMap<InteractionKey, (u64, Arc<Style>)>,
}

impl Variants {
    fn get(&mut self, key: InteractionKey) -> Option<Arc<Style>> {
        self.uses += 1;
        let (used, style) = self.styles.get_mut(&key)?;
        *used = self.uses;
        Some(Arc::clone(style))
    }

    fn insert(&mut self, key: InteractionKey, style: Arc<Style>) {
        self.uses += 1;
        if self.styles.len() >= VARIANT_LIMIT && !self.styles.contains_key(&key) {
            let oldest = self
                .styles
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                self.styles.remove(&oldest);
            }
        }
        self.styles.insert(key, (self.uses, style));
    }
}

/// Everything that varies with the cascade.
#[derive(Debug, Clone)]
struct Style {
    prepared: PreparedTree,
    paints: Vec<BoxPaint>,
    texts: Vec<Option<TextLeaf>>,
    ids: Vec<Option<String>>,
    originals: Vec<usize>,
    markers: BTreeMap<usize, TextLeaf>,
    backgrounds: BTreeMap<usize, (BackgroundImage, image::Handle)>,
    important: Vec<Vec<PropertyId>>,
    em_lengths: Vec<Vec<PropertyId>>,
    weights: Vec<u16>,
    marker_images: BTreeMap<usize, image::Handle>,
    transitions: Vec<Vec<ElementTransition>>,
    animations: Vec<Vec<ElementAnimation>>,
    key: InteractionKey,
    gradients: Vec<Option<Arc<GradientPaint>>>,
    transforms: Vec<Option<Arc<ElementTransform>>>,
}

/// An owned, width-independent styled tree. One `BoxTree` can lay out at any size.
#[derive(Debug, Clone)]
pub struct BoxTree {
    prepared: PreparedTree,
    paints: Vec<BoxPaint>,
    /// Inline content per arena slot.
    texts: Vec<Option<TextLeaf>>,
    /// The CSS id per arena slot.
    ids: Vec<Option<String>>,
    /// The source node per projected index.
    originals: Vec<usize>,
    /// Outside list markers, by projected index of their item.
    markers: BTreeMap<usize, TextLeaf>,
    /// Background images that can load, by projected index.
    backgrounds: BTreeMap<usize, (BackgroundImage, image::Handle)>,
    source: Arc<Source>,
    /// Longhands won by `!important`, per projected index.
    important: Vec<Vec<PropertyId>>,
    /// Length longhands given in `em` or `ex`, per projected index.
    em_lengths: Vec<Vec<PropertyId>>,
    /// Computed `font-weight`, 100 to 900, per projected index.
    weights: Vec<u16>,
    /// Images that replace outside markers, by projected index of their item.
    marker_images: BTreeMap<usize, image::Handle>,
    /// Temporary values of running animations, laid over every interaction styling.
    presentation: Option<Arc<Vec<animation::Presented>>>,
    /// The transitions each element asks for, by projected index (CSS Transitions 1).
    transitions: Vec<Vec<ElementTransition>>,
    /// The animations each element asks for, by projected index (CSS Animations 1).
    animations: Vec<Vec<ElementAnimation>>,
    /// The interaction state it was styled for.
    key: InteractionKey,
    /// Background gradients, by projected index (CSS Images 3).
    gradients: Vec<Option<Arc<GradientPaint>>>,
    /// Transforms, by projected index (CSS Transforms 1).
    transforms: Vec<Option<Arc<ElementTransform>>>,
}

/// Two trees are equal when they draw alike and restyle from the same sheets.
impl PartialEq for BoxTree {
    fn eq(&self, other: &Self) -> bool {
        self.prepared == other.prepared
            && self.paints == other.paints
            && self.texts == other.texts
            && self.markers == other.markers
            && self.backgrounds == other.backgrounds
            && self.marker_images == other.marker_images
            && self.gradients == other.gradients
            && self.transforms == other.transforms
            && (Arc::ptr_eq(&self.source, &other.source)
                || self.source.sheets == other.source.sheets)
    }
}

impl BoxTree {
    /// Cascades `css` over a captured tree. Only the capture evidence is kept.
    ///
    /// The native payloads are dropped, with whatever was set on them: an adapter
    /// button's `on_press`, a checkbox's `on_toggle`, a slider's `on_change` and style
    /// closures never run, and a radio's, checkbox's or toggler's label lays out as a
    /// text leaf with no control. Put native widgets in boxes with
    /// [`BoxTreeView::widget`] and press messages with [`BoxTreeView::on_press`].
    ///
    /// # Errors
    /// Returns a stylesheet, capture, cascade or unsupported-display failure.
    pub fn new<Message, Theme, Renderer>(
        root: CapturedElement<'_, Message, Theme, Renderer>,
        css: &str,
    ) -> Result<Self, BoxTreeError> {
        Self::with_base(root, css, Path::new("."))
    }

    /// As [`BoxTree::new`], resolving relative `url()`s against `base`. Only files under it
    /// load: a `url()` with a scheme, an absolute path or `..` draws nothing, as does one that
    /// a symlink leads out of the base. A `url()`'s `%` escapes decode, and its query and
    /// fragment are dropped. A missing or undecodable file draws nothing too, and the box shows
    /// its background color.
    ///
    /// # Errors
    /// Returns a stylesheet, capture, cascade or unsupported-display failure.
    pub fn with_base<Message, Theme, Renderer>(
        root: CapturedElement<'_, Message, Theme, Renderer>,
        css: &str,
        base: &Path,
    ) -> Result<Self, BoxTreeError> {
        let (_, construction) = root.into_parts();
        let sheets = vec![css.to_owned()];
        let source = Arc::new(Source {
            reach: Source::reach_of(&sheets),
            capture: NEXT_CAPTURE.fetch_add(1, Ordering::Relaxed),
            construction,
            sheets,
            base: base.to_owned(),
            media: MediaEnvironment::default(),
            variants: Mutex::new(Variants::default()),
            parsed: OnceLock::new(),
            refused: Mutex::new(BTreeSet::new()),
            layout_refusal: Mutex::new(None),
        });
        Self::unstated(&source)
    }

    /// The same captured tree under other author sheets, later ones over earlier ones. Use
    /// it to reload edited styles: the capture stays, and a rejected sheet leaves this tree
    /// as it was. Widget state that survives a view rebuild keeps its hover, focus and
    /// scroll offsets.
    ///
    /// # Errors
    /// Returns a stylesheet, cascade or unsupported-display failure.
    pub fn restyled(&self, sheets: &[&str]) -> Result<Self, BoxTreeError> {
        let sheets: Vec<String> = sheets.iter().map(|sheet| (*sheet).to_owned()).collect();
        let source = Arc::new(Source {
            reach: Source::reach_of(&sheets),
            capture: self.source.capture,
            construction: self.source.construction.clone(),
            sheets,
            base: self.source.base.clone(),
            media: self.source.media,
            variants: Mutex::new(Variants::default()),
            parsed: OnceLock::new(),
            refused: Mutex::new(BTreeSet::new()),
            layout_refusal: Mutex::new(None),
        });
        Self::unstated(&source)
    }

    /// The same captured tree and sheets, with their media queries answered against
    /// `media`: `@media` and `@import` conditions on the viewport's size and the color
    /// scheme (Media Queries Level 3). A tree starts with no viewport, where no size
    /// feature matches. Nothing follows the widget's own size: give the viewport again
    /// when it changes, for example from the window's resize events. A tree whose
    /// environment is already `media` is returned as it is.
    ///
    /// # Errors
    /// Returns a stylesheet, cascade or unsupported-display failure.
    pub fn with_media(&self, media: MediaEnvironment) -> Result<Self, BoxTreeError> {
        if self.source.media == media {
            return Ok(self.clone());
        }
        let source = Arc::new(Source {
            reach: Source::reach_of(&self.source.sheets),
            capture: self.source.capture,
            construction: self.source.construction.clone(),
            sheets: self.source.sheets.clone(),
            base: self.source.base.clone(),
            media,
            variants: Mutex::new(Variants::default()),
            parsed: OnceLock::new(),
            refused: Mutex::new(BTreeSet::new()),
            layout_refusal: Mutex::new(None),
        });
        Self::unstated(&source)
    }

    /// What this tree's media queries are answered against.
    #[must_use]
    pub fn media(&self) -> MediaEnvironment {
        self.source.media
    }

    /// The styling with no interaction, kept so that leaving it and coming back, or
    /// comparing a first hover against it, does not cascade it again.
    fn unstated(source: &Arc<Source>) -> Result<Self, BoxTreeError> {
        let key = (None, None, None);
        let built = Self::styled(source, key)?;
        source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, Arc::new(built.style()));
        Ok(built)
    }

    /// This tree restyled for an interaction state, cascading it the first time. A
    /// refused restyle keeps this styling: hovering must not take the view away.
    fn variant(&self, key: InteractionKey) -> Self {
        let mut tree = self.unpresented_variant(key);
        tree.presentation.clone_from(&self.presentation);
        tree.present();
        tree
    }

    fn unpresented_variant(&self, key: InteractionKey) -> Self {
        match self.cached_style(key) {
            Some(style) => Self::from_style((*style).clone(), Arc::clone(&self.source)),
            // This tree may carry a frame's presented values; those are not a style to keep.
            None => self.clone(),
        }
    }

    fn is_refused(&self, key: InteractionKey) -> bool {
        self.source
            .refused
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&key)
    }

    /// Remembers a refused restyle, and says why once: it is not asked again.
    fn refuse(&self, key: InteractionKey, error: &BoxTreeError) {
        log::warn!(target: "iced_css::box_tree", "restyle for {key:?} refused: {error}");
        let mut refused = self
            .source
            .refused
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if refused.len() >= VARIANT_LIMIT {
            refused.clear();
        }
        refused.insert(key);
    }

    /// The cascaded styling for a key, without copying it; `None` if it is refused.
    fn cached_style(&self, key: InteractionKey) -> Option<Arc<Style>> {
        let key = self.source.normal_key(key);
        let cached = self
            .source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key);
        if cached.is_some() {
            return cached;
        }
        if self.is_refused(key) {
            return None;
        }
        let built = match Self::styled(&self.source, key) {
            Ok(built) => built,
            Err(error) => {
                self.refuse(key, &error);
                return None;
            }
        };
        let style = Arc::new(built.style());
        self.source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, Arc::clone(&style));
        Some(style)
    }

    /// Whether two interaction states style this tree alike, as `variant` would show them.
    /// Presented values lie over both the same way. It copies neither styling: pointer
    /// moves ask this on every change of box.
    fn same_variant(&self, first: InteractionKey, second: InteractionKey) -> bool {
        let normal = |key| self.source.normal_key(key);
        if normal(first) == normal(second) {
            return true;
        }
        // The second is the one shown, by the callers: asked for first, it is the latest
        // used when the other needs room, and so is never the one dropped for it.
        let other = self.cached_style(second);
        match (self.cached_style(first), other) {
            (Some(one), Some(other)) => {
                Arc::ptr_eq(&one, &other)
                    || (one.prepared == other.prepared
                        && one.paints == other.paints
                        && one.texts == other.texts
                        && one.markers == other.markers
                        && one.backgrounds == other.backgrounds
                        && one.marker_images == other.marker_images
                        && one.gradients == other.gradients
                        && one.transforms == other.transforms
                        // What `!important` wins decides which presented values lie over it.
                        && one.important == other.important)
            }
            (None, None) => true,
            _ => false,
        }
    }

    fn style(&self) -> Style {
        Style {
            prepared: self.prepared.clone(),
            paints: self.paints.clone(),
            texts: self.texts.clone(),
            ids: self.ids.clone(),
            originals: self.originals.clone(),
            markers: self.markers.clone(),
            backgrounds: self.backgrounds.clone(),
            important: self.important.clone(),
            em_lengths: self.em_lengths.clone(),
            weights: self.weights.clone(),
            marker_images: self.marker_images.clone(),
            transitions: self.transitions.clone(),
            animations: self.animations.clone(),
            key: self.key,
            gradients: self.gradients.clone(),
            transforms: self.transforms.clone(),
        }
    }

    fn from_style(style: Style, source: Arc<Source>) -> Self {
        Self {
            prepared: style.prepared,
            paints: style.paints,
            texts: style.texts,
            ids: style.ids,
            originals: style.originals,
            markers: style.markers,
            backgrounds: style.backgrounds,
            source,
            important: style.important,
            em_lengths: style.em_lengths,
            weights: style.weights,
            marker_images: style.marker_images,
            presentation: None,
            transitions: style.transitions,
            animations: style.animations,
            key: style.key,
            gradients: style.gradients,
            transforms: style.transforms,
        }
    }

    /// The slot a widget for each CSS id occupies, in this styling: the leaf box itself,
    /// or the anonymous block of generated text it replaces when the box has only that.
    /// Display none has none. The first box that can hold it wins.
    fn leaf_slots(&self, ids: &[String]) -> Vec<Option<usize>> {
        let prepared = &self.prepared;
        let mut found: BTreeMap<&str, usize> = BTreeMap::new();
        for (arena, value) in self.ids.iter().enumerate() {
            let Some(id) = value.as_deref() else {
                continue;
            };
            if prepared.anonymous[arena] || found.contains_key(id) {
                continue;
            }
            let children = &prepared.boxes[arena].children;
            let slot = match children.first() {
                None => arena,
                Some(first) if children.iter().all(|child| prepared.anonymous[*child]) => *first,
                Some(_) => continue,
            };
            found.insert(id, slot);
        }
        ids.iter()
            .map(|id| found.get(id.as_str()).copied())
            .collect()
    }

    fn styled(source: &Arc<Source>, key: InteractionKey) -> Result<Self, BoxTreeError> {
        Self::styled_with(source, key, &[])
    }

    /// The `@keyframes` rule `name` names in this tree's sheets, as its keyframes: their
    /// places, the longhands each declares, and the easing each sets for what follows it.
    pub(crate) fn keyframe_rule(&self, name: &str) -> Option<Vec<KeyframeSummary>> {
        let Parsed(_, graphs) = self.source.parsed.get()?;
        let rules = keyframe_rules(graphs);
        let rule = rules.get(name)?;
        Some(
            rule.frames
                .iter()
                .map(|frame| {
                    let mut longhands = Vec::new();
                    let mut easing = None;
                    for parsed in &frame.declarations {
                        if parsed.property.descriptor().name == "animation-timing-function" {
                            if let iced_css_core::values::property::SpecifiedValue::Transition(
                                iced_css_core::values::property::TransitionValue::Easings(list),
                            ) = &parsed.value
                            {
                                easing = list.first().map(|easing| {
                                    crate::evaluation::tree_layout::css_easing(
                                        easing,
                                        colors().numeric,
                                    )
                                });
                            }
                            continue;
                        }
                        for longhand in parsed.property.longhands() {
                            if !longhands.contains(longhand) {
                                longhands.push(*longhand);
                            }
                        }
                    }
                    KeyframeSummary {
                        offsets: frame.offsets.clone(),
                        longhands,
                        easing,
                    }
                })
                .collect(),
        )
    }

    /// This tree's styling with a keyframe layer over it; see [`Self::styled_with`].
    pub(crate) fn keyframe_layer(&self, layer: &[(usize, String, usize)]) -> Option<Self> {
        Self::styled_with(&self.source, self.key, layer).ok()
    }

    /// This styling with keyframes laid over it: each (source node, `@keyframes` name,
    /// keyframe rule index) puts that rule's declarations over the node's own winners, as
    /// an animation at that keyframe does (CSS Animations 1).
    fn styled_with(
        source: &Arc<Source>,
        key: InteractionKey,
        layer: &[(usize, String, usize)],
    ) -> Result<Self, BoxTreeError> {
        let construction = source.construction.clone();

        let count = construction.nodes().len();
        // The selector source extends hover and active to ancestors (5.11.3).
        let mut states = vec![ConstructionState::default(); count];
        // A key from before a rebuild may name a node this tree does not have.
        if let Some(state) = key.0.and_then(|node| states.get_mut(node)) {
            state.hovered = true;
        }
        if let Some(state) = key.1.and_then(|node| states.get_mut(node)) {
            state.active = true;
        }
        if let Some(state) = key.2.and_then(|node| states.get_mut(node)) {
            state.focused = true;
        }
        let mut host = InitialSourceHost::new(
            NonZeroU64::new(COUNT as u64).expect("positive"),
            ProgressLimits {
                addresses: NonZeroU64::new(COUNT as u64).expect("positive"),
                bindings: big(),
                tracks: big(),
                retained_bytes: nz(BYTES),
                work: nz(WORK),
            },
        );
        host.prepare(
            construction,
            states,
            None,
            PublicationLimits {
                nodes: big(),
                snapshot_bytes: nz(BYTES),
                work: nz(WORK),
                selectors: selectors(),
                identities: SourceChangeLimits {
                    entries: big(),
                    retained_bytes: nz(BYTES),
                    work: nz(WORK),
                },
            },
        )
        .map_err(failed)?
        .commit();
        let snapshot = host
            .snapshot()
            .ok_or_else(|| BoxTreeError("snapshot".to_owned()))?;
        let projection =
            prepare_tree_source_projection(&host, snapshot, nz(WORK)).map_err(failed)?;
        // Parsed once per source: every interaction state cascades the same rules.
        let Parsed(catalog, graphs) = if let Some(parsed) = source.parsed.get() {
            parsed
        } else {
            let catalog = UaDisplayCatalog::new(UaDisplayLimits {
                rules: big(),
                retained_bytes: nz(BYTES),
                work: nz(WORK),
                values: colors(),
            })
            .map_err(failed)?;
            let graphs = source
                .sheets
                .iter()
                .zip(0_u64..)
                .map(|(css, order)| with_media_environment(source.media, || parse(css, order)))
                .collect::<Result<Vec<_>, _>>()?;
            source.parsed.get_or_init(|| Parsed(catalog, graphs))
        };
        let ids: Vec<SheetId> = (1..=graphs.len() as u64)
            .map(|id| SheetId::new(NonZeroU64::new(id).expect("positive")))
            .collect();
        let sheets = AuthorSheets::new(
            ids.iter()
                .map(|id| AuthorSheet {
                    id: *id,
                    enabled: true,
                    membership: Membership::Persistent,
                })
                .collect(),
            vec![],
            true,
            None,
            // This bound is itself charged per membership query, so keep it small,
            // but large enough for the pairwise duplicate-id check of `n` sheets.
            SheetLimits {
                entries: big(),
                retained_bytes: nz(BYTES),
                work: nz(100usize
                    .saturating_add(10usize.saturating_mul(ids.len()))
                    .saturating_add(ids.len().saturating_mul(ids.len().saturating_add(1)) / 2)),
            },
        )
        .map_err(failed)?;
        let bindings: Vec<GraphBinding<'_>> = graphs
            .iter()
            .zip(&ids)
            .map(|(graph, id)| GraphBinding {
                graph,
                origin: GraphOrigin::Author(*id),
            })
            .collect();
        let bundle = DisplayRuleBundle::new(
            catalog,
            &bindings,
            &sheets,
            BundleLimits {
                graphs: big(),
                rules: big(),
                retained_bytes: nz(BYTES),
                work: nz(WORK),
                membership_work: nz(WORK),
            },
        )
        .map_err(failed)?;
        // CSS Custom Properties: each element's custom properties, and what its values
        // holding `var()` parse to with them, before the passes that read the winners.
        let source_media = source.media;
        let variables = if bundle.uses_variables() {
            let source = snapshot.selector_source(selectors()).map_err(failed)?;
            let count = snapshot.construction().nodes().len();
            let mut per_target = per_target();
            per_target.per_rule.matching.retained_bytes = per_target
                .per_rule
                .matching
                .retained_bytes
                .min(nz(isize::MAX as usize));
            // Every parent follows its children in the source's order.
            Some(
                // Values substituted here parse at the same viewport as the sheets.
                with_media_environment(source_media, || {
                    resolve_variables(&bundle, &source, (0..count).rev(), per_target, colors())
                })
                .map_err(failed)?,
            )
        } else {
            None
        };
        let bundle = match &variables {
            Some(variables) => bundle.with_variables(variables),
            None => bundle,
        };
        // A keyframe layer: of two `@keyframes` with one name, the later wins.
        let mut animated: std::collections::HashMap<
            usize,
            Vec<&iced_css_core::values::property::ParsedProperty>,
        > = std::collections::HashMap::new();
        if !layer.is_empty() {
            let rules = keyframe_rules(graphs);
            for (node, name, frame) in layer {
                if let Some(frame) = rules
                    .get(name.as_str())
                    .and_then(|rule| rule.frames.get(*frame))
                {
                    animated
                        .entry(*node)
                        .or_default()
                        .extend(frame.declarations.iter().filter(|parsed| {
                            parsed.property.descriptor().name != "animation-timing-function"
                        }));
                }
            }
        }
        let bundle = if animated.is_empty() {
            bundle
        } else {
            bundle.with_animated(&animated)
        };
        let display = evaluate_source_display(
            snapshot,
            &bundle,
            SourceDisplayLimits {
                nodes: big(),
                retained_bytes: nz(BYTES),
                work: nz(WORST_CASE),
                // This pass visits each box once and the later one three times: stop here, at
                // the same size, before a cascade that could never finish starts at all.
                rule_visits: nz(RULE_VISITS / 3),
                matching_work: nz(WORST_CASE),
                declaration_offers: nz(WORST_CASE),
                membership_work: nz(WORST_CASE),
                selectors: selectors(),
                per_target: per_target(),
            },
        )
        .map_err(failed)?;
        // Font sizes come from the rows; generated content needs them and shapes the tree.
        let sized =
            prepare_captured_tree(&projection, &display, facts(), Some(16.0), |_| None, &[])
                .map_err(failed)?;
        let TreePaint {
            mut paints,
            styles,
            generated,
            markers,
            spacings,
            backgrounds,
            collapses,
            vertical_aligns,
            captions_below,
            important,
            em_lengths,
            marker_images,
            outside_items,
            grids,
            grid_items,
            transitions,
            animations,
            gradients,
            transforms,
        } = evaluate_tree_paint(&projection, &display, facts(), &sized.font_sizes, 16.0)
            .map_err(failed)?;
        let weights: Vec<u16> = styles.iter().map(|style| style.weight).collect();
        let mut prepared = prepare_captured_tree(
            &projection,
            &display,
            facts(),
            Some(16.0),
            |_| None,
            &generated,
        )
        .map_err(failed)?;
        // 17.2.1: an anonymous table inherits from its parent, the parent of the element it
        // wraps, so it takes `border-collapse` and `border-spacing` from there, not from the
        // cell. That parent may be an inline element, which has no box here.
        let inherits: Vec<usize> = prepared
            .positions
            .iter()
            .enumerate()
            .map(|(arena, &(owner, _))| {
                let anonymous_table = prepared.anonymous[arena]
                    && matches!(prepared.boxes[arena].table, Some(TableRole::Table { .. }));
                anonymous_table
                    .then(|| {
                        let original = projection.original_index(owner)?;
                        let parent = source.construction.nodes()[original].parent()?;
                        projection.projected_index(parent)
                    })
                    .flatten()
                    .unwrap_or(owner)
            })
            .collect();
        let mut collapse_spacing: Vec<Option<[f32; 2]>> = vec![None; prepared.boxes.len()];
        // 17.6.2: collapsed borders share one line: the cells on each inner grid line overlap
        // by the widest border on that line (`collapsed_gaps`), and the table's edge is the
        // outer cells'. The widest on each axis is only the fallback spacing;
        // `resolve_collapsed_edges` picks what each shared edge draws.
        for table in 0..prepared.boxes.len() {
            if !matches!(prepared.boxes[table].table, Some(TableRole::Table { .. }))
                || !collapses[inherits[table]]
            {
                continue;
            }
            resolve_collapsed_edges(&mut prepared, table, &mut paints);
            // By cell, for drawing to find a box's own by search.
            prepared.border_cuts.sort_unstable_by_key(|cut| cut.0);
            prepared.thin_sides.sort_unstable_by_key(|thin| thin.0);
            prepared.collapsed_losers.sort_unstable();
            // 17.6.2: a collapsed table has no padding.
            prepared.boxes[table].padding = [Dim::Px(0.0); 4];
            let (mut across, mut down) = (0.0_f32, 0.0_f32);
            let mut stack = prepared.boxes[table].children.clone();
            while let Some(part) = stack.pop() {
                match prepared.boxes[part].table {
                    Some(TableRole::Cell) => {
                        let edges = prepared.boxes[part].border;
                        across = across.max(edges[1]).max(edges[3]);
                        down = down.max(edges[0]).max(edges[2]);
                    }
                    Some(TableRole::Table { .. }) | None => {}
                    Some(_) => stack.extend(prepared.boxes[part].children.iter().copied()),
                }
            }
            if across > 0.0 || down > 0.0 {
                // Each inner grid line overlaps its neighbours by its own widest border, in
                // the table's logical order: in rtl a cell's start is its right side.
                use iced_css_core::formatting::block_flow::{table_grid, table_parts};
                let (rows, _) = table_parts(&prepared.boxes, table);
                let (placed, columns) = table_grid(&prepared.boxes, table, &rows);
                let (start, end) = if prepared.boxes[table].direction == Direction::Rtl {
                    (1, 3)
                } else {
                    (3, 1)
                };
                let mut lines = [
                    vec![0.0_f32; columns.saturating_sub(1)],
                    vec![0.0_f32; rows.len().saturating_sub(1)],
                ];
                for item in &placed {
                    let edges = prepared.boxes[item.cell].border;
                    let after = item.column + item.col_span;
                    let below = item.row + item.row_span;
                    for (axis, line, width) in [
                        (0, item.column.checked_sub(1), edges[start]),
                        (0, (after < columns).then(|| after - 1), edges[end]),
                        (1, item.row.checked_sub(1), edges[0]),
                        (1, (below < rows.len()).then(|| below - 1), edges[2]),
                    ] {
                        if let Some(line) = line {
                            lines[axis][line] = lines[axis][line].max(width);
                        }
                    }
                }
                prepared.boxes[table].collapsed_gaps =
                    Some(lines.map(|line| line.into_iter().map(|width| -width).collect()));
                prepared.boxes[table].border = [0.0; 4];
                // An anonymous table's owner is a cell, whose paint is its own.
                if !prepared.anonymous[table] {
                    paints[prepared.positions[table].0].border = [None; 4];
                }
                collapse_spacing[table] = Some([-across, -down]);
            }
        }
        // Each slot's enclosing box owner: anonymous blocks answer to their block container.
        let mut container_of: Vec<usize> =
            prepared.positions.iter().map(|&(owner, _)| owner).collect();
        for (arena, value) in prepared.boxes.iter().enumerate() {
            for &child in &value.children {
                // Through an anonymous parent, its own block container: parents come first.
                if prepared.anonymous[child] {
                    container_of[child] = if prepared.anonymous[arena] {
                        container_of[arena]
                    } else {
                        prepared.positions[arena].0
                    };
                }
            }
        }
        for (arena, value) in prepared.boxes.iter_mut().enumerate() {
            let owner = prepared.positions[arena].0;
            if let Some(TableRole::Table { spacing }) = &mut value.table {
                *spacing = collapse_spacing[arena].unwrap_or(spacings[inherits[arena]]);
            }
            // An anonymous cell has initial values, not those of the box it wraps.
            value.cell_align = match vertical_aligns[owner].filter(|_| !prepared.anonymous[arena]) {
                Some("middle") => RowAlign::Center,
                Some("bottom") => RowAlign::Right,
                _ => RowAlign::Left,
            };
            value.line_align = match vertical_aligns[owner].filter(|_| !prepared.anonymous[arena]) {
                Some("top" | "text-top") => RowAlign::Left,
                Some("middle") => RowAlign::Center,
                _ => RowAlign::Right,
            };
            // 10.8.1: on the line's baseline unless aligned to its top, middle or bottom.
            value.line_baseline = !matches!(
                vertical_aligns[owner].filter(|_| !prepared.anonymous[arena]),
                Some("top" | "text-top" | "middle" | "bottom" | "text-bottom")
            );
            value.clips = prepared.clipped[arena] || prepared.scrolls[arena];
            value.caption_below = captions_below[owner];
            value.fixed_table = paints[owner].fixed_table && !prepared.anonymous[arena];
            // An anonymous box has no sizes of its own to read as its border box's.
            value.border_box = paints[owner].border_box && !prepared.anonymous[arena];
            // CSS Flexbox 1: the element's own box is the container, and the item of its
            // parent; an anonymous box around its text is a plain item.
            if !prepared.anonymous[arena] {
                value.flex = paints[owner].flex;
                value.flex_item = paints[owner].flex_item;
                // CSS Grid 1, likewise.
                value.grid = grids[owner].clone();
                value.grid_item = grid_items[owner].clone();
                if value.flex.is_some() || value.grid.is_some() {
                    value.establishes_bfc = true;
                }
            }
            if value.inline_row.is_some() {
                // 16.2: a line's alignment comes from its block container.
                value.inline_row = Some(match styles[container_of[arena]].align {
                    "center" => RowAlign::Center,
                    "right" => RowAlign::Right,
                    _ => RowAlign::Left,
                });
                // 16.6: and so does whether its boxes may wrap.
                value.row_no_wrap =
                    matches!(styles[container_of[arena]].white_space, "pre" | "nowrap");
            }
        }
        let nodes = snapshot.construction().nodes();
        let raw_paints = paints.clone();
        let generated_of = |run: RunSource| -> Option<&GeneratedRun> {
            match run {
                RunSource::Node(_) => None,
                RunSource::Before(owner) => generated[owner][0].as_ref(),
                RunSource::After(owner) => generated[owner][1].as_ref(),
            }
        };
        let build_leaf = |arena: usize| -> Option<TextLeaf> {
            let runs = &prepared.runs[arena];
            // Block-level generated content in a box of its own: the box paints the
            // background, so its text paints none, its own or an enclosing inline's.
            let boxed = prepared
                .generated_boxes
                .binary_search_by_key(&arena, |entry| entry.0)
                .is_ok();
            let first = *runs.first()?;
            let style_of = |run: RunSource| -> &TextStyle {
                match (run, generated_of(run)) {
                    (_, Some(made)) => &made.style,
                    (RunSource::Node(at) | RunSource::Before(at) | RunSource::After(at), None) => {
                        &styles[at]
                    }
                }
            };
            let contents: Vec<Option<&str>> = runs
                .iter()
                .map(|&run| {
                    match (run, generated_of(run)) {
                        // `content:""` with sides holds a place for them, as an empty
                        // inline element does (10.8).
                        (_, Some(made))
                            if made.text.is_empty()
                                && !made.block
                                && made.frame.as_ref().is_some_and(|frame| {
                                    let sides = frame_sides(frame);
                                    [
                                        sides.margin,
                                        sides.border,
                                        sides.padding,
                                        sides.percent_margin,
                                        sides.percent_padding,
                                    ]
                                    .iter()
                                    .flatten()
                                    .any(|side| *side > 0.0)
                                }) =>
                        {
                            None
                        }
                        (_, Some(made)) => Some(made.text.as_str()),
                        (RunSource::Node(at), None) => projection
                            .original_index(at)
                            .and_then(|original| nodes[original].text()),
                        _ => Some(""),
                    }
                })
                .collect();
            // A word runs on through the runs of its leaf, so `capitalize` finds its
            // words in the joined text (CSS Text 2.1).
            let starts = if runs
                .iter()
                .any(|&run| style_of(run).text_transform == "capitalize")
            {
                word_starts(&contents.iter().copied().flatten().collect::<String>())
            } else {
                Vec::new()
            };
            let mut base = 0;
            let mut texts: Vec<(String, &'static str)> = runs
                .iter()
                .zip(&contents)
                .map(|(&run, &content)| {
                    let style = style_of(run);
                    // An element's own run is an empty inline's placeholder: zero width.
                    content.map_or(("\u{200B}".to_owned(), PLACEHOLDER), |content| {
                        let text = transformed(content, style.text_transform, &starts, base);
                        base += content.len();
                        (text, style.white_space)
                    })
                })
                .collect();
            collapse(&mut texts);
            let decorated_by = |from: Option<usize>| {
                from.map(|from| {
                    (
                        projection.original_index(from).unwrap_or_default(),
                        raw_paints[from].color.unwrap_or([0.0; 3]),
                        prepared.font_sizes[from],
                    )
                })
            };
            let runs = runs
                .iter()
                .zip(texts)
                .map(|(&run, (text, _))| {
                    let style = style_of(run);
                    let mut background_from = None;
                    let mut backgrounds = Vec::new();
                    // Every inline background up from `at`; for generated text, only boxes
                    // of their own (an inline element's slot is its paragraph's).
                    let chain = |start: usize, generated: bool| {
                        let mut found = Vec::new();
                        let mut owner = Some(start);
                        while let Some(here) = owner {
                            let own_box = !generated
                                || prepared.slot[here].is_some_and(|slot| prepared.anonymous[slot]);
                            if own_box
                                && let (Some(color), Some(node)) =
                                    (raw_paints[here].background, projection.original_index(here))
                            {
                                found.push((color, node, raw_paints[here].visible));
                            }
                            owner = prepared.inline_parent[here];
                        }
                        found
                    };
                    let (size, color, background, visible) = match (run, generated_of(run)) {
                        (run_source, Some(made)) => {
                            // Generated text keeps an enclosing inline's background.
                            let (RunSource::Node(at)
                            | RunSource::Before(at)
                            | RunSource::After(at)) = run_source;
                            let mut owner = (!boxed).then_some(at);
                            let mut background = made.background.filter(|_| made.visible && !boxed);
                            if !boxed {
                                let side = usize::from(matches!(run_source, RunSource::After(_)));
                                backgrounds.extend(
                                    made.background.map(|color| {
                                        (color, generated_owner(at, side), made.visible)
                                    }),
                                );
                                backgrounds.extend(chain(at, true));
                            }
                            while background.is_none() {
                                let Some(here) = owner else { break };
                                if prepared.slot[here].is_some_and(|slot| prepared.anonymous[slot])
                                {
                                    background = raw_paints[here]
                                        .background
                                        .filter(|_| raw_paints[here].visible);
                                    if background.is_some() {
                                        background_from = projection.original_index(here);
                                    }
                                }
                                owner = prepared.inline_parent[here];
                            }
                            (made.size, made.color, background, made.visible)
                        }
                        (
                            RunSource::Node(at) | RunSource::Before(at) | RunSource::After(at),
                            None,
                        ) => {
                            // An inline background paints behind its own and its children's
                            // text.
                            backgrounds = chain(at, false);
                            let mut owner = Some(at);
                            let mut background = None;
                            while let Some(here) = owner {
                                background = raw_paints[here]
                                    .background
                                    .filter(|_| raw_paints[here].visible);
                                if background.is_some() {
                                    background_from = projection.original_index(here);
                                    break;
                                }
                                owner = prepared.inline_parent[here];
                            }
                            (
                                prepared.font_sizes[at],
                                raw_paints[at].color,
                                background,
                                raw_paints[at].visible,
                            )
                        }
                    };
                    TextRun {
                        text,
                        size,
                        line_size: None,
                        spacer: false,
                        edge_style: "solid",
                        inline_boxes: Vec::new(),
                        edge: None,
                        color,
                        alpha: match run {
                            RunSource::Node(at) | RunSource::Before(at) | RunSource::After(at) => {
                                raw_paints[at].opacity.unwrap_or(1.0)
                            }
                        },
                        background,
                        background_from,
                        backgrounds,
                        percent_room: None,
                        side_of: None,
                        fades: Vec::new(),
                        edge_alpha: 1.0,
                        overhang: 0.0,
                        pull: 0.0,
                        pulled: 0.0,
                        generated: match run {
                            RunSource::Node(_) => None,
                            RunSource::Before(at) => Some(generated_owner(at, 0)),
                            RunSource::After(at) => Some(generated_owner(at, 1)),
                        },
                        marker_image: None,
                        weight: style.weight,
                        italic: style.italic,
                        family: style.family.clone(),
                        underline: style.underline,
                        line_through: style.line_through,
                        underline_from: decorated_by(style.underline_from),
                        line_through_from: decorated_by(style.line_through_from),
                        overline: style.overline,
                        overline_from: decorated_by(style.overline_from),
                        visible,
                        owner: {
                            let (RunSource::Node(at)
                            | RunSource::Before(at)
                            | RunSource::After(at)) = run;
                            projection.original_index(at).unwrap_or_default()
                        },
                    }
                })
                .collect();
            let lead = style_of(first);
            // 16.6: a line may break wherever any of its runs lets it, so only text that
            // is `pre` or `nowrap` throughout stays on its line.
            let wraps = prepared.runs[arena]
                .iter()
                .any(|&run| !matches!(style_of(run).white_space, "pre" | "nowrap"));
            // 10.8.1: one line height per leaf, so where its runs differ, the strut of
            // the block container, not the height of the inline its first run sits in.
            // Generated content in a box of its own is that container.
            let line_height = if boxed
                || prepared.runs[arena]
                    .iter()
                    .all(|&run| style_of(run).line_height == lead.line_height)
            {
                lead.line_height
            } else {
                styles[container_of[arena]].line_height
            };
            // CSS Text 3 and Overflow 3: as the block container of the lines says.
            let block = if prepared.anonymous[arena] && !boxed {
                &styles[container_of[arena]]
            } else {
                lead
            };
            let breaks = match (block.word_break, block.overflow_wrap) {
                ("break-all", _) => TextBreaks::Everywhere,
                ("break-word", _) | (_, "anywhere") => TextBreaks::Anywhere,
                (_, "break-word") => TextBreaks::LongWords,
                _ => TextBreaks::Words,
            };
            let clips = |projected: usize| {
                prepared
                    .slot
                    .get(projected)
                    .copied()
                    .flatten()
                    .is_some_and(|slot| prepared.clipped[slot] || prepared.scrolls[slot])
            };
            let container = if prepared.anonymous[arena] && !boxed {
                container_of[arena]
            } else {
                prepared.positions[arena].0
            };
            let ellipsis = styles[container].ellipsis && clips(container);
            Some(TextLeaf {
                runs,
                // 16.2: alignment belongs to the block container of the lines; generated
                // content in a box of its own is that container.
                align: if prepared.anonymous[arena] && !boxed {
                    styles[container_of[arena]].align
                } else {
                    lead.align
                },
                wraps,
                breaks,
                ellipsis,
                line_height,
                percent_indent: None,
                hang: 0.0,
            })
        };
        let texts: Vec<Option<TextLeaf>> = (0..prepared.boxes.len()).map(build_leaf).collect();
        // A block's own paint already drew its box: a block text leaf's run has no
        // highlight. An inline element has no box paint at all (9.2.1.1). An anonymous
        // block made for an element's generated content leaves that element's paint alone.
        let mut texts = texts;
        for (arena, &(projected, _)) in prepared.positions.iter().enumerate() {
            if prepared.anonymous[arena] {
                if prepared.slot[projected] == Some(arena) {
                    paints[projected].under_text = paints[projected].background;
                    paints[projected].under_text_border = paints[projected].border;
                    paints[projected].background = None;
                    paints[projected].border = [None; 4];
                }
            } else if let Some(leaf) = &mut texts[arena] {
                for (run, source) in leaf.runs.iter_mut().zip(&prepared.runs[arena]) {
                    if matches!(source, RunSource::Node(_)) {
                        run.background = None;
                        run.background_from = None;
                    }
                }
            }
        }
        // 15.6: small capitals are simulated by scaling capital letters. 8.6: an inline box's
        // sides open before its first run and close after its last, whatever lines it breaks
        // across; spaces take the room across, as for `text-indent`.
        // 10.8: an inline `:before` or `:after` is a box inside its element, numbered past
        // the projected indices: `count + 2 * element + side`.
        let count = prepared.inline_boxes.len();
        let pseudo_box = |source: RunSource| match source {
            // A block one's entry has no sides, so its chain is its element's.
            RunSource::Before(at) | RunSource::After(at) => generated_of(source)
                .map(|_| count + 2 * at + usize::from(matches!(source, RunSource::After(_)))),
            RunSource::Node(_) => None,
        };
        let element_of = |id: usize| if id >= count { (id - count) / 2 } else { id };
        let mut box_sides = prepared.inline_boxes.clone();
        let mut box_parent = prepared.inline_parent.clone();
        let mut box_paints = raw_paints.clone();
        for (at, pair) in generated.iter().enumerate().take(count) {
            for made in pair {
                let frame = made
                    .as_ref()
                    .filter(|made| !made.block)
                    .and_then(|made| made.frame.as_ref());
                box_sides.push(frame.map(frame_sides));
                box_parent.push(prepared.inline_boxes[at].is_some().then_some(at));
                box_paints.push(frame.map_or_else(BoxPaint::default, |frame| frame.paint));
            }
        }
        // The inline boxes around a box, outermost first.
        let inline_chain = |mut at: Option<usize>| {
            let mut found = Vec::new();
            while let Some(here) = at {
                let sided = box_sides[here].is_some_and(|sides| {
                    [
                        sides.margin,
                        sides.border,
                        sides.padding,
                        sides.percent_margin,
                        sides.percent_padding,
                    ]
                    .iter()
                    .flatten()
                    .any(|side| *side != 0.0)
                });
                if sided {
                    found.push(here);
                }
                at = box_parent[here];
            }
            found.reverse();
            found
        };
        let mut last_leaf: BTreeMap<usize, usize> = BTreeMap::new();
        for (arena, runs) in prepared.runs.iter().enumerate() {
            // A leaf of collapsed white space draws nothing, so it cannot close a box.
            if texts[arena].as_ref().is_none_or(TextLeaf::is_empty) {
                continue;
            }
            for &source in runs {
                let (RunSource::Node(at) | RunSource::Before(at) | RunSource::After(at)) = source;
                for element in inline_chain(Some(pseudo_box(source).unwrap_or(at))) {
                    last_leaf.insert(element, arena);
                }
            }
        }
        let mut opened = BTreeSet::new();
        let inside_image = |at: usize| {
            marker_images[at]
                .as_deref()
                .and_then(|uri| local_image(&source.base, uri))
        };
        for (arena, runs) in prepared.runs.iter().enumerate() {
            let Some(leaf) = &mut texts[arena] else {
                continue;
            };
            let boxed = !leaf.is_empty();
            // Each run, its element, and the innermost box it sits in.
            let mut tagged: Vec<(TextRun, usize, usize)> = Vec::with_capacity(leaf.runs.len());
            for (mut run, &source) in leaf.runs.drain(..).zip(runs) {
                let (RunSource::Node(at) | RunSource::Before(at) | RunSource::After(at)) = source;
                let style = match generated_of(source) {
                    Some(made) => &made.style,
                    None => &styles[at],
                };
                let small_caps = style.small_caps;
                let start = pseudo_box(source).unwrap_or(at);
                // Whether the run is in a `:before` or `:after` box with sides of its own.
                let in_box = start != at && inline_chain(Some(start)).last() == Some(&start);
                // 12.5.1: an inside marker takes a run of its own, the marker's text, in its
                // list item's font, color and visibility rather than the `:before`'s; with an
                // image or a `:before` box, outside that box.
                let marker = match source {
                    RunSource::Before(_) => generated_of(source)
                        .and_then(|made| made.marker)
                        .filter(|length| run.text.is_char_boundary(*length)),
                    _ => None,
                };
                let image = marker.and_then(|_| inside_image(at));
                let mut pieces = Vec::with_capacity(2);
                match marker {
                    Some(length) => {
                        let rest = run.text.split_off(length);
                        let item = &styles[at];
                        let decorated_by = |from: Option<usize>| {
                            from.map(|from| {
                                (
                                    projection.original_index(from).unwrap_or_default(),
                                    raw_paints[from].color.unwrap_or([0.0; 3]),
                                    prepared.font_sizes[from],
                                )
                            })
                        };
                        pieces.push((
                            TextRun {
                                size: prepared.font_sizes[at],
                                color: raw_paints[at].color,
                                alpha: raw_paints[at].opacity.unwrap_or(1.0),
                                visible: raw_paints[at].visible,
                                weight: item.weight,
                                italic: item.italic,
                                family: item.family.clone(),
                                underline: item.underline,
                                line_through: item.line_through,
                                overline: item.overline,
                                underline_from: decorated_by(item.underline_from),
                                line_through_from: decorated_by(item.line_through_from),
                                overline_from: decorated_by(item.overline_from),
                                marker_image: image.map(|handle| MarkerImage {
                                    handle,
                                    fallback: None,
                                    shown: None,
                                }),
                                // Outside the box, the marker shows none of its background.
                                background: if in_box { None } else { run.background },
                                background_from: if in_box { None } else { run.background_from },
                                backgrounds: if in_box {
                                    Vec::new()
                                } else {
                                    run.backgrounds.clone()
                                },
                                ..run.clone()
                            },
                            at,
                        ));
                        // An empty `content` keeps a place for its box's sides.
                        if !rest.is_empty() || in_box {
                            let text = if rest.is_empty() {
                                "\u{200B}".to_owned()
                            } else {
                                rest
                            };
                            pieces.push((TextRun { text, ..run }, start));
                        }
                    }
                    None => pieces.push((run, start)),
                }
                for (mut run, start) in pieces {
                    // 16.4: more room at each space, by the same stand-in spaces as an indent.
                    if style.word_spacing > 0.0
                        && let Some(extra) = indent_run(&run, style.word_spacing)
                    {
                        // Joined so no line breaks inside them: before a space they stay at
                        // the end of a line it breaks, and after a no-break space no line
                        // breaks after them either.
                        let joined = extra
                            .text
                            .chars()
                            .map(String::from)
                            .collect::<Vec<_>>()
                            .join("\u{2060}");
                        let wide = format!("{joined}\u{2060} ");
                        let wide_fixed = format!("\u{a0}\u{2060}{joined}\u{2060}");
                        run.text = run.text.replace(' ', &wide).replace('\u{a0}', &wide_fixed);
                    }
                    // Less by drawing each space smaller.
                    let parts = if style.word_spacing < 0.0 && run.marker_image.is_none() {
                        narrow_spaces(&run, style.word_spacing)
                    } else {
                        vec![run]
                    };
                    for run in parts {
                        let spaces = run.text.chars().all(|c| matches!(c, ' ' | '\u{a0}'));
                        if small_caps && run.marker_image.is_none() && !spaces {
                            tagged.extend(
                                small_capitals(&run)
                                    .into_iter()
                                    .map(|part| (part, at, start)),
                            );
                        } else {
                            tagged.push((run, at, start));
                        }
                    }
                }
            }
            // 9.10: `unicode-bidi` is exactly these control characters around the element's
            // text, opened and closed within each paragraph.
            let bidi_chain = |start: usize| {
                let mut found = Vec::new();
                let mut at = Some(start);
                while let Some(here) = at {
                    let inline = prepared.inline_boxes[here].is_some();
                    let style = projection
                        .original_index(here)
                        .and_then(|original| display.bidi_style(original));
                    if let Some(style) = style {
                        let rtl = style.direction() == ComputedDirection::RightToLeft;
                        match style.unicode_bidi() {
                            ComputedUnicodeBidi::Embed if inline => {
                                found.push((here, if rtl { '\u{202B}' } else { '\u{202A}' }));
                            }
                            ComputedUnicodeBidi::BidiOverride => {
                                found.push((here, if rtl { '\u{202E}' } else { '\u{202D}' }));
                            }
                            _ => {}
                        }
                    }
                    at = prepared.inline_parent[here];
                }
                found.reverse();
                found
            };
            // A block's override reaches its inline content, not only its generated text.
            let container = if prepared.anonymous[arena] {
                container_of[arena]
            } else {
                prepared.positions[arena].0
            };
            let block_override = projection
                .original_index(container)
                .and_then(|original| display.bidi_style(original))
                .filter(|style| style.unicode_bidi() == ComputedUnicodeBidi::BidiOverride)
                .map(|style| {
                    let rtl = style.direction() == ComputedDirection::RightToLeft;
                    (container, if rtl { '\u{202E}' } else { '\u{202D}' })
                });
            let bidi_chains: Vec<Vec<(usize, char)>> = tagged
                .iter()
                .map(|(run, at, _)| {
                    // Controls around nothing would make blank text a line.
                    if run.text.is_empty() {
                        return Vec::new();
                    }
                    let mut chain = bidi_chain(*at);
                    if let Some(entry) = block_override
                        && !chain.contains(&entry)
                    {
                        chain.insert(0, entry);
                    }
                    chain
                })
                .collect();
            for (index, (run, _, _)) in tagged.iter_mut().enumerate() {
                let here = &bidi_chains[index];
                let before = index
                    .checked_sub(1)
                    .map_or(&[][..], |at| bidi_chains[at].as_slice());
                let after = bidi_chains.get(index + 1).map_or(&[][..], Vec::as_slice);
                let opening: String = here
                    .iter()
                    .filter(|entry| !before.contains(entry))
                    .map(|entry| entry.1)
                    .collect();
                let closing = here.iter().filter(|entry| !after.contains(entry)).count();
                // A forced break ends a paragraph and every embedding with it (9.10).
                if !here.is_empty() && run.text.contains('\n') {
                    let reopen: String = here.iter().map(|entry| entry.1).collect();
                    let close = "\u{202C}".repeat(here.len());
                    run.text = run.text.replace('\n', &format!("{close}\n{reopen}"));
                }
                if !opening.is_empty() || closing > 0 {
                    run.text = format!("{opening}{}{}", run.text, "\u{202C}".repeat(closing));
                }
            }
            let chains: Vec<Vec<usize>> = tagged
                .iter()
                .map(|(_, _, start)| inline_chain(Some(*start)))
                .collect();
            let mut placed = Vec::with_capacity(tagged.len());
            for (index, (mut run, _, _)) in tagged.into_iter().enumerate() {
                let here = &chains[index];
                if !boxed {
                    placed.push(run);
                    continue;
                }
                let sides_of = |element: usize| {
                    box_sides[element].unwrap_or(InlineBoxSides {
                        margin: [0.0; 4],
                        border: [0.0; 4],
                        padding: [0.0; 4],
                        percent_margin: [0.0; 4],
                        percent_padding: [0.0; 4],
                    })
                };
                // 11.2: a hidden box paints nothing but keeps its room: its sides are kept,
                // marked hidden.
                let decorations = |elements: &[usize]| -> Vec<InlineDecoration> {
                    elements
                        .iter()
                        .map(|&element| {
                            let sides = sides_of(element);
                            let colors = box_paints[element].border;
                            InlineDecoration {
                                padding: [sides.padding[0], sides.padding[2]],
                                padding_share: [
                                    (sides.padding[0], sides.percent_padding[0]),
                                    (sides.padding[2], sides.percent_padding[2]),
                                ],
                                border: [
                                    (sides.border[0], colors[0]),
                                    (sides.border[2], colors[2]),
                                ],
                                border_style: [
                                    box_paints[element].border_style[0],
                                    box_paints[element].border_style[2],
                                ],
                                background: box_paints[element].background,
                                // A generated box answers to no source node: no element's
                                // animation or outline is its.
                                owner: if element < count {
                                    projection.original_index(element).unwrap_or_default()
                                } else {
                                    usize::MAX - (element - count)
                                },
                                visible: box_paints[element].visible,
                                alpha: 1.0,
                                border_alpha: [1.0; 2],
                            }
                        })
                        .collect()
                };
                // A margin lies in the boxes around this one, and border and padding in this
                // one as well, as one space holding an exact border bar.
                // The background an inline shows from around it, and whose it is.
                let around = |mut at: Option<usize>| {
                    while let Some(here) = at {
                        if let Some(color) = box_paints[here]
                            .background
                            .filter(|_| box_paints[here].visible)
                        {
                            let from = (here < count)
                                .then(|| projection.original_index(here))
                                .flatten();
                            return (Some(color), from);
                        }
                        at = box_parent[here];
                    }
                    (None, None)
                };
                // A box's key in decorations and backgrounds.
                let box_key = |here: usize| {
                    if here < count {
                        projection.original_index(here).unwrap_or_default()
                    } else {
                        usize::MAX - (here - count)
                    }
                };
                // Every background up from `at`, nearest first, as `TextRun::backgrounds`.
                let chain_around = |mut at: Option<usize>| {
                    let mut found = Vec::new();
                    while let Some(here) = at {
                        if let Some(color) = box_paints[here].background {
                            let from = if here < count {
                                projection.original_index(here).unwrap_or_default()
                            } else {
                                usize::MAX - (here - count)
                            };
                            found.push((color, from, box_paints[here].visible));
                        }
                        at = box_parent[here];
                    }
                    found
                };
                let side = |element: usize, across: usize, run: &TextRun, depth: usize| {
                    let sides = sides_of(element);
                    let owner_of = |at: Option<usize>| {
                        at.and_then(|at| projection.original_index(element_of(at)))
                            .unwrap_or(run.owner)
                    };

                    // A spacer whose room has a percentage is sized at layout; it starts
                    // with its fixed part, or none.
                    let spacer = |pixels: f32| {
                        indent_run(run, pixels).or_else(|| {
                            indent_run(run, run.size.max(1.0)).map(|spacer| TextRun {
                                text: String::new(),
                                ..spacer
                            })
                        })
                    };
                    let percent_margin = sides.percent_margin[across];
                    let margin = (sides.margin[across] > 0.0 || percent_margin > 0.0)
                        .then(|| spacer(sides.margin[across]))
                        .flatten()
                        .map(|spacer| TextRun {
                            // Sized again at layout: for a share of the width, or a font size
                            // an animation moves.
                            percent_room: Some((sides.margin[across], percent_margin, false)),
                            side_of: Some((box_key(element), across)),
                            background: around(box_parent[element]).0,
                            background_from: around(box_parent[element]).1,
                            backgrounds: chain_around(box_parent[element]),
                            inline_boxes: decorations(&here[..depth]),
                            // Nothing hidden answers input: the nearest box around that shows.
                            owner: owner_of(
                                std::iter::successors(box_parent[element], |at| box_parent[*at])
                                    .find(|at| box_paints[*at].visible)
                                    .or(Some(container_of[arena])),
                            ),
                            ..spacer
                        });
                    let bar = sides.border[across];
                    let room = bar + sides.padding[across];
                    let visible = box_paints[element].visible;
                    let edge = box_paints[element].border[across]
                        .filter(|_| bar > 0.0)
                        .map(|color| (across == 3, bar, color));
                    // Up to the next space that fits: a thin border must keep its bar.
                    let step = run.size.max(1.0) / 6.0;
                    let percent_padding = sides.percent_padding[across];
                    // A negative margin pulls the box over what is beside it: its border and
                    // padding's spaces narrow by it and the box draws past them; what is
                    // left narrows the space beside it (`pull_aside`).
                    // Beside a percentage padding, whose room is known only at layout, it is
                    // taken again there, out of that room alone; until then, and where no
                    // width applies, out of its fixed part.
                    let pulled = (-sides.margin[across]).max(0.0);
                    let later = if percent_padding > 0.0 { pulled } else { 0.0 };
                    let overhang = pulled.min(room);
                    let kept = room - overhang;
                    let pull = if later > 0.0 { 0.0 } else { pulled - overhang };
                    let inner = (room > 0.0 || percent_padding > 0.0 || pull > 0.0)
                        .then(|| {
                            if kept > 0.0 || percent_padding > 0.0 {
                                spacer((kept / step).ceil() * step * 1.0001)
                            } else {
                                // Nothing of its own left: a place to draw its box from.
                                indent_run(run, run.size.max(1.0)).map(|spacer| TextRun {
                                    text: "\u{200B}".to_owned(),
                                    ..spacer
                                })
                            }
                        })
                        .flatten()
                        .map(|spacer| TextRun {
                            percent_room: Some((
                                if later > 0.0 { room } else { kept },
                                percent_padding,
                                true,
                            )),
                            overhang,
                            pull,
                            pulled: later,
                            side_of: Some((box_key(element), across)),
                            // Its own background comes with its box, over its padding too.
                            background: if box_paints[element].background.is_some() && visible {
                                None
                            } else {
                                around(box_parent[element]).0
                            },
                            background_from: if box_paints[element].background.is_some() && visible
                            {
                                None
                            } else {
                                around(box_parent[element]).1
                            },
                            // Its padding shows its own box's background, or those around it.
                            backgrounds: chain_around(Some(element)),
                            inline_boxes: decorations(&here[..=depth]),
                            edge,
                            edge_style: box_paints[element].border_style[across],
                            visible,
                            owner: owner_of(Some(element)),
                            ..spacer
                        });
                    [margin, inner]
                };
                let before = index
                    .checked_sub(1)
                    .map_or(&[][..], |at| chains[at].as_slice());
                for (depth, &element) in here.iter().enumerate() {
                    // 8.6: only the element's first box, wherever blocks cut it, opens it.
                    if !before.contains(&element) && opened.insert(element) {
                        placed.extend(side(element, 3, &run, depth).into_iter().flatten());
                    }
                }
                let after = chains.get(index + 1).map_or(&[][..], Vec::as_slice);
                let mut closing = Vec::new();
                for (depth, &element) in here.iter().enumerate().rev() {
                    if !after.contains(&element) && last_leaf.get(&element) == Some(&arena) {
                        let mut parts = side(element, 1, &run, depth);
                        parts.reverse();
                        closing.extend(parts.into_iter().flatten());
                    }
                }
                run.inline_boxes = decorations(here);
                placed.push(run);
                placed.extend(closing);
            }
            pull_aside(&mut placed);
            leaf.runs = placed;
        }
        // 16.1: `text-indent` applies to the first line of a block container, so to the leaf
        // that holds it: spaces of a known fraction of the font size come closest.
        let mut parent_of = vec![None; prepared.boxes.len()];
        for (arena, value) in prepared.boxes.iter().enumerate() {
            for &child in &value.children {
                parent_of[child] = Some(arena);
            }
        }
        // An anonymous box must come first in its parent.
        let leads = |prepared: &PreparedTree, at: usize| {
            !prepared.anonymous[at]
                || parent_of[at].is_none_or(|parent: usize| {
                    prepared.boxes[parent].children.iter().find(|child| {
                        let child = &prepared.boxes[**child];
                        child.float.is_none() && !child.out_of_flow
                    }) == Some(&at)
                })
        };
        // A line of inline-blocks indents its first line itself (`first_indent`), by a
        // percentage once its width is known; its text pieces take none.
        for (arena, &owner) in container_of.iter().enumerate() {
            if prepared.boxes[arena].inline_row.is_none() || !leads(&prepared, arena) {
                continue;
            }
            let percent = styles[owner].text_indent_percent;
            if percent == 0.0 {
                prepared.boxes[arena].first_indent = styles[owner].text_indent;
            } else {
                prepared.row_indents.push((arena, percent));
            }
        }
        for arena in 0..prepared.boxes.len() {
            let owner = if prepared.anonymous[arena] {
                container_of[arena]
            } else {
                prepared.positions[arena].0
            };
            // Generated content in a box of its own indents by its own `text-indent`.
            let made = prepared
                .generated_boxes
                .binary_search_by_key(&arena, |entry| entry.0)
                .ok()
                .and_then(|_| prepared.runs[arena].first())
                .and_then(|&run| generated_of(run));
            let (indent, percent) = made.map_or(
                (styles[owner].text_indent, styles[owner].text_indent_percent),
                |made| (made.style.text_indent, made.style.text_indent_percent),
            );
            let Some(leaf) = texts[arena]
                .as_mut()
                .filter(|leaf| (indent != 0.0 || percent != 0.0) && !leaf.is_empty())
            else {
                continue;
            };
            // A piece of a line of inline-blocks takes none: its line indents itself. An
            // inline-block with its own text has its own first line.
            let in_row = prepared.anonymous[arena]
                && parent_of[arena]
                    .is_some_and(|parent| prepared.boxes[parent].inline_row.is_some());
            if (made.is_none() && !leads(&prepared, arena)) || in_row {
                continue;
            }
            if indent < 0.0 || percent < 0.0 {
                // A negative indent hangs the first line out past the start edge.
                if percent < 0.0 {
                    let fallback = leaf.runs.first().map_or(0, |run| run.owner);
                    leaf.percent_indent = Some(PercentIndent {
                        percent,
                        owner: projection.original_index(owner).unwrap_or(fallback),
                        inserted: false,
                    });
                } else {
                    leaf.hang = -indent;
                }
                continue;
            }
            if percent > 0.0 {
                // Its width is known only at layout: the widget sizes it there.
                let fallback = leaf.runs.first().map_or(0, |run| run.owner);
                leaf.percent_indent = Some(PercentIndent {
                    percent,
                    owner: projection.original_index(owner).unwrap_or(fallback),
                    inserted: false,
                });
                continue;
            }
            let lead = leaf.runs.iter().find(|run| !run.text.is_empty());
            if let Some(mut spacer) = lead.and_then(|first| indent_run(first, indent)) {
                // Blank space of the block, not of whatever the first word sits in.
                spacer.owner = projection.original_index(owner).unwrap_or(spacer.owner);
                // Its room, for a font size an animation moves.
                spacer.percent_room = Some((indent, 0.0, false));
                leaf.runs.insert(0, spacer);
            }
        }
        // 9.10: a paragraph's base direction is its block's, and the shaper would guess it from
        // the first strong character: a mark at the start settles it. Left-to-right text needs
        // one only if it holds right-to-left characters to mislead the guess.
        for (arena, value) in prepared.boxes.iter().enumerate() {
            let Some(leaf) = texts[arena].as_mut().filter(|leaf| !leaf.is_empty()) else {
                continue;
            };
            let rtl = value.direction == Direction::Rtl;
            let mark = match (rtl, leaf.runs.iter().any(|run| holds_rtl(&run.text))) {
                (true, _) => '\u{200F}',
                (false, true) => '\u{200E}',
                (false, false) => continue,
            };
            // An inside marker's image comes first, however empty its run is now.
            if let Some(first) = leaf
                .runs
                .iter_mut()
                .find(|run| !run.text.is_empty() || run.marker_image.is_some())
            {
                first.text.insert(0, mark);
            }
            // Each line after a forced break is a paragraph of its own.
            for run in &mut leaf.runs {
                if run.text.contains('\n') {
                    run.text = run.text.replace('\n', &format!("\n{mark}"));
                }
            }
            // After a final break there is no line for a mark to open.
            if let Some(last) = leaf.runs.iter_mut().rev().find(|run| !run.text.is_empty())
                && last.text.ends_with(&format!("\n{mark}"))
            {
                last.text.pop();
            }
        }
        let marker_leaves = markers
            .into_iter()
            .enumerate()
            .filter_map(|(projected, text)| {
                let text = text?;
                prepared.slot[projected]?;
                let style = &styles[projected];
                Some((
                    projected,
                    TextLeaf {
                        runs: vec![TextRun {
                            text,
                            size: prepared.font_sizes[projected],
                            line_size: None,
                            spacer: false,
                            edge_style: "solid",
                            inline_boxes: Vec::new(),
                            edge: None,
                            color: raw_paints[projected].color,
                            alpha: raw_paints[projected].opacity.unwrap_or(1.0),
                            background: None,
                            background_from: None,
                            percent_room: None,
                            side_of: None,
                            fades: Vec::new(),
                            edge_alpha: 1.0,
                            overhang: 0.0,
                            pull: 0.0,
                            pulled: 0.0,
                            generated: None,
                            marker_image: None,
                            backgrounds: Vec::new(),
                            weight: style.weight,
                            italic: style.italic,
                            family: style.family.clone(),
                            underline: false,
                            line_through: false,
                            underline_from: None,
                            line_through_from: None,
                            overline: false,
                            overline_from: None,
                            visible: raw_paints[projected].visible,
                            owner: projection.original_index(projected).unwrap_or_default(),
                        }],
                        align: "right",
                        wraps: false,
                        breaks: TextBreaks::Words,
                        ellipsis: false,
                        line_height: style.line_height,
                        percent_indent: None,
                        hang: 0.0,
                    },
                ))
            })
            .collect();
        let marker_image_handles = marker_images
            .into_iter()
            .enumerate()
            .filter_map(|(projected, uri)| {
                // Only outside markers have a place of their own for an image.
                outside_items[projected].then_some(())?;
                Some((projected, local_image(&source.base, &uri?)?))
            })
            .collect();
        let background_images = backgrounds
            .into_iter()
            .enumerate()
            .filter_map(|(projected, value)| {
                let value = value?;
                prepared.slot[projected].filter(|slot| !prepared.anonymous[*slot])?;
                let handle = local_image(&source.base, &value.uri)?;
                Some((projected, (value, handle)))
            })
            .collect();
        let originals: Vec<usize> = (0..count)
            .map(|projected| projection.original_index(projected).unwrap_or_default())
            .collect();
        // 17.6.1.1: `empty-cells: hide` draws no background or border on a cell with no
        // content but collapsed white space and out-of-flow boxes. It has no effect in a table
        // with collapsed borders, whose edges are shared.
        // 17.6.3: with collapsed borders, inset draws like ridge and outset like groove.
        for (arena, value) in prepared.boxes.iter().enumerate() {
            let owner = prepared.positions[arena].0;
            // The table decides, an anonymous one by what it inherits.
            let mut table = parent_of[arena];
            while let Some(at) = table {
                if matches!(prepared.boxes[at].table, Some(TableRole::Table { .. })) {
                    break;
                }
                table = parent_of[at];
            }
            let collapsed = table.is_some_and(|at| collapses[inherits[at]]);
            if value.table == Some(TableRole::Cell) && !prepared.anonymous[arena] && collapsed {
                for line in &mut paints[owner].border_style {
                    *line = match *line {
                        "inset" => "ridge",
                        "outset" => "groove",
                        other => other,
                    };
                }
            }
        }
        let blank = |arena: usize| texts[arena].as_ref().is_none_or(TextLeaf::is_empty);
        for (arena, value) in prepared.boxes.iter().enumerate() {
            let owner = prepared.positions[arena].0;
            if value.table != Some(TableRole::Cell)
                || prepared.anonymous[arena]
                || !paints[owner].hide_empty
                || !blank(arena)
            {
                continue;
            }
            let empty = value.children.iter().all(|&child| {
                let inner = &prepared.boxes[child];
                inner.out_of_flow
                    || (prepared.anonymous[child]
                        && inner.children.is_empty()
                        && !prepared.runs[child].is_empty()
                        && blank(child))
            });
            let mut table = parent_of[arena];
            while let Some(at) = table {
                if matches!(prepared.boxes[at].table, Some(TableRole::Table { .. })) {
                    break;
                }
                table = parent_of[at];
            }
            let collapsed = table.is_some_and(|at| collapses[inherits[at]]);
            paints[owner].hidden_empty = empty && !collapsed;
        }
        // 17.6.1.1: a row whose cells, spanning ones included, all hide with nothing visible
        // takes no height, in the separated model.
        use iced_css_core::formatting::block_flow::{table_grid, table_parts};
        let mut hidden_rows = Vec::new();
        for (table, value) in prepared.boxes.iter().enumerate() {
            if !matches!(value.table, Some(TableRole::Table { .. })) || collapses[inherits[table]] {
                continue;
            }
            let (rows, _) = table_parts(&prepared.boxes, table);
            let (placed, _) = table_grid(&prepared.boxes, table, &rows);
            // How many cells cover each row, and how many of those show, counted by where
            // each span starts and ends rather than by scanning every cell per row.
            let mut changes = vec![(0_isize, 0_isize); rows.len() + 1];
            for item in &placed {
                let paint = paints[prepared.positions[item.cell].0];
                let hides = !prepared.anonymous[item.cell]
                    && (paint.hidden_empty || (paint.hide_empty && !paint.visible));
                let shows = isize::from(!hides);
                let end = item.row + item.row_span;
                changes[item.row].0 += 1;
                changes[item.row].1 += shows;
                changes[end].0 -= 1;
                changes[end].1 -= shows;
            }
            let (mut covering, mut showing) = (0, 0);
            for (index, &row) in rows.iter().enumerate() {
                covering += changes[index].0;
                showing += changes[index].1;
                if covering > 0 && showing == 0 {
                    hidden_rows.push(row);
                }
            }
        }
        for row in hidden_rows {
            prepared.boxes[row].hidden_row = true;
        }
        let ids = prepared
            .positions
            .iter()
            .map(|&(projected, _)| {
                nodes[originals[projected]]
                    .metadata()
                    .css_id()
                    .map(str::to_owned)
            })
            .collect();
        Ok(Self {
            prepared,
            paints,
            texts,
            ids,
            originals,
            markers: marker_leaves,
            backgrounds: background_images,
            source: Arc::clone(source),
            important,
            em_lengths,
            weights,
            marker_images: marker_image_handles,
            presentation: None,
            transitions,
            animations,
            key,
            gradients,
            transforms,
        })
    }

    /// Geometry and paint by projected (preorder) index; `None` is display none.
    ///
    /// # Errors
    /// Returns a layout failure at this size.
    pub fn layout(
        &self,
        width: f32,
        height: Option<f32>,
    ) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
        self.prepared.layout(
            FlowViewport {
                left: 0.0,
                top: 0.0,
                width,
                height,
            },
            FlowLimits {
                boxes: big(),
                depth: nz(2_000),
                work: nz(LAYOUT_WORK),
            },
        )
    }

    /// Lays out with `measure(leaf, width)` giving each text block's height.
    ///
    /// # Errors
    /// Returns a layout failure at this size.
    pub fn layout_text(
        &self,
        width: f32,
        height: Option<f32>,
        measure: &mut dyn FnMut(&TextLeaf, f32) -> f32,
    ) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
        self.prepared.layout_measured(
            FlowViewport {
                left: 0.0,
                top: 0.0,
                width,
                height,
            },
            FlowLimits {
                boxes: big(),
                depth: nz(2_000),
                work: nz(LAYOUT_WORK),
            },
            &mut |arena, used| {
                let leaf = self.texts[arena].as_ref()?;
                // White space alone generates no line box (16.6.1).
                if leaf.is_empty() {
                    return Some(0.0);
                }
                // Paint re-derives this width after f32 translation, so both round up
                // to the same whole pixel: a line that fit when measured still fits.
                Some(measure(leaf, used.ceil()))
            },
        )
    }

    /// Lays out as the widget does, with `renderer`'s text: each text block's height,
    /// shrink-to-fit widths and the baselines rows of inline-blocks line up on come from its
    /// paragraphs and their fonts, and text widens past floats. It matches a widget viewed
    /// with no hosted widgets, given the same `height` the widget is (its viewport's); a list
    /// marker image is not loaded, so the marker keeps its fallback text.
    ///
    /// # Errors
    /// Returns a layout failure at this size.
    pub fn layout_with<Renderer: text::Renderer<Font = iced::Font>>(
        &self,
        renderer: &Renderer,
        width: f32,
        height: Option<f32>,
    ) -> Result<Vec<Option<FlowGeometry>>, TreeLayoutError> {
        let mut tree = self.clone();
        tree.set_text_widths(|leaf| {
            let longest = paragraph(renderer, leaf, f32::INFINITY).min_bounds().width;
            (narrowest_width(renderer, leaf), longest)
        });
        let geometry = tree.layout_passes(renderer, width, height, &mut |_, _| None)?;
        Ok(tree.prepared.by_projected(&geometry))
    }

    /// Lays out by arena slot with `renderer`'s text, as `layout_with` and the widget do.
    /// `hosted(arena, width)` gives a hosted widget's height; other leaves are text. A
    /// percentage `text-indent` or inline side needs its block's width: a second layout
    /// gives its spacers room, when the first changed one.
    fn layout_passes<Renderer: text::Renderer<Font = iced::Font>>(
        &mut self,
        renderer: &Renderer,
        width: f32,
        height: Option<f32>,
        hosted: &mut dyn FnMut(usize, f32) -> Option<f32>,
    ) -> Result<Vec<FlowGeometry>, TreeLayoutError> {
        let mut geometry = Vec::new();
        for pass in 0..2 {
            let current = &*self;
            let mut baselines: Baselines = BTreeMap::new();
            geometry = current.layout_arena(
                width,
                height,
                &mut |arena, width| {
                    hosted(arena, width).or_else(|| {
                        current.texts[arena]
                            .as_ref()
                            .map(|leaf| leaf_height(renderer, leaf, width))
                    })
                },
                &mut |arena, narrow, until, wide| {
                    // A hanging first line stays beside the floats as it is.
                    let leaf = current.texts[arena]
                        .as_ref()
                        .filter(|leaf| leaf.hang <= 0.0)?;
                    let (_, second, first_height) = split_leaf(renderer, leaf, narrow, until)?;
                    let rest: Renderer::Paragraph = paragraph(renderer, &second, wide);
                    Some(first_height + rest.min_bounds().height)
                },
                // Once per leaf and width in a pass: rows ask again for nested boxes.
                &mut |arena, width, split| {
                    let key = split.map(|(until, wide)| (until.to_bits(), wide.to_bits()));
                    *baselines
                        .entry((arena, width.to_bits(), key))
                        .or_insert_with(|| {
                            let leaf = current.texts[arena].as_ref()?;
                            leaf_baseline(renderer, leaf, width, split)
                        })
                },
            )?;
            if pass == 1 {
                break;
            }
            let before = self.percent_leaves();
            if !self.apply_percent_indents(&geometry) {
                break;
            }
            // Their spaces are text a shrink-to-fit box fits.
            self.refit_texts(&before, |leaf| {
                let longest = paragraph(renderer, leaf, f32::INFINITY).min_bounds().width;
                (narrowest_width(renderer, leaf), longest)
            });
        }
        Ok(geometry)
    }

    /// Lays out by arena slot, with `measure(arena, width)` giving each leaf's height and
    /// `split` its height when it widens past a float.
    fn layout_arena(
        &self,
        width: f32,
        height: Option<f32>,
        measure: &mut dyn FnMut(usize, f32) -> Option<f32>,
        split: &mut dyn FnMut(usize, f32, f32, f32) -> Option<f32>,
        baseline: &mut dyn FnMut(usize, f32, Option<(f32, f32)>) -> Option<f32>,
    ) -> Result<Vec<FlowGeometry>, TreeLayoutError> {
        self.prepared.layout_slots_baselines(
            FlowViewport {
                left: 0.0,
                top: 0.0,
                width,
                height,
            },
            FlowLimits {
                boxes: big(),
                depth: nz(2_000),
                work: nz(LAYOUT_WORK),
            },
            &mut |arena, used| {
                if self.texts[arena].as_ref().is_some_and(TextLeaf::is_empty) {
                    return Some(0.0);
                }
                measure(arena, used.ceil())
            },
            &mut |arena, narrow, until, wide| split(arena, narrow.ceil(), until, wide.ceil()),
            &mut |arena, used, split| {
                baseline(
                    arena,
                    used.ceil(),
                    split.map(|(until, wide)| (until, wide.ceil())),
                )
            },
        )
    }

    /// Sizes each percentage `text-indent` and inline side for the width its leaf laid out
    /// in, `geometry` by arena slot. Returns whether any leaf changed, and so needs another
    /// layout.
    fn apply_percent_indents(&mut self, geometry: &[FlowGeometry]) -> bool {
        let mut changed = false;
        // A piece of a line of inline-blocks is sized before any of this: a share of its own
        // width would wrap it. Its percentage sides keep their fixed part only.
        let mut in_row = vec![false; self.prepared.boxes.len()];
        for value in &self.prepared.boxes {
            if value.inline_row.is_some() {
                for &child in &value.children {
                    in_row[child] = self.prepared.anonymous[child];
                }
            }
        }
        let mut parent_of = vec![None; self.prepared.boxes.len()];
        for (arena, value) in self.prepared.boxes.iter().enumerate() {
            for &child in &value.children {
                parent_of[child] = Some(arena);
            }
        }
        // A line of inline-blocks' first line, by its block's containing block's width.
        for &(row, percent) in &self.prepared.row_indents {
            let block = parent_of[row];
            let Some(basis) = block
                .and_then(|block| parent_of[block])
                .or(block)
                .and_then(|outer| geometry.get(outer))
                .map(|outer| (outer.content.right - outer.content.left).max(0.0))
            else {
                continue;
            };
            let indent = percent / 100.0 * basis;
            changed |= self.prepared.boxes[row].first_indent != indent;
            self.prepared.boxes[row].first_indent = indent;
        }
        for (arena, leaf) in self.texts.iter_mut().enumerate() {
            let Some(leaf) = leaf.as_mut() else { continue };
            let Some(place) = geometry.get(arena).map(|outer| outer.content) else {
                continue;
            };
            // 16.1: an indent's percentage is of its block's containing block's width: the
            // content box of the box it sits in, or its own at the root.
            let block = if self.prepared.anonymous[arena] {
                parent_of[arena]
            } else {
                Some(arena)
            };
            let basis = block
                .and_then(|block| parent_of[block])
                .and_then(|parent| geometry.get(parent))
                .map_or(place, |outer| outer.content);
            let basis = (basis.right - basis.left).max(0.0);
            // Inline sides with a percentage: their stand-in spaces, in place.
            let width = if in_row[arena] {
                0.0
            } else {
                (place.right - place.left).max(0.0)
            };
            for run in &mut leaf.runs {
                // Percentage paddings above and below, which take no room in the line.
                for decoration in &mut run.inline_boxes {
                    for (padding, (fixed, percent)) in
                        decoration.padding.iter_mut().zip(decoration.padding_share)
                    {
                        if percent != 0.0 {
                            *padding = fixed + percent / 100.0 * width;
                        }
                    }
                }
                let Some(text) = room_text(run, width) else {
                    continue;
                };
                if run.pulled > 0.0 {
                    run.overhang = percent_overhang(run, width);
                }
                if run.text != text {
                    run.text = text;
                    changed = true;
                }
            }
            // 9.10: the paragraph's direction mark leads its first text. Side spaces that
            // were empty until sized here came before it, or before the spaces holding it: it
            // moves onto them.
            if let Some((first, marked)) = stranded_mark(leaf) {
                let mark = leaf.runs[marked].text.remove(0);
                leaf.runs[first].text.insert(0, mark);
                changed = true;
            }
            let Some(mut indent) = leaf.percent_indent else {
                continue;
            };
            // A negative one hangs the first line by that share of the width.
            if indent.percent < 0.0 {
                let hang = -indent.percent / 100.0 * basis;
                changed |= leaf.hang != hang;
                leaf.hang = hang;
                continue;
            }
            let is_mark = |mark: &char| matches!(mark, '\u{200E}' | '\u{200F}');
            let old = indent.inserted.then(|| leaf.runs.remove(0));
            // The paragraph's direction mark goes back where `styled` put it...
            if let Some(mark) = old.as_ref().and_then(|run| run.text.chars().next())
                && is_mark(&mark)
                && let Some(first) = leaf
                    .runs
                    .iter_mut()
                    .find(|run| !run.text.is_empty() || run.marker_image.is_some())
            {
                first.text.insert(0, mark);
            }
            let width = indent.percent / 100.0 * basis;
            let lead = leaf.runs.iter().position(|run| !run.text.is_empty());
            let spacer = lead
                .and_then(|at| indent_run(&leaf.runs[at], width))
                .map(|mut spacer| {
                    spacer.owner = indent.owner;
                    // ...and moves to the spaces opening the paragraph, to settle its direction.
                    if let Some(at) = lead
                        && leaf.runs[at]
                            .text
                            .chars()
                            .next()
                            .is_some_and(|c| is_mark(&c))
                    {
                        let mark = leaf.runs[at].text.remove(0);
                        spacer.text.insert(0, mark);
                    }
                    spacer
                });
            changed |= old.as_ref().map(|run| &run.text) != spacer.as_ref().map(|run| &run.text);
            indent.inserted = spacer.is_some();
            if let Some(spacer) = spacer {
                leaf.runs.insert(0, spacer);
            }
            leaf.percent_indent = Some(indent);
        }
        changed
    }

    /// 17.6.1.1 counts a cell hosting a widget as empty, but the widget shows: the rows its
    /// cell covers keep their height.
    fn keep_rows_with_widgets(&mut self, slots: impl Iterator<Item = usize>) {
        use iced_css_core::formatting::block_flow::{table_grid, table_parts};
        let boxes = &self.prepared.boxes;
        if !boxes.iter().any(|value| value.hidden_row) {
            return;
        }
        let mut parent = vec![None; boxes.len()];
        for (at, value) in boxes.iter().enumerate() {
            for &child in &value.children {
                parent[child] = Some(at);
            }
        }
        let is_cell = |at: &usize| boxes[*at].table == Some(TableRole::Cell);
        let mut kept = Vec::new();
        for slot in slots {
            let Some(cell) = Some(slot).filter(is_cell).or(parent[slot].filter(is_cell)) else {
                continue;
            };
            let mut table = parent[cell];
            while let Some(at) = table {
                if matches!(boxes[at].table, Some(TableRole::Table { .. })) {
                    break;
                }
                table = parent[at];
            }
            let Some(table) = table else {
                continue;
            };
            let (rows, _) = table_parts(boxes, table);
            let (placed, _) = table_grid(boxes, table, &rows);
            if let Some(item) = placed.iter().find(|item| item.cell == cell) {
                kept.extend_from_slice(&rows[item.row..item.row + item.row_span]);
            }
        }
        for row in kept {
            self.prepared.boxes[row].hidden_row = false;
        }
    }

    /// Loads each inside marker's image with `load`, which gives its size, and puts spaces
    /// that wide, and half an em for the gap, in place of the marker's text. One that does
    /// not load keeps its text. A paragraph's direction marks around it stay, and in a
    /// right-to-left one its spaces open with a mark, which marks where their room ends.
    fn apply_marker_images(&mut self, mut load: impl FnMut(&image::Handle) -> Option<Size>) {
        let control = |mark: &char| {
            matches!(
                mark,
                '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
            )
        };
        for (arena, leaf) in self.texts.iter_mut().enumerate() {
            let Some(leaf) = leaf else { continue };
            // The block's direction: a percentage indent's spaces can come before the mark.
            let rtl = self.prepared.boxes[arena].direction == Direction::Rtl;
            for run in &mut leaf.runs {
                let Some(mut marker) = run.marker_image.take() else {
                    continue;
                };
                let fallback = marker
                    .fallback
                    .get_or_insert_with(|| run.text.clone())
                    .clone();
                marker.shown = load(&marker.handle);
                run.spacer = marker.shown.is_some();
                run.text = match marker.shown {
                    Some(size) => {
                        let mut lead: String = fallback.chars().take_while(control).collect();
                        let body = &fallback[lead.len()..];
                        // Its own, as a percentage indent takes the paragraph's mark.
                        if rtl {
                            lead.push('\u{200F}');
                        }
                        let trail: usize = body
                            .chars()
                            .rev()
                            .take_while(control)
                            .map(char::len_utf8)
                            .sum();
                        let spaces = indent_run(run, size.width + run.size * 0.5)
                            .map(|spacer| spacer.text)
                            .unwrap_or_default();
                        format!("{lead}{spaces}{}", &body[body.len() - trail..])
                    }
                    None => fallback,
                };
                run.marker_image = Some(marker);
            }
        }
    }

    /// Supplies each text leaf's (narrowest, longest) width for shrink-to-fit.
    pub fn set_text_widths(&mut self, mut widths: impl FnMut(&TextLeaf) -> (f32, f32)) {
        for (arena, leaf) in self.texts.iter().enumerate() {
            if let Some(leaf) = leaf {
                let (minimum, preferred) = widths(leaf);
                self.prepared.boxes[arena].content = Some(FlowContent {
                    height: 0.0,
                    preferred_width: preferred,
                    preferred_min_width: minimum,
                });
            }
        }
    }

    /// The leaves [`Self::apply_percent_indents`] can rewrite, by arena slot, as they are
    /// now: the others need no copy to tell that they stayed.
    fn percent_leaves(&self) -> Vec<(usize, TextLeaf)> {
        self.texts
            .iter()
            .enumerate()
            .filter_map(|(arena, leaf)| {
                let leaf = leaf.as_ref()?;
                let sized = leaf.percent_indent.is_some()
                    || stranded_mark(leaf).is_some()
                    || leaf.runs.iter().any(|run| {
                        run.percent_room.is_some()
                            || run.inline_boxes.iter().any(|decoration| {
                                decoration
                                    .padding_share
                                    .iter()
                                    .any(|(_, percent)| *percent != 0.0)
                            })
                    });
                sized.then(|| (arena, leaf.clone()))
            })
            .collect()
    }

    /// [`Self::set_text_widths`] again for the leaves whose text differs from `before`,
    /// [`Self::percent_leaves`] as it was.
    fn refit_texts(
        &mut self,
        before: &[(usize, TextLeaf)],
        mut widths: impl FnMut(&TextLeaf) -> (f32, f32),
    ) {
        for &(arena, ref old) in before {
            if let Some(leaf) = self.texts.get(arena).and_then(Option::as_ref)
                && leaf != old
            {
                let (minimum, preferred) = widths(leaf);
                self.prepared.boxes[arena].content = Some(FlowContent {
                    height: 0.0,
                    preferred_width: preferred,
                    preferred_min_width: minimum,
                });
            }
        }
    }

    /// Outside list markers, by projected index of their item.
    #[must_use]
    pub fn markers(&self) -> &BTreeMap<usize, TextLeaf> {
        &self.markers
    }

    /// The background images that can load, by projected index.
    #[must_use]
    pub fn background_images(&self) -> Vec<(usize, &BackgroundImage)> {
        self.backgrounds
            .iter()
            .map(|(projected, (value, _))| (*projected, value))
            .collect()
    }

    /// The inline content of each box, by projected index of its first run.
    #[must_use]
    pub fn text_leaves(&self) -> Vec<(usize, &TextLeaf)> {
        self.texts
            .iter()
            .enumerate()
            .filter_map(|(arena, leaf)| Some((self.prepared.positions[arena].0, leaf.as_ref()?)))
            .collect()
    }

    /// Paint by projected index.
    #[must_use]
    pub fn paints(&self) -> &[BoxPaint] {
        &self.paints
    }

    /// The list items that show an image for their marker, by projected index.
    #[must_use]
    pub fn list_images(&self) -> Vec<usize> {
        self.marker_images.keys().copied().collect()
    }

    /// Projected box indices in 9.9 paint order: flow blocks, then floats, then
    /// positioned boxes in tree order, each with the descendants that belong to them. It
    /// includes cells `empty-cells: hide` keeps from drawing; see `BoxPaint::hidden_empty`..
    #[must_use]
    pub fn paint_order(&self) -> Vec<usize> {
        self.paint_steps(&BTreeSet::new())
            .into_iter()
            .filter_map(|step| match step {
                PaintStep::Box(index) => Some(index),
                PaintStep::Text(_)
                | PaintStep::Outline(_)
                | PaintStep::Marker(_)
                | PaintStep::CellBorder(_)
                | PaintStep::Column(_)
                | PaintStep::ColumnGroup(_)
                | PaintStep::Generated(_) => None,
            })
            .collect()
    }

    fn paint_steps(&self, embedded: &BTreeSet<usize>) -> Vec<PaintStep> {
        let prepared = &self.prepared;
        let mut steps = Vec::with_capacity(prepared.boxes.len());
        if prepared.boxes.is_empty() {
            return steps;
        }
        self.stacking_context(0, embedded, &mut steps);

        // 17.6.2: a collapsed table's cells draw their borders after all its cell backgrounds,
        // wherever positioning put those: neighbours overlap along the shared edges.
        let mut tables: BTreeMap<usize, (usize, Vec<usize>)> = BTreeMap::new();
        for (at, step) in steps.iter().enumerate() {
            if let PaintStep::Box(index) = *step
                && let Some(table) = self.paints[index].collapsed_cell
            {
                tables.entry(table).or_default().1.push(index);
                tables.entry(table).or_default().0 = at;
            }
        }
        // After the last box anywhere in the table, whose content could cover a band too.
        for (table, group) in &mut tables {
            let mut inside = BTreeSet::new();
            let mut stack = vec![*table];
            while let Some(arena) = stack.pop() {
                inside.insert(prepared.positions[arena].0);
                // A positioned box paints in its own layer, above the grid: a popup in a cell
                // must cover the lines.
                stack.extend(
                    prepared.boxes[arena]
                        .children
                        .iter()
                        .copied()
                        // Cells, rows and groups stay the table's own, positioned or not; a
                        // positioned table or caption paints above the grid like any box.
                        .filter(|child| {
                            prepared.positions[*child].1 == Position::Static
                                || matches!(
                                    prepared.boxes[*child].table,
                                    Some(
                                        TableRole::Cell
                                            | TableRole::Row
                                            | TableRole::HeaderGroup
                                            | TableRole::BodyGroup
                                            | TableRole::FooterGroup
                                    )
                                )
                        }),
                );
            }
            for (at, step) in steps.iter().enumerate() {
                if let PaintStep::Box(index) = *step
                    && inside.contains(&index)
                {
                    group.0 = group.0.max(at);
                }
            }
        }
        let mut groups: Vec<_> = tables.into_values().collect();
        groups.sort_unstable_by_key(|group| std::cmp::Reverse(group.0));
        for (at, cells) in groups {
            steps.splice(at + 1..at + 1, cells.into_iter().map(PaintStep::CellBorder));
        }
        // 18.4: outlines may be drawn above everything else; an inline element's around
        // each of its line's pieces.
        for (projected, paint) in self.paints.iter().enumerate() {
            let boxed = prepared.slot[projected].is_some_and(|slot| !prepared.anonymous[slot]);
            let inline = prepared.inline_boxes[projected].is_some();
            if paint.visible && paint.outline.is_some() && (boxed || inline) {
                steps.push(PaintStep::Outline(projected));
            }
        }
        steps
    }

    /// Of `steps` from `paint_steps`, the ones that draw: a hidden empty cell keeps its
    /// place for input, and draws only around a hosted widget.
    fn drawn_steps(&self, mut steps: Vec<PaintStep>, embedded: &BTreeSet<usize>) -> Vec<PaintStep> {
        steps.retain(|step| {
            let PaintStep::Box(index) = *step else {
                return true;
            };
            !self.paints[index].hidden_empty
                || self.prepared.slot[index].is_some_and(|slot| embedded.contains(&slot))
        });
        steps
    }

    /// E.2: the root's box, negative z-index children, its flow and floats, then z-index
    /// auto and 0 in tree order, then positive. Auto boxes form no context of their own:
    /// their positioned descendants join this one.
    fn stacking_context(
        &self,
        root: usize,
        embedded: &BTreeSet<usize>,
        steps: &mut Vec<PaintStep>,
    ) {
        let prepared = &self.prepared;
        let mut positioned = BTreeSet::new();
        let mut own = Vec::new();
        self.paint_context(root, embedded, &mut positioned, &mut own);
        let mut layers = Vec::new();
        while let Some(arena) = positioned.pop_first() {
            let mut inner = Vec::new();
            let z_index = self.paints[prepared.positions[arena].0].z_index;
            match z_index {
                Some(_) => self.stacking_context(arena, embedded, &mut inner),
                None => self.paint_context(arena, embedded, &mut positioned, &mut inner),
            }
            layers.push((z_index.unwrap_or(0), arena, inner));
        }
        layers.sort_by_key(|layer| (layer.0, layer.1));
        let (below, above): (Vec<_>, Vec<_>) = layers.into_iter().partition(|layer| layer.0 < 0);
        let mut own = own.into_iter();
        steps.extend(own.next());
        steps.extend(below.into_iter().flat_map(|layer| layer.2));
        steps.extend(own);
        steps.extend(above.into_iter().flat_map(|layer| layer.2));
    }

    /// One 9.9 layer: this box and its in-flow blocks, then its floats, each as a unit,
    /// then the in-flow text. Positioned descendants go to the caller's pending set.
    fn paint_context(
        &self,
        root: usize,
        embedded: &BTreeSet<usize>,
        positioned: &mut BTreeSet<usize>,
        steps: &mut Vec<PaintStep>,
    ) {
        let prepared = &self.prepared;
        let mut floats = Vec::new();
        let mut texts = Vec::new();
        let mut marks = Vec::new();
        let mut stack = vec![root];
        while let Some(arena) = stack.pop() {
            if let Ok(at) = prepared
                .generated_boxes
                .binary_search_by_key(&arena, |entry| entry.0)
            {
                steps.push(PaintStep::Generated(at));
            }
            if !prepared.anonymous[arena] {
                let projected = prepared.positions[arena].0;
                steps.push(PaintStep::Box(projected));
                if self.markers.contains_key(&projected)
                    || self.marker_images.contains_key(&projected)
                {
                    marks.push(projected);
                }
            }
            if self.texts[arena].is_some() || embedded.contains(&arena) {
                texts.push(arena);
            }
            // 17.5.1: a table paints in layers, its row groups then its rows under every
            // cell, so a later row's background cannot cover a cell spanning into it.
            if matches!(prepared.boxes[arena].table, Some(TableRole::Table { .. })) {
                // Column groups, then columns, come between the table and its row groups.
                // A group's columns are recorded together, so its first is where it changes.
                let mut previous = None;
                for (at, &(table, _, group)) in prepared.table_columns.iter().enumerate() {
                    if table != arena {
                        continue;
                    }
                    if group.is_some() && group != previous {
                        steps.push(PaintStep::ColumnGroup(at));
                    }
                    previous = group;
                }
                for (at, &(table, ..)) in prepared.table_columns.iter().enumerate() {
                    if table == arena {
                        steps.push(PaintStep::Column(at));
                    }
                }
                let mut cells = Vec::new();
                let mut rows = Vec::new();
                for &child in &prepared.boxes[arena].children {
                    if prepared.positions[child].1 != Position::Static {
                        positioned.insert(child);
                        continue;
                    }
                    match prepared.boxes[child].table {
                        Some(
                            TableRole::HeaderGroup | TableRole::BodyGroup | TableRole::FooterGroup,
                        ) => {
                            if !prepared.anonymous[child] {
                                steps.push(PaintStep::Box(prepared.positions[child].0));
                            }
                            if self.texts[child].is_some() || embedded.contains(&child) {
                                texts.push(child);
                            }
                            for &row in &prepared.boxes[child].children {
                                if prepared.positions[row].1 != Position::Static {
                                    positioned.insert(row);
                                } else {
                                    rows.push(row);
                                }
                            }
                        }
                        Some(TableRole::Row) => rows.push(child),
                        // Captions are blocks beside the grid.
                        _ => cells.push(child),
                    }
                }
                for &row in &rows {
                    if !prepared.anonymous[row] {
                        steps.push(PaintStep::Box(prepared.positions[row].0));
                    }
                    if self.texts[row].is_some() || embedded.contains(&row) {
                        texts.push(row);
                    }
                    for &cell in &prepared.boxes[row].children {
                        if prepared.positions[cell].1 != Position::Static {
                            positioned.insert(cell);
                        } else {
                            cells.push(cell);
                        }
                    }
                }
                stack.extend(cells.into_iter().rev());
                continue;
            }
            for &child in prepared.boxes[arena].children.iter().rev() {
                if prepared.positions[child].1 != Position::Static {
                    positioned.insert(child);
                } else if prepared.boxes[child].float.is_some() {
                    floats.push(child);
                } else {
                    stack.push(child);
                }
            }
        }
        floats.sort_unstable();
        for float in floats {
            self.paint_context(float, embedded, positioned, steps);
        }
        steps.extend(texts.into_iter().map(PaintStep::Text));
        steps.extend(marks.into_iter().map(PaintStep::Marker));
    }

    /// Wraps the tree as an iced element that fills the size it is given.
    pub fn view<'a, Message: Clone + 'a, Theme: 'a, Renderer>(
        self,
    ) -> Element<'a, Message, Theme, Renderer>
    where
        Renderer: text::Renderer<Font = iced::Font> + BackgroundRenderer + 'a,
    {
        self.interactive().into()
    }

    /// Starts a view that can host native widgets and publish messages.
    #[must_use]
    pub fn interactive<'a, Message, Theme, Renderer>(
        self,
    ) -> BoxTreeView<'a, Message, Theme, Renderer> {
        BoxTreeView {
            tree: self,
            widgets: Vec::new(),
            presses: Vec::new(),
        }
    }

    /// Each arena slot's clip, the padding boxes of its clipping ancestors (11.1.1).
    /// Absolute boxes escape ancestors below their containing block; fixed boxes, all.
    fn clips(&self, geometry: &[Option<FlowGeometry>]) -> Vec<Option<FlowRect>> {
        let prepared = &self.prepared;
        let count = prepared.boxes.len();
        let mut parents = vec![None; count];
        for (arena, value) in prepared.boxes.iter().enumerate() {
            for &child in &value.children {
                parents[child] = Some(arena);
            }
        }
        let meet = |clip: Option<FlowRect>, more: FlowRect| Some(narrowed(clip, more));
        let mut clips: Vec<Option<FlowRect>> = vec![None; count];
        // `clip` alone, down through the box tree: a fixed box escapes `overflow`, not this.
        let mut cut: Vec<Option<FlowRect>> = vec![None; count];
        for arena in 0..count {
            let position = prepared.positions[arena].1;
            let owner = prepared.positions[arena].0;
            let own = geometry
                .get(owner)
                .copied()
                .flatten()
                .and_then(|outer| own_clip(self.paints[owner].clip, position, outer.border));
            let above = parents[arena].and_then(|parent: usize| cut[parent]);
            cut[arena] = own.map_or(above, |own| meet(above, own));
            if position == Position::Fixed {
                clips[arena] = cut[arena];
                continue;
            }
            let mut holder = parents[arena];
            if position == Position::Absolute {
                while let Some(at) = holder {
                    if prepared.positions[at].1 != Position::Static {
                        break;
                    }
                    holder = parents[at];
                }
            }
            if let Some(holder) = holder {
                clips[arena] = clips[holder];
                if prepared.clipped[holder]
                    && let Some(Some(outer)) = geometry.get(prepared.positions[holder].0)
                {
                    clips[arena] = meet(clips[arena], outer.padding);
                }
            }
            if let Some(own) = own {
                clips[arena] = meet(clips[arena], own);
            }
        }
        clips
    }
}

/// A tree view under construction: native widgets by CSS id, and press messages.
pub struct BoxTreeView<'a, Message, Theme, Renderer> {
    tree: BoxTree,
    widgets: Vec<(String, Element<'a, Message, Theme, Renderer>)>,
    presses: Vec<(String, Message)>,
}

impl<'a, Message, Theme, Renderer> BoxTreeView<'a, Message, Theme, Renderer> {
    /// Places a native widget in the leaf box with this CSS id. The box sizes as CSS
    /// says; a box with auto height takes the widget's height at the used width, except
    /// that a Fill-height widget fills a definite box and adds nothing to an auto one.
    /// The widget shows only while such a box exists, so a rule can hide or reveal it.
    /// A box holds one widget: another for the same id replaces it.
    #[must_use]
    pub fn widget(
        mut self,
        id: &str,
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let element = element.into();
        match self.widgets.iter_mut().find(|(held, _)| held == id) {
            Some(entry) => entry.1 = element,
            None => self.widgets.push((id.to_owned(), element)),
        }
        self
    }

    /// Publishes `message` when the primary button presses this box or a descendant
    /// not handling it first (the nearest such box wins).
    #[must_use]
    pub fn on_press(mut self, id: &str, message: Message) -> Self {
        self.presses.push((id.to_owned(), message));
        self
    }
}

impl<'a, Message, Theme, Renderer> From<BoxTreeView<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: text::Renderer<Font = iced::Font> + BackgroundRenderer + 'a,
{
    fn from(view: BoxTreeView<'a, Message, Theme, Renderer>) -> Self {
        let tree = view.tree;
        let (ids, elements): (Vec<_>, Vec<_>) = view.widgets.into_iter().unzip();
        Element::new(BoxTreeWidget {
            base: tree.clone(),
            tree,
            key: (None, None, None),
            presenting: false,
            measured: false,
            geometry: Vec::new(),
            slot_geometry: Vec::new(),
            column_rects: Vec::new(),
            scroll_limits: Vec::new(),
            canvas_height: 0.0,
            canvas_width: 0.0,
            clips: Vec::new(),
            steps: Vec::new(),
            drawn: Vec::new(),
            paragraphs: BTreeMap::new(),
            allocations: Vec::new(),
            loaded: BTreeSet::new(),
            loaded_markers: BTreeSet::new(),
            rest_paragraphs: BTreeMap::new(),
            marker_paragraphs: BTreeMap::new(),
            elements,
            ids,
            slots: Vec::new(),
            slot_owners: Vec::new(),
            presses: view.presses,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaintStep {
    /// A table column's background, by its place in `PreparedTree::table_columns`.
    Column(usize),
    /// A column group's background, by its first column's place in
    /// `PreparedTree::table_columns`.
    ColumnGroup(usize),
    /// Block-level generated content's box, by its place in `PreparedTree::generated_boxes`.
    Generated(usize),
    /// A collapsed table's cell, drawing only its borders.
    CellBorder(usize),
    /// A box's background and border, by projected index.
    Box(usize),
    /// A text leaf or hosted widget, by arena slot.
    Text(usize),
    /// A box's outline, by projected index.
    Outline(usize),
    /// A list item's outside marker, by projected index.
    Marker(usize),
}

#[derive(Debug, Clone, Default)]
struct Interaction {
    hovered: Option<usize>,
    active: Option<usize>,
    focused: Option<usize>,
    /// Vertical scroll offsets by source node. They outlive restyles and view rebuilds.
    scrolled: BTreeMap<usize, f32>,
    /// Horizontal scroll offsets by source node.
    scrolled_x: BTreeMap<usize, f32>,
    /// The source whose nodes the indices above name.
    source: Option<Arc<Source>>,
    /// Scrolling moved boxes under the pointer: the next redraw finds what it is over.
    rehover: bool,
    /// The interaction state last laid out, so a change of it can start transitions.
    seen: Option<InteractionKey>,
    /// The widget's own animations, once it has any: transitions its interaction restyles
    /// started (CSS Transitions 1), and the `@keyframes` animations its stylings name.
    transitions: Option<WidgetTransitions>,
}

/// The widget's own animations, shared by the clones of its state.
#[derive(Clone, Default)]
struct WidgetTransitions(Arc<Mutex<BoxTreeAnimations>>);

impl std::fmt::Debug for WidgetTransitions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WidgetTransitions")
    }
}

impl WidgetTransitions {
    fn lock(&self) -> std::sync::MutexGuard<'_, BoxTreeAnimations> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Interaction {
    /// The tree to draw for `key` over `base`, and whether it shows transitions. A
    /// change of interaction state starts the transitions its new styling asks for, from
    /// the styling it leaves (CSS Transitions 1).
    fn styled_for(&mut self, base: &BoxTree, key: InteractionKey, now: Instant) -> (BoxTree, bool) {
        let previous = self.seen.replace(key);
        if let Some(previous) = previous.filter(|previous| *previous != key) {
            let after = base.unpresented_variant(key);
            if after
                .transitions
                .iter()
                .any(|transitions| !transitions.is_empty())
            {
                let before = base.unpresented_variant(previous);
                let held = self
                    .transitions
                    .get_or_insert_with(WidgetTransitions::default);
                // A refused timing starts nothing; the change shows at once.
                let _ = held.lock().transition(&before, &after, now);
            }
            // What headed for the styling left behind stops here.
            if let Some(held) = &self.transitions {
                held.lock().advance(now, &after);
            }
        }
        // CSS Animations 1: the animations this styling names run by themselves.
        let shown = base.unpresented_variant(key);
        let named = shown.animations.iter().any(|list| !list.is_empty());
        if named
            || self
                .transitions
                .as_ref()
                .is_some_and(|held| held.lock().has_keyframes())
        {
            let held = self
                .transitions
                .get_or_insert_with(WidgetTransitions::default);
            held.lock().play(&shown, now);
        }
        match &self.transitions {
            Some(held) if held.lock().presents() => {
                let presented = held
                    .lock()
                    .presented_over(&shown, base.presentation.clone());
                (presented, true)
            }
            _ => (base.variant(key), false),
        }
    }

    /// Moves the widget's transitions on to `now`; whether they still run.
    fn advance_transitions(&mut self, base: &BoxTree, key: InteractionKey, now: Instant) -> bool {
        let Some(held) = &self.transitions else {
            return false;
        };
        let mut animations = held.lock();
        if !animations.is_animating() {
            return false;
        }
        animations.advance(now, &base.unpresented_variant(key));
        // The frame after the last shows where it ends.
        true
    }
}

impl PartialEq for Interaction {
    fn eq(&self, other: &Self) -> bool {
        (self.hovered, self.active, self.focused) == (other.hovered, other.active, other.focused)
            && self.scrolled == other.scrolled
            && self.scrolled_x == other.scrolled_x
    }
}

impl Interaction {
    /// Moves the state to `source`'s node numbers. Source nodes are numbered in postorder,
    /// so another capture numbers the same elements differently; a restyle does not.
    fn follow(&mut self, source: &Arc<Source>) {
        let Some(old) = self.source.replace(Arc::clone(source)) else {
            return;
        };
        if old.capture == source.capture || *self == Self::default() {
            return;
        }
        let carried = Carried::new(old.construction.nodes(), source.construction.nodes());
        for held in [&mut self.hovered, &mut self.active, &mut self.focused] {
            *held = held.and_then(|node| carried.at(node));
        }
        if let Some(seen) = &mut self.seen {
            for held in [&mut seen.0, &mut seen.1, &mut seen.2] {
                *held = held.and_then(|node| carried.at(node));
            }
        }
        for offsets in [&mut self.scrolled, &mut self.scrolled_x] {
            *offsets = std::mem::take(offsets)
                .into_iter()
                .filter_map(|(node, offset)| Some((carried.at(node)?, offset)))
                .collect();
        }
    }
}

/// Where one capture's elements are in another: by CSS id where one element alone has it,
/// and below or without such an element by the path down, a keyed child by its key and
/// any other by its position among its siblings.
struct Carried<'a> {
    old: &'a [crate::capture::ConstructionNode],
    new: &'a [crate::capture::ConstructionNode],
    /// Each side's CSS ids, with the node when only one has the id.
    ids: [BTreeMap<&'a str, Option<usize>>; 2],
    /// Each side's children per node, in order.
    children: [Vec<Vec<usize>>; 2],
}

impl<'a> Carried<'a> {
    fn new(
        old: &'a [crate::capture::ConstructionNode],
        new: &'a [crate::capture::ConstructionNode],
    ) -> Self {
        let ids = [old, new].map(|nodes| {
            let mut ids = BTreeMap::new();
            for (index, node) in nodes.iter().enumerate() {
                if let Some(id) = node.metadata().css_id() {
                    ids.entry(id)
                        .and_modify(|only| *only = None)
                        .or_insert(Some(index));
                }
            }
            ids
        });
        let children = [old, new].map(|nodes| {
            let mut children = vec![Vec::new(); nodes.len()];
            for (index, node) in nodes.iter().enumerate() {
                if let Some(list) = node.parent().and_then(|parent| children.get_mut(parent)) {
                    list.push(index);
                }
            }
            children
        });
        Self {
            old,
            new,
            ids,
            children,
        }
    }

    /// The new node for the old one, or `None` when the element is gone or is another one.
    fn at(&self, node: usize) -> Option<usize> {
        // Up to an element its id names on both sides, or to the root.
        let mut path = Vec::new();
        let mut here = node;
        let mut found = loop {
            let record = self.old.get(here)?;
            if let Some(id) = record.metadata().css_id()
                && self.ids[0].get(id).is_some_and(Option::is_some)
            {
                match *self.ids[1].get(id)? {
                    Some(named) => break named,
                    // Several elements have the id now: only the path can tell them apart.
                    None => {}
                }
            }
            let Some(parent) = record.parent() else {
                break self.new.len().checked_sub(1)?;
            };
            let position = self.children[0]
                .get(parent)?
                .iter()
                .position(|child| *child == here)?;
            path.push((here, position));
            here = parent;
        };
        // And down again, as long as each step finds the same kind of element.
        for (old, position) in path.into_iter().rev() {
            let record = &self.old[old];
            let siblings = &self.children[1][found];
            found = match record.source_key() {
                Some(key) => {
                    let mut keyed = siblings
                        .iter()
                        .copied()
                        .filter(|child| self.new[*child].source_key() == Some(key));
                    keyed.next().filter(|_| keyed.next().is_none())?
                }
                None => *siblings.get(position)?,
            };
            let next = &self.new[found];
            if next.source_key() != record.source_key()
                || next.metadata().css_id() != record.metadata().css_id()
                || next.canonical_type() != record.canonical_type()
            {
                return None;
            }
        }
        Some(found)
    }
}

/// 18.1: the system cursor for a keyword; `None` is `auto`, which depends on the context.
fn css_cursor(keyword: &str) -> Option<mouse::Interaction> {
    use mouse::Interaction as Shown;
    Some(match keyword {
        "default" => Shown::Idle,
        "pointer" => Shown::Pointer,
        "crosshair" => Shown::Crosshair,
        "move" => Shown::Move,
        "text" => Shown::Text,
        "wait" => Shown::Wait,
        "help" => Shown::Help,
        "progress" => Shown::Progress,
        "e-resize" | "w-resize" => Shown::ResizingHorizontally,
        "n-resize" | "s-resize" => Shown::ResizingVertically,
        "ne-resize" | "sw-resize" => Shown::ResizingDiagonallyUp,
        "nw-resize" | "se-resize" => Shown::ResizingDiagonallyDown,
        _ => return None,
    })
}

/// What an inline box paints beyond its text, above and below: its padding, and its top and
/// bottom borders. They do not change the line's height (10.8.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InlineDecoration {
    pub padding: [f32; 2],
    /// The top and bottom padding's fixed pixels and percentage, of the block's content
    /// width: `padding` once its width is known.
    pub padding_share: [(f32, f32); 2],
    pub border: [(f32, Option<[f32; 3]>); 2],
    /// The `border-top-style` and `border-bottom-style`.
    pub border_style: [&'static str; 2],
    pub background: Option<[f32; 3]>,
    /// The source node of the box; for a `:before` or `:after` box, which is no source
    /// node's, its `generated_owner`, past `usize::MAX / 2`.
    pub owner: usize,
    /// 11.2: a hidden box keeps its room and paints nothing; kept for an animated
    /// `visibility` to show.
    pub visible: bool,
    /// The opacity of `background`, 1 but while it fades.
    pub alpha: f32,
    /// The opacity of the top and bottom border colors, 1 but while they fade.
    pub border_alpha: [f32; 2],
}

/// Whether text has characters of the right-to-left scripts.
fn holds_rtl(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character,
            '\u{590}'..='\u{8FF}'
                | '\u{FB1D}'..='\u{FDFF}'
                | '\u{FE70}'..='\u{FEFF}'
                | '\u{10800}'..='\u{10FFF}'
                | '\u{1E800}'..='\u{1EFFF}'
        )
    })
}

/// A run with its spaces and no-break spaces in runs of their own, `spacing` (negative)
/// narrower each: drawn at a smaller size, a space taken as a quarter of the font size
/// wide and narrowing no further than to none. Their line keeps its height.
fn narrow_spaces(run: &TextRun, spacing: f32) -> Vec<TextRun> {
    let scale = (1.0 + spacing / (run.size * 0.25)).max(0.0);
    let narrow = TextRun {
        text: String::new(),
        size: (run.size * scale).max(0.5),
        line_size: Some(run.line_size.unwrap_or(run.size)),
        ..run.clone()
    };
    let mut parts: Vec<TextRun> = Vec::new();
    let mut spaces = None;
    for character in run.text.chars() {
        let here = matches!(character, ' ' | '\u{a0}');
        if spaces != Some(here) {
            parts.push(if here {
                narrow.clone()
            } else {
                TextRun {
                    text: String::new(),
                    ..run.clone()
                }
            });
            spaces = Some(here);
        }
        if let Some(part) = parts.last_mut() {
            part.text.push(character);
        }
    }
    if parts.is_empty() {
        parts.push(run.clone());
    }
    parts
}

/// A run in capitals, with what was lowercase at a smaller size.
fn small_capitals(run: &TextRun) -> Vec<TextRun> {
    let mut parts: Vec<TextRun> = Vec::new();
    let mut lower = false;
    for character in run.text.chars() {
        let here = character.is_lowercase();
        if parts.is_empty() || here != lower {
            parts.push(TextRun {
                text: String::new(),
                size: if here { run.size * 0.8 } else { run.size },
                line_size: here.then_some(run.size),
                ..run.clone()
            });
            lower = here;
        }
        if let Some(part) = parts.last_mut() {
            part.text.extend(character.to_uppercase());
        }
    }
    parts
}

/// The owner a `:before` (side 0) or `:after` (side 1) box of the projected `element` keys
/// under in decorations: one of its own, past `usize::MAX / 2`, which no source node is.
#[must_use]
pub fn generated_owner(element: usize, side: usize) -> usize {
    usize::MAX - (2 * element + side)
}

/// Whether a decoration's owner is a `:before` or `:after` box.
#[must_use]
pub fn is_generated(owner: usize) -> bool {
    owner > usize::MAX / 2
}

/// An inline `:before` or `:after`'s sides, from its frame, as an inline element's.
fn frame_sides(frame: &GeneratedFrame) -> InlineBoxSides {
    let pixels = |dims: [Dim; 4]| {
        dims.map(|value| match value {
            Dim::Px(pixels) | Dim::Mix(pixels, _) => pixels,
            _ => 0.0,
        })
    };
    let percent = |dims: [Dim; 4]| {
        dims.map(|value| match value {
            Dim::Percent(percent) | Dim::Mix(_, percent) => percent.max(0.0),
            _ => 0.0,
        })
    };
    InlineBoxSides {
        margin: pixels(frame.margin),
        border: frame.border,
        padding: pixels(frame.padding),
        percent_margin: percent(frame.margin),
        percent_padding: percent(frame.padding),
    }
}

/// Invisible spaces about `indent` wide at a font of `size`, empty when too small to show.
fn spaces(size: f32, indent: f32) -> String {
    // No line is this wide: more only costs shaping, and text past it is off the box anyway.
    let mut left = (indent / size.max(1.0)).min(512.0);
    let mut text = String::new();
    for (width, space) in [
        (1.0, '\u{2003}'),
        (0.5, '\u{2002}'),
        (1.0 / 3.0, '\u{2004}'),
        (0.25, '\u{2005}'),
        (1.0 / 6.0, '\u{2006}'),
    ] {
        let count = (left / width).floor();
        text.extend(std::iter::repeat_n(space, count as usize));
        left -= count * width;
    }
    text
}

/// What a typical space is wide, as a share of the font size, where it cannot be measured.
const SPACE_SHARE: f32 = 0.25;

/// Narrows the space beside each side that a negative margin pulls past its own border and
/// padding (`TextRun::pull`): the one before a start side, the one after an end side. A
/// space is taken to be `SPACE_SHARE` of its font wide; a pull wider than it goes no further.
fn pull_aside(runs: &mut [TextRun]) {
    for at in 0..runs.len() {
        let (pull, side) = (runs[at].pull, runs[at].side_of.map(|side| side.1));
        if pull <= 0.0 {
            continue;
        }
        let near = if side == Some(3) {
            (0..at).rev().find(|near| !runs[*near].text.is_empty())
        } else {
            (at + 1..runs.len()).find(|near| !runs[*near].text.is_empty())
        };
        let Some(near) = near else { continue };
        let beside = &mut runs[near];
        let size = beside.size;
        let narrower = spaces(size, (SPACE_SHARE * size - pull).max(0.0));
        if side == Some(3) && beside.text.ends_with(' ') {
            beside.text.pop();
            beside.text.push_str(&narrower);
        } else if side != Some(3) && beside.text.starts_with(' ') {
            beside.text.replace_range(..1, &narrower);
        }
    }
}

/// What a negative margin beside a percentage padding takes out of the room it has at
/// `width` (`TextRun::pulled`): no more than that room.
fn percent_overhang(run: &TextRun, width: f32) -> f32 {
    match run.percent_room {
        Some((fixed, percent, _)) if run.pulled > 0.0 => {
            run.pulled.min((fixed + percent / 100.0 * width).max(0.0))
        }
        _ => 0.0,
    }
}

/// A side's or indent's spaces for its room (`TextRun::percent_room`) at the run's size,
/// with `width` the block's content width; a paragraph's direction mark on it stays.
fn room_text(run: &TextRun, width: f32) -> Option<String> {
    let (fixed, percent, rounded) = run.percent_room?;
    let mut room = fixed + percent / 100.0 * width;
    let over = percent_overhang(run, width);
    room -= over;
    if rounded {
        let step = run.size.max(1.0) / 6.0;
        room = (room / step).ceil() * step * 1.0001;
    }
    let mut text = spaces(run.size, room);
    // A box drawn past spaces it has none of keeps a place to draw from.
    if text.is_empty() && (run.overhang > 0.0 || over > 0.0) {
        text.push('\u{200B}');
    }
    if let Some(mark) = run
        .text
        .chars()
        .next()
        .filter(|mark| matches!(mark, '\u{200E}' | '\u{200F}'))
    {
        text.insert(0, mark);
    }
    Some(text)
}

/// 9.10: a paragraph's direction mark that no longer leads its first text, as the first
/// run with text and the run holding the mark.
fn stranded_mark(leaf: &TextLeaf) -> Option<(usize, usize)> {
    let starts = |run: &TextRun| {
        run.text
            .chars()
            .next()
            .is_some_and(|mark| matches!(mark, '\u{200E}' | '\u{200F}'))
    };
    let first = leaf.runs.iter().position(|run| !run.text.is_empty())?;
    if starts(&leaf.runs[first]) {
        return None;
    }
    let at = leaf.runs[first..]
        .iter()
        .position(|run| starts(run) || (!run.spacer && !run.text.is_empty()))?;
    starts(&leaf.runs[first + at]).then_some((first, first + at))
}

/// Invisible spaces about `indent` wide at `first`'s font, or `None` when too small to show.
fn indent_run(first: &TextRun, indent: f32) -> Option<TextRun> {
    let text = spaces(first.size, indent);
    (!text.is_empty()).then(|| TextRun {
        text,
        spacer: true,
        background: None,
        background_from: None,
        percent_room: None,
        side_of: None,
        fades: Vec::new(),
        edge_alpha: 1.0,
        overhang: 0.0,
        pull: 0.0,
        pulled: 0.0,
        generated: None,
        marker_image: None,
        backgrounds: Vec::new(),
        underline: false,
        line_through: false,
        overline: false,
        inline_boxes: Vec::new(),
        edge: None,
        ..first.clone()
    })
}

/// 17.6.2.1: on an edge two cells of a collapsed table share, only the winner's border draws,
/// and a `hidden` one draws neither. A cell's side first takes the border of a row, row
/// group, column, column group or the table that lies along it and beats its own. A cell
/// that is not visible has no say. Cells that span the edge alike settle all of it; a cell
/// beside a taller or wider one settles the stretch they share, and a loser leaves out only
/// that stretch (`border_cuts`).
fn resolve_collapsed_edges(prepared: &mut PreparedTree, table: usize, paints: &mut [BoxPaint]) {
    use iced_css_core::formatting::{
        block_flow::{table_grid, table_parts},
        table_borders::{
            BorderCandidate, BorderConflictLimits, BorderOrder, BorderOrigin, BorderStyle,
            CollapsedBorder, resolve_collapsed_border,
        },
    };
    let style_of = |keyword: &str| match keyword {
        "hidden" => BorderStyle::Hidden,
        "inset" => BorderStyle::Inset,
        "groove" => BorderStyle::Groove,
        "outset" => BorderStyle::Outset,
        "ridge" => BorderStyle::Ridge,
        "dotted" => BorderStyle::Dotted,
        "dashed" => BorderStyle::Dashed,
        "solid" => BorderStyle::Solid,
        "double" => BorderStyle::Double,
        _ => BorderStyle::None,
    };
    let (rows, _) = table_parts(&prepared.boxes, table);
    let (placed, columns) = table_grid(&prepared.boxes, table, &rows);
    // In an rtl table the first column is on the right, so its far side is its left.
    let rtl = prepared.boxes[table].direction == Direction::Rtl;
    let (near, far) = if rtl { (3, 1) } else { (1, 3) };
    // The cells that start each column of a row, and each row of a column: all a scan
    // across or down reads, in time along each cell's span rather than its area.
    let mut first_column: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut first_row: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (at, item) in placed.iter().enumerate() {
        for row in item.row..item.row + item.row_span {
            first_column.insert((row, item.column), at);
        }
        for column in item.column..item.column + item.col_span {
            first_row.insert((item.row, column), at);
        }
    }
    // Each pair of cells that share some edge, once: across to the end side, and down. A
    // pair that spans the edge alike shares all of it; others share a stretch.
    let mut meetings = Vec::new();
    for item in &placed {
        let mut seen = Vec::new();
        for row in item.row..item.row + item.row_span {
            if let Some(&at) = first_column.get(&(row, item.column + item.col_span))
                && !seen.contains(&at)
            {
                seen.push(at);
                let other = &placed[at];
                let whole = other.row == item.row && other.row_span == item.row_span;
                meetings.push((item, near, other, far, whole));
            }
        }
        let mut seen = Vec::new();
        for column in item.column..item.column + item.col_span {
            if let Some(&at) = first_row.get(&(item.row + item.row_span, column))
                && !seen.contains(&at)
            {
                seen.push(at);
                let other = &placed[at];
                let whole = other.column == item.column && other.col_span == item.col_span;
                meetings.push((item, 2, other, 0, whole));
            }
        }
    }
    let limit = NonZeroUsize::new(2).expect("positive");
    let limits = BorderConflictLimits {
        candidates: limit,
        work: limit,
    };
    for item in &placed {
        if !prepared.anonymous[item.cell] {
            paints[prepared.positions[item.cell].0].collapsed_cell = Some(table);
        }
    }
    // 17.6.2.1: the rows, row groups, columns and column groups whose edge a cell's side lies
    // on contend for it too, and on its outer edges the table; each loses a tie to the ones
    // before it. Where one wins, the cell draws it; `hidden` there leaves none. A part that
    // is not visible has no say, as a cell has none.
    struct Layer {
        owner: usize,
        widths: [f32; 4],
        origin: BorderOrigin,
        order: BorderOrder,
        /// Its arena slot; a column or a column group has no box.
        slot: Option<usize>,
        rows: (usize, usize),
        columns: (usize, usize),
    }
    let mut layers: Vec<Layer> = Vec::new();
    {
        let mut layer = |owner: usize,
                         widths: Option<[f32; 4]>,
                         origin,
                         slot,
                         rows: (usize, usize),
                         columns: (usize, usize)| {
            let widths = widths.or_else(|| {
                let at = prepared
                    .layer_borders
                    .binary_search_by_key(&owner, |entry| entry.0)
                    .ok()?;
                Some(prepared.layer_borders[at].1)
            });
            if paints[owner].visible && paints[owner].border_style != ["none"; 4] {
                layers.push(Layer {
                    owner,
                    widths: widths.unwrap_or_default(),
                    origin,
                    order: BorderOrder {
                        inline_start: columns.0,
                        block_start: rows.0,
                    },
                    slot,
                    rows,
                    columns,
                });
            }
        };
        let row_at: BTreeMap<usize, usize> = rows
            .iter()
            .enumerate()
            .map(|(index, &row)| (row, index))
            .collect();
        for (index, &row) in rows.iter().enumerate() {
            if !prepared.anonymous[row] {
                layer(
                    prepared.positions[row].0,
                    None,
                    BorderOrigin::Row,
                    Some(row),
                    (index, index + 1),
                    (0, columns),
                );
            }
        }
        for &group in &prepared.boxes[table].children {
            if prepared.anonymous[group]
                || !matches!(
                    prepared.boxes[group].table,
                    Some(TableRole::HeaderGroup | TableRole::BodyGroup | TableRole::FooterGroup)
                )
            {
                continue;
            }
            let own = || {
                prepared.boxes[group]
                    .children
                    .iter()
                    .filter_map(|row| row_at.get(row).copied())
            };
            if let (Some(first), Some(last)) = (own().min(), own().max()) {
                layer(
                    prepared.positions[group].0,
                    None,
                    BorderOrigin::RowGroup,
                    Some(group),
                    (first, last + 1),
                    (0, columns),
                );
            }
        }
        // A table's columns are recorded in column order, a group's together.
        let own: Vec<(usize, Option<usize>)> = prepared
            .table_columns
            .iter()
            .filter(|entry| entry.0 == table)
            .map(|entry| (entry.1, entry.2))
            .collect();
        for (at, &(column, group)) in own.iter().enumerate() {
            layer(
                column,
                None,
                BorderOrigin::Column,
                None,
                (0, rows.len()),
                (at, at + 1),
            );
            if let Some(group) = group
                && (at == 0 || own[at - 1].1 != Some(group))
            {
                let end = own[at..]
                    .iter()
                    .position(|entry| entry.1 != Some(group))
                    .map_or(own.len(), |past| at + past);
                layer(
                    group,
                    None,
                    BorderOrigin::ColumnGroup,
                    None,
                    (0, rows.len()),
                    (at, end),
                );
            }
        }
        if !prepared.anonymous[table] {
            layer(
                prepared.positions[table].0,
                Some(prepared.boxes[table].border),
                BorderOrigin::Table,
                Some(table),
                (0, rows.len()),
                (0, columns),
            );
        }
    }
    // Whose border each cell's side now carries, where not its own, and that one's box.
    let mut carried: BTreeMap<(usize, usize), (BorderOrigin, BorderOrder, Option<usize>)> =
        BTreeMap::new();
    let (start, end) = if rtl { (1, 3) } else { (3, 1) };
    for item in &placed {
        let cell_owner = prepared.positions[item.cell].0;
        if prepared.anonymous[item.cell] || !paints[cell_owner].visible {
            continue;
        }
        let (row_end, column_end) = (item.row + item.row_span, item.column + item.col_span);
        for layer in &layers {
            // A side lies on a part's edge where the cell ends with it, beside or inside it.
            let across = item.column < layer.columns.1 && column_end > layer.columns.0;
            let down = item.row < layer.rows.1 && row_end > layer.rows.0;
            for (on, side) in [
                (across && item.row == layer.rows.0, 0),
                (across && row_end == layer.rows.1, 2),
                (down && item.column == layer.columns.0, start),
                (down && column_end == layer.columns.1, end),
            ] {
                if !on {
                    continue;
                }
                let (origin, order) = carried.get(&(item.cell, side)).map_or(
                    (
                        BorderOrigin::Cell,
                        BorderOrder {
                            inline_start: item.column,
                            block_start: item.row,
                        },
                    ),
                    |entry| (entry.0, entry.1),
                );
                let (Ok(cell), Ok(own)) = (
                    BorderCandidate::new(
                        style_of(paints[cell_owner].border_style[side]),
                        prepared.boxes[item.cell].border[side],
                        origin,
                        order,
                    ),
                    BorderCandidate::new(
                        style_of(paints[layer.owner].border_style[side]),
                        layer.widths[side],
                        layer.origin,
                        layer.order,
                    ),
                ) else {
                    continue;
                };
                match resolve_collapsed_border(&[cell, own], limits) {
                    Ok(CollapsedBorder::Winner { index: 1, .. }) => {
                        paints[cell_owner].border[side] = paints[layer.owner].border[side];
                        paints[cell_owner].border_style[side] =
                            paints[layer.owner].border_style[side];
                        prepared.boxes[item.cell].border[side] = own.width();
                        carried.insert((item.cell, side), (layer.origin, layer.order, layer.slot));
                    }
                    // The side is `hidden` from here on, to the cell across it too.
                    Ok(CollapsedBorder::Suppressed) => {
                        paints[cell_owner].border[side] = None;
                        paints[cell_owner].border_style[side] = "hidden";
                        prepared.boxes[item.cell].border[side] = 0.0;
                        carried.remove(&(item.cell, side));
                    }
                    _ => {}
                }
            }
        }
    }
    let mut losers = Vec::new();
    let mut cuts = Vec::new();
    for (first, first_side, second, second_side, whole) in meetings {
        // An anonymous cell's owner is the box it wraps, and that box's borders are its own.
        if prepared.anonymous[first.cell] || prepared.anonymous[second.cell] {
            continue;
        }
        let (first_owner, second_owner) = (
            prepared.positions[first.cell].0,
            prepared.positions[second.cell].0,
        );
        if !paints[first_owner].visible || !paints[second_owner].visible {
            continue;
        }
        let candidate = |item: &iced_css_core::formatting::block_flow::GridCell, side: usize| {
            let (origin, order) = carried.get(&(item.cell, side)).map_or(
                (
                    BorderOrigin::Cell,
                    BorderOrder {
                        inline_start: item.column,
                        block_start: item.row,
                    },
                ),
                |entry| (entry.0, entry.1),
            );
            BorderCandidate::new(
                style_of(paints[prepared.positions[item.cell].0].border_style[side]),
                prepared.boxes[item.cell].border[side],
                origin,
                order,
            )
        };
        let (Ok(one), Ok(two)) = (candidate(first, first_side), candidate(second, second_side))
        else {
            continue;
        };
        let Ok(result) = resolve_collapsed_border(&[one, two], limits) else {
            continue;
        };
        let (keep_first, keep_second) = match result {
            CollapsedBorder::Winner { index, .. } => (index == 0, index == 1),
            CollapsedBorder::Absent | CollapsedBorder::Suppressed => (false, false),
        };
        // Along a stretch, a loser only gives way there, and in layout clears the wider of
        // its own border and the winner's.
        if !whole {
            match result {
                CollapsedBorder::Winner { .. } if keep_first => {
                    cuts.push((second.cell, second_side, first.cell, one.width()));
                }
                CollapsedBorder::Winner { .. } => {
                    cuts.push((first.cell, first_side, second.cell, two.width()));
                }
                // `hidden` there: neither draws along the other, and neither takes room.
                CollapsedBorder::Suppressed => {
                    cuts.push((first.cell, first_side, second.cell, 0.0));
                    cuts.push((second.cell, second_side, first.cell, 0.0));
                }
                CollapsedBorder::Absent => {}
            }
            continue;
        }
        // A loser keeps the winner's width in layout, so its content clears the winner's band.
        let width = match (keep_first, keep_second) {
            (true, _) => one.width(),
            (_, true) => two.width(),
            // 17.6.2.1: `hidden` leaves no border at all, and none takes no room.
            _ => 0.0,
        };
        if !keep_first {
            losers.push((first.cell, first_owner, first_side, width));
        }
        if !keep_second {
            losers.push((second.cell, second_owner, second_side, width));
        }
    }
    // After every comparison, so one edge's outcome does not feed another's.
    for (cell, owner, side, width) in losers {
        paints[owner].border[side] = None;
        prepared.boxes[cell].border[side] = width;
        prepared.collapsed_losers.push((cell, side));
        carried.remove(&(cell, side));
    }
    // What a side still draws of a table, a row or a row group follows that box's color.
    for (&(cell, side), &(_, _, slot)) in &carried {
        if let Some(slot) = slot {
            prepared.table_borders.push((cell, side, slot));
        }
    }
    let mut thin: Vec<(usize, usize, f32)> = Vec::new();
    for &(cell, side, winner, width) in &cuts {
        let own = prepared.boxes[cell].border[side];
        if width > own && !thin.iter().any(|entry| (entry.0, entry.1) == (cell, side)) {
            thin.push((cell, side, own));
        }
        prepared.boxes[cell].border[side] = own.max(width);
        prepared.border_cuts.push((cell, side, winner));
    }
    prepared.thin_sides.extend(thin);
}

/// A file under the base: no scheme, no absolute path, no `..`, and none reached through a
/// symlink that leads out of it. A `url()` is a relative URL: its query and fragment name no
/// file, and the escapes in its path decode.
fn local_image(base: &Path, uri: &str) -> Option<image::Handle> {
    let scheme = uri
        .split('/')
        .next()
        .is_some_and(|first| first.contains(':'));
    if scheme {
        return None;
    }
    let path = uri.split(['?', '#']).next().unwrap_or_default();
    // Each segment on its own: an escaped `/` or NUL names no file.
    let decoded = path
        .split('/')
        .map(|segment| {
            let bytes = segment.as_bytes();
            let mut out = Vec::with_capacity(bytes.len());
            let mut at = 0;
            while at < bytes.len() {
                let escape = (bytes[at] == b'%')
                    .then(|| segment.get(at + 1..at + 3))
                    .flatten()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok());
                if let Some(byte) = escape {
                    out.push(byte);
                    at += 3;
                } else {
                    out.push(bytes[at]);
                    at += 1;
                }
            }
            String::from_utf8(out)
                .ok()
                .filter(|segment| !segment.contains(['/', '\0']))
        })
        .collect::<Option<Vec<String>>>()?
        .join("/");
    let inside = Path::new(&decoded)
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if !inside {
        return None;
    }
    let file = base.join(&decoded);
    // A missing file draws nothing either way; one that exists must really be under the base.
    if let (Ok(real), Ok(root)) = (file.canonicalize(), base.canonicalize())
        && !real.starts_with(&root)
    {
        return None;
    }
    Some(image::Handle::from_path(file))
}

/// An outside operation over a widget whose box is hidden or absent: it is not there to
/// focus, as it takes no input. Everything else reaches it, since its state stays.
struct Unfocusable<'a>(&'a mut dyn Operation);

impl Operation for Unfocusable<'_> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        self.0
            .traverse(&mut |operation| operate(&mut Unfocusable(operation)));
    }

    fn container(&mut self, id: Option<&iced::widget::Id>, bounds: Rectangle) {
        self.0.container(id, bounds);
    }

    fn scrollable(
        &mut self,
        id: Option<&iced::widget::Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        translation: Vector,
        state: &mut dyn iced::advanced::widget::operation::Scrollable,
    ) {
        self.0
            .scrollable(id, bounds, content_bounds, translation, state);
    }

    fn text_input(
        &mut self,
        id: Option<&iced::widget::Id>,
        bounds: Rectangle,
        state: &mut dyn iced::advanced::widget::operation::TextInput,
    ) {
        self.0.text_input(id, bounds, state);
    }

    fn text(&mut self, id: Option<&iced::widget::Id>, bounds: Rectangle, text: &str) {
        self.0.text(id, bounds, text);
    }

    fn custom(
        &mut self,
        id: Option<&iced::widget::Id>,
        bounds: Rectangle,
        state: &mut dyn std::any::Any,
    ) {
        self.0.custom(id, bounds, state);
    }
}

/// What Tab asks of the focusable widgets inside a hosted element.
enum HostFocus {
    Has(bool),
    IsFocused(bool),
    /// How many it holds, and which of them, in order, has the focus.
    Find {
        count: usize,
        focused: Option<usize>,
    },
    /// Focus the one at `target` in order, counting them in `seen`, and none of the rest.
    Focus {
        target: usize,
        seen: usize,
    },
    Unfocus,
}

impl Operation for HostFocus {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn focusable(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn iced::advanced::widget::operation::Focusable,
    ) {
        match self {
            Self::Has(found) => *found = true,
            Self::IsFocused(found) => *found |= state.is_focused(),
            Self::Find { count, focused } => {
                if state.is_focused() {
                    focused.get_or_insert(*count);
                }
                *count += 1;
            }
            Self::Focus { target, seen } => {
                if *seen == *target {
                    state.focus();
                } else {
                    state.unfocus();
                }
                *seen += 1;
            }
            Self::Unfocus => state.unfocus(),
        }
    }
}

/// The pixels one wheel line scrolls.
const WHEEL_LINE: f32 = 40.0;

/// Calls `visit(box, reaches)` on every box that scrolls with `root`: its descendants,
/// except absolute boxes whose containing block is outside it (they stay put, 11.1.1) and
/// fixed boxes. The root's own containing block is the viewport, so everything but a
/// fixed box moves with it. `reaches` is false under a nearer clip: those boxes move but
/// do not extend the scrollable area.
fn scrolling_descendants(prepared: &PreparedTree, root: usize, mut visit: impl FnMut(usize, bool)) {
    let holder = root == 0 || prepared.positions[root].1 != Position::Static;
    // The last flag is what an absolute box here reaches: a clip on a static box between
    // it and its containing block does not clip it.
    let mut stack: Vec<(usize, bool, bool, bool)> = prepared.boxes[root]
        .children
        .iter()
        .map(|child| (*child, holder, true, true))
        .collect();
    while let Some((arena, holder_inside, reaches, escapes)) = stack.pop() {
        let position = prepared.positions[arena].1;
        if position == Position::Fixed || (position == Position::Absolute && !holder_inside) {
            continue;
        }
        let reaches = if position == Position::Absolute {
            escapes
        } else {
            reaches
        };
        visit(arena, reaches);
        let inside = holder_inside || position != Position::Static;
        let further = reaches && !prepared.clipped[arena];
        let escapes = if position == Position::Static {
            escapes
        } else {
            further
        };
        stack.extend(
            prepared.boxes[arena]
                .children
                .iter()
                .map(|child| (*child, inside, further, escapes)),
        );
    }
}

/// 11.1.2: the rectangle `clip` cuts an absolutely positioned box and its contents to.
fn own_clip(
    clip: Option<[Option<f32>; 4]>,
    position: Position,
    border: FlowRect,
) -> Option<FlowRect> {
    let sides = clip.filter(|_| matches!(position, Position::Absolute | Position::Fixed))?;
    Some(FlowRect {
        top: sides[0].map_or(border.top, |side| border.top + side),
        right: sides[1].map_or(border.right, |side| border.left + side),
        bottom: sides[2].map_or(border.bottom, |side| border.top + side),
        left: sides[3].map_or(border.left, |side| border.left + side),
    })
}

/// Where a box's text reaches, from its place's top left: down, and across from `from` to
/// `to`, which run past the place where the text is wider.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TextExtent {
    height: f32,
    from: f32,
    to: f32,
}

/// `more` within `clip`, when there is one. Disjoint rectangles leave an inverted one.
fn narrowed(clip: Option<FlowRect>, more: FlowRect) -> FlowRect {
    clip.map_or(more, |clip| FlowRect {
        left: clip.left.max(more.left),
        right: clip.right.min(more.right),
        top: clip.top.max(more.top),
        bottom: clip.bottom.min(more.bottom),
    })
}

/// How far each scrolling box can scroll down and across, after moving its contents by
/// its offsets. Parents come first in arena order, so nested scrollers measure moved
/// contents. Across, a box scrolls toward its end: an ltr box's contents past its right,
/// an rtl box's past its left (11.1.1). The offset counts from the start either way.
fn apply_scrolling(
    prepared: &PreparedTree,
    geometry: &mut [FlowGeometry],
    scrolled: impl Fn(usize) -> (f32, f32),
    text_size: impl Fn(usize, &FlowGeometry) -> Option<TextExtent>,
    cut: impl Fn(usize, &FlowGeometry) -> Option<FlowRect>,
    viewport: Size,
) -> Vec<(f32, f32)> {
    let mut limits = vec![(0.0_f32, 0.0_f32); prepared.boxes.len()];
    // What `clip` cuts away scrolls nowhere: each box's reach stops at the clips between it
    // and the scroller.
    let any_cut = (0..geometry.len()).any(|arena| cut(arena, &geometry[arena]).is_some());
    let mut parents = vec![None; if any_cut { prepared.boxes.len() } else { 0 }];
    if any_cut {
        for (arena, value) in prepared.boxes.iter().enumerate() {
            for &child in &value.children {
                parents[child] = Some(arena);
            }
        }
    }
    for arena in 0..geometry.len() {
        if !prepared.scrolls[arena] {
            continue;
        }
        let padding = geometry[arena].padding;
        // The root scrolls the viewport, however big the root itself is.
        let (bottom, right) = if arena == 0 {
            (viewport.height, viewport.width)
        } else {
            (padding.bottom, padding.right)
        };
        let (mut reach, mut reach_x) = (bottom, right);
        let start_x = if arena == 0 { 0.0 } else { padding.left };
        let mut reach_left = start_x;
        // A scroller's own text scrolls in it too.
        let own_text = text_size(arena, &geometry[arena]);
        if let Some(text) = own_text {
            let place = geometry[arena].leaf_band.unwrap_or(geometry[arena].content);
            reach = reach.max(place.top + text.height);
            reach_x = reach_x.max(place.left + text.to);
            reach_left = reach_left.min(place.left + text.from);
        }
        let scroller = arena;
        scrolling_descendants(prepared, arena, |child, reaches| {
            if reaches {
                let mut bound: Option<FlowRect> = None;
                let mut at = any_cut.then_some(child);
                while let Some(here) = at.filter(|here| *here != scroller) {
                    if let Some(own) = cut(here, &geometry[here]) {
                        bound = Some(narrowed(bound, own));
                    }
                    at = parents[here];
                }
                // Only what shows through every clip counts, and only as far as it shows.
                let shows = |top: f32, right: f32, bottom: f32, left: f32| {
                    // A true overlap: disjoint nested clips leave an inverted bound.
                    bound.is_none_or(|bound| {
                        bottom.min(bound.bottom) > top.max(bound.top)
                            && right.min(bound.right) > left.max(bound.left)
                    })
                };
                let (low, high, before) = bound
                    .map_or((f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY), |bound| {
                        (bound.bottom, bound.right, bound.left)
                    });
                let box_of = &geometry[child];
                let border = box_of.border;
                if shows(border.top, border.right, border.bottom, border.left) {
                    reach = reach.max(border.bottom.min(low));
                    reach_x = reach_x.max(border.right.min(high));
                    reach_left = reach_left.min(border.left.max(before));
                }
                // Text runs past a box too small for it, unless that box clips it.
                if !prepared.clipped[child]
                    && let Some(text) = text_size(child, box_of)
                {
                    let place = box_of.leaf_band.unwrap_or(box_of.content);
                    let (bottom, right, left) = (
                        place.top + text.height,
                        place.left + text.to,
                        place.left + text.from,
                    );
                    if shows(place.top, right, bottom, left) {
                        reach = reach.max(bottom.min(low));
                        reach_x = reach_x.max(right.min(high));
                        reach_left = reach_left.min(left.max(before));
                    }
                }
            }
        });
        // 11.1.1: the scrollable area includes the box's own end padding; at the root,
        // the box's whole edge.
        let (trailing, trailing_x) = if arena == 0 {
            reach = reach.max(geometry[0].border.bottom);
            reach_x = reach_x.max(geometry[0].border.right);
            reach_left = reach_left.min(geometry[0].border.left);
            (0.0, 0.0)
        } else {
            (
                padding.bottom - geometry[arena].content.bottom,
                padding.right - geometry[arena].content.right,
            )
        };
        // A box shrunk below its content (a cell in a hidden row) has a negative trailing
        // edge; it still never scrolls back past its start.
        let limit = if reach > bottom {
            (reach + trailing - bottom).max(0.0)
        } else {
            0.0
        };
        let ltr = prepared.boxes[arena].direction == Direction::Ltr;
        let limit_x = if ltr && reach_x > right {
            (reach_x + trailing_x - right).max(0.0)
        } else if !ltr && reach_left < start_x {
            let end_padding = if arena == 0 {
                0.0
            } else {
                geometry[arena].content.left - padding.left
            };
            (start_x - reach_left + end_padding).max(0.0)
        } else {
            0.0
        };
        limits[arena] = (limit, limit_x);
        let (down, across) = scrolled(arena);
        let (offset, offset_x) = (down.clamp(0.0, limit), across.clamp(0.0, limit_x));
        if offset == 0.0 && offset_x == 0.0 {
            continue;
        }
        // Scrolling toward an rtl box's end moves its contents right.
        let shift_x = if ltr { -offset_x } else { offset_x };
        if own_text.is_some() {
            let own = &mut geometry[arena];
            let rectangles = [Some(&mut own.content), own.leaf_band.as_mut()];
            for place in rectangles.into_iter().flatten() {
                place.left += shift_x;
                place.right += shift_x;
                place.top -= offset;
                place.bottom -= offset;
            }
        }
        let mut moved = Vec::new();
        scrolling_descendants(prepared, arena, |child, _| moved.push(child));
        for child in moved {
            geometry[child].move_by(shift_x, -offset);
        }
    }
    limits
}

struct BoxTreeWidget<'a, Message, Theme, Renderer: text::Renderer<Font = iced::Font>> {
    base: BoxTree,
    /// The base restyled for `key`.
    tree: BoxTree,
    key: InteractionKey,
    /// `tree` shows the widget's own transitions, so it is styled again each layout.
    presenting: bool,
    measured: bool,
    geometry: Vec<Option<FlowGeometry>>,
    /// The same layout by arena slot, including anonymous blocks.
    slot_geometry: Vec<FlowGeometry>,
    /// Each `PreparedTree::table_columns` entry's background area, and its group's, from the
    /// last layout.
    column_rects: Vec<(Option<Rectangle>, Option<Rectangle>)>,
    /// How far each arena slot can scroll, down and across.
    scroll_limits: Vec<(f32, f32)>,
    /// The widget's height at the last layout.
    canvas_height: f32,
    canvas_width: f32,
    clips: Vec<Option<FlowRect>>,
    /// Paint order at the last layout, front last; hit tests walk it on every pointer event.
    steps: Vec<PaintStep>,
    /// The same steps as they draw.
    drawn: Vec<PaintStep>,
    elements: Vec<Element<'a, Message, Theme, Renderer>>,
    /// The CSS id naming each element's box.
    ids: Vec<String>,
    /// Each element's arena slot in the current styling, while it has a box.
    slots: Vec<Option<usize>>,
    /// The other way: the first element in each arena slot, by slot.
    slot_owners: Vec<Option<usize>>,
    presses: Vec<(String, Message)>,
    /// Shaped text and its top-left corner relative to the layout, by arena.
    paragraphs: BTreeMap<usize, (Renderer::Paragraph, Point)>,
    /// The text after the first paragraph's, by arena slot: past a shorter float or a
    /// hanging first line, or each line of right-to-left text aligned left or centered.
    /// Each with its corner and its leaf.
    rest_paragraphs: BTreeMap<usize, Vec<(Renderer::Paragraph, Point, TextLeaf)>>,
    /// Holds the background images while they draw.
    allocations: Vec<image::Allocation>,
    /// The projected indices whose background image loaded.
    loaded: BTreeSet<usize>,
    /// The projected indices whose marker image loaded; an image that did not, draws the
    /// marker text instead (12.6.2).
    loaded_markers: BTreeSet<usize>,
    /// Shaped outside markers and their corners, by projected index of their item.
    marker_paragraphs: BTreeMap<usize, (Renderer::Paragraph, Point)>,
}

/// Family names live for the program: iced fonts hold a `&'static str`, and a stylesheet
/// names few distinct families.
fn interned(name: &str) -> &'static str {
    static NAMES: std::sync::Mutex<BTreeMap<String, &'static str>> =
        std::sync::Mutex::new(BTreeMap::new());
    let mut names = NAMES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(found) = names.get(name) {
        return found;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.insert(name.to_owned(), leaked);
    leaked
}

fn font_of(base: iced::Font, run: &TextRun) -> iced::Font {
    use iced::font::{Family, Style, Weight};
    let family = match run.family.as_deref() {
        None => base.family,
        Some("serif") => Family::Serif,
        Some("sans-serif") => Family::SansSerif,
        Some("cursive") => Family::Cursive,
        Some("fantasy") => Family::Fantasy,
        Some("monospace") => Family::Monospace,
        Some(name) => Family::Name(interned(name)),
    };
    iced::Font {
        family,
        weight: match run.weight {
            0..=149 => Weight::Thin,
            150..=249 => Weight::ExtraLight,
            250..=349 => Weight::Light,
            350..=449 => Weight::Normal,
            450..=549 => Weight::Medium,
            550..=649 => Weight::Semibold,
            650..=749 => Weight::Bold,
            750..=849 => Weight::ExtraBold,
            _ => Weight::Black,
        },
        style: if run.italic {
            Style::Italic
        } else {
            Style::Normal
        },
        ..base
    }
}

fn line_height_of(leaf: &TextLeaf) -> text::LineHeight {
    match leaf.line_height {
        TextLineHeight::Normal => text::LineHeight::default(),
        TextLineHeight::Multiplier(factor) => text::LineHeight::Relative(factor),
        TextLineHeight::Px(px) => text::LineHeight::Absolute(Pixels(px)),
    }
}

/// The baseline of a laid-out leaf's last line below its top, as cosmic-text places it:
/// the line's top, half of what its height leaves over its fonts' ascent and descent, and
/// the ascent, over the text on that line, from the metrics of the faces that draw it.
fn last_baseline<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
    shaped: &Renderer::Paragraph,
) -> Option<f32> {
    use iced::advanced::graphics::text::{cosmic_text, font_system, to_attributes};
    use unicode_script::{Script, UnicodeScript};
    let pieces: Vec<(usize, Rectangle)> = (0..leaf.runs.len())
        .flat_map(|at| {
            shaped
                .span_bounds(at)
                .into_iter()
                .map(move |area| (at, area))
        })
        .collect();
    let top = pieces
        .iter()
        .map(|piece| piece.1.y)
        .fold(f32::NEG_INFINITY, f32::max);
    if !top.is_finite() {
        return None;
    }
    // The last line's text, by the offsets across it: they count from the start of the
    // part after the last forced break, which the shaper lays out as a line of its own.
    let whole = leaf.text();
    let part = cosmic_text::BidiParagraphs::new(&whole)
        .last()
        .map_or(0, |part| part.as_ptr() as usize - whole.as_ptr() as usize);
    let (mut low, mut high) = (usize::MAX, 0);
    let mut x = 0.0;
    while x <= shaped.min_bounds().width + 1.0 {
        if let Some(text::Hit::CharOffset(offset)) = shaped.hit_test(Point::new(x, top + 0.5)) {
            low = low.min(part + offset);
            high = high.max(part + offset);
        }
        x += 1.0;
    }
    let mut starts = Vec::with_capacity(leaf.runs.len());
    let mut seen = 0;
    for run in &leaf.runs {
        starts.push(seen);
        seen += run.text.len();
    }
    let base = renderer.default_font();
    let mut fonts = font_system().write().ok()?;
    let fonts = fonts.raw();
    let (mut ascent, mut descent, mut line) = (0.0_f32, 0.0_f32, 0.0_f32);
    for &(at, area) in pieces.iter().filter(|piece| (piece.1.y - top).abs() < 0.5) {
        let run = &leaf.runs[at];
        if run.text.is_empty() {
            continue;
        }
        // What of it is on that line; all of it when the scan found none.
        let begin = starts[at];
        let on_line = (low < high)
            .then(|| {
                let from = low.clamp(begin, begin + run.text.len()) - begin;
                let to = high.clamp(begin, begin + run.text.len()) - begin;
                run.text.get(from..to)
            })
            .flatten()
            .filter(|text| !text.is_empty())
            .unwrap_or(&run.text);
        let attrs = to_attributes(font_of(base, run));
        let keys = fonts.get_font_matches(&attrs);
        let families = [&attrs.family];
        // cosmic-text shapes it word by word: each stretch up to a break opportunity, less its
        // trailing whitespace, then each of those whitespace characters alone.
        let mut words = Vec::new();
        let mut start = 0;
        for (end, _) in unicode_linebreak::linebreaks(on_line) {
            let word = on_line[start..end].trim_end_matches(char::is_whitespace);
            if !word.is_empty() {
                words.push(word);
            }
            let blank = start + word.len();
            words.extend(
                on_line[blank..end]
                    .char_indices()
                    .map(|(at, c)| &on_line[blank + at..blank + at + c.len_utf8()]),
            );
            start = end;
        }
        for word in words {
            // Its faces as cosmic-text shapes it: the family's at its weight, else the common
            // fallbacks and those for the word's own scripts, else the closest match. Each
            // character takes the first that has it, and the first takes what none has,
            // blanks included.
            let scripts = word
                .chars()
                .map(|c| c.script())
                .fold(Vec::new(), |mut all, script| {
                    if !matches!(
                        script,
                        Script::Common | Script::Inherited | Script::Latin | Script::Unknown
                    ) && !all.contains(&script)
                    {
                        all.push(script);
                    }
                    all
                });
            let mut missing: Vec<char> = word
                .chars()
                .filter(|c| {
                    !c.is_whitespace()
                        && !matches!(
                            c,
                            '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2069}'
                                | '\u{FEFF}'
                        )
                })
                .collect();
            let mut faces = cosmic_text::fallback::FontFallbackIter::new(
                fonts,
                &keys,
                &families,
                &scripts,
                word,
                attrs.weight,
            );
            let Some(first) = faces.next() else {
                continue;
            };
            let mut used = Vec::new();
            let mut face = Some(first.clone());
            while let Some(font) = face.filter(|_| !missing.is_empty()) {
                let map = font.as_swash().charmap();
                let before = missing.len();
                missing.retain(|c| map.map(*c) == 0);
                if missing.len() < before {
                    used.push(font);
                }
                face = faces.next();
            }
            if used.is_empty() || !missing.is_empty() {
                used.push(first);
            }
            for font in used {
                let metrics = font.metrics();
                let scale = run.size / f32::from(metrics.units_per_em);
                ascent = ascent.max(metrics.ascent * scale);
                descent = descent.max(-metrics.descent * scale);
            }
        }
        line = line.max(area.height);
    }
    (line > 0.0).then(|| top + (line - (ascent + descent)) / 2.0 + ascent)
}

fn paragraph<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
    width: f32,
) -> Renderer::Paragraph {
    let base = renderer.default_font();
    let spans: Vec<text::Span<'_, (), iced::Font>> = leaf
        .runs
        .iter()
        .map(|run| {
            let mut span = text::Span::new(run.text.as_str())
                .size(Pixels(run.size))
                .font(font_of(base, run));
            // A line of no height never ends cosmic-text's pass over a buffer's lines, and
            // `font-size: 0` and `line-height: 0` both ask for one: such a span's line is
            // the least there is.
            let line_size = run.line_size.unwrap_or(run.size);
            let height = line_height_of(leaf).to_absolute(Pixels(line_size)).0;
            if run.line_size.is_some() || height <= 0.0 {
                let height = Pixels(height.max(f32::MIN_POSITIVE));
                span = span.line_height(text::LineHeight::Absolute(height));
            }
            if !run.visible {
                return span.color(Color::TRANSPARENT);
            }
            if let Some(c) = run.color {
                span = span.color(Color::from_rgba(c[0], c[1], c[2], run.alpha));
            }
            span
        })
        .collect();
    // cosmic-text panics on a buffer whose font size or line height is zero. Every span
    // carries its own size, so the buffer's own only have to be more than nothing.
    let size = Pixels(
        leaf.runs
            .first()
            .map_or(16.0, |run| run.size)
            .max(f32::MIN_POSITIVE),
    );
    let mut line_height = line_height_of(leaf);
    if line_height.to_absolute(size).0 <= 0.0 {
        line_height = text::LineHeight::Absolute(Pixels(f32::MIN_POSITIVE));
    }
    Renderer::Paragraph::with_spans(text::Text {
        content: spans.as_slice(),
        bounds: Size::new(width, f32::INFINITY),
        size,
        line_height,
        font: base,
        // A right-to-left paragraph aligned right draws nothing in iced 0.14, and left shifts by
        // the width; the default draws it at the corner the caller works out. Every such
        // paragraph starts with the mark `styled` puts there.
        align_x: match leaf.align {
            _ if leaf.text().starts_with('\u{200F}') => text::Alignment::Default,
            "center" => text::Alignment::Center,
            "right" => text::Alignment::Right,
            "justify" => text::Alignment::Justified,
            _ => text::Alignment::Left,
        },
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: match (leaf.wraps, leaf.breaks) {
            (false, _) => text::Wrapping::None,
            (true, TextBreaks::Words) => text::Wrapping::Word,
            (true, TextBreaks::LongWords | TextBreaks::Anywhere) => text::Wrapping::WordOrGlyph,
            (true, TextBreaks::Everywhere) => text::Wrapping::Glyph,
        },
    })
}

/// Inside list markers' images, each at the start of its run's room (its end in a
/// right-to-left paragraph), centered on the line.
fn draw_marker_images<Renderer: text::Renderer<Font = iced::Font> + BackgroundRenderer>(
    leaf: &TextLeaf,
    shaped: &Renderer::Paragraph,
    renderer: &mut Renderer,
    corner: Point,
    clip: Rectangle,
    rtl: bool,
) {
    for (index, run) in leaf.runs.iter().enumerate() {
        let Some((handle, size)) = run
            .marker_image
            .as_ref()
            .filter(|_| run.visible)
            .and_then(|marker| Some((&marker.handle, marker.shown?)))
        else {
            continue;
        };
        let Some(area) = shaped.span_bounds(index).into_iter().next() else {
            continue;
        };
        // A run's bounds start at its first glyph, in a right-to-left one the zero-width mark
        // `apply_marker_images` leaves at its right end.
        let left = if rtl { area.x - size.width } else { area.x };
        renderer.draw_background(
            handle,
            Rectangle {
                x: corner.x + left,
                y: corner.y + area.y + (area.height - size.height) / 2.0,
                width: size.width,
                height: size.height,
            },
            clip,
        );
    }
}

/// Inline backgrounds and text decorations. A paragraph only draws glyphs: these sit in
/// its run rectangles. `corner` is where the paragraph is drawn.
fn draw_run_decorations<Renderer: text::Renderer<Font = iced::Font>>(
    leaf: &TextLeaf,
    shaped: &Renderer::Paragraph,
    renderer: &mut Renderer,
    corner: Point,
) {
    let quad = |renderer: &mut Renderer, bounds: Rectangle, color: [f32; 3], alpha: f32| {
        renderer.fill_quad(
            Quad {
                bounds,
                ..Quad::default()
            },
            Color::from_rgba(color[0], color[1], color[2], alpha),
        );
    };
    for (index, run) in leaf.runs.iter().enumerate() {
        // A hidden run keeps its room inside the visible boxes around it.
        let sided = run.inline_boxes.iter().any(|sides| sides.visible);
        if !run.visible && !sided && run.background.is_none() && run.fades.is_empty() {
            continue;
        }
        let color = run.color.unwrap_or([0.0; 3]);
        // 16.3.1: a line takes the color and size of the box that set it.
        let decoration = |from: Option<(usize, [f32; 3], f32)>| {
            from.map_or((color, run.size), |(_, color, size)| (color, size))
        };
        for area in shaped.span_bounds(index) {
            let area = Rectangle {
                x: area.x + corner.x,
                y: area.y + corner.y,
                ..area
            };
            // 10.6.1: an inline box wraps the font's content area, not the taller line box.
            let (content_top, content_height) = if sided {
                let height = area.height.min(run.line_size.unwrap_or(run.size) * 1.3);
                (area.y + (area.height - height) / 2.0, height)
            } else {
                (area.y, area.height)
            };
            let content_bottom = content_top + content_height;
            for sides in run.inline_boxes.iter().filter(|sides| sides.visible) {
                let top = content_top - sides.padding[0];
                let bottom = content_bottom + sides.padding[1];
                if let Some(background) = sides.background {
                    let bounds = Rectangle {
                        y: top,
                        height: bottom - top,
                        ..area
                    };
                    quad(renderer, bounds, background, sides.alpha);
                }
                let edges = [
                    (top - sides.border[0].0, sides.border[0], 0),
                    (bottom, sides.border[1], 2),
                ];
                for (at, (height, edge), side) in edges {
                    let alpha = sides.border_alpha[side / 2];
                    if let Some(edge) = edge.filter(|_| height > 0.0) {
                        let bounds = Rectangle {
                            y: at,
                            height,
                            ..area
                        };
                        let style = sides.border_style[side / 2];
                        for (piece, shade) in band_pieces(bounds, side, style, edge) {
                            quad(renderer, piece, shade, alpha);
                        }
                    }
                }
            }
            if let Some(background) = run.background {
                let bounds = Rectangle {
                    y: content_top,
                    height: content_height,
                    ..area
                };
                quad(renderer, bounds, background, 1.0);
            }
            // Inline backgrounds fading over what shows beneath them, outermost first.
            for &(_, color, alpha) in &run.fades {
                let bounds = Rectangle {
                    y: content_top,
                    height: content_height,
                    ..area
                };
                quad(renderer, bounds, color, alpha);
            }
            if !run.visible {
                continue;
            }
            // A bar draws with its own box, the innermost one, which shows when its spaces do;
            // past them, under a negative margin, with the box's overhang below.
            if let (Some((start, width, edge)), Some(sides)) = (run.edge, run.inline_boxes.last())
                && run.overhang <= 0.0
            {
                let top = content_top - sides.padding[0] - sides.border[0].0;
                let bottom = content_bottom + sides.padding[1] + sides.border[1].0;
                let bounds = Rectangle {
                    x: if start {
                        area.x
                    } else {
                        area.x + area.width - width
                    },
                    y: top,
                    width,
                    height: bottom - top,
                };
                let side = if start { 3 } else { 1 };
                for (piece, shade) in band_pieces(bounds, side, run.edge_style, edge) {
                    quad(renderer, piece, shade, run.edge_alpha);
                }
            }
            // Line boxes may be taller than the text: measure from their centre, by size.
            let line = |offset: f32, size: f32| Rectangle {
                x: area.x,
                y: area.y + area.height / 2.0 + offset,
                width: area.width,
                height: (size / 14.0).max(1.0),
            };
            if run.underline {
                let (color, size) = decoration(run.underline_from);
                quad(renderer, line(size * 0.4, size), color, 1.0);
            }
            if run.line_through {
                let (color, size) = decoration(run.line_through_from);
                quad(renderer, line(0.0, size), color, 1.0);
            }
            if run.overline {
                let (color, size) = decoration(run.overline_from);
                quad(renderer, line(-size * 0.55, size), color, 1.0);
            }
        }
    }
    // A negative margin pulls a box past its spaces (`TextRun::overhang`): its own box draws
    // that strip outward, with its bar at the outer edge, after every run, so what follows
    // inside a box around it does not cover it.
    for (index, run) in leaf.runs.iter().enumerate() {
        let Some((key, side)) = run.side_of.filter(|_| run.overhang > 0.0) else {
            continue;
        };
        let Some(sides) = run
            .inline_boxes
            .iter()
            .rev()
            .find(|sides| sides.owner == key && sides.visible)
        else {
            continue;
        };
        // A start side's spaces come before its text: left, as the bars on a line take them
        // (right-to-left words are as approximate as their sides).
        let left = side == 3;
        for area in shaped.span_bounds(index) {
            let area = Rectangle {
                x: area.x + corner.x,
                y: area.y + corner.y,
                ..area
            };
            let height = area.height.min(run.line_size.unwrap_or(run.size) * 1.3);
            let content_top = area.y + (area.height - height) / 2.0;
            let top = content_top - sides.padding[0];
            let bottom = content_top + height + sides.padding[1];
            let strip = Rectangle {
                x: if left {
                    area.x - run.overhang
                } else {
                    area.x + area.width
                },
                width: run.overhang,
                ..area
            };
            if let Some(background) = sides.background {
                let bounds = Rectangle {
                    y: top,
                    height: bottom - top,
                    ..strip
                };
                quad(renderer, bounds, background, sides.alpha);
            }
            for (at, (thick, edge), band) in [
                (top - sides.border[0].0, sides.border[0], 0),
                (bottom, sides.border[1], 2),
            ] {
                if let Some(edge) = edge.filter(|_| thick > 0.0) {
                    let bounds = Rectangle {
                        y: at,
                        height: thick,
                        ..strip
                    };
                    let style = sides.border_style[band / 2];
                    for (piece, shade) in band_pieces(bounds, band, style, edge) {
                        quad(renderer, piece, shade, sides.border_alpha[band / 2]);
                    }
                }
            }
            if let (true, Some((_, width, edge))) = (run.visible, run.edge) {
                let bounds = Rectangle {
                    x: if left {
                        strip.x
                    } else {
                        strip.x + strip.width - width
                    },
                    y: top - sides.border[0].0,
                    width,
                    height: bottom + sides.border[1].0 - (top - sides.border[0].0),
                };
                let bar = if left { 3 } else { 1 };
                for (piece, shade) in band_pieces(bounds, bar, run.edge_style, edge) {
                    quad(renderer, piece, shade, run.edge_alpha);
                }
            }
        }
    }
}

/// Whether text's lines can be split apart. Forced breaks give several buffer lines, whose
/// offsets do not add up. Bidirectional text reorders per line and its second part would
/// re-detect its base direction.
fn splittable(text: &str) -> bool {
    !forces_break(text)
        && !text.chars().any(|character| {
            matches!(
                character,
                '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}'
                | '\u{590}'..='\u{8FF}'
                | '\u{FB1D}'..='\u{FDFF}'
                | '\u{FE70}'..='\u{FEFF}'
                | '\u{10800}'..='\u{10FFF}'
                | '\u{1E800}'..='\u{1EFFF}'
            )
        })
}

/// Whether text holds a forced line break.
fn forces_break(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character,
            '\n' | '\r' | '\x0B' | '\x0C' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
    })
}

/// Splits a leaf where its lines at `narrow` first start at or below `until`, for text
/// that widens past a float. Returns the two parts and the first part's height.
fn split_leaf<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
    narrow: f32,
    until: f32,
) -> Option<(TextLeaf, TextLeaf, f32)> {
    let whole = leaf.text();
    if !splittable(&whole) {
        return None;
    }
    // Lines start at x = 0 only when left aligned, so scan an aligned-left copy. The same
    // line breaks result whatever the alignment. The band's position can differ by a float
    // rounding between layout and paint, which must not move the cut across a line.
    let until = (until * 4.0).round() / 4.0;
    let scanning = TextLeaf {
        align: "left",
        ..leaf.clone()
    };
    let shaped = paragraph::<Renderer>(renderer, &scanning, narrow);
    let height = shaped.min_bounds().height;
    let offset_at = |y: f32| {
        shaped
            .hit_test(Point::new(0.0, y))
            .map(|text::Hit::CharOffset(offset)| offset)
    };
    // The line holding the point just above `until` still runs beside the float.
    let start = offset_at((until - 0.5).max(0.0))?;
    let mut y = until.max(0.0);
    let cut = loop {
        if y >= height {
            return None;
        }
        let here = offset_at(y)?;
        if here != start {
            break here;
        }
        y += 1.0;
    };
    let (first, second) = cut_leaf(leaf, cut)?;
    let first_height = paragraph::<Renderer>(renderer, &first, narrow)
        .min_bounds()
        .height;
    Some((first, second, first_height))
}

/// A leaf's runs before and after the byte offset `cut` into its text, `None` unless it
/// falls on a character boundary inside it.
fn cut_leaf(leaf: &TextLeaf, cut: usize) -> Option<(TextLeaf, TextLeaf)> {
    let whole = leaf.text();
    if cut == 0 || cut >= whole.len() || !whole.is_char_boundary(cut) {
        return None;
    }
    let (mut first, mut second) = (Vec::new(), Vec::new());
    let mut seen = 0;
    for run in &leaf.runs {
        let end = seen + run.text.len();
        if end <= cut {
            first.push(run.clone());
        } else if seen >= cut {
            second.push(run.clone());
        } else {
            let at = cut - seen;
            first.push(TextRun {
                text: run.text[..at].to_owned(),
                ..run.clone()
            });
            second.push(TextRun {
                text: run.text[at..].to_owned(),
                ..run.clone()
            });
        }
        seen = end;
    }
    let first = TextLeaf {
        runs: first,
        ..leaf.clone()
    };
    let second = TextLeaf {
        runs: second,
        ..leaf.clone()
    };
    Some((first, second))
}

/// Splits a leaf after its first line at `wide`, whatever the direction of its text: the
/// lines hold consecutive text, so the second starts at the lowest offset on it. The second
/// part opens with the paragraph's direction mark too. `None` for a single line.
fn split_first_line<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
    wide: f32,
) -> Option<(TextLeaf, TextLeaf, f32)> {
    // Every line starts at x = 0 when aligned left; the breaks are the same.
    let scanning = TextLeaf {
        align: "left",
        ..leaf.clone()
    };
    let shaped = paragraph::<Renderer>(renderer, &scanning, wide);
    let height = shaped.min_bounds().height;
    let offset_at = |x: f32, y: f32| {
        shaped
            .hit_test(Point::new(x, y))
            .map(|text::Hit::CharOffset(offset)| offset)
    };
    let start = offset_at(0.0, 0.5)?;
    let mut y = 1.0;
    while offset_at(0.0, y)? == start {
        y += 1.0;
        if y >= height {
            return None;
        }
    }
    let mut cut = usize::MAX;
    let mut x = 0.0;
    while x <= wide {
        if let Some(offset) = offset_at(x, y) {
            cut = cut.min(offset);
        }
        x += 1.0;
    }
    let (first, mut second) = cut_leaf(leaf, cut)?;
    let mark = leaf
        .text()
        .chars()
        .next()
        .filter(|mark| matches!(mark, '\u{200E}' | '\u{200F}'));
    if let (Some(mark), Some(run)) = (
        mark,
        second.runs.iter_mut().find(|run| !run.text.is_empty()),
    ) {
        run.text.insert(0, mark);
    }
    let first_height = paragraph::<Renderer>(renderer, &first, wide)
        .min_bounds()
        .height;
    Some((first, second, first_height))
}

/// A leaf laid out a paragraph at a time where one paragraph would not do, top to bottom,
/// each piece with the width it lays out in: a hanging first line (`TextLeaf::hang`), and
/// each line of right-to-left text aligned left or centered, as iced aligns such a
/// paragraph's lines right within its widest. Forced breaks part it first, as the shaper
/// parts its lines. `None` when one paragraph does.
fn pieces<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
    width: f32,
) -> Option<Vec<(TextLeaf, f32)>> {
    use iced::advanced::graphics::text::cosmic_text::BidiParagraphs;
    let text = leaf.text();
    let per_line = text.starts_with('\u{200F}') && matches!(leaf.align, "left" | "center");
    if (leaf.hang <= 0.0 && !per_line) || leaf.is_empty() {
        return None;
    }
    let start = text.as_ptr() as usize;
    let mut pieces = Vec::new();
    for (index, part) in BidiParagraphs::new(&text).enumerate() {
        let from = part.as_ptr() as usize - start;
        let mut rest = slice_leaf(leaf, from..from + part.len());
        let hangs = index == 0 && leaf.hang > 0.0;
        let mut room = if hangs {
            (width + leaf.hang).ceil()
        } else {
            width
        };
        if hangs || per_line {
            while let Some((mut line, after, _)) = split_first_line(renderer, &rest, room) {
                // 16.6.1: spaces at the end of a line are removed; in its own paragraph a
                // right-to-left line's last space would show at its visual start, the left.
                if per_line
                    && let Some(last) = line.runs.iter_mut().rev().find(|run| !run.text.is_empty())
                {
                    let kept = last.text.trim_end_matches(' ').len();
                    last.text.truncate(kept);
                }
                pieces.push((line, room));
                rest = after;
                room = width;
                if !per_line {
                    break;
                }
            }
        }
        pieces.push((rest, room));
    }
    Some(pieces)
}

/// A line too long for `width`, cut where it and a "…" after it fit, or `None` when the
/// whole line fits. With no room even for one character, only the "…" shows.
fn ellipsized<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
    width: f32,
) -> Option<TextLeaf> {
    let fits = |leaf: &TextLeaf| {
        paragraph::<Renderer>(renderer, leaf, f32::INFINITY)
            .min_bounds()
            .width
            <= width
    };
    if fits(leaf) {
        return None;
    }
    let text = leaf.text();
    let ends: Vec<usize> = text.char_indices().map(|(at, _)| at).collect();
    // The leaf cut before byte `end`, with the "…" on the last run that keeps text.
    let cut = |end: usize| {
        let mut cut = slice_leaf(leaf, 0..end);
        let last = cut
            .runs
            .iter()
            .rposition(|run| !run.text.is_empty())
            .unwrap_or(0);
        if let Some(run) = cut.runs.get_mut(last) {
            run.text.push('\u{2026}');
        }
        cut
    };
    // The most characters that fit, found by halving.
    let (mut low, mut high) = (0usize, ends.len());
    while low < high {
        let middle = (low + high).div_ceil(2);
        if fits(&cut(ends[middle - 1]
            + text[ends[middle - 1]..]
                .chars()
                .next()
                .map_or(0, char::len_utf8)))
        {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let end = if low == 0 {
        0
    } else {
        ends[low - 1]
            + text[ends[low - 1]..]
                .chars()
                .next()
                .map_or(0, char::len_utf8)
    };
    Some(cut(end))
}

/// A leaf over the byte range `range` of its text: every run, cut to it. The shaper skips
/// empty runs, and keeping them keeps each span's index the run's in the whole leaf.
fn slice_leaf(leaf: &TextLeaf, range: std::ops::Range<usize>) -> TextLeaf {
    let mut seen = 0;
    let runs = leaf
        .runs
        .iter()
        .map(|run| {
            let (begin, end) = (seen, seen + run.text.len());
            seen = end;
            let from = range.start.clamp(begin, end) - begin;
            let to = range.end.clamp(begin, end) - begin;
            TextRun {
                text: run.text[from..to.max(from)].to_owned(),
                ..run.clone()
            }
        })
        .collect();
    TextLeaf {
        runs,
        ..leaf.clone()
    }
}

/// A leaf's height at `width`, laid out as [`pieces`] lays it out.
fn leaf_height<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
    width: f32,
) -> f32 {
    match pieces(renderer, leaf, width) {
        Some(pieces) => pieces
            .iter()
            .map(|(piece, room)| {
                paragraph::<Renderer>(renderer, piece, *room)
                    .min_bounds()
                    .height
            })
            .sum(),
        None => {
            paragraph::<Renderer>(renderer, leaf, width)
                .min_bounds()
                .height
        }
    }
}

/// Leaves' baselines already asked for in a layout pass: by arena slot, width and float
/// split (`leaf_baseline`), as bits.
type Baselines = BTreeMap<(usize, u32, Option<(u32, u32)>), Option<f32>>;

/// [`last_baseline`] of a leaf at `width`, laid out as [`pieces`] lays it out, or, past a
/// shorter float `until` below its top, as `wide` from there (`split`), as paint lays its
/// rest out.
fn leaf_baseline<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
    width: f32,
    split: Option<(f32, f32)>,
) -> Option<f32> {
    if let Some((until, wide)) = split
        && let Some((_, rest, first_height)) = split_leaf(renderer, leaf, width, until)
    {
        return last_baseline(renderer, &rest, &paragraph(renderer, &rest, wide))
            .map(|baseline| first_height + baseline);
    }
    let Some(pieces) = pieces(renderer, leaf, width) else {
        return last_baseline(renderer, leaf, &paragraph(renderer, leaf, width));
    };
    let ((last, room), above) = pieces.split_last()?;
    let above: f32 = above
        .iter()
        .map(|(piece, room)| {
            paragraph::<Renderer>(renderer, piece, *room)
                .min_bounds()
                .height
        })
        .sum();
    last_baseline(renderer, last, &paragraph(renderer, last, *room))
        .map(|baseline| above + baseline)
}

/// The narrowest width a leaf can wrap to: its widest single word.
fn narrowest_width<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
) -> f32 {
    // Text that will not break is as narrow as it is long.
    if !leaf.wraps {
        return paragraph(renderer, leaf, f32::INFINITY).min_bounds().width;
    }
    let mut widest = 0.0_f32;
    // A word can run across runs: `<b>a</b>b`, or the sizes of small capitals.
    let mut word: Vec<TextRun> = Vec::new();
    let mut flush = |word: &mut Vec<TextRun>| {
        if !word.is_empty() {
            let single = TextLeaf {
                runs: std::mem::take(word),
                ..leaf.clone()
            };
            widest = widest.max(
                paragraph(renderer, &single, f32::INFINITY)
                    .min_bounds()
                    .width,
            );
        }
    };
    // The spaces that hold an indent or an inline box's sides stay with their word.
    for run in &leaf.runs {
        if run.spacer {
            word.push(run.clone());
            continue;
        }
        // A no-break space joins its neighbours, so only breaking white space ends a word.
        let breaks =
            |c: char| c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{2007}' | '\u{202f}');
        // Where a line may break between any two characters, each is a word of its own.
        if matches!(leaf.breaks, TextBreaks::Anywhere | TextBreaks::Everywhere) {
            for c in run.text.chars() {
                if !breaks(c) {
                    word.push(TextRun {
                        text: c.to_string(),
                        ..run.clone()
                    });
                }
                flush(&mut word);
            }
            continue;
        }
        for chunk in run.text.split_inclusive(breaks) {
            let body = chunk.trim_end_matches(breaks);
            if !body.is_empty() {
                word.push(TextRun {
                    text: body.to_owned(),
                    ..run.clone()
                });
            }
            if body.len() != chunk.len() {
                flush(&mut word);
            }
        }
    }
    flush(&mut word);
    widest
}

fn rectangle(left: f32, right: f32, top: f32, bottom: f32) -> Rectangle {
    Rectangle {
        x: left,
        y: top,
        width: (right - left).max(0.0),
        height: (bottom - top).max(0.0),
    }
}

/// The four border edge bands, top, right, bottom, left. Sides may differ in color.
fn border_bands(border: FlowRect, padding: FlowRect) -> [Rectangle; 4] {
    [
        rectangle(border.left, border.right, border.top, padding.top),
        rectangle(padding.right, border.right, padding.top, padding.bottom),
        rectangle(border.left, border.right, padding.bottom, border.bottom),
        rectangle(border.left, padding.left, padding.top, padding.bottom),
    ]
}

/// Which of the four sides between `border` and `padding` draw `double` as two lines.
fn double_sides(border: FlowRect, padding: FlowRect, styles: [&str; 4]) -> [bool; 4] {
    let thick = [
        padding.top - border.top,
        border.right - padding.right,
        border.bottom - padding.bottom,
        padding.left - border.left,
    ];
    std::array::from_fn(|side| styles[side] == "double" && thick[side] >= 3.0)
}

/// Where two `double` sides meet, their lines join into two rings, as `band_pieces` alone
/// cannot: the outer line of the left and right sides runs on to the outer line of the top
/// and bottom, and the inner line of the top and bottom stops at theirs. `pieces` are one
/// side's two lines, outer first; `doubles` from `double_sides`.
fn join_double(
    pieces: &mut [(Rectangle, [f32; 3])],
    side: usize,
    border: FlowRect,
    padding: FlowRect,
    doubles: [bool; 4],
) {
    if !doubles[side] || pieces.len() != 2 {
        return;
    }
    let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
    if side == 0 || side == 2 {
        let inner = &mut pieces[1].0;
        let (left, right) = (padding.left - border.left, border.right - padding.right);
        if doubles[1] && near(inner.x + inner.width, border.right) {
            inner.width = (inner.width - right * 2.0 / 3.0).max(0.0);
        }
        if doubles[3] && near(inner.x, border.left) {
            let cut = (left * 2.0 / 3.0).min(inner.width);
            inner.x += cut;
            inner.width -= cut;
        }
    } else {
        let outer = &mut pieces[0].0;
        let (top, bottom) = (padding.top - border.top, border.bottom - padding.bottom);
        if doubles[2] && near(outer.y + outer.height, padding.bottom) {
            outer.height += bottom * 2.0 / 3.0;
        }
        if doubles[0] && near(outer.y, padding.top) {
            outer.y -= top * 2.0 / 3.0;
            outer.height += top * 2.0 / 3.0;
        }
    }
}

/// The rectangles that draw one side of a border or outline in its 8.5.3 style. The spec
/// leaves the exact pattern and shades to the renderer. Side is top, right, bottom, left.
fn band_pieces(
    band: Rectangle,
    side: usize,
    style: &str,
    color: [f32; 3],
) -> Vec<(Rectangle, [f32; 3])> {
    let across = side == 0 || side == 2;
    let (thick, long) = if across {
        (band.height, band.width)
    } else {
        (band.width, band.height)
    };
    if thick <= 0.0 || long <= 0.0 || matches!(style, "none" | "hidden") {
        return Vec::new();
    }
    // Under half a pixel a pattern cannot show, and an animation passes through such widths;
    // past this many pieces it would only cost time. Both draw as a plain line.
    const MOST_PIECES: f32 = 20_000.0;
    let plain = thick < 0.5
        || !long.is_finite()
        || (style == "dotted" && long / (thick * 2.0) > MOST_PIECES)
        || (style == "dashed" && long / (thick * 5.0) > MOST_PIECES);
    if plain && matches!(style, "dotted" | "dashed") {
        return vec![(band, color)];
    }
    // A piece at `along` on the side and `depth` in from its outer edge.
    let outer_first = side == 0 || side == 3;
    let piece = |along: f32, length: f32, depth: f32, size: f32| {
        let inset = if outer_first {
            depth
        } else {
            thick - depth - size
        };
        if across {
            Rectangle::new(
                Point::new(band.x + along, band.y + inset),
                Size::new(length, size),
            )
        } else {
            Rectangle::new(
                Point::new(band.x + inset, band.y + along),
                Size::new(size, length),
            )
        }
    };
    let dark = color.map(|channel| channel * 0.6);
    let light = color.map(|channel| channel + (1.0 - channel) * 0.35);
    let repeat = |dash: f32, gap: f32| {
        // Counted, not stepped: adding a small step to a large coordinate can change nothing.
        let step = dash + gap;
        (0..(long / step).ceil() as usize)
            .map(|at| {
                let along = at as f32 * step;
                (piece(along, dash.min(long - along), 0.0, thick), color)
            })
            .collect::<Vec<_>>()
    };
    let halves = |outside: [f32; 3], inside: [f32; 3]| {
        vec![
            (piece(0.0, long, 0.0, thick / 2.0), outside),
            (piece(0.0, long, thick / 2.0, thick / 2.0), inside),
        ]
    };
    match style {
        "dashed" => repeat(thick * 3.0, thick * 2.0),
        "dotted" => repeat(thick, thick),
        // 8.5.3: two lines and the space between add up to the width.
        "double" if thick >= 3.0 => vec![
            (piece(0.0, long, 0.0, thick / 3.0), color),
            (piece(0.0, long, thick * 2.0 / 3.0, thick / 3.0), color),
        ],
        "groove" => halves(dark, light),
        "ridge" => halves(light, dark),
        "inset" => vec![(band, if outer_first { dark } else { light })],
        "outset" => vec![(band, if outer_first { light } else { dark })],
        _ => vec![(band, color)],
    }
}

/// What a renderer needs to draw background images. iced only provides this with its
/// `image` feature, so without this crate's `image` or `image-without-codecs` feature they
/// draw nothing.
pub trait BackgroundRenderer {
    /// Decodes the image if it has not been, blocking. iced frees an image once nothing
    /// holds its allocation, so the caller keeps this as long as it draws the image.
    fn load_background(&self, handle: &image::Handle) -> Option<image::Allocation>;
    /// The image's size in pixels, once loaded.
    fn background_size(&self, handle: &image::Handle) -> Option<(f32, f32)>;
    /// Draws the image in `bounds`, clipped to `clip`.
    fn draw_background(&mut self, handle: &image::Handle, bounds: Rectangle, clip: Rectangle);
}

#[cfg(feature = "image-without-codecs")]
impl<Renderer: image::Renderer<Handle = image::Handle>> BackgroundRenderer for Renderer {
    fn load_background(&self, handle: &image::Handle) -> Option<image::Allocation> {
        self.load_image(handle).ok()
    }
    fn background_size(&self, handle: &image::Handle) -> Option<(f32, f32)> {
        self.measure_image(handle)
            .map(|size| (size.width as f32, size.height as f32))
    }
    fn draw_background(&mut self, handle: &image::Handle, bounds: Rectangle, clip: Rectangle) {
        self.draw_image(image::Image::new(handle.clone()), bounds, clip);
    }
}

#[cfg(not(feature = "image-without-codecs"))]
impl<Renderer> BackgroundRenderer for Renderer {
    fn load_background(&self, _handle: &image::Handle) -> Option<image::Allocation> {
        None
    }
    fn background_size(&self, _handle: &image::Handle) -> Option<(f32, f32)> {
        None
    }
    fn draw_background(&mut self, _handle: &image::Handle, _bounds: Rectangle, _clip: Rectangle) {}
}

fn moved_rectangle(rect: FlowRect, origin: Point) -> Rectangle {
    let moved = rectangle(rect.left, rect.right, rect.top, rect.bottom);
    Rectangle {
        x: moved.x + origin.x,
        y: moved.y + origin.y,
        ..moved
    }
}

/// Whether `next` needs a new layer after `step`: a layer paints images and then text over
/// all its quads. Text may follow text, but anything after an image needs a fresh layer,
/// since text brings quads of its own for backgrounds and decorations.
fn breaks_layer(tree: &BoxTree, step: PaintStep, next: PaintStep) -> bool {
    match step {
        PaintStep::Marker(index) if tree.marker_images.contains_key(&index) => true,
        PaintStep::Text(arena)
            if tree.texts[arena].as_ref().is_some_and(|leaf| {
                leaf.runs
                    .iter()
                    .any(|run| run.marker_image.as_ref().is_some_and(|m| m.shown.is_some()))
            }) =>
        {
            true
        }
        PaintStep::Text(_) | PaintStep::Marker(_) => {
            !matches!(next, PaintStep::Text(_) | PaintStep::Marker(_))
        }
        PaintStep::Box(index) => tree.backgrounds.contains_key(&index),
        PaintStep::Outline(_)
        | PaintStep::CellBorder(_)
        | PaintStep::Column(_)
        | PaintStep::ColumnGroup(_)
        | PaintStep::Generated(_) => false,
    }
}

/// A background image in `area`, placed against `positioning` (14.2.1).
fn draw_background_image<Renderer: BackgroundRenderer>(
    renderer: &mut Renderer,
    background: &(BackgroundImage, image::Handle),
    positioning: Rectangle,
    area: Rectangle,
) {
    let (value, handle) = background;
    let Some((natural_width, natural_height)) = renderer.background_size(handle) else {
        return;
    };
    if natural_width <= 0.0 || natural_height <= 0.0 || area.width <= 0.0 || area.height <= 0.0 {
        return;
    }
    // CSS Backgrounds 3: `background-size`; an `auto` side keeps the image's proportions.
    let length = |offset: BackgroundOffset, room: f32| match offset {
        BackgroundOffset::Px(pixels) => pixels,
        BackgroundOffset::Percent(percent) => room * percent / 100.0,
    };
    let ratio = natural_width / natural_height;
    let (width, height) = match value.size {
        BackgroundSize::Cover | BackgroundSize::Contain => {
            let across = positioning.width / natural_width;
            let down = positioning.height / natural_height;
            let scale = if value.size == BackgroundSize::Cover {
                across.max(down)
            } else {
                across.min(down)
            };
            (natural_width * scale, natural_height * scale)
        }
        BackgroundSize::Sized(None, None) => (natural_width, natural_height),
        BackgroundSize::Sized(Some(across), None) => {
            let width = length(across, positioning.width);
            (width, width / ratio)
        }
        BackgroundSize::Sized(None, Some(down)) => {
            let height = length(down, positioning.height);
            (height * ratio, height)
        }
        BackgroundSize::Sized(Some(across), Some(down)) => (
            length(across, positioning.width),
            length(down, positioning.height),
        ),
    };
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let place = |offset: BackgroundOffset, start: f32, room: f32, extent: f32| match offset {
        BackgroundOffset::Px(pixels) => start + pixels,
        BackgroundOffset::Percent(percent) => start + (room - extent) * percent / 100.0,
    };
    let x0 = place(value.position[0], positioning.x, positioning.width, width);
    let y0 = place(value.position[1], positioning.y, positioning.height, height);
    // Tiles run from the first one reaching the area to the last one inside it.
    let tiles = |origin: f32, extent: f32, start: f32, room: f32, repeat: bool| {
        if repeat {
            let first = origin - ((origin - start) / extent).ceil() * extent;
            let count = ((start + room - first) / extent).ceil() as usize;
            (first, count)
        } else {
            (origin, 1)
        }
    };
    let (first_x, columns) = tiles(x0, width, area.x, area.width, value.repeat_x);
    let (first_y, rows) = tiles(y0, height, area.y, area.height, value.repeat_y);
    // `area` is already clipped to what can show. One pixel over a 4K window still draws.
    if columns.saturating_mul(rows) > 16_000_000 {
        return;
    }
    for row in 0..rows {
        for column in 0..columns {
            let bounds = Rectangle {
                x: first_x + column as f32 * width,
                y: first_y + row as f32 * height,
                width,
                height,
            };
            if bounds.intersects(&area) {
                renderer.draw_background(handle, bounds, area);
            }
        }
    }
}

/// One box's outline, four bands just outside its border edge (18.4).
fn draw_outline<Renderer: renderer::Renderer>(
    tree: &BoxTree,
    geometry: &[Option<FlowGeometry>],
    renderer: &mut Renderer,
    origin: Point,
    index: usize,
) {
    let (Some(Some(geometry)), Some((width, color))) =
        (geometry.get(index), tree.paints[index].outline)
    else {
        return;
    };
    let inner = geometry.border;
    let outer = FlowRect {
        left: inner.left - width,
        right: inner.right + width,
        top: inner.top - width,
        bottom: inner.bottom + width,
    };
    let style = tree.paints[index].outline_style;
    let doubles = double_sides(outer, inner, [style; 4]);
    for (side, band) in border_bands(outer, inner).into_iter().enumerate() {
        let mut pieces = band_pieces(band, side, style, color);
        join_double(&mut pieces, side, outer, inner, doubles);
        for (piece, shade) in pieces {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: piece.x + origin.x,
                        y: piece.y + origin.y,
                        ..piece
                    },
                    ..Quad::default()
                },
                Color::from_rgb(shade[0], shade[1], shade[2]),
            );
        }
    }
}

/// 17.5.1: each recorded column's area and its column group's, across the cells that sit
/// wholly inside the column or group and down the table's rows. Arena-slot geometry, so
/// anonymous cells count as cells.
fn column_rects(
    prepared: &PreparedTree,
    slot_geometry: &[FlowGeometry],
) -> Vec<(Option<Rectangle>, Option<Rectangle>)> {
    use iced_css_core::formatting::block_flow::{table_grid, table_parts};
    let columns = &prepared.table_columns;
    let mut rects = vec![(None, None); columns.len()];
    let mut done = vec![false; columns.len()];
    for first in 0..columns.len() {
        if done[first] {
            continue;
        }
        let table = columns[first].0;
        let (rows, _) = table_parts(&prepared.boxes, table);
        let (placed, _) = table_grid(&prepared.boxes, table, &rows);
        let (mut top, mut bottom) = (f32::INFINITY, f32::NEG_INFINITY);
        for item in &placed {
            if let Some(cell) = slot_geometry.get(item.cell) {
                top = top.min(cell.border.top);
                bottom = bottom.max(cell.border.bottom);
            }
        }
        // The area across the cells wholly inside columns `start..end`.
        let across = |start: usize, end: usize| {
            let (mut left, mut right) = (f32::INFINITY, f32::NEG_INFINITY);
            for item in placed
                .iter()
                .filter(|item| item.column >= start && item.column + item.col_span <= end)
            {
                if let Some(cell) = slot_geometry.get(item.cell) {
                    left = left.min(cell.border.left);
                    right = right.max(cell.border.right);
                }
            }
            (left < right && top < bottom).then_some(Rectangle {
                x: left,
                y: top,
                width: right - left,
                height: bottom - top,
            })
        };
        let members: Vec<usize> = (first..columns.len())
            .filter(|at| columns[*at].0 == table)
            .collect();
        for (column, &at) in members.iter().enumerate() {
            done[at] = true;
            let group = columns[at].2.map(|group| {
                let start = members
                    .iter()
                    .position(|other| columns[*other].2 == Some(group))
                    .unwrap_or(column);
                let end = members
                    .iter()
                    .rposition(|other| columns[*other].2 == Some(group))
                    .map_or(column + 1, |last| last + 1);
                across(start, end)
            });
            rects[at] = (across(column, column + 1), group.flatten());
        }
    }
    rects
}

/// Draws a column's background color, or its group's, over its area from [`column_rects`].
fn draw_column<Renderer: renderer::Renderer>(
    tree: &BoxTree,
    rects: &[(Option<Rectangle>, Option<Rectangle>)],
    renderer: &mut Renderer,
    origin: Point,
    at: usize,
    group: bool,
) {
    let Some(&(_, column, group_owner)) = tree.prepared.table_columns.get(at) else {
        return;
    };
    let (owner, area) = match (group, rects.get(at)) {
        (false, Some((area, _))) => (column, area),
        (true, Some((_, area))) => match group_owner {
            Some(owner) => (owner, area),
            None => return,
        },
        _ => return,
    };
    let paint = tree.paints[owner];
    let (Some(color), Some(area)) = (paint.background.filter(|_| paint.visible), area) else {
        return;
    };
    renderer.fill_quad(
        Quad {
            bounds: Rectangle {
                x: origin.x + area.x,
                y: origin.y + area.y,
                ..*area
            },
            ..Quad::default()
        },
        Color::from_rgb(color[0], color[1], color[2]),
    );
}

/// One of a box's three areas.
fn area_rect(geometry: &FlowGeometry, area: BoxArea) -> FlowRect {
    match area {
        BoxArea::Border => geometry.border,
        BoxArea::Padding => geometry.padding,
        BoxArea::Content => geometry.content,
    }
}

/// The corner radii of a box's inner area: each outer radius less the larger of the two
/// distances in from the border edge at that corner.
fn inner_radii(
    radius: iced::border::Radius,
    outer: FlowRect,
    inner: FlowRect,
) -> iced::border::Radius {
    let [top, right, bottom, left] = [
        inner.top - outer.top,
        outer.right - inner.right,
        outer.bottom - inner.bottom,
        inner.left - outer.left,
    ];
    let less = |radius: f32, a: f32, b: f32| (radius - a.max(b)).max(0.0);
    iced::border::Radius {
        top_left: less(radius.top_left, left, top),
        top_right: less(radius.top_right, top, right),
        bottom_right: less(radius.bottom_right, right, bottom),
        bottom_left: less(radius.bottom_left, bottom, left),
    }
}

/// CSS Images 3, 3.1: a gradient over a `width` by `height` box as iced draws one. Stops
/// without a place share the room between their neighbours; a stop before another is
/// moved up to it. iced's gradient line is twice the larger of the box's half-extents
/// along the angle, CSS's the distance between the corners' projections onto it: places
/// are moved from one to the other, and what falls beyond iced's ends is cut there, so
/// the corners past iced's ends show the end colors. At most eight stops show.
fn linear_gradient(
    gradient: &GradientPaint,
    width: f32,
    height: f32,
    through: f32,
) -> Option<iced::gradient::Linear> {
    use std::f32::consts::{FRAC_PI_2, PI};
    if width <= 0.0 || height <= 0.0 || gradient.stops.len() < 2 {
        return None;
    }
    let angle = match gradient.aim {
        GradientAim::Angle(angle) => angle,
        GradientAim::Toward(0, -1) => 0.0,
        GradientAim::Toward(1, 0) => FRAC_PI_2,
        GradientAim::Toward(0, _) => PI,
        GradientAim::Toward(-1, 0) => 3.0 * FRAC_PI_2,
        // A corner: the line meeting the other two corners lies across the middle.
        GradientAim::Toward(across, down) => {
            f32::atan2(f32::from(across) * height, -f32::from(down) * width)
        }
    };
    let (sin, cos) = angle.sin_cos();
    let css = (width * sin).abs() + (height * cos).abs();
    let iced = (width * sin.abs()).max(height * cos.abs());
    if css <= 0.0 || iced <= 0.0 {
        return None;
    }
    // Places along the CSS line, from 0 to 1.
    let count = gradient.stops.len();
    let mut places: Vec<Option<f32>> = gradient
        .stops
        .iter()
        .map(|(_, _, position)| {
            position.map(|position| match position {
                Dim::Px(pixels) => pixels / css,
                Dim::Percent(percent) => percent / 100.0,
                Dim::Mix(pixels, percent) | Dim::Clamped(pixels, percent) => {
                    pixels / css + percent / 100.0
                }
                Dim::Auto => 0.0,
            })
        })
        .collect();
    places[0] = places[0].or(Some(0.0));
    places[count - 1] = places[count - 1].or(Some(1.0));
    let mut highest = f32::NEG_INFINITY;
    for place in places.iter_mut().flatten() {
        highest = highest.max(*place);
        *place = highest;
    }
    let mut at = 0;
    while at < count {
        if places[at].is_some() {
            at += 1;
            continue;
        }
        let before = places[at - 1].expect("placed");
        let next = (at..count)
            .find(|index| places[*index].is_some())
            .expect("last placed");
        let after = places[next].expect("placed");
        let gaps = (next - at + 1) as f32;
        for (step, place) in places[at..next].iter_mut().enumerate() {
            *place = Some(before + (after - before) * (step + 1) as f32 / gaps);
        }
        at = next;
    }
    let color = |(rgb, alpha, _): &([f32; 3], f32, Option<Dim>)| [rgb[0], rgb[1], rgb[2], *alpha];
    let stops: Vec<(f32, [f32; 4])> = places
        .iter()
        .zip(&gradient.stops)
        .map(|(place, stop)| {
            (
                ((place.expect("placed") - 0.5) * css) / iced + 0.5,
                color(stop),
            )
        })
        .collect();
    // The color at `u` along iced's line.
    let sample = |u: f32| -> [f32; 4] {
        match stops.iter().position(|stop| stop.0 >= u) {
            None => stops[count - 1].1,
            Some(0) => stops[0].1,
            Some(next) => {
                let (a, b) = (stops[next - 1], stops[next]);
                let span = b.0 - a.0;
                let t = if span > 0.0 { (u - a.0) / span } else { 1.0 };
                std::array::from_fn(|channel| a.1[channel] + (b.1[channel] - a.1[channel]) * t)
            }
        }
    };
    let mut shown: Vec<(f32, [f32; 4])> = Vec::new();
    if stops[0].0 > 0.0 || stops[count - 1].0 < 0.0 {
        shown.push((0.0, sample(0.0)));
    }
    shown.extend(
        stops
            .iter()
            .copied()
            .filter(|stop| (0.0..=1.0).contains(&stop.0)),
    );
    if stops[count - 1].0 < 1.0 || stops[0].0 > 1.0 {
        shown.push((1.0, sample(1.0)));
    }
    if shown.first().is_none_or(|first| first.0 > 0.0) {
        shown.insert(0, (0.0, sample(0.0)));
    }
    if shown.last().is_some_and(|last| last.0 < 1.0) {
        shown.push((1.0, sample(1.0)));
    }
    // Eight at most: the ends, and those between spread evenly.
    if shown.len() > 8 {
        let last = shown.len() - 1;
        shown = (0..8).map(|pick| shown[pick * last / 7]).collect();
    }
    let mut linear = iced::gradient::Linear::new(iced::Radians(angle));
    for (offset, [r, g, b, a]) in shown {
        linear = linear.add_stop(
            offset.clamp(0.0, 1.0),
            Color::from_rgba(r, g, b, a * through),
        );
    }
    Some(linear)
}

/// The arena slot a paint step draws, for its transform.
fn step_arena(prepared: &PreparedTree, step: PaintStep) -> Option<usize> {
    match step {
        PaintStep::Box(index)
        | PaintStep::Outline(index)
        | PaintStep::Marker(index)
        | PaintStep::CellBorder(index) => prepared.slot.get(index).copied().flatten(),
        PaintStep::Text(arena) => Some(arena),
        PaintStep::Generated(at) => prepared.generated_boxes.get(at).map(|(arena, _)| *arena),
        PaintStep::Column(_) | PaintStep::ColumnGroup(_) => None,
    }
}

/// CSS Transforms 1: an element's own transform about its origin, for a border box at
/// `border`, when iced can draw it: translations and equal positive scalings.
fn local_transformation(
    transform: &ElementTransform,
    border: Rectangle,
) -> Option<iced::Transformation> {
    let resolve = |dim: Dim, basis: f32| match dim {
        Dim::Px(pixels) => pixels,
        Dim::Percent(percent) => basis * percent / 100.0,
        Dim::Mix(pixels, percent) | Dim::Clamped(pixels, percent) => {
            pixels + basis * percent / 100.0
        }
        Dim::Auto => 0.0,
    };
    let (width, height) = (border.width, border.height);
    let origin = (
        border.x + resolve(transform.origin[0], width),
        border.y + resolve(transform.origin[1], height),
    );
    let mut matrix = iced::Transformation::translate(origin.0, origin.1);
    for step in &transform.steps {
        matrix = matrix
            * match *step {
                TransformStep::Translate(across, down) => {
                    iced::Transformation::translate(resolve(across, width), resolve(down, height))
                }
                TransformStep::Scale(across, down)
                    if (across - down).abs() <= f32::EPSILON && across > 0.0 =>
                {
                    iced::Transformation::scale(across)
                }
                TransformStep::Scale(..) => return None,
            };
    }
    Some(matrix * iced::Transformation::translate(-origin.0, -origin.1))
}

/// Which of a box's quads to draw. An image goes between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoxParts {
    /// The borders of a collapsed table's cell, in their later step.
    CellBorder,
    All,
    Background,
    Border,
}

/// One box's background and border bands, offset to the layout origin.
fn draw_box<Renderer: renderer::Renderer>(
    tree: &BoxTree,
    geometry: &[Option<FlowGeometry>],
    renderer: &mut Renderer,
    origin: Point,
    index: usize,
    parts: BoxParts,
) {
    let Some(Some(here)) = geometry.get(index) else {
        return;
    };
    // The stretches of its sides a neighbour's border won.
    let prepared = &tree.prepared;
    let arena = prepared.slot[index];
    let own = |cell: usize| Some(cell) == arena;
    let from = prepared
        .border_cuts
        .partition_point(|cut| arena.is_some_and(|arena| cut.0 < arena));
    let cuts: Vec<(usize, f32, f32)> = prepared.border_cuts[from..]
        .iter()
        .take_while(|cut| own(cut.0))
        .filter_map(|&(_, side, winner)| {
            let border = geometry.get(prepared.positions[winner].0)?.as_ref()?.border;
            Some(if side == 0 || side == 2 {
                (side, border.left, border.right)
            } else {
                (side, border.top, border.bottom)
            })
        })
        .collect();
    // A side widened to clear a neighbour draws its own width, at its outer edge.
    let mut here = *here;
    let from = prepared
        .thin_sides
        .partition_point(|thin| arena.is_some_and(|arena| thin.0 < arena));
    for &(_, side, width) in prepared.thin_sides[from..]
        .iter()
        .take_while(|thin| own(thin.0))
    {
        let (border, padding) = (here.border, &mut here.padding);
        match side {
            0 => padding.top = border.top + width,
            1 => padding.right = border.right - width,
            2 => padding.bottom = border.bottom - width,
            _ => padding.left = border.left + width,
        }
    }
    let gradient = tree.gradients.get(index).and_then(Option::as_deref);
    paint_box(
        renderer,
        origin,
        tree.paints[index],
        &here,
        parts,
        &cuts,
        gradient,
    );
}

/// A box's corner radii in pixels: a percentage is of its border box's shorter side, and
/// radii too large for a side shrink together until they fit (CSS Backgrounds 3, 5.5).
fn corner_radii(radius: [[f32; 2]; 4], border: FlowRect) -> iced::border::Radius {
    let (width, height) = (border.right - border.left, border.bottom - border.top);
    let basis = width.min(height).max(0.0);
    let mut corners = radius.map(|[pixels, percent]| (pixels + percent / 100.0 * basis).max(0.0));
    // Top, right, bottom, left: the two corners on each side.
    let mut scale = 1.0_f32;
    for (side, (first, second)) in [
        (width, (0, 1)),
        (height, (1, 2)),
        (width, (2, 3)),
        (height, (3, 0)),
    ] {
        let sum = corners[first] + corners[second];
        if sum > side && sum > 0.0 {
            scale = scale.min(side.max(0.0) / sum);
        }
    }
    for corner in &mut corners {
        *corner *= scale;
    }
    iced::border::Radius {
        top_left: corners[0],
        top_right: corners[1],
        bottom_right: corners[2],
        bottom_left: corners[3],
    }
}

/// The width of a box's border when its four sides are one solid ring: one color at one
/// opacity, one width. `None` when they differ or draw nothing.
fn uniform_border(paint: &BoxPaint, geometry: &FlowGeometry) -> Option<f32> {
    let (border, padding) = (geometry.border, geometry.padding);
    let widths = [
        padding.top - border.top,
        border.right - padding.right,
        border.bottom - padding.bottom,
        padding.left - border.left,
    ];
    let same = |values: [f32; 4]| values.iter().all(|value| (value - values[0]).abs() < 0.01);
    (widths[0] > 0.0
        && same(widths)
        && paint
            .border
            .iter()
            .all(|color| color.is_some() && *color == paint.border[0])
        && paint
            .border_fade
            .iter()
            .all(|fade| *fade == paint.border_fade[0])
        && paint.border_style.iter().all(|style| *style == "solid"))
    .then_some(widths[0])
}

/// A box's background and border bands from its paint, offset to the layout origin.
fn paint_box<Renderer: renderer::Renderer>(
    renderer: &mut Renderer,
    origin: Point,
    paint: BoxPaint,
    geometry: &FlowGeometry,
    parts: BoxParts,
    cuts: &[(usize, f32, f32)],
    gradient: Option<&GradientPaint>,
) {
    // CSS Color 3 `opacity`, through the box's ancestors: on everything it draws here.
    let through = paint.opacity.unwrap_or(1.0);
    let quad = |renderer: &mut Renderer, bounds: Rectangle, color: [f32; 3], alpha: f32| {
        if bounds.width > 0.0 && bounds.height > 0.0 {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: bounds.x + origin.x,
                        y: bounds.y + origin.y,
                        ..bounds
                    },
                    ..Quad::default()
                },
                Color::from_rgba(color[0], color[1], color[2], alpha * through),
            );
        }
    };
    if !paint.visible {
        return;
    }
    let parts = match (paint.collapsed_cell.is_some(), parts) {
        (_, BoxParts::CellBorder) => BoxParts::Border,
        (true, BoxParts::Border) => return,
        (true, _) => BoxParts::Background,
        (false, parts) => parts,
    };
    let border = geometry.border;
    let radius = corner_radii(paint.radius, border);
    let rounded = radius != iced::border::Radius::default();
    let outer = {
        let bounds = rectangle(border.left, border.right, border.top, border.bottom);
        Rectangle {
            x: bounds.x + origin.x,
            y: bounds.y + origin.y,
            ..bounds
        }
    };
    // CSS Backgrounds 3: the shadow of the border box, under the background. A box with
    // no background color still casts it, from a quad that fills with nothing.
    let shadow = paint
        .shadow
        .map_or_else(iced::Shadow::default, |shadow| iced::Shadow {
            color: Color::from_rgba(
                shadow.color[0],
                shadow.color[1],
                shadow.color[2],
                shadow.alpha * through,
            ),
            offset: iced::Vector::new(shadow.offset[0], shadow.offset[1]),
            blur_radius: shadow.blur,
        });
    // CSS Backgrounds 3: the background color shows within `background-clip`; the shadow
    // stays the border box's.
    let clipped = paint.background_clip != BoxArea::Border;
    if (paint.background.is_none() || clipped)
        && paint.shadow.is_some()
        && parts != BoxParts::Border
        && outer.width > 0.0
        && outer.height > 0.0
    {
        renderer.fill_quad(
            Quad {
                bounds: outer,
                border: iced::Border {
                    radius,
                    ..iced::Border::default()
                },
                shadow,
                ..Quad::default()
            },
            Color::TRANSPARENT,
        );
    }
    if let Some(color) = paint.background.filter(|_| parts != BoxParts::Border) {
        // 14.2: the background extends under the border. CSS Backgrounds 3: to its curve.
        let (bounds, radius, shadow) = if clipped {
            let area = area_rect(geometry, paint.background_clip);
            (
                moved_rectangle(area, origin),
                inner_radii(radius, border, area),
                iced::Shadow::default(),
            )
        } else {
            (outer, radius, shadow)
        };
        renderer.fill_quad(
            Quad {
                bounds,
                border: iced::Border {
                    radius,
                    ..iced::Border::default()
                },
                shadow,
                ..Quad::default()
            },
            Color::from_rgba(
                color[0],
                color[1],
                color[2],
                paint.background_fade.unwrap_or(1.0) * through,
            ),
        );
    }
    // CSS Images 3: a gradient over the padding box, above the background color.
    // Sized to its `background-origin` box, and shown where that meets its clip box.
    if let Some(gradient) = gradient.filter(|_| parts != BoxParts::Border) {
        let placed = area_rect(geometry, paint.background_origin);
        let shown = area_rect(geometry, paint.background_clip);
        let area = FlowRect {
            left: placed.left.max(shown.left),
            right: placed.right.min(shown.right),
            top: placed.top.max(shown.top),
            bottom: placed.bottom.min(shown.bottom),
        };
        let bounds = moved_rectangle(area, origin);
        if bounds.width > 0.0
            && bounds.height > 0.0
            && let Some(linear) = linear_gradient(
                gradient,
                placed.right - placed.left,
                placed.bottom - placed.top,
                through,
            )
        {
            renderer.fill_quad(
                Quad {
                    bounds,
                    border: iced::Border {
                        radius: inner_radii(radius, border, area),
                        ..iced::Border::default()
                    },
                    ..Quad::default()
                },
                iced::Background::Gradient(iced::Gradient::Linear(linear)),
            );
        }
    }
    // A rounded box whose four sides are one solid border draws it as one curved ring.
    // Sides that differ, or are not solid, or lose a stretch to a neighbour, stay square.
    if rounded
        && parts != BoxParts::Background
        && cuts.is_empty()
        && let Some(width) = uniform_border(&paint, geometry)
        && let Some(color) = paint.border[0]
    {
        if outer.width > 0.0 && outer.height > 0.0 {
            renderer.fill_quad(
                Quad {
                    bounds: outer,
                    border: iced::Border {
                        color: Color::from_rgba(
                            color[0],
                            color[1],
                            color[2],
                            paint.border_fade[0].unwrap_or(1.0) * through,
                        ),
                        width,
                        radius,
                    },
                    ..Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
        return;
    }
    if parts != BoxParts::Background {
        let bands = border_bands(border, geometry.padding);
        let mut doubles = double_sides(border, geometry.padding, paint.border_style);
        for (double, color) in doubles.iter_mut().zip(paint.border) {
            *double &= color.is_some();
        }
        for (side, (band, color)) in bands.into_iter().zip(paint.border).enumerate() {
            let Some(color) = color else { continue };
            // What a neighbour won is left out, along the band's length.
            let across = side == 0 || side == 2;
            let (start, end) = if across {
                (band.x, band.x + band.width)
            } else {
                (band.y, band.y + band.height)
            };
            let mut spans = vec![(start, end)];
            for &(_, from, to) in cuts.iter().filter(|cut| cut.0 == side) {
                spans = spans
                    .into_iter()
                    .flat_map(|(low, high)| {
                        [(low, high.min(from)), (low.max(to), high)]
                            .into_iter()
                            .filter(|(low, high)| high > low)
                    })
                    .collect();
            }
            for (low, high) in spans {
                let part = if across {
                    Rectangle {
                        x: low,
                        width: high - low,
                        ..band
                    }
                } else {
                    Rectangle {
                        y: low,
                        height: high - low,
                        ..band
                    }
                };
                // A side fading to or from transparent: at its opacity. Bands meet without
                // overlapping, so no corner paints twice.
                let alpha = paint.border_fade[side].unwrap_or(1.0);
                let mut pieces = band_pieces(part, side, paint.border_style[side], color);
                join_double(&mut pieces, side, border, geometry.padding, doubles);
                for (piece, shade) in pieces {
                    quad(renderer, piece, shade, alpha);
                }
            }
        }
    }
}

impl<Message, Theme, Renderer> BoxTreeWidget<'_, Message, Theme, Renderer>
where
    Renderer: text::Renderer<Font = iced::Font> + BackgroundRenderer,
{
    /// The topmost box under a point relative to the layout, as a source node.
    fn hit(&self, point: Point) -> Option<usize> {
        for &step in self.steps.iter().rev() {
            // Inline text answers for the run under the point; blank line space for nothing.
            if let PaintStep::Text(arena) = step
                && self.tree.prepared.anonymous[arena]
                && let Some((shaped, corner)) = self.paragraphs.get(&arena)
                && let Some(leaf) = self.tree.texts[arena].as_ref()
                && self
                    .clips
                    .get(arena)
                    .copied()
                    .flatten()
                    .is_none_or(|clip: FlowRect| {
                        point.x >= clip.left
                            && point.x < clip.right
                            && point.y >= clip.top
                            && point.y < clip.bottom
                    })
            {
                // The shaper snaps a point outside its text to the nearest run, so the
                // run's own rectangles decide.
                let rest = self
                    .rest_paragraphs
                    .get(&arena)
                    .into_iter()
                    .flatten()
                    .map(|(shaped, corner, leaf)| (shaped, *corner, leaf));
                for (shaped, corner, leaf) in std::iter::once((shaped, *corner, leaf)).chain(rest) {
                    let local = Point::new(point.x - corner.x, point.y - corner.y);
                    // Every run sits on a line between the paragraph's top and bottom, so a
                    // point above or below them misses without asking the shaper.
                    if local.y < 0.0 || local.y >= shaped.min_bounds().height {
                        continue;
                    }
                    if let Some(span) = shaped.hit_span(local)
                        && let Some(run) = leaf.runs.get(span)
                        && run.visible
                        && shaped
                            .span_bounds(span)
                            .iter()
                            .any(|rect| rect.contains(local))
                    {
                        return Some(run.owner);
                    }
                }
            }
            let PaintStep::Box(step) = step else {
                continue;
            };
            let Some(Some(geometry)) = self.geometry.get(step) else {
                continue;
            };
            if !self.tree.paints[step].visible {
                continue;
            }
            let arena = self.tree.prepared.slot[step];
            let clip = arena.and_then(|arena| self.clips.get(arena).copied().flatten());
            let inside = |rect: FlowRect| {
                point.x >= rect.left
                    && point.x < rect.right
                    && point.y >= rect.top
                    && point.y < rect.bottom
            };
            if inside(geometry.border) && clip.is_none_or(inside) {
                return self.tree.originals.get(step).copied();
            }
        }
        None
    }

    fn press_message(&self, hit: Option<usize>) -> Option<Message>
    where
        Message: Clone,
    {
        let nodes = self.tree.source.construction.nodes();
        let mut at = hit;
        while let Some(node) = at {
            let id = nodes[node].metadata().css_id();
            if let Some(found) = self
                .presses
                .iter()
                .find(|(owner, _)| Some(owner.as_str()) == id)
            {
                return Some(found.1.clone());
            }
            at = nodes[node].parent();
        }
        None
    }

    /// What a hosted widget sees of the cursor: it only gets it while its own box is
    /// the topmost at the point, so overlays and clips stop input reaching it.
    fn child_cursor(
        &self,
        index: usize,
        cursor: mouse::Cursor,
        top: Option<usize>,
        pressed: Option<usize>,
    ) -> mouse::Cursor {
        let Some(arena) = self.slots.get(index).copied().flatten() else {
            return mouse::Cursor::Unavailable;
        };
        let projected = self.tree.prepared.positions[arena].0;
        let owner = self.tree.originals.get(projected).copied();
        // A press that started on the widget keeps it the cursor, as a drag needs.
        if (top.is_some() && top == owner) || (pressed.is_some() && pressed == owner) {
            cursor
        } else {
            mouse::Cursor::Unavailable
        }
    }

    fn visible_slot(&self, index: usize) -> Option<usize> {
        let arena = self.slots.get(index).copied().flatten()?;
        self.tree.paints[self.tree.prepared.positions[arena].0]
            .visible
            .then_some(arena)
    }

    /// Boxes that take focus, in tab order: positive `tabindex` ascending, then the rest
    /// in document order. A press message makes a box focusable; a negative `tabindex`
    /// keeps it out of the Tab order while a click can still focus it.
    fn tab_order(&self, hosts: &[(usize, usize)]) -> Vec<usize> {
        let nodes = self.tree.source.construction.nodes();
        let mut order: Vec<(i64, usize, usize)> = Vec::new();
        for (projected, &node) in self.tree.originals.iter().enumerate() {
            let rendered =
                self.tree.prepared.slot[projected].is_some() && self.tree.paints[projected].visible;
            if !rendered {
                continue;
            }
            let hosted = hosts.iter().any(|host| host.1 == node).then_some(0);
            match self.tab_index(nodes, node).or(hosted) {
                Some(index) if index >= 0 => {
                    order.push((if index == 0 { i64::MAX } else { index }, projected, node));
                }
                _ => {}
            }
        }
        order.sort_unstable();
        order.into_iter().map(|(_, _, node)| node).collect()
    }

    /// Runs `operation` on one hosted element.
    fn operate_host(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        index: usize,
        operation: &mut HostFocus,
    ) {
        if let (Some(element), Some(child), Some(child_layout)) = (
            self.elements.get_mut(index),
            tree.children.get_mut(index),
            layout.children().nth(index),
        ) {
            element
                .as_widget_mut()
                .operate(child, child_layout, renderer, operation);
        }
    }

    /// The hosted elements holding something focusable, with their source nodes, by index.
    fn focus_hosts(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
    ) -> Vec<(usize, usize)> {
        let mut hosts = Vec::new();
        for index in 0..self.elements.len() {
            let Some(arena) = self.visible_slot(index) else {
                continue;
            };
            let node = self.tree.originals[self.tree.prepared.positions[arena].0];
            let mut has = HostFocus::Has(false);
            self.operate_host(tree, layout, renderer, index, &mut has);
            if matches!(has, HostFocus::Has(true)) {
                hosts.push((index, node));
            }
        }
        hosts
    }

    /// Whether `node` hosts something focusable that does not have the focus now.
    fn host_lost_focus(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        node: Option<usize>,
    ) -> bool {
        let hosts = self.focus_hosts(tree, layout, renderer);
        let Some(&(index, _)) = hosts.iter().find(|host| Some(host.1) == node) else {
            return false;
        };
        let mut found = HostFocus::IsFocused(false);
        self.operate_host(tree, layout, renderer, index, &mut found);
        !matches!(found, HostFocus::IsFocused(true))
    }

    /// The box from `node` outward hosting the widget that has the focus.
    fn focused_host(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        node: Option<usize>,
    ) -> Option<usize> {
        let hosts = self.focus_hosts(tree, layout, renderer);
        let nodes = self.tree.source.construction.nodes();
        let mut chain = Vec::new();
        let mut at = node;
        while let Some(here) = at {
            chain.push(here);
            at = nodes[here].parent();
        }
        for (index, host) in hosts {
            if !chain.contains(&host) {
                continue;
            }
            let mut found = HostFocus::IsFocused(false);
            self.operate_host(tree, layout, renderer, index, &mut found);
            if matches!(found, HostFocus::IsFocused(true)) {
                return Some(host);
            }
        }
        None
    }

    /// A box's `tabindex`; a press message implies 0.
    fn tab_index(&self, nodes: &[crate::capture::ConstructionNode], node: usize) -> Option<i64> {
        let metadata = nodes[node].metadata();
        let explicit = metadata
            .attribute("tabindex")
            .and_then(|text| text.trim().parse::<i64>().ok());
        let pressable = metadata
            .css_id()
            .is_some_and(|id| self.presses.iter().any(|(owner, _)| owner == id));
        explicit.or(pressable.then_some(0))
    }

    /// Whether this source node has a rendered, visible box.
    fn takes_focus(&self, node: usize) -> bool {
        self.tree
            .originals
            .iter()
            .position(|value| *value == node)
            .is_some_and(|projected| {
                self.tree.prepared.slot[projected].is_some() && self.tree.paints[projected].visible
            })
    }

    /// The scroll offsets that bring `node`'s box into view, innermost scroller first.
    fn scroll_into_view(
        &self,
        node: usize,
        scrolled: &mut BTreeMap<usize, f32>,
        scrolled_x: &mut BTreeMap<usize, f32>,
    ) {
        let nodes = self.tree.source.construction.nodes();
        let Some(projected) = self.tree.originals.iter().position(|value| *value == node) else {
            return;
        };
        let Some(Some(geometry)) = self.geometry.get(projected) else {
            return;
        };
        let (mut top, mut bottom) = (geometry.border.top, geometry.border.bottom);
        let (mut left, mut right) = (geometry.border.left, geometry.border.right);
        let positions = &self.tree.prepared.positions;
        // A fixed box scrolls with nothing, and an absolute one only with boxes from its
        // containing block outward.
        let mut position = self.tree.prepared.slot[projected].map(|slot| positions[slot].1);
        let mut at = nodes[node].parent();
        while let Some(ancestor) = at {
            if position == Some(Position::Fixed) {
                return;
            }
            at = nodes[ancestor].parent();
            let Some(index) = self
                .tree
                .originals
                .iter()
                .position(|value| *value == ancestor)
            else {
                continue;
            };
            let Some(slot) = self.tree.prepared.slot[index] else {
                continue;
            };
            let here = positions[slot].1;
            // A static box between an absolute one and its containing block moves nothing.
            let skipped =
                position == Some(Position::Absolute) && here == Position::Static && slot != 0;
            if !skipped {
                position = Some(here);
            }
            let Some(&(limit, limit_x)) = self.scroll_limits.get(slot).filter(|_| !skipped) else {
                continue;
            };
            let Some(area) = self.slot_geometry.get(slot).map(|outer| outer.padding) else {
                continue;
            };
            let (end, end_x) = if slot == 0 {
                (self.canvas_height, self.canvas_width)
            } else {
                (area.bottom, area.right)
            };
            let bring = |offset: &mut BTreeMap<usize, f32>,
                         limit: f32,
                         start: f32,
                         stop: f32,
                         low: f32,
                         high: f32| {
                let current = offset
                    .get(&ancestor)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(0.0, limit);
                let delta = if low < start {
                    low - start
                } else if high > stop {
                    (high - stop).min(low - start)
                } else {
                    0.0
                };
                let wanted = (current + delta).clamp(0.0, limit);
                if wanted != current {
                    offset.insert(ancestor, wanted);
                }
                wanted - current
            };
            let moved = bring(scrolled, limit, area.top, end, top, bottom);
            top -= moved;
            bottom -= moved;
            if self.tree.prepared.boxes[slot].direction == Direction::Rtl {
                // Mirrored, so the offset grows toward the box's left end.
                let moved = bring(scrolled_x, limit_x, -end_x, -area.left, -right, -left);
                left += moved;
                right += moved;
            } else {
                let moved = bring(scrolled_x, limit_x, area.left, end_x, left, right);
                left -= moved;
                right -= moved;
            }
        }
    }

    /// The nearest focusable box from `node` outward, as a click finds it.
    fn focus_target(&self, node: Option<usize>) -> Option<usize> {
        let nodes = self.tree.source.construction.nodes();
        let mut at = node;
        while let Some(here) = at {
            if self.tab_index(nodes, here).is_some() {
                return Some(here);
            }
            at = nodes[here].parent();
        }
        None
    }

    /// A background image that loaded. A failed one must not draw: some renderers report a
    /// missing file as one pixel, which would then tile across the whole box.
    fn loaded_background(&self, projected: usize) -> Option<&(BackgroundImage, image::Handle)> {
        self.loaded
            .contains(&projected)
            .then(|| self.tree.backgrounds.get(&projected))
            .flatten()
    }

    /// 18.4: an inline element's outline goes around each piece of it on a line: the
    /// runs within it, as tall as its padding and border reach above and below their text.
    fn draw_inline_outline(&self, renderer: &mut Renderer, origin: Point, projected: usize) {
        let paint = self.tree.paints[projected];
        // Its step is pushed only for a visible element.
        let Some((width, color)) = paint.outline else {
            return;
        };
        // Its own inline content only: what reaches it through inline parents. A float, an
        // absolute box or an inline-block inside it starts boxes of its own.
        let prepared = &self.tree.prepared;
        let nodes = self.tree.source.construction.nodes().len();
        let mut projected_of = vec![None; nodes];
        for (at, &node) in self.tree.originals.iter().enumerate() {
            if let Some(slot) = projected_of.get_mut(node) {
                *slot = Some(at);
            }
        }
        let inside = |owner: usize| {
            let mut at = projected_of.get(owner).copied().flatten();
            while let Some(here) = at {
                if here == projected {
                    return true;
                }
                at = prepared.inline_parent[here];
            }
            false
        };
        let root = self.tree.originals[projected];
        // One piece per line, keyed by the line's top, which every span on it shares.
        let mut pieces: Vec<(f32, Rectangle)> = Vec::new();
        let shaped = self
            .paragraphs
            .iter()
            .filter_map(|(arena, (shaped, corner))| {
                Some((self.tree.texts[*arena].as_ref()?, shaped, *corner))
            })
            .chain(
                self.rest_paragraphs
                    .values()
                    .flatten()
                    .map(|(shaped, corner, leaf)| (leaf, shaped, *corner)),
            );
        for (leaf, shaped, corner) in shaped {
            for (index, run) in leaf.runs.iter().enumerate() {
                if !inside(run.owner) {
                    continue;
                }
                let own = run.inline_boxes.iter().find(|sides| sides.owner == root);
                for area in shaped.span_bounds(index) {
                    // As for inline sides: the font's content area, then padding and border.
                    let height = area.height.min(run.line_size.unwrap_or(run.size) * 1.3);
                    let top = area.y + (area.height - height) / 2.0;
                    let (above, below) = own.map_or((0.0, 0.0), |sides| {
                        (
                            sides.padding[0] + sides.border[0].0,
                            sides.padding[1] + sides.border[1].0,
                        )
                    });
                    let piece = Rectangle {
                        x: origin.x + corner.x + area.x,
                        y: origin.y + corner.y + top - above,
                        width: area.width,
                        height: height + above + below,
                    };
                    let line = corner.y + area.y;
                    match pieces
                        .iter_mut()
                        .find(|(other, _)| (other - line).abs() < 0.5)
                    {
                        Some((_, other)) => *other = other.union(&piece),
                        None => pieces.push((line, piece)),
                    }
                }
            }
        }
        for (_, piece) in pieces {
            let inner = FlowRect {
                left: piece.x,
                right: piece.x + piece.width,
                top: piece.y,
                bottom: piece.y + piece.height,
            };
            let outer = FlowRect {
                left: inner.left - width,
                right: inner.right + width,
                top: inner.top - width,
                bottom: inner.bottom + width,
            };
            let doubles = double_sides(outer, inner, [paint.outline_style; 4]);
            for (side, band) in border_bands(outer, inner).into_iter().enumerate() {
                let mut pieces = band_pieces(band, side, paint.outline_style, color);
                join_double(&mut pieces, side, outer, inner, doubles);
                for (part, shade) in pieces {
                    renderer.fill_quad(
                        Quad {
                            bounds: part,
                            ..Quad::default()
                        },
                        Color::from_rgb(shade[0], shade[1], shade[2]),
                    );
                }
            }
        }
    }

    /// Each arena slot's transform, its ancestors' with its element's own, drawn at
    /// `origin`; `None` draws as laid out.
    fn transformations(&self, origin: Point) -> Vec<Option<iced::Transformation>> {
        let prepared = &self.tree.prepared;
        let count = prepared.boxes.len();
        let mut found: Vec<Option<iced::Transformation>> = vec![None; count];
        if self.tree.transforms.iter().all(Option::is_none) || count == 0 {
            return found;
        }
        let mut stack = vec![(0usize, None::<iced::Transformation>)];
        while let Some((arena, above)) = stack.pop() {
            let owner = prepared.positions[arena].0;
            let own = (prepared.slot.get(owner).copied().flatten() == Some(arena))
                .then(|| self.tree.transforms.get(owner).cloned().flatten())
                .flatten()
                .and_then(|transform| {
                    let border = self.geometry.get(owner)?.as_ref()?.border;
                    local_transformation(&transform, moved_rectangle(border, origin))
                });
            let here = match (above, own) {
                (Some(above), Some(own)) => Some(above * own),
                (above, own) => above.or(own),
            };
            found[arena] = here;
            for &child in &prepared.boxes[arena].children {
                stack.push((child, here));
            }
        }
        found
    }

    fn embedded(&self) -> BTreeSet<usize> {
        self.slots.iter().copied().flatten().collect()
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for BoxTreeWidget<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: text::Renderer<Font = iced::Font> + BackgroundRenderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Interaction>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Interaction::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.elements.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.state
            .downcast_mut::<Interaction>()
            .follow(&self.base.source);
        tree.diff_children(&self.elements);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let available = limits.max();
        let state = tree.state.downcast_mut::<Interaction>();
        // A rebuilt view keeps this state, moved to this capture's nodes; `styled` ignores
        // a node this tree lacks.
        state.follow(&self.base.source);
        let interaction = state.clone();
        let key = (interaction.hovered, interaction.active, interaction.focused);
        let changed = state.seen != Some(key);
        let animating = state
            .transitions
            .as_ref()
            .is_some_and(|held| held.lock().is_animating());
        if key != self.key || changed || self.presenting || animating {
            let (styled, presenting) = state.styled_for(&self.base, key, Instant::now());
            // Unchanged and not moving: the tree in hand keeps its measurements.
            if key != self.key || presenting || self.presenting {
                self.key = key;
                self.tree = styled;
                self.measured = false;
            }
            self.presenting = presenting;
        }
        if !self.measured {
            self.measured = true;
            self.slots = self.tree.leaf_slots(&self.ids);
            self.slot_owners = vec![None; self.tree.prepared.boxes.len()];
            for (index, slot) in self.slots.iter().enumerate() {
                if let Some(owner) = slot.and_then(|arena| self.slot_owners.get_mut(arena)) {
                    owner.get_or_insert(index);
                }
            }
            self.allocations.clear();
            self.loaded.clear();
            for (projected, (_, handle)) in &self.tree.backgrounds {
                if let Some(allocation) = renderer.load_background(handle) {
                    self.allocations.push(allocation);
                    self.loaded.insert(*projected);
                }
            }
            self.loaded_markers.clear();
            for (projected, handle) in &self.tree.marker_images {
                if let Some(allocation) = renderer.load_background(handle) {
                    self.allocations.push(allocation);
                    self.loaded_markers.insert(*projected);
                }
            }
            let mut allocations = Vec::new();
            self.tree.apply_marker_images(|handle| {
                allocations.push(renderer.load_background(handle)?);
                let (width, height) = renderer.background_size(handle)?;
                Some(Size::new(width, height))
            });
            self.allocations.extend(allocations);
            self.tree.set_text_widths(|leaf| {
                let longest = paragraph(renderer, leaf, f32::INFINITY).min_bounds().width;
                (narrowest_width(renderer, leaf), longest)
            });
            for (index, element) in self.elements.iter_mut().enumerate() {
                let Some(arena) = self.slots[index] else {
                    continue;
                };
                let unbounded =
                    layout::Limits::new(Size::ZERO, Size::new(f32::INFINITY, f32::INFINITY));
                let natural = element
                    .as_widget_mut()
                    .layout(&mut tree.children[index], renderer, &unbounded)
                    .size()
                    .width;
                // A Fill-width widget reports no natural width of its own.
                let natural = if natural.is_finite() { natural } else { 0.0 };
                self.tree.prepared.boxes[arena].content = Some(FlowContent {
                    height: 0.0,
                    preferred_width: natural,
                    preferred_min_width: natural,
                });
            }
        }
        self.tree
            .keep_rows_with_widgets(self.slots.iter().copied().flatten());
        let (elements, owners, children) =
            (&mut self.elements, &self.slot_owners, &mut tree.children);
        let laid = self.tree.layout_passes(
            renderer,
            available.width,
            Some(available.height),
            &mut |arena, width| {
                let index = owners.get(arena).copied().flatten()?;
                // A Fill-height widget takes what the box has, and asks for nothing.
                if elements[index].as_widget().size().height.is_fill() {
                    return Some(0.0);
                }
                let limits = layout::Limits::new(Size::ZERO, Size::new(width, f32::INFINITY));
                Some(
                    elements[index]
                        .as_widget_mut()
                        .layout(&mut children[index], renderer, &limits)
                        .size()
                        .height,
                )
            },
        );
        // A refused layout draws nothing, and says why once.
        let mut refusal = self
            .tree
            .source
            .layout_refusal
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.slot_geometry = match laid {
            Ok(geometry) => {
                *refusal = None;
                geometry
            }
            Err(error) => {
                let said = error.to_string();
                if refusal.as_deref() != Some(said.as_str()) {
                    log::warn!(target: "iced_css::box_tree", "layout refused: {said}");
                    *refusal = Some(said);
                }
                Vec::new()
            }
        };
        drop(refusal);
        let owners = &self.slot_owners;
        self.canvas_height = available.height;
        self.canvas_width = available.width;
        let originals = &self.tree.originals;
        let positions = &self.tree.prepared.positions;
        let texts = &self.tree.texts;
        let boxes = &self.tree.prepared.boxes;
        let paints = &self.tree.paints;
        // Plain text the extents shape, for placing it below at the same width.
        let shaped_plain = std::cell::RefCell::new(BTreeMap::new());
        self.scroll_limits = apply_scrolling(
            &self.tree.prepared,
            &mut self.slot_geometry,
            |arena| {
                let node = originals.get(positions[arena].0).copied();
                let read = |map: &BTreeMap<usize, f32>| {
                    node.and_then(|node| map.get(&node).copied()).unwrap_or(0.0)
                };
                (read(&interaction.scrolled), read(&interaction.scrolled_x))
            },
            |arena, geometry| {
                // What layout and paint lay out: nothing blank or under a hosted widget, at
                // the width rounded the same way. Split text already sized its box.
                let leaf = texts[arena].as_ref().filter(|leaf| !leaf.is_empty())?;
                let auto_height = boxes[arena].height == Dim::Auto;
                if (geometry.leaf_after.is_some() && auto_height)
                    || owners.get(arena).is_some_and(Option::is_some)
                {
                    return None;
                }
                let place = geometry.leaf_band.unwrap_or(geometry.content);
                let width = (place.right - place.left).ceil();
                // Where paint puts the lines: right ones overflow leftward, centered ones
                // spill half each way; a hanging first line starts past the start edge.
                let extent = |block: f32, left: f32, room: f32, far: f32| match leaf.align {
                    "right" => (left + room - block, far),
                    "center" => (left + (room - block) / 2.0, left + (room + block) / 2.0),
                    _ => (left, left + block),
                };
                if let Some(pieces) = pieces(renderer, leaf, width) {
                    let rtl =
                        boxes[arena].direction == iced_css_core::formatting::block::Direction::Rtl;
                    let (mut from, mut to, mut height) = (f32::INFINITY, f32::NEG_INFINITY, 0.0);
                    for (piece, room) in &pieces {
                        let size = paragraph::<Renderer>(renderer, piece, *room).min_bounds();
                        let left = if rtl { 0.0 } else { width - room };
                        let (near, far) = extent(size.width, left, *room, left + room);
                        from = from.min(near);
                        to = to.max(far);
                        height += size.height;
                    }
                    return Some(TextExtent { height, from, to });
                }
                let shaped: Renderer::Paragraph = paragraph(renderer, leaf, width);
                let size = shaped.min_bounds();
                shaped_plain
                    .borrow_mut()
                    .insert((arena, width.to_bits()), shaped);
                let (from, to) = extent(size.width, 0.0, width, place.right - place.left);
                Some(TextExtent {
                    height: size.height,
                    from,
                    to,
                })
            },
            |arena, outer| {
                own_clip(
                    paints[positions[arena].0].clip,
                    positions[arena].1,
                    outer.border,
                )
            },
            available,
        );
        let mut shaped_plain = shaped_plain.into_inner();
        self.geometry = self.tree.prepared.by_projected(&self.slot_geometry);
        self.clips = self.tree.clips(&self.geometry);
        self.column_rects = column_rects(&self.tree.prepared, &self.slot_geometry);
        self.paragraphs.clear();
        self.rest_paragraphs.clear();
        for (arena, leaf) in self.tree.texts.iter().enumerate() {
            let Some(leaf) = leaf.as_ref().filter(|leaf| !leaf.is_empty()) else {
                continue;
            };
            let Some(geometry) = self.slot_geometry.get(arena) else {
                continue;
            };
            if self.slot_owners.get(arena).is_some_and(Option::is_some) {
                continue;
            }
            let place = geometry.leaf_band.unwrap_or(geometry.content);
            let width = (place.right - place.left).ceil();
            let block_corner = |block: f32, place: FlowRect, width: f32, top: f32| {
                Point::new(
                    match leaf.align {
                        "center" => place.left + (width - block) / 2.0,
                        "right" => place.left + width - block,
                        _ => place.left,
                    },
                    top,
                )
            };
            // A paragraph at a time where one would not do, each where its own width puts
            // it: a hanging line reaches out past the start edge (left, or right in a
            // right-to-left block). It never widens past a float: layout gives it no
            // `leaf_after`.
            if let Some(pieces) = pieces(renderer, leaf, width) {
                let rtl = self.tree.prepared.boxes[arena].direction
                    == iced_css_core::formatting::block::Direction::Rtl;
                let mut top = place.top;
                let mut rest = Vec::new();
                for (index, (piece, room)) in pieces.into_iter().enumerate() {
                    let shaped: Renderer::Paragraph = paragraph(renderer, &piece, room);
                    let shift = if rtl { 0.0 } else { room - width };
                    let line_place = FlowRect {
                        left: place.left - shift,
                        ..place
                    };
                    let corner = block_corner(shaped.min_bounds().width, line_place, room, top);
                    top += shaped.min_bounds().height;
                    if index == 0 {
                        self.paragraphs.insert(arena, (shaped, corner));
                    } else {
                        rest.push((shaped, corner, piece));
                    }
                }
                self.rest_paragraphs.insert(arena, rest);
                continue;
            }
            // Past a shorter float the rest of the text takes the wider band.
            let parts = geometry.leaf_after.and_then(|after| {
                split_leaf(renderer, leaf, width, after.top - place.top).map(|parts| (after, parts))
            });
            if let Some((after, (first, second, first_height))) = parts {
                let wide = (after.right - after.left).ceil();
                let shaped_first: Renderer::Paragraph = paragraph(renderer, &first, width);
                let shaped_rest: Renderer::Paragraph = paragraph(renderer, &second, wide);
                let corner_first =
                    block_corner(shaped_first.min_bounds().width, place, width, place.top);
                let corner_rest = block_corner(
                    shaped_rest.min_bounds().width,
                    after,
                    wide,
                    place.top + first_height,
                );
                self.paragraphs.insert(arena, (shaped_first, corner_first));
                self.rest_paragraphs
                    .insert(arena, vec![(shaped_rest, corner_rest, second)]);
                continue;
            }
            // A paragraph aligns its lines within its widest line and draws from that
            // block's corner: place the block in the content box.
            // Shaped once: the same paragraph gives the block width and is stored.
            // CSS Overflow 3: a line that overflows its clipping box ends in "…".
            if leaf.ellipsis
                && !leaf.wraps
                && let Some(cut) = ellipsized(renderer, leaf, width)
            {
                let shaped: Renderer::Paragraph = paragraph(renderer, &cut, f32::INFINITY);
                let corner = block_corner(shaped.min_bounds().width, place, width, place.top);
                self.paragraphs.insert(arena, (shaped, corner));
                continue;
            }
            let shaped: Renderer::Paragraph = shaped_plain
                .remove(&(arena, width.to_bits()))
                .unwrap_or_else(|| paragraph(renderer, leaf, width));
            let corner = block_corner(shaped.min_bounds().width, place, width, place.top);
            self.paragraphs.insert(arena, (shaped, corner));
        }
        let embedded = self.embedded();
        self.steps = self.tree.paint_steps(&embedded);
        self.drawn = self.tree.drawn_steps(self.steps.clone(), &embedded);
        self.marker_paragraphs.clear();
        for (projected, leaf) in &self.tree.markers {
            let Some(Some(geometry)) = self.geometry.get(*projected) else {
                continue;
            };
            let shaped: Renderer::Paragraph = paragraph(renderer, leaf, f32::INFINITY);
            // 12.5.1: to the side of the principal box, at its first line, with DP21's gap.
            let gap = leaf.runs[0].size * 0.5;
            let width = shaped.min_bounds().width;
            let left = if self.tree.prepared.boxes[self.tree.prepared.slot[*projected].unwrap_or(0)]
                .direction
                == iced_css_core::formatting::block::Direction::Rtl
            {
                geometry.border.right + gap
            } else {
                geometry.border.left - gap - width
            };
            self.marker_paragraphs
                .insert(*projected, (shaped, Point::new(left, geometry.content.top)));
        }
        let mut nodes = Vec::with_capacity(self.elements.len());
        for (index, element) in self.elements.iter_mut().enumerate() {
            let place = self.slots[index]
                .and_then(|arena| self.slot_geometry.get(arena))
                .map(|geometry| geometry.leaf_band.unwrap_or(geometry.content));
            let node = match place {
                Some(place) => {
                    let width = (place.right - place.left).max(0.0);
                    let height = if element.as_widget().size().height.is_fill() {
                        (place.bottom - place.top).max(0.0)
                    } else {
                        f32::INFINITY
                    };
                    let limits = layout::Limits::new(Size::ZERO, Size::new(width, height));
                    element
                        .as_widget_mut()
                        .layout(&mut tree.children[index], renderer, &limits)
                        .move_to(Point::new(place.left, place.top))
                }
                // No box: no size, but the widget's own node shape, which its `operate`
                // still walks.
                None => {
                    let limits = layout::Limits::new(Size::ZERO, Size::ZERO);
                    let node = element.as_widget_mut().layout(
                        &mut tree.children[index],
                        renderer,
                        &limits,
                    );
                    layout::Node::with_children(Size::ZERO, node.children().to_vec())
                }
            };
            nodes.push(node);
        }
        layout::Node::with_children(available, nodes)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        let shown: Vec<_> = (0..self.elements.len())
            .map(|index| self.visible_slot(index).is_some())
            .collect();
        operation.traverse(&mut |operation| {
            for (index, ((element, child), child_layout)) in self
                .elements
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
                .enumerate()
            {
                let widget = element.as_widget_mut();
                if shown[index] {
                    widget.operate(child, child_layout, renderer, operation);
                } else {
                    widget.operate(child, child_layout, renderer, &mut Unfocusable(operation));
                }
            }
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Each frame moves the widget's own transitions on, and asks for the next while
        // they run; the frame after the last shows where they end.
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let state = tree.state.downcast_mut::<Interaction>();
            if state.advance_transitions(&self.base, self.key, *now) {
                shell.invalidate_layout();
                shell.request_redraw();
            }
        }
        let top = cursor
            .position_in(layout.bounds())
            .and_then(|point| self.hit(point));
        let pressed = tree.state.downcast_ref::<Interaction>().active;
        let cursors: Vec<_> = (0..self.elements.len())
            .map(|index| self.child_cursor(index, cursor, top, pressed))
            .collect();
        let shown: Vec<_> = (0..self.elements.len())
            .map(|index| self.visible_slot(index).is_some())
            .collect();
        for (index, ((element, child), child_layout)) in self
            .elements
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .enumerate()
        {
            // A widget without a visible box takes no input.
            if !shown[index] {
                continue;
            }
            element.as_widget_mut().update(
                child,
                event,
                child_layout,
                cursors[index],
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
        if let Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            repeat,
            ..
        }) = event
        {
            if shell.is_event_captured() {
                // A widget that took Escape dropped its focus: `:focus` goes with it.
                let escaped = matches!(
                    key.as_ref(),
                    keyboard::Key::Named(keyboard::key::Named::Escape)
                );
                let held = tree.state.downcast_ref::<Interaction>().focused;
                if escaped && self.host_lost_focus(tree, layout, renderer, held) {
                    tree.state.downcast_mut::<Interaction>().focused = None;
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
                return;
            }
            // A rebuilt view may have fewer nodes, and a restyle may have hidden the box.
            let mut focused = tree
                .state
                .downcast_ref::<Interaction>()
                .focused
                .filter(|node| self.takes_focus(*node));
            let mut hosts = Vec::new();
            // Tab within a host: the element and which of its focusables takes the focus.
            let mut within = None;
            // Ctrl, Alt or Logo with a key is a shortcut, the app's to take.
            if modifiers.control() || modifiers.alt() || modifiers.logo() {
                return;
            }
            let next = match key.as_ref() {
                keyboard::Key::Named(keyboard::key::Named::Tab) => {
                    // A widget focused by a click counts as where Tab starts from.
                    hosts = self.focus_hosts(tree, layout, renderer);
                    for &(index, node) in &hosts {
                        let mut found = HostFocus::Find {
                            count: 0,
                            focused: None,
                        };
                        self.operate_host(tree, layout, renderer, index, &mut found);
                        if let HostFocus::Find {
                            count,
                            focused: Some(at),
                        } = found
                        {
                            focused = Some(node);
                            // Through a host's focusables before leaving it.
                            let step = if modifiers.shift() {
                                at.checked_sub(1)
                            } else {
                                (at + 1 < count).then_some(at + 1)
                            };
                            within = step.map(|to| (index, to));
                        }
                    }
                    if within.is_some() {
                        focused
                    } else {
                        let order = self.tab_order(&hosts);
                        if order.is_empty() {
                            return;
                        }
                        let at =
                            focused.and_then(|node| order.iter().position(|item| *item == node));
                        let count = order.len();
                        let index = match (at, modifiers.shift()) {
                            (Some(index), false) => (index + 1) % count,
                            (Some(index), true) => (index + count - 1) % count,
                            (None, false) => 0,
                            (None, true) => count - 1,
                        };
                        Some(order[index])
                    }
                }
                keyboard::Key::Named(keyboard::key::Named::Escape) if focused.is_some() => None,
                keyboard::Key::Named(keyboard::key::Named::Enter | keyboard::key::Named::Space)
                    if focused.is_some() =>
                {
                    // Keys in a focused widget are its own, whatever boxes hold it.
                    let hosts = self.focus_hosts(tree, layout, renderer);
                    for (index, _) in hosts {
                        let mut found = HostFocus::IsFocused(false);
                        self.operate_host(tree, layout, renderer, index, &mut found);
                        if matches!(found, HostFocus::IsFocused(true)) {
                            return;
                        }
                    }
                    // A held Space presses once, as a browser's does on release; Enter repeats.
                    let held_space = *repeat
                        && matches!(
                            key.as_ref(),
                            keyboard::Key::Named(keyboard::key::Named::Space)
                        );
                    if let Some(message) = self.press_message(focused) {
                        if !held_space {
                            shell.publish(message);
                        }
                        shell.capture_event();
                    }
                    return;
                }
                _ => return,
            };
            // Every element, hidden ones too: their widgets keep their state.
            for index in 0..self.elements.len() {
                let entered = hosts
                    .iter()
                    .any(|&(host, node)| host == index && next == Some(node));
                let mut change = match within {
                    Some((host, target)) if host == index => HostFocus::Focus { target, seen: 0 },
                    Some(_) => HostFocus::Unfocus,
                    // Backward, a host is entered at its last focusable.
                    None if entered && modifiers.shift() => {
                        let mut found = HostFocus::Find {
                            count: 0,
                            focused: None,
                        };
                        self.operate_host(tree, layout, renderer, index, &mut found);
                        let count = match found {
                            HostFocus::Find { count, .. } => count,
                            _ => 0,
                        };
                        HostFocus::Focus {
                            target: count.saturating_sub(1),
                            seen: 0,
                        }
                    }
                    None if entered => HostFocus::Focus { target: 0, seen: 0 },
                    None => HostFocus::Unfocus,
                };
                self.operate_host(tree, layout, renderer, index, &mut change);
            }
            shell.request_redraw();
            let state = tree.state.downcast_mut::<Interaction>();
            let before = (state.hovered, state.active, state.focused);
            state.focused = next;
            let after = (state.hovered, state.active, state.focused);
            shell.capture_event();
            let (mut scrolled, mut scrolled_x) = (state.scrolled.clone(), state.scrolled_x.clone());
            if let Some(node) = next {
                self.scroll_into_view(node, &mut scrolled, &mut scrolled_x);
            }
            let moved = scrolled != state.scrolled || scrolled_x != state.scrolled_x;
            state.scrolled = scrolled;
            state.scrolled_x = scrolled_x;
            state.rehover |= moved;
            if moved || !self.base.same_variant(after, before) {
                shell.invalidate_layout();
                shell.request_redraw();
            }
            return;
        }
        // A finger presses and lifts as the left button does; iced moves the cursor to it.
        let (pressed, released) = match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => (true, false),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                (false, true)
            }
            Event::Mouse(_) | Event::Touch(_) => (false, false),
            // Laid out again after a scroll: the still pointer is over other boxes now.
            Event::Window(window::Event::RedrawRequested(_)) => {
                let state = tree.state.downcast_mut::<Interaction>();
                if std::mem::take(&mut state.rehover) && state.hovered != top {
                    state.hovered = top;
                    let key = (state.hovered, state.active, state.focused);
                    if !self.base.same_variant(key, self.key) {
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }
                }
                return;
            }
            _ => return,
        };
        // The same point over the same boxes as `top`: the children change neither.
        let hit = top;
        // A widget the press focused, as a box on the pressed chain, is what has `:focus`.
        let clicked_host = if pressed {
            self.focused_host(tree, layout, renderer, hit)
        } else {
            None
        };
        let focus_after_press = clicked_host.or_else(|| self.focus_target(hit));
        let state = tree.state.downcast_mut::<Interaction>();
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            if shell.is_event_captured() {
                return;
            }
            let (amount, amount_x) = match delta {
                mouse::ScrollDelta::Lines { x, y } => (-y * WHEEL_LINE, -x * WHEEL_LINE),
                mouse::ScrollDelta::Pixels { x, y } => (-y, -x),
            };
            // The nearest box that can still move takes the wheel, per axis.
            let nodes = self.tree.source.construction.nodes();
            let (mut left, mut left_x) = (amount != 0.0, amount_x != 0.0);
            let mut moved = false;
            let mut at = hit;
            // What the wheel is over moves only with its containing blocks.
            let mut position = None;
            while let Some(node) = at {
                let arena = self
                    .tree
                    .originals
                    .iter()
                    .position(|value| *value == node)
                    .and_then(|projected| self.tree.prepared.slot[projected]);
                let here = arena.map(|arena| self.tree.prepared.positions[arena].1);
                // Past a fixed box only the root, standing for the viewport, moves it on.
                let skipped = arena != Some(0)
                    && (position == Some(Position::Fixed)
                        || (position == Some(Position::Absolute)
                            && here == Some(Position::Static)));
                if !skipped && here.is_some() {
                    position = here;
                }
                if let Some(&(limit, limit_x)) = arena
                    .filter(|_| !skipped)
                    .and_then(|arena| self.scroll_limits.get(arena))
                {
                    let current = state
                        .scrolled
                        .get(&node)
                        .copied()
                        .unwrap_or(0.0)
                        .clamp(0.0, limit);
                    let current_x = state
                        .scrolled_x
                        .get(&node)
                        .copied()
                        .unwrap_or(0.0)
                        .clamp(0.0, limit_x);
                    let wanted = (current + amount).clamp(0.0, limit);
                    // An rtl box's offset grows leftward.
                    let rtl = arena.is_some_and(|arena| {
                        self.tree.prepared.boxes[arena].direction == Direction::Rtl
                    });
                    let toward_end = if rtl { -amount_x } else { amount_x };
                    let wanted_x = (current_x + toward_end).clamp(0.0, limit_x);
                    if left && wanted != current {
                        state.scrolled.insert(node, wanted);
                        (left, moved) = (false, true);
                    }
                    if left_x && wanted_x != current_x {
                        state.scrolled_x.insert(node, wanted_x);
                        (left_x, moved) = (false, true);
                    }
                }
                at = nodes[node].parent();
            }
            if moved {
                state.rehover = true;
                shell.capture_event();
                shell.invalidate_layout();
                shell.request_redraw();
            }
            return;
        }
        let mut next = state.clone();
        next.hovered = hit;
        if pressed {
            next.active = hit;
            next.focused = focus_after_press;
            if !shell.is_event_captured()
                && let Some(message) = self.press_message(hit)
            {
                shell.publish(message);
                shell.capture_event();
            }
        } else if released {
            next.active = None;
        }
        if next != *state {
            let key = (next.hovered, next.active, next.focused);
            *state = next;
            // Only states that a rule distinguishes need a new layout. Compare styles
            // before measuring: the current tree carries measured widths.
            if !self.base.same_variant(key, self.key) {
                shell.invalidate_layout();
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let top = cursor
            .position_in(layout.bounds())
            .and_then(|point| self.hit(point));
        let pressed = tree.state.downcast_ref::<Interaction>().active;
        let inner = self
            .elements
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .enumerate()
            .filter(|(index, _)| self.visible_slot(*index).is_some())
            .map(|(index, ((element, child), child_layout))| {
                element.as_widget().mouse_interaction(
                    child,
                    child_layout,
                    self.child_cursor(index, cursor, top, pressed),
                    viewport,
                    renderer,
                )
            })
            .max()
            .unwrap_or_default();
        if inner == mouse::Interaction::default() && top.is_some() {
            let hit = top;
            if let Some(chosen) = hit
                .and_then(|node| self.tree.originals.iter().position(|value| *value == node))
                .and_then(|projected| css_cursor(self.tree.paints[projected].cursor))
            {
                return chosen;
            }
            if self.press_message(hit).is_some() {
                return mouse::Interaction::Pointer;
            }
        }
        inner
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        // Layers are absolute in iced: one clipped to these bounds alone would paint past a
        // clipping ancestor, such as a scrollable this view is taller than.
        let Some(visible) = bounds.intersection(viewport) else {
            return;
        };
        let origin = bounds.position();
        let steps = &self.drawn;
        let top = cursor.position_in(bounds).and_then(|point| self.hit(point));
        let pressed = tree.state.downcast_ref::<Interaction>().active;
        let prepared = &self.tree.prepared;
        let clip_of = |step: &PaintStep| -> Rectangle {
            let arena = match *step {
                PaintStep::Box(index)
                | PaintStep::Outline(index)
                | PaintStep::Marker(index)
                | PaintStep::CellBorder(index) => prepared.slot[index],
                PaintStep::Text(arena) => Some(arena),
                PaintStep::Column(at) | PaintStep::ColumnGroup(at) => {
                    prepared.table_columns.get(at).map(|entry| entry.0)
                }
                PaintStep::Generated(at) => prepared.generated_boxes.get(at).map(|entry| entry.0),
            };
            let mut clip = arena.and_then(|arena| self.clips.get(arena).copied().flatten());
            // A box clips its own text as it clips its children.
            if let PaintStep::Text(arena) = *step
                && prepared.clipped[arena]
                && let Some(outer) = self.slot_geometry.get(arena)
            {
                clip = Some(narrowed(clip, outer.padding));
            }
            clip.map_or(visible, |clip| {
                visible
                    .intersection(&Rectangle {
                        x: clip.left + origin.x,
                        y: clip.top + origin.y,
                        width: (clip.right - clip.left).max(0.0),
                        height: (clip.bottom - clip.top).max(0.0),
                    })
                    .unwrap_or(Rectangle::new(origin, Size::ZERO))
            })
        };
        // 14.2: the root's background covers the whole canvas.
        if let Some((color, fade)) = prepared.positions.first().and_then(|(root, _)| {
            let paint = &self.tree.paints[*root];
            paint.background.map(|color| (color, paint.background_fade))
        }) {
            renderer.fill_quad(
                Quad {
                    bounds,
                    ..Quad::default()
                },
                Color::from_rgba(color[0], color[1], color[2], fade.unwrap_or(1.0)),
            );
        }
        // The canvas scrolls with the root, unlike any other scroller's own box: where the
        // root's box is drawn, shifted by its clamped scroll.
        let root_origin = {
            let interaction = tree.state.downcast_ref::<Interaction>();
            let root = prepared.positions.first().map(|(root, _)| *root);
            let original = root.and_then(|root| self.tree.originals.get(root));
            let limits = self.scroll_limits.first().copied().unwrap_or((0.0, 0.0));
            let scrolled = original
                .and_then(|original| interaction.scrolled.get(original))
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, limits.0);
            let across = original
                .and_then(|original| interaction.scrolled_x.get(original))
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, limits.1);
            let rtl = prepared
                .boxes
                .first()
                .is_some_and(|root| root.direction == Direction::Rtl);
            Point::new(
                if rtl {
                    origin.x + across
                } else {
                    origin.x - across
                },
                origin.y - scrolled,
            )
        };
        // 14.2: the root's image covers the canvas too, placed against the root box.
        let viewport_box = bounds;
        if let Some(background) = prepared
            .positions
            .first()
            .and_then(|(root, _)| self.loaded_background(*root))
            && let Some(Some(geometry)) = self.geometry.get(prepared.positions[0].0)
        {
            let positioning = if background.0.fixed {
                viewport_box
            } else {
                moved_rectangle(area_rect(geometry, background.0.origin), root_origin)
            };
            renderer.with_layer(visible, |renderer| {
                draw_background_image(renderer, background, positioning, visible);
            });
        }
        let transformations = self.transformations(origin);
        // A layer draws its text and images over its quads, so anything painted after them
        // needs a layer of its own, as does any change of clip.
        let mut start = 0;
        while start < steps.len() {
            let clip = clip_of(&steps[start]);
            let mut end = start + 1;
            while end < steps.len()
                && clip_of(&steps[end]) == clip
                && !breaks_layer(&self.tree, steps[end - 1], steps[end])
            {
                end += 1;
            }
            renderer.with_layer(clip, |renderer| {
                for step in &steps[start..end] {
                    // A box paints within its border box: one wholly outside the clip
                    // draws nothing.
                    if let PaintStep::Box(index) = *step
                        && index != prepared.positions[0].0
                        && let Some(Some(geometry)) = self.geometry.get(index)
                        && moved_rectangle(geometry.border, origin)
                            .intersection(&clip)
                            .is_none()
                    {
                        continue;
                    }
                    // CSS Transforms 1: drawn through its boxes' transforms.
                    let transformation = step_arena(prepared, *step)
                        .and_then(|arena| transformations.get(arena).copied().flatten());
                    let paint = |renderer: &mut Renderer| match *step {
                        PaintStep::Box(index) => {
                            let root = index == prepared.positions[0].0;
                            let image = self
                                .loaded_background(index)
                                .filter(|_| self.tree.paints[index].visible);
                            match (root, image, self.geometry.get(index)) {
                                // The canvas already has the root's background and image.
                                (true, _, _) => draw_box(
                                    &self.tree,
                                    &self.geometry,
                                    renderer,
                                    root_origin,
                                    index,
                                    BoxParts::Border,
                                ),
                                (false, Some(background), Some(Some(geometry))) => {
                                    draw_box(
                                        &self.tree,
                                        &self.geometry,
                                        renderer,
                                        origin,
                                        index,
                                        BoxParts::Background,
                                    );
                                    // 14.2.1: placed from the padding edge, painted out to the
                                    // border edge, under the border. CSS Backgrounds 3: from
                                    // its origin box, within its clip box.
                                    let placed = moved_rectangle(
                                        area_rect(geometry, background.0.origin),
                                        origin,
                                    );
                                    let positioning = if background.0.fixed {
                                        viewport_box
                                    } else {
                                        placed
                                    };
                                    let area = moved_rectangle(
                                        area_rect(geometry, background.0.clip),
                                        origin,
                                    )
                                    .intersection(&clip)
                                    .unwrap_or(Rectangle::new(origin, Size::ZERO));
                                    draw_background_image(renderer, background, positioning, area);
                                    renderer.with_layer(clip, |renderer| {
                                        draw_box(
                                            &self.tree,
                                            &self.geometry,
                                            renderer,
                                            origin,
                                            index,
                                            BoxParts::Border,
                                        );
                                    });
                                }
                                _ => draw_box(
                                    &self.tree,
                                    &self.geometry,
                                    renderer,
                                    origin,
                                    index,
                                    BoxParts::All,
                                ),
                            }
                        }
                        PaintStep::Generated(at) => {
                            if let Some(&(arena, paint)) = prepared.generated_boxes.get(at)
                                && let Some(geometry) = self.slot_geometry.get(arena)
                            {
                                paint_box(
                                    renderer,
                                    origin,
                                    paint,
                                    geometry,
                                    BoxParts::All,
                                    &[],
                                    None,
                                );
                            }
                        }
                        PaintStep::Column(at) => {
                            draw_column(
                                &self.tree,
                                &self.column_rects,
                                renderer,
                                origin,
                                at,
                                false,
                            );
                        }
                        PaintStep::ColumnGroup(at) => {
                            draw_column(&self.tree, &self.column_rects, renderer, origin, at, true);
                        }
                        PaintStep::CellBorder(index) => draw_box(
                            &self.tree,
                            &self.geometry,
                            renderer,
                            origin,
                            index,
                            BoxParts::CellBorder,
                        ),
                        PaintStep::Outline(index) => {
                            let inline =
                                prepared.slot[index].is_none_or(|slot| prepared.anonymous[slot]);
                            if inline {
                                self.draw_inline_outline(renderer, origin, index);
                            } else {
                                let origin = if index == prepared.positions[0].0 {
                                    root_origin
                                } else {
                                    origin
                                };
                                draw_outline(&self.tree, &self.geometry, renderer, origin, index);
                            }
                        }
                        PaintStep::Marker(index) => {
                            if let Some((handle, geometry)) = self
                                .tree
                                .marker_images
                                .get(&index)
                                .filter(|_| {
                                    self.loaded_markers.contains(&index)
                                        && self.tree.paints[index].visible
                                })
                                .zip(self.geometry.get(index).copied().flatten())
                            {
                                if let Some((width, height)) = renderer.background_size(handle) {
                                    let gap = self.tree.prepared.font_sizes[index] * 0.5;
                                    let rtl = self.tree.prepared.slot[index].is_some_and(|slot| {
                                        self.tree.prepared.boxes[slot].direction
                                            == iced_css_core::formatting::block::Direction::Rtl
                                    });
                                    let left = if rtl {
                                        geometry.border.right + gap
                                    } else {
                                        geometry.border.left - gap - width
                                    };
                                    renderer.draw_background(
                                        handle,
                                        Rectangle {
                                            x: origin.x + left,
                                            y: origin.y + geometry.content.top,
                                            width,
                                            height,
                                        },
                                        clip,
                                    );
                                }
                            } else if let Some((shaped, corner)) =
                                self.marker_paragraphs.get(&index)
                            {
                                let position = Point::new(origin.x + corner.x, origin.y + corner.y);
                                renderer.fill_paragraph(shaped, position, Color::BLACK, clip);
                            }
                        }
                        PaintStep::Text(arena) => {
                            if let Some((shaped, corner)) = self.paragraphs.get(&arena) {
                                let corner = Point::new(origin.x + corner.x, origin.y + corner.y);
                                if let Some(leaf) = self.tree.texts[arena].as_ref() {
                                    draw_run_decorations(leaf, shaped, renderer, corner);
                                    let rtl = self.tree.prepared.boxes[arena].direction
                                        == iced_css_core::formatting::block::Direction::Rtl;
                                    draw_marker_images(leaf, shaped, renderer, corner, clip, rtl);
                                }
                                renderer.fill_paragraph(shaped, corner, Color::BLACK, clip);
                            }
                            for (shaped, corner, leaf) in
                                self.rest_paragraphs.get(&arena).into_iter().flatten()
                            {
                                let corner = Point::new(origin.x + corner.x, origin.y + corner.y);
                                draw_run_decorations(leaf, shaped, renderer, corner);
                                renderer.fill_paragraph(shaped, corner, Color::BLACK, clip);
                            }
                            if let Some(index) = self.slot_owners.get(arena).copied().flatten()
                                && let Some(child_layout) = layout.children().nth(index)
                                && self.visible_slot(index).is_some()
                            {
                                self.elements[index].as_widget().draw(
                                    &tree.children[index],
                                    renderer,
                                    theme,
                                    style,
                                    child_layout,
                                    self.child_cursor(index, cursor, top, pressed),
                                    // Layers do not nest: a widget opening its own clips
                                    // to this, and must not paint past a clipping box.
                                    &clip,
                                );
                            }
                        }
                    };
                    match transformation {
                        Some(transformation) => {
                            renderer
                                .with_transformation(transformation, |renderer| paint(renderer));
                        }
                        None => paint(renderer),
                    }
                }
            });
            start = end;
        }
        // A plain position indicator at each scrolled box's right padding edge.
        renderer.with_layer(visible, |renderer| {
            let interaction = tree.state.downcast_ref::<Interaction>();
            for (arena, &(limit, limit_x)) in self.scroll_limits.iter().enumerate() {
                let Some(geometry) = self.slot_geometry.get(arena) else {
                    continue;
                };
                let node = self.tree.originals.get(prepared.positions[arena].0);
                let area = geometry.padding;
                let read = |map: &BTreeMap<usize, f32>, limit: f32| {
                    node.and_then(|node| map.get(node))
                        .copied()
                        .unwrap_or(0.0)
                        .clamp(0.0, limit)
                };
                // The bar is proportional: what shows, over all there is.
                let track = |shown: f32, limit: f32, offset: f32| {
                    let thumb = (shown * shown / (shown + limit)).max(8.0);
                    (thumb, (shown - thumb).max(0.0) * offset / limit)
                };
                let mut bars = Vec::new();
                if limit > 0.0 {
                    let end = if arena == 0 {
                        bounds.height
                    } else {
                        area.bottom
                    };
                    let shown = (end - area.top).max(1.0);
                    let (thumb, along) = track(shown, limit, read(&interaction.scrolled, limit));
                    let edge = if arena == 0 { bounds.width } else { area.right };
                    bars.push(Rectangle {
                        x: origin.x + edge - 4.0,
                        y: origin.y + area.top + along,
                        width: 4.0,
                        height: thumb,
                    });
                }
                if limit_x > 0.0 {
                    let end = if arena == 0 { bounds.width } else { area.right };
                    let shown = (end - area.left).max(1.0);
                    let (thumb, along) =
                        track(shown, limit_x, read(&interaction.scrolled_x, limit_x));
                    let bottom = if arena == 0 {
                        bounds.height
                    } else {
                        area.bottom
                    };
                    let rtl = prepared.boxes[arena].direction == Direction::Rtl;
                    bars.push(Rectangle {
                        x: if rtl {
                            origin.x + end - along - thumb
                        } else {
                            origin.x + area.left + along
                        },
                        y: origin.y + bottom - 4.0,
                        width: thumb,
                        height: 4.0,
                    });
                }
                for bar in bars {
                    // An ancestor's clip hides the bar along with the box.
                    let visible =
                        self.clips
                            .get(arena)
                            .copied()
                            .flatten()
                            .map_or(Some(bar), |clip| {
                                bar.intersection(&Rectangle {
                                    x: clip.left + origin.x,
                                    y: clip.top + origin.y,
                                    width: (clip.right - clip.left).max(0.0),
                                    height: (clip.bottom - clip.top).max(0.0),
                                })
                            });
                    if let Some(bounds) = visible {
                        renderer.fill_quad(
                            Quad {
                                bounds,
                                ..Quad::default()
                            },
                            Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                        );
                    }
                }
            }
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        // A widget without a visible box shows nothing over the view either.
        let shown: Vec<_> = (0..self.elements.len())
            .map(|index| self.visible_slot(index).is_some())
            .collect();
        let overlays: Vec<_> = self
            .elements
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .enumerate()
            .filter(|(index, _)| shown[*index])
            .filter_map(|(_, ((element, child), child_layout))| {
                element.as_widget_mut().overlay(
                    child,
                    child_layout,
                    renderer,
                    viewport,
                    translation,
                )
            })
            .collect();
        (!overlays.is_empty()).then(|| overlay::Group::with_children(overlays).overlay())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::adapters;
    use iced::{Background, Theme, Transformation};
    use iced_css_core::source::AttributeLimits;

    fn collapsed(runs: &[(&str, &'static str)]) -> Vec<String> {
        let mut texts: Vec<_> = runs
            .iter()
            .map(|&(text, mode)| (text.to_owned(), mode))
            .collect();
        collapse(&mut texts);
        texts.into_iter().map(|(text, _)| text).collect()
    }

    #[test]
    fn a_collapsible_space_after_a_preserved_one_stays() {
        assert_eq!(collapsed(&[("a ", "pre"), (" b", "normal")]), ["a ", " b"]);
        assert_eq!(collapsed(&[("a", "pre"), ("  b", "normal")]), ["a", " b"]);
    }

    #[test]
    fn a_form_feed_is_no_collapsible_space() {
        assert_eq!(collapsed(&[("a\x0Cb", "normal")]), ["a\x0Cb"]);
        assert_eq!(collapsed(&[("a \x0C b", "normal")]), ["a \x0C b"]);
    }

    #[test]
    fn a_preserved_newline_drops_the_space_ending_an_earlier_run() {
        assert_eq!(
            collapsed(&[("total ", "pre-line"), ("\n42", "pre-line")]),
            ["total", "\n42"]
        );
        assert_eq!(
            collapsed(&[
                ("total ", "normal"),
                ("", PLACEHOLDER),
                ("\n42", "pre-line")
            ]),
            ["total", "", "\n42"]
        );
        // A preserved space stays.
        assert_eq!(
            collapsed(&[("total ", "pre"), ("\n42", "pre-line")]),
            ["total ", "\n42"]
        );
    }

    #[derive(Default)]
    struct Recording(Vec<(Rectangle, Color)>);
    impl renderer::Renderer for Recording {
        fn start_layer(&mut self, _: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _: Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, quad: Quad, background: impl Into<Background>) {
            let Background::Color(color) = background.into() else {
                panic!("box paint is a solid color");
            };
            self.0.push((quad.bounds, color));
        }
        fn allocate_image(
            &mut self,
            _: &iced::advanced::image::Handle,
            _: impl FnOnce(Result<iced::advanced::image::Allocation, iced::advanced::image::Error>)
            + Send
            + 'static,
        ) {
        }
        fn reset(&mut self, _: Rectangle) {}
    }

    fn node(
        id: &str,
        children: Vec<CapturedElement<'static, (), Theme, Recording>>,
    ) -> CapturedElement<'static, (), Theme, Recording> {
        let bound = nz(1000);
        adapters::column_from_vec(children, capture_limits())
            .unwrap()
            .style_id(
                id,
                AttributeLimits {
                    entries: bound,
                    retained_bytes: bound,
                    work: bound,
                },
            )
            .unwrap()
            .erase()
    }

    #[test]
    fn draw_emits_backgrounds_then_border_bands_at_the_layout_origin() {
        let root = node("a", vec![node("b", vec![])]);
        let tree = BoxTree::new(
            root,
            "#a { background-color:#ff0000; height:50px; }
             #b { width:20px; height:10px; border-top:2px solid #0000ff; }",
        )
        .unwrap();
        let geometry = tree.layout(100.0, Some(80.0)).unwrap();
        let mut renderer = Recording::default();
        for index in tree.paint_order() {
            draw_box(
                &tree,
                &geometry,
                &mut renderer,
                Point::new(5.0, 7.0),
                index,
                BoxParts::All,
            );
        }
        let outlined = BoxTree::new(
            node("a", vec![]),
            "#a { height:10px; width:10px; outline:2px solid #00ff00; }",
        )
        .unwrap();
        let box_geometry = outlined.layout(100.0, None).unwrap();
        let mut outline = Recording::default();
        draw_outline(&outlined, &box_geometry, &mut outline, Point::ORIGIN, 0);
        assert_eq!(
            outline.0[0].0,
            Rectangle::new(Point::new(-2.0, -2.0), Size::new(14.0, 2.0))
        );
        assert_eq!(outline.0.len(), 4);
        assert_eq!(
            renderer.0,
            vec![
                (
                    Rectangle::new(Point::new(5.0, 7.0), Size::new(100.0, 50.0)),
                    Color::from_rgb(1.0, 0.0, 0.0)
                ),
                (
                    Rectangle::new(Point::new(5.0, 7.0), Size::new(20.0, 2.0)),
                    Color::from_rgb(0.0, 0.0, 1.0)
                ),
            ]
        );
    }

    impl text::Renderer for Recording {
        type Font = iced::Font;
        type Paragraph = ();
        type Editor = ();
        const ICON_FONT: iced::Font = iced::Font::DEFAULT;
        const CHECKMARK_ICON: char = '0';
        const ARROW_DOWN_ICON: char = '0';
        const SCROLL_UP_ICON: char = '0';
        const SCROLL_DOWN_ICON: char = '0';
        const SCROLL_LEFT_ICON: char = '0';
        const SCROLL_RIGHT_ICON: char = '0';
        const ICED_LOGO: char = '0';
        fn default_font(&self) -> iced::Font {
            iced::Font::default()
        }
        fn default_size(&self) -> iced::Pixels {
            iced::Pixels(16.0)
        }
        fn fill_paragraph(&mut self, _: &(), _: Point, _: Color, _: Rectangle) {}
        fn fill_editor(&mut self, _: &(), _: Point, _: Color, _: Rectangle) {}
        fn fill_text(&mut self, _: text::Text, _: Point, _: Color, _: Rectangle) {}
    }

    #[cfg(not(feature = "image-without-codecs"))]
    #[test]
    fn draw_skips_boxes_wholly_outside_their_clip() {
        let root = node(
            "page",
            vec![node("clip", vec![node("in", vec![]), node("out", vec![])])],
        );
        let tree = BoxTree::new(
            root,
            "#page { background-color:#000; }
             #clip { overflow:hidden; height:10px; }
             #in { height:10px; background-color:#f00; }
             #out { height:10px; background-color:#00f; }",
        )
        .unwrap();
        let mut element = tree.view::<(), Theme, Recording>();
        let mut state = iced::advanced::widget::Tree::new(&element);
        let limits = layout::Limits::new(Size::ZERO, Size::new(100.0, 80.0));
        let mut renderer = Recording::default();
        let laid = element
            .as_widget_mut()
            .layout(&mut state, &renderer, &limits);
        element.as_widget().draw(
            &state,
            &mut renderer,
            &Theme::Light,
            &renderer::Style::default(),
            Layout::new(&laid),
            mouse::Cursor::Unavailable,
            &Rectangle::new(Point::ORIGIN, Size::new(100.0, 80.0)),
        );
        let colors: Vec<Color> = renderer.0.iter().map(|quad| quad.1).collect();
        assert!(colors.contains(&Color::from_rgb(1.0, 0.0, 0.0)));
        assert!(
            !colors.contains(&Color::from_rgb(0.0, 0.0, 1.0)),
            "#out is clipped away"
        );
        // The root paints the canvas wherever it is.
        assert!(colors.contains(&Color::from_rgb(0.0, 0.0, 0.0)));
    }

    #[test]
    fn an_rtl_scroller_scrolls_toward_its_left_end() {
        let tree = BoxTree::new(
            node("page", vec![node("box", vec![node("wide", vec![])])]),
            "#box { direction:rtl; overflow:auto; width:100px; height:40px; padding-left:5px; }
             #wide { width:300px; height:40px; }",
        )
        .unwrap();
        let mut geometry: Vec<FlowGeometry> = tree
            .layout(800.0, Some(600.0))
            .unwrap()
            .into_iter()
            .flatten()
            .collect();
        let wide = geometry[2].border;
        let limits = apply_scrolling(
            &tree.prepared,
            &mut geometry,
            |arena| (0.0, if arena == 1 { 30.0 } else { 0.0 }),
            |_, _| None,
            |_, _| None,
            Size::new(800.0, 600.0),
        );
        // 300px of contents overflow the 105px padding box by 195px; its 5px end padding
        // follows them.
        assert_eq!(limits[1].1, 200.0);
        assert_eq!(geometry[2].border.left, wide.left + 30.0);
        assert_eq!(geometry[2].border.right, wide.right + 30.0);
    }

    /// The scroll limits of `#box`, arena slot 1, under `css`.
    fn clipped_limits(
        children: Vec<CapturedElement<'static, (), Theme, Recording>>,
        css: &str,
    ) -> (f32, f32) {
        let tree = BoxTree::new(node("page", vec![node("box", children)]), css).unwrap();
        let mut geometry: Vec<FlowGeometry> = tree
            .layout(800.0, Some(600.0))
            .unwrap()
            .into_iter()
            .flatten()
            .collect();
        let positions = &tree.prepared.positions;
        apply_scrolling(
            &tree.prepared,
            &mut geometry,
            |_| (0.0, 0.0),
            |_, _| None,
            |arena, outer| {
                own_clip(
                    tree.paints[positions[arena].0].clip,
                    positions[arena].1,
                    outer.border,
                )
            },
            Size::new(800.0, 600.0),
        )[1]
    }

    #[test]
    fn a_percentage_indent_replaces_its_spacer_on_every_layout() {
        let mut tree = BoxTree::new(
            node("p", vec![]),
            "#p { width:200px; text-indent:25%; } #p:before { content:\"words\"; }",
        )
        .unwrap();
        let geometry = tree
            .layout_arena(
                800.0,
                None,
                &mut |_, _| Some(20.0),
                &mut |_, _, _, _| None,
                &mut |_, _, _| None,
            )
            .unwrap();
        let spacers = |tree: &BoxTree| {
            tree.texts
                .iter()
                .flatten()
                .map(|leaf| leaf.runs.iter().filter(|run| run.spacer).count())
                .sum::<usize>()
        };
        assert!(tree.apply_percent_indents(&geometry));
        assert_eq!(spacers(&tree), 1);
        // The same width again: the same spacer, not a second one.
        assert!(!tree.apply_percent_indents(&geometry));
        assert_eq!(spacers(&tree), 1);
    }

    #[test]
    fn only_leaves_a_percentage_can_rewrite_are_copied_for_the_second_layout() {
        let plain = BoxTree::new(
            node("p", vec![]),
            "#p { width:200px; } #p:before { content:\"words\"; }",
        )
        .unwrap();
        assert!(plain.texts.iter().flatten().count() > 0);
        assert!(plain.percent_leaves().is_empty());
        for css in [
            "#p { width:200px; text-indent:25%; } #p:before { content:\"words\"; }",
            "#p { width:200px; text-indent:-25%; } #p:before { content:\"words\"; }",
            "#p { width:200px; } #p:before { content:\"words\"; padding:10%; margin:0 5%; }",
            "#p { width:200px; direction:rtl; }
             #p:before { content:\"words\"; padding-left:10%; }",
        ] {
            let mut tree = BoxTree::new(node("p", vec![]), css).unwrap();
            let geometry = tree
                .layout_arena(
                    800.0,
                    None,
                    &mut |_, _| Some(20.0),
                    &mut |_, _, _, _| None,
                    &mut |_, _, _| None,
                )
                .unwrap();
            let all = tree.texts.clone();
            let copied = tree.percent_leaves();
            tree.apply_percent_indents(&geometry);
            let rewritten: Vec<usize> = (0..all.len())
                .filter(|arena| all[*arena] != tree.texts[*arena])
                .collect();
            assert!(!rewritten.is_empty(), "{css}");
            for arena in rewritten {
                assert!(copied.iter().any(|(at, _)| *at == arena), "{css}");
            }
        }
    }

    #[test]
    fn clip_stops_what_a_scroller_can_reach() {
        let scroller = "#box { overflow:auto; width:100px; height:40px; position:relative; }";
        // Cut to 150 by 60 from 300 by 90: 50 across and 20 down past the 100 by 40 box.
        let limits = clipped_limits(
            vec![node("wide", vec![])],
            &format!(
                "{scroller} #wide {{ position:absolute; top:0; left:0; width:300px; height:90px;
                 clip:rect(0px, 150px, 60px, 0px); }}"
            ),
        );
        assert_eq!(limits, (20.0, 50.0));
        // A descendant of the clipped box: cut by its clip, and nothing where it shows none.
        let inner = |left: &str| {
            clipped_limits(
                vec![node("a", vec![node("b", vec![])])],
                &format!(
                    "{scroller} #a {{ position:absolute; top:0; left:0; width:50px; height:10px;
                     clip:rect(0px, 50px, 300px, 0px); }}
                     #b {{ position:absolute; top:0; left:{left}; width:100px; height:150px; }}"
                ),
            )
        };
        assert_eq!(inner("30px"), (110.0, 0.0));
        assert_eq!(inner("500px"), (0.0, 0.0));
        // Two disjoint clips leave nothing to show, though each side alone overlaps.
        let limits = clipped_limits(
            vec![node("a", vec![node("c", vec![])])],
            &format!(
                "{scroller} #a {{ position:absolute; top:0; left:0; width:200px; height:10px;
                 clip:rect(0px, 50px, 300px, 0px); }}
                 #c {{ position:absolute; top:0; left:0; width:200px; height:300px;
                 clip:rect(0px, 200px, 300px, 100px); }}"
            ),
        );
        assert_eq!(limits, (0.0, 0.0));
        // The same one above the other, and two clips meeting at the nearer right side.
        let nested = |outer: &str, inner: &str, width: &str| {
            clipped_limits(
                vec![node("a", vec![node("c", vec![])])],
                &format!(
                    "{scroller} #a {{ position:absolute; top:0; left:0; width:80px; height:10px;
                     clip:rect({outer}); }}
                     #c {{ position:absolute; top:0; left:0; width:{width}; height:300px;
                     clip:rect({inner}); }}"
                ),
            )
        };
        assert_eq!(
            nested("0px, 300px, 50px, 0px", "100px, 300px, 300px, 0px", "50px"),
            (0.0, 0.0)
        );
        assert_eq!(
            nested("0px, 150px, 30px, 0px", "0px, 80px, 30px, 0px", "200px"),
            (0.0, 0.0)
        );
        // In rtl the clip's left side stops the reach leftward.
        let limits = clipped_limits(
            vec![node("wide", vec![])],
            &format!(
                "{scroller} #box {{ direction:rtl; }}
                 #wide {{ position:absolute; top:0; right:0; width:300px; height:10px;
                 clip:rect(0px, 300px, 10px, 150px); }}"
            ),
        );
        assert_eq!(limits, (0.0, 50.0));
    }

    #[test]
    fn right_aligned_text_extends_an_rtl_scroller_leftward() {
        let tree = BoxTree::new(
            node("page", vec![node("box", vec![node("line", vec![])])]),
            "#box { direction:rtl; overflow:auto; width:100px; height:40px; }",
        )
        .unwrap();
        let mut geometry: Vec<FlowGeometry> = tree
            .layout(800.0, Some(600.0))
            .unwrap()
            .into_iter()
            .flatten()
            .collect();
        // A 260px line right-aligned in the 100px box reaches 160px past its left.
        let limits = apply_scrolling(
            &tree.prepared,
            &mut geometry,
            |_| (0.0, 0.0),
            |arena, _| {
                (arena == 2).then_some(TextExtent {
                    height: 10.0,
                    from: -160.0,
                    to: 100.0,
                })
            },
            |_, _| None,
            Size::new(800.0, 600.0),
        );
        assert_eq!(limits[1].1, 160.0);
    }

    #[test]
    fn a_scrollers_own_text_moves_with_its_offset() {
        let tree = BoxTree::new(
            node("page", vec![node("line", vec![])]),
            "#line { overflow:auto; width:60px; height:20px; }",
        )
        .unwrap();
        let mut geometry: Vec<FlowGeometry> = tree
            .layout(800.0, Some(600.0))
            .unwrap()
            .into_iter()
            .flatten()
            .collect();
        let left = geometry[1].content.left;
        let padding = geometry[1].padding;
        let limits = apply_scrolling(
            &tree.prepared,
            &mut geometry,
            |_| (0.0, 15.0),
            |arena, _| {
                (arena == 1).then_some(TextExtent {
                    height: 10.0,
                    from: 0.0,
                    to: 100.0,
                })
            },
            |_, _| None,
            Size::new(800.0, 600.0),
        );
        assert_eq!(limits[1].1, 40.0);
        assert_eq!(geometry[1].content.left, left - 15.0);
        assert_eq!(geometry[1].padding, padding);
    }

    #[test]
    fn a_local_image_url_is_a_relative_url_under_the_base() {
        let root = std::env::temp_dir().join(format!("local-image-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let base = root.join("base");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("my tile.png"), b"").unwrap();
        let at = |name: &str| Some(image::Handle::from_path(base.join(name)));
        assert_eq!(local_image(&base, "my%20tile.png"), at("my tile.png"));
        assert_eq!(local_image(&base, "tile.png?v=2"), at("tile.png"));
        assert_eq!(local_image(&base, "sprite.png#icon"), at("sprite.png"));
        // Escapes decode to no separators and no way out.
        assert_eq!(local_image(&base, "a%2Fb.png"), None);
        assert_eq!(local_image(&base, "%2e%2e/x.png"), None);
        assert_eq!(local_image(&base, "a%00.png"), None);
        #[cfg(unix)]
        {
            let outside = root.join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(outside.join("private.png"), b"").unwrap();
            std::os::unix::fs::symlink(&outside, base.join("link")).unwrap();
            assert_eq!(local_image(&base, "link/private.png"), None);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_scroller_shrunk_below_its_content_never_gets_a_negative_limit() {
        let tree = BoxTree::new(
            node("page", vec![node("line", vec![])]),
            "#line { overflow:auto; width:60px; height:20px; }",
        )
        .unwrap();
        let mut geometry: Vec<FlowGeometry> = tree
            .layout(800.0, Some(600.0))
            .unwrap()
            .into_iter()
            .flatten()
            .collect();
        // As a hidden row's stretch leaves a cell: its edges above its content's end.
        geometry[1].padding.bottom -= 20.0;
        geometry[1].border.bottom -= 20.0;
        let limits = apply_scrolling(
            &tree.prepared,
            &mut geometry,
            |_| (5.0, 0.0),
            |arena, _| {
                (arena == 1).then_some(TextExtent {
                    height: 10.0,
                    from: 0.0,
                    to: 10.0,
                })
            },
            |_, _| None,
            Size::new(800.0, 600.0),
        );
        assert_eq!(limits[1], (0.0, 0.0));
    }

    #[test]
    fn a_hidden_empty_cell_still_draws_for_a_hosted_widget() {
        let tree = BoxTree::new(
            node("t", vec![node("r", vec![node("c", vec![])])]),
            "#t { display:table; empty-cells:hide; }
             #r { display:table-row; }
             #c { display:table-cell; background-color:#ff0000; }",
        )
        .unwrap();
        let drawn = |embedded: &BTreeSet<usize>| {
            tree.drawn_steps(tree.paint_steps(embedded), embedded)
                .iter()
                .any(|step| matches!(step, PaintStep::Box(2)))
        };
        assert!(!drawn(&BTreeSet::new()));
        // Input still finds it.
        assert!(
            tree.paint_steps(&BTreeSet::new())
                .iter()
                .any(|step| matches!(step, PaintStep::Box(2)))
        );
        let slot = tree.prepared.slot[2].unwrap();
        assert!(drawn(&BTreeSet::from([slot])));
    }

    #[test]
    fn a_shifted_slot_does_not_lend_a_presented_color_to_its_new_owner() {
        type Node = CapturedElement<'static, (), Theme, iced::Renderer>;
        let bound = nz(1000);
        let limits = AttributeLimits {
            entries: bound,
            retained_bytes: bound,
            work: bound,
        };
        let line = |id: &str, words: &'static str| -> Node {
            adapters::text(words, capture_limits())
                .unwrap()
                .style_id(id, limits)
                .unwrap()
                .erase()
        };
        let root: Node = adapters::column_from_vec(
            vec![line("y", "gone"), line("x", "moves"), line("z", "fixed")],
            capture_limits(),
        )
        .unwrap()
        .style_id("page", limits)
        .unwrap()
        .erase();
        let sheet = |color: &str| {
            format!(
                "#y, #x, #z {{ display:block; }}
                 #y:hover {{ display:none; }}
                 #x {{ color:{color}; }}
                 #z {{ color:#ffffff; }}"
            )
        };
        let from = BoxTree::new(root, &sheet("#000000")).unwrap();
        let to = from.restyled(&[&sheet("#ffffff")]).unwrap();
        let mut animations = BoxTreeAnimations::new();
        let start = std::time::Instant::now();
        let second = crate::animation::clock::AnimationTiming::new(
            std::time::Duration::from_secs(1),
            std::time::Duration::ZERO,
        );
        animations
            .request(&from, &to, "x", &["color"], second, start)
            .unwrap();
        animations.advance(start + std::time::Duration::from_millis(500), &to);
        let hovered = animations
            .presented(&to)
            .variant((Some(to.originals[1]), None, None));
        let leaves = hovered.text_leaves();
        let fixed = leaves
            .iter()
            .find(|(_, leaf)| leaf.text() == "fixed")
            .unwrap();
        assert_eq!(fixed.1.runs[0].color, Some([1.0; 3]));
    }

    #[test]
    fn hovering_starts_the_transitions_its_styling_asks_for() {
        let tree = BoxTree::new(
            node("page", vec![node("a", vec![]), node("b", vec![])]),
            "#a, #b { width:100px; height:10px; }
             #a:hover { width:200px; transition: width 1s linear; }
             #b:hover { width:300px; }",
        )
        .unwrap();
        let by_id = |id: &str| {
            tree.originals[tree
                .ids
                .iter()
                .position(|value| value.as_deref() == Some(id))
                .unwrap()]
        };
        let (a, b) = (by_id("a"), by_id("b"));
        let width = |shown: &BoxTree, at: usize| {
            let geometry = shown.layout(600.0, None).unwrap();
            let border = geometry[at].unwrap().border;
            border.right - border.left
        };
        let start = Instant::now();
        let at = |millis: u64| start + std::time::Duration::from_millis(millis);
        let mut state = Interaction::default();
        let none = (None, None, None);
        // The first styling seen starts nothing.
        let (shown, moving) = state.styled_for(&tree, none, start);
        assert_eq!((width(&shown, 1), moving), (100.0, false));
        // Hovering `a`: its hover rule asks for a transition.
        let (_, moving) = state.styled_for(&tree, (Some(a), None, None), start);
        assert!(moving);
        assert!(state.advance_transitions(&tree, (Some(a), None, None), at(500)));
        let (shown, _) = state.styled_for(&tree, (Some(a), None, None), at(500));
        assert_eq!(width(&shown, 1), 150.0);
        // Leaving it halfway: no rule asks for one back, so it applies at once.
        let (shown, _) = state.styled_for(&tree, none, at(500));
        assert_eq!(width(&shown, 1), 100.0);
        // `b` asks for none.
        let (shown, moving) = state.styled_for(&tree, (Some(b), None, None), at(600));
        assert_eq!((width(&shown, 2), moving), (300.0, false));
        // Hovering `a` again runs to the end, and then shows the hover styling alone.
        let (_, moving) = state.styled_for(&tree, (Some(a), None, None), at(1000));
        assert!(moving);
        assert!(state.advance_transitions(&tree, (Some(a), None, None), at(2100)));
        assert!(!state.advance_transitions(&tree, (Some(a), None, None), at(2200)));
        let (shown, moving) = state.styled_for(&tree, (Some(a), None, None), at(2100));
        assert_eq!((width(&shown, 1), moving), (200.0, false));
    }

    #[test]
    fn the_widget_runs_the_animations_its_styling_names() {
        let tree = BoxTree::new(
            node("page", vec![node("a", vec![])]),
            "@keyframes grow { from { width: 100px } to { width: 300px } }
             #a { width:50px; height:10px; }
             #a:hover { animation: grow 1s linear; }",
        )
        .unwrap();
        let a = tree.originals[tree
            .ids
            .iter()
            .position(|value| value.as_deref() == Some("a"))
            .unwrap()];
        let width = |shown: &BoxTree| {
            let geometry = shown.layout(600.0, None).unwrap();
            let border = geometry[1].unwrap().border;
            border.right - border.left
        };
        let start = Instant::now();
        let at = |millis: u64| start + std::time::Duration::from_millis(millis);
        let mut state = Interaction::default();
        let none = (None, None, None);
        let hovered = (Some(a), None, None);
        let (shown, presenting) = state.styled_for(&tree, none, start);
        assert_eq!((width(&shown), presenting), (50.0, false));
        // Hovering names the animation: it starts there.
        let (shown, presenting) = state.styled_for(&tree, hovered, start);
        assert_eq!((width(&shown), presenting), (100.0, true));
        assert!(state.advance_transitions(&tree, hovered, at(500)));
        let (shown, _) = state.styled_for(&tree, hovered, at(500));
        assert_eq!(width(&shown), 200.0);
        // Its end shows the element's own value again, and frames stop.
        assert!(state.advance_transitions(&tree, hovered, at(1100)));
        let (shown, presenting) = state.styled_for(&tree, hovered, at(1100));
        assert_eq!((width(&shown), presenting), (50.0, false));
        assert!(!state.advance_transitions(&tree, hovered, at(1200)));
    }

    #[test]
    fn linear_gradients_compute_and_fit_iced_s_gradient_line() {
        let tree = BoxTree::new(
            node("page", vec![node("a", vec![]), node("b", vec![])]),
            "#a { color:#00ff00; background-image: linear-gradient(to right, #ff0000, currentColor 25%, rgba(0, 0, 255, 0.5)); }
             #b { background: linear-gradient(45deg, #000000, #ffffff); }",
        )
        .unwrap();
        let a = tree.gradients[1].as_deref().unwrap();
        assert_eq!(a.aim, GradientAim::Toward(1, 0));
        assert_eq!(a.stops[1].0, [0.0, 1.0, 0.0]);
        assert!((a.stops[2].1 - 0.5).abs() < 0.01);
        assert_eq!(a.stops[1].2, Some(Dim::Percent(25.0)));
        let offsets = |linear: iced::gradient::Linear| {
            linear
                .stops
                .iter()
                .flatten()
                .map(|stop| (stop.offset, stop.color.r))
                .collect::<Vec<_>>()
        };
        // Across: the lines agree.
        let across = linear_gradient(a, 200.0, 100.0, 1.0).unwrap();
        assert_eq!(
            offsets(across)
                .iter()
                .map(|stop| stop.0)
                .collect::<Vec<_>>(),
            [0.0, 0.25, 1.0]
        );
        // Diagonal on a square: CSS's line (the corners' projections, 141) is twice iced's
        // (the larger half-extent along it, 71), so its ends fall beyond iced's and are cut
        // there, at the colors a quarter of the way in.
        let b = tree.gradients[2].as_deref().unwrap();
        let diagonal = offsets(linear_gradient(b, 100.0, 100.0, 1.0).unwrap());
        let reach = 0.25;
        assert_eq!(diagonal.len(), 2);
        assert!((diagonal[0].1 - reach).abs() < 1e-4);
        assert!((diagonal[1].1 - (1.0 - reach)).abs() < 1e-4);
        // A corner, with stops placed between their neighbours.
        let corner = GradientPaint {
            aim: GradientAim::Toward(1, 1),
            stops: vec![
                ([0.0; 3], 1.0, None),
                ([0.5; 3], 1.0, None),
                ([1.0; 3], 1.0, None),
            ],
        };
        let linear = linear_gradient(&corner, 100.0, 100.0, 1.0).unwrap();
        assert!((linear.angle.0 - 3.0 * std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        assert_eq!(offsets(linear)[1], (0.5, 0.5));
    }

    #[test]
    fn only_boxes_a_rule_can_select_make_a_new_styling() {
        let tree = |css: &str| {
            BoxTree::new(
                node(
                    "page",
                    vec![node("a", vec![node("in_a", vec![])]), node("b", vec![])],
                ),
                css,
            )
            .unwrap()
        };
        let by_id = |tree: &BoxTree, id: &str| {
            tree.originals[tree
                .ids
                .iter()
                .position(|value| value.as_deref() == Some(id))
                .unwrap()]
        };
        let none = (None, None, None);
        let css = "#a:hover #in_a { color:#ff0000; }";
        let shown = tree(css);
        let (a, in_a, b) = (
            by_id(&shown, "a"),
            by_id(&shown, "in_a"),
            by_id(&shown, "b"),
        );
        let normal = |key| shown.source.normal_key(key);
        assert_eq!(normal((Some(b), None, None)), none);
        assert_eq!(normal((Some(in_a), None, None)), (Some(a), None, None));
        assert_eq!(normal((None, Some(a), None)), none);
        // Hover inside #a still reaches the rule.
        let styled = shown.variant((Some(in_a), None, None));
        let target = styled
            .ids
            .iter()
            .position(|value| value.as_deref() == Some("in_a"))
            .unwrap();
        assert_eq!(
            styled.paints[styled.prepared.positions[target].0].color,
            Some([1.0, 0.0, 0.0])
        );
        // Focus stays on its own box.
        let shown = tree("#a:focus { color:#ff0000; }");
        assert_eq!(shown.source.normal_key((None, None, Some(in_a))), none);
        assert_eq!(
            shown.source.normal_key((None, None, Some(a))),
            (None, None, Some(a))
        );
        // What cannot be read reaches everything.
        for css in [
            "*:hover { color:#ff0000; }",
            ":hover { color:#ff0000; }",
            "div:hover { color:#ff0000; }",
            "[a=\"b c\"]:hover { color:#ff0000; }",
            "#a { color:#ff0000; } #\\62:hover { color:#00ff00; }",
        ] {
            let shown = tree(css);
            assert_eq!(
                shown.source.normal_key((Some(b), None, None)),
                (Some(b), None, None),
                "{css}"
            );
        }
        // Text that decoding changes: a NUL in a name, or wide characters under `@charset`.
        for css in [
            "#a\0:hover { color:#ff0000; }",
            "@\0c\0h\0a\0r\0s\0e\0t\0 \0\"\0U\0T\0F\0-\x001\x006\0L\0E\0\"\0;\0#\0a\0:\0h\0o\0v\0e\0r\0{\0c\0o\0l\0o\0r\0:\0r\0e\0d\0}\0",
        ] {
            let reach = PseudoReach::of(&[css.to_owned()], ":hover");
            assert!(reach.any, "{css:?}");
        }
        // Whitespace of more than one byte before the name, in a selector or a comment.
        for pseudo in [":hover", ":active", ":focus"] {
            for space in ['\u{a0}', '\u{85}', '\u{2003}', '\u{3000}'] {
                let reach = PseudoReach::of(&[format!("a{space}b{pseudo}{{color:red}}")], pseudo);
                assert!(reach.any, "{space:?} {pseudo}");
                let reach = PseudoReach::of(
                    &[format!(
                        "/* x{space}{pseudo} */ p{space}#a{pseudo}{{color:red}}"
                    )],
                    pseudo,
                );
                assert!(reach.any, "{space:?} {pseudo}");
                assert!(reach.ids.contains("a"), "{space:?} {pseudo}");
            }
        }
        assert!(BoxTree::new(node("a", vec![]), "a\u{a0}b:hover{color:red}").is_ok());
        // A comment after the colon, and a name whose lowercase depends on its context.
        let shown = tree("#a:/**/hover { color:#ff0000; }");
        assert_eq!(
            shown.source.normal_key((Some(b), None, None)),
            (Some(b), None, None)
        );
        let named = BoxTree::new(
            node("\u{391}\u{3a3}", vec![]),
            "#\u{391}\u{3a3}:hover { color:#ff0000; }",
        )
        .unwrap();
        let only = named.originals[0];
        assert_eq!(
            named.source.normal_key((Some(only), None, None)),
            (Some(only), None, None)
        );
        // What is not in the box's paint still counts as a difference.
        for css in [
            "#a:hover { background-image:url(b.png); }",
            "#a { display:list-item; } #a:hover { list-style-type:square; }",
        ] {
            let shown = tree(css);
            assert!(!shown.same_variant((Some(a), None, None), none), "{css}");
        }
        // No rule at all: nothing differs, and nothing cascades again.
        let shown = tree("#a { color:#ff0000; }");
        assert!(shown.same_variant((Some(a), Some(b), Some(a)), none));
    }

    #[test]
    fn the_styling_cache_drops_the_one_unused_longest_and_keeps_the_one_shown() {
        let count = VARIANT_LIMIT + 6;
        let children: Vec<_> = (0..count)
            .map(|at| node(&format!("c{at}"), vec![]))
            .collect();
        let css: String = (0..count)
            .map(|at| format!("#c{at}:hover {{ color:#ff0000; }}"))
            .collect();
        let shown = BoxTree::new(node("page", children), &css).unwrap();
        let card = |at: usize| {
            let id = format!("c{at}");
            let found = shown
                .ids
                .iter()
                .position(|value| value.as_deref() == Some(&*id));
            (Some(shown.originals[found.unwrap()]), None, None)
        };
        let cached = |key: InteractionKey| {
            let variants = shown.source.variants.lock().unwrap();
            variants
                .styles
                .get(&key)
                .map(|(_, style)| Arc::clone(style))
        };
        let none = (None, None, None);
        // The sheets are parsed once, and the styling with no interaction is kept: a first
        // hover cascades the hovered state alone.
        assert!(shown.source.parsed.get().is_some());
        let unstated = cached(none).expect("the first styling is kept");
        assert!(!shown.same_variant(card(0), none));
        assert!(Arc::ptr_eq(&unstated, &cached(none).unwrap()));
        // A sweep over more boxes than the cache holds, one box shown throughout.
        let first = cached(card(0)).unwrap();
        for at in 1..count {
            assert!(!shown.same_variant(card(at), card(0)));
            assert!(Arc::ptr_eq(&first, &cached(card(0)).unwrap()), "{at}");
        }
        assert_eq!(
            shown.source.variants.lock().unwrap().styles.len(),
            VARIANT_LIMIT
        );
        // The latest stay, the earliest went, and not everything at once.
        assert!(cached(card(count - 1)).is_some());
        assert!(cached(card(count - 2)).is_some());
        assert!(cached(none).is_none());
        assert!(cached(card(1)).is_none());
    }

    #[test]
    fn a_cascade_too_large_to_finish_is_refused() {
        let children: Vec<_> = (0..20_000)
            .map(|at| node(&format!("c{at}"), vec![]))
            .collect();
        let css = "a{color:red}".repeat(6_000);
        let refused = BoxTree::new(node("page", children), &css).unwrap_err();
        assert!(refused.to_string().contains("RuleVisits"), "{refused}");
        // A sheet well past 100,000 characters is ordinary.
        let css = "a{color:red}".repeat(10_000);
        assert!(BoxTree::new(node("page", vec![]), &css).is_ok());
    }

    #[test]
    fn double_sides_join_into_two_rings() {
        let border = FlowRect {
            left: 0.0,
            right: 30.0,
            top: 0.0,
            bottom: 30.0,
        };
        let padding = FlowRect {
            left: 9.0,
            right: 21.0,
            top: 9.0,
            bottom: 21.0,
        };
        let doubles = double_sides(border, padding, ["double"; 4]);
        assert_eq!(doubles, [true; 4]);
        let pieces: Vec<Vec<Rectangle>> = border_bands(border, padding)
            .into_iter()
            .enumerate()
            .map(|(side, band)| {
                let mut pieces = band_pieces(band, side, "double", [0.0; 3]);
                join_double(&mut pieces, side, border, padding, doubles);
                pieces.into_iter().map(|piece| piece.0).collect()
            })
            .collect();
        // The top's inner line stops at the sides' inner lines; the left's outer line runs
        // up to the top's outer line, leaving no notch.
        assert_eq!(pieces[0][0], rectangle(0.0, 30.0, 0.0, 3.0));
        assert_eq!(pieces[0][1], rectangle(6.0, 24.0, 6.0, 9.0));
        assert_eq!(pieces[3][0], rectangle(0.0, 3.0, 3.0, 27.0));
        assert_eq!(pieces[3][1], rectangle(6.0, 9.0, 9.0, 21.0));
        assert_eq!(pieces[2][1], rectangle(6.0, 24.0, 21.0, 24.0));
        assert_eq!(pieces[1][0], rectangle(27.0, 30.0, 3.0, 27.0));
        // Beside a solid side, a double one keeps its lines as they were.
        let mixed = double_sides(border, padding, ["double", "solid", "double", "solid"]);
        let mut top = band_pieces(border_bands(border, padding)[0], 0, "double", [0.0; 3]);
        join_double(&mut top, 0, border, padding, mixed);
        assert_eq!(top[1].0, rectangle(0.0, 30.0, 6.0, 9.0));
    }

    #[test]
    fn border_styles_cut_a_band_into_its_pattern() {
        let top = Rectangle::new(Point::new(10.0, 20.0), Size::new(30.0, 3.0));
        let left = Rectangle::new(Point::new(10.0, 20.0), Size::new(3.0, 30.0));
        let bottom = Rectangle::new(Point::new(10.0, 20.0), Size::new(30.0, 4.0));
        let color = [0.5; 3];
        assert_eq!(band_pieces(top, 0, "solid", color), vec![(top, color)]);
        // Three long dashes with gaps of two widths, the last cut to the band's end.
        let dashes = band_pieces(top, 0, "dashed", color);
        let spans: Vec<(f32, f32)> = dashes
            .iter()
            .map(|piece| (piece.0.x, piece.0.width))
            .collect();
        assert_eq!(
            spans,
            vec![(10.0, 9.0), (25.0, 9.0), (40.0, 0.0)]
                .into_iter()
                .filter(|span| span.1 > 0.0)
                .collect::<Vec<_>>()
        );
        let dots = band_pieces(left, 3, "dotted", color);
        assert_eq!(dots.len(), 5);
        assert_eq!(
            dots[1].0,
            Rectangle::new(Point::new(10.0, 26.0), Size::new(3.0, 3.0))
        );
        let double = band_pieces(top, 0, "double", color);
        assert_eq!(double[0].0.y, 20.0);
        assert_eq!(double[1].0.y, 22.0);
        // Too thin for two lines and a space.
        let thin = Rectangle::new(Point::new(0.0, 0.0), Size::new(30.0, 2.0));
        assert_eq!(band_pieces(thin, 0, "double", color).len(), 1);
        // The dark half is outside for a groove, at the far edge on a bottom side.
        let groove = band_pieces(bottom, 2, "groove", color);
        assert_eq!(groove[0].1, [0.3; 3]);
        assert_eq!(groove[0].0.y, 22.0);
        assert_eq!(groove[1].0.y, 20.0);
        let ridge = band_pieces(bottom, 0, "ridge", color);
        assert_eq!(ridge[1].1, [0.3; 3]);
        assert_eq!(band_pieces(top, 0, "inset", color)[0].1, [0.3; 3]);
        assert_ne!(band_pieces(bottom, 2, "inset", color)[0].1, [0.3; 3]);
        assert_ne!(band_pieces(top, 0, "outset", color)[0].1, [0.3; 3]);
        assert_eq!(band_pieces(bottom, 2, "outset", color)[0].1, [0.3; 3]);
        // Nothing to cut.
        let none = Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 3.0));
        assert!(band_pieces(none, 0, "dashed", color).is_empty());
        assert!(band_pieces(top, 0, "none", color).is_empty());
        assert!(band_pieces(top, 0, "hidden", color).is_empty());
        // Too thin or too long for a pattern: one line, and no hang.
        for (width, height) in [
            (800.0, 0.00001),
            (800.0, 0.0000001),
            (40_000_000.0, 1.0),
            (f32::INFINITY, 1.0),
        ] {
            let band = Rectangle::new(Point::ORIGIN, Size::new(width, height));
            for style in ["dotted", "dashed"] {
                assert_eq!(
                    band_pieces(band, 0, style, color).len(),
                    1,
                    "{style} {width} {height}"
                );
            }
        }
    }

    #[test]
    fn styles_inherit_and_collapsed_cells_map_inset_and_outset() {
        let tree = BoxTree::new(
            node("p", vec![node("c", vec![])]),
            "#p { border:4px dashed #ff0000; outline:2px dotted #ff0000; }
             #c { border:4px solid #ff0000; border-style:inherit;
                  outline-style:inherit; outline-width:2px; }",
        )
        .unwrap();
        let child = tree.paints[1];
        assert_eq!(child.border_style, ["dashed"; 4]);
        assert_eq!(child.outline_style, "dotted");
        assert!(child.outline.is_some());
        let tree = BoxTree::new(
            node(
                "t",
                vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
            ),
            "#t { display:table; border-collapse:collapse; }
             #r { display:table-row; }
             #a { display:table-cell; border:4px inset #ff0000; }
             #b { display:table-cell; border:4px outset #ff0000; }",
        )
        .unwrap();
        assert_eq!(tree.paints[2].border_style, ["ridge"; 4]);
        assert_eq!(tree.paints[3].border_style, ["groove"; 4]);
    }

    #[test]
    fn a_refused_interaction_is_remembered_and_keeps_the_style() {
        // Deep enough that comparing this selector's long id overruns its budget, only on hover.
        let mut deep = node("leaf", vec![]);
        for at in 0..200 {
            deep = node(&format!("n{at}"), vec![deep]);
        }
        let tree = BoxTree::new(
            deep,
            &format!(
                "#{} *:hover {{ color:#ff0000; }} #leaf {{ color:#00ff00; }}",
                "n".repeat(10_000)
            ),
        )
        .unwrap();
        let key = (Some(tree.originals[tree.originals.len() - 1]), None, None);
        let shown = tree.variant(key);
        assert_eq!(shown.paints, tree.paints);
        assert!(tree.is_refused(tree.source.normal_key(key)));
    }

    #[test]
    fn collapsed_style_mapping_follows_the_table_and_real_cells_only() {
        let styles = |css: &str, at: usize| {
            let tree = BoxTree::new(
                node("t", vec![node("r", vec![node("c", vec![])])]),
                &format!(
                    "#t {{ display:table; }} #r {{ display:table-row; }}
                     #c {{ display:table-cell; border:4px inset #ff0000; }} {css}"
                ),
            )
            .unwrap();
            tree.paints[at].border_style[0]
        };
        assert_eq!(
            styles(
                "#t { border-collapse:separate; } #c { border-collapse:collapse; }",
                2
            ),
            "inset"
        );
        assert_eq!(
            styles(
                "#t { border-collapse:collapse; } #c { border-collapse:separate; }",
                2
            ),
            "ridge"
        );
        // A block wrapped in an anonymous cell keeps its own.
        let tree = BoxTree::new(
            node("t", vec![node("x", vec![])]),
            "#t { display:table; border-collapse:collapse; } #x { border:4px inset #ff0000; }",
        )
        .unwrap();
        assert_eq!(tree.paints[1].border_style[0], "inset");
    }

    #[test]
    fn outline_inherits_width_and_color_with_its_style() {
        let child = |css: &str| {
            let tree = BoxTree::new(node("p", vec![node("c", vec![])]), css).unwrap();
            tree.paints[1].outline
        };
        assert_eq!(
            child("#p { outline:1px dotted #ff0000; } #c { outline:inherit; }"),
            Some((1.0, [1.0, 0.0, 0.0]))
        );
        assert_eq!(
            child("#p { outline:0 solid #ff0000; } #c { outline:inherit; }"),
            None
        );
        assert_eq!(
            child("#p { outline-style:none; } #c { outline:inherit; }"),
            None
        );
        // The color inherits with no outline drawn above it.
        let red = Some((2.0, [1.0, 0.0, 0.0]));
        assert_eq!(
            child(
                "#p { color:#000; outline-color:#ff0000; } #c { outline:2px solid; outline-color:inherit; }"
            ),
            red
        );
        assert_eq!(
            child(
                "#p { color:#000; outline:0 solid #ff0000; } #c { outline:2px solid; outline-color:inherit; }"
            ),
            red
        );
        // `invert` stays what it is: each box's own `color` stands in for it.
        assert_eq!(
            child(
                "#p { color:#ff0000; outline:1px solid; } #c { color:#0000ff; outline:inherit; }"
            ),
            Some((1.0, [0.0, 0.0, 1.0]))
        );
    }

    #[test]
    fn generated_content_inherits_its_elements_lengths() {
        let generated = |css: &str| {
            let tree = BoxTree::new(
                node("e", vec![]),
                &format!(
                    "#e {{ width:40px; margin-left:2em; font-size:10px;
                           border:thick solid; }}
                     #e:before {{ content:\"x\"; display:block; font-size:20px; {css} }}"
                ),
            )
            .unwrap();
            let arena = tree.prepared.generated_boxes[0].0;
            let value = tree.prepared.boxes[arena].clone();
            (value.width, value.margin[3], value.border[0])
        };
        // 6.2.1: `inherit` takes the element's value, its `em` in the element's font size.
        assert_eq!(
            generated(
                "width:inherit; margin-left:inherit; border-style:solid; border-width:inherit;"
            ),
            (Dim::Px(40.0), Dim::Px(20.0), 5.0)
        );
        assert_eq!(
            generated("border-style:solid;"),
            (Dim::Auto, Dim::Px(0.0), 3.0)
        );
        // The element's border width computes to none where its own style draws none.
        let tree = BoxTree::new(
            node("e", vec![]),
            "#e { border-width:thick; border-style:none; }
             #e:before { content:\"x\"; display:block; border-style:solid;
                         border-width:inherit; }",
        )
        .unwrap();
        let arena = tree.prepared.generated_boxes[0].0;
        assert_eq!(tree.prepared.boxes[arena].border[0], 0.0);
        // An element that inherits passes its parent's on.
        let tree = BoxTree::new(
            node("p", vec![node("e", vec![])]),
            "#p { width:30px; margin-left:1em; font-size:10px; border:thin solid; }
             #e { width:inherit; margin-left:inherit; border:solid; border-width:inherit; }
             #e:before { content:\"x\"; display:block; width:inherit; margin-left:inherit;
                         border-style:solid; border-width:inherit; }",
        )
        .unwrap();
        let arena = tree.prepared.generated_boxes[0].0;
        let value = &tree.prepared.boxes[arena];
        assert_eq!(
            (value.width, value.margin[3], value.border[0]),
            (Dim::Px(30.0), Dim::Px(10.0), 1.0)
        );
    }

    #[test]
    fn a_collapsed_tables_border_competes_on_its_outer_edges() {
        let (red, blue) = (Some([1.0, 0.0, 0.0]), Some([0.0, 0.0, 1.0]));
        // The left cell's left, top and shared right side; its table's left.
        let sides = |table: &str, cell: &str| {
            let tree = BoxTree::new(
                node(
                    "t",
                    vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
                ),
                &format!(
                    "#t {{ display:table; border-collapse:collapse; border:{table}; }}
                     #r {{ display:table-row; }}
                     #a, #b {{ display:table-cell; width:20px; height:20px; border:{cell}; }}"
                ),
            )
            .unwrap();
            let a = tree.prepared.slot[2].unwrap();
            (
                (tree.paints[2].border[3], tree.prepared.boxes[a].border[3]),
                tree.paints[2].border[0],
                tree.paints[2].border[1],
                tree.paints[0].border[3],
            )
        };
        // 17.6.2.1: wider wins; on a tie the cell's; `hidden` leaves none. Inner edges are
        // the cells' alone, and the table draws none of its own.
        assert_eq!(
            sides("4px solid #00f", "2px solid #f00"),
            ((blue, 4.0), blue, red, None)
        );
        assert_eq!(
            sides("2px solid #00f", "2px solid #f00"),
            ((red, 2.0), red, red, None)
        );
        assert_eq!(
            sides("2px solid #00f", "2px dotted #f00"),
            ((blue, 2.0), blue, red, None)
        );
        assert_eq!(
            sides("2px hidden #00f", "4px solid #f00"),
            ((None, 0.0), None, red, None)
        );
        // The cell draws the table's style too.
        let styled = BoxTree::new(
            node(
                "t",
                vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
            ),
            "#t { display:table; border-collapse:collapse; border:4px dotted #00f; }
             #r { display:table-row; }
             #a, #b { display:table-cell; width:20px; height:20px; border:2px solid #f00; }",
        )
        .unwrap();
        assert_eq!(styled.paints[2].border_style[3], "dotted");
        assert_eq!(styled.paints[2].border_style[1], "solid");
        // A hidden cell has no say, and keeps its own width.
        let hidden = BoxTree::new(
            node(
                "t",
                vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
            ),
            "#t { display:table; border-collapse:collapse; border:4px solid #00f; }
             #r { display:table-row; }
             #a, #b { display:table-cell; width:20px; height:20px; border:2px solid #f00; }
             #a { visibility:hidden; }",
        )
        .unwrap();
        let a = hidden.prepared.slot[2].unwrap();
        assert_eq!(hidden.prepared.boxes[a].border[3], 2.0);
        // Without cell borders, the cells draw the table's.
        assert_eq!(
            sides("3px solid #00f", "none"),
            ((blue, 3.0), blue, None, None)
        );
    }

    #[test]
    fn collapsed_cells_resolve_their_shared_edges() {
        let (red, blue) = (Some([1.0, 0.0, 0.0]), Some([0.0, 0.0, 1.0]));
        let edges = |left: &str, right: &str| {
            let tree = BoxTree::new(
                node(
                    "t",
                    vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
                ),
                &format!(
                    "#t {{ display:table; border-collapse:collapse; }} #r {{ display:table-row; }}
                     #a, #b {{ display:table-cell; width:20px; height:20px; }}
                     #a {{ border-right:{left}; }} #b {{ border-left:{right}; }}"
                ),
            )
            .unwrap();
            (tree.paints[2].border[1], tree.paints[3].border[3])
        };
        // 17.6.2.1: wider first, then the style order, then the cell farther left.
        assert_eq!(edges("2px solid #f00", "4px dotted #00f"), (None, blue));
        assert_eq!(edges("2px dotted #f00", "2px solid #00f"), (None, blue));
        assert_eq!(edges("2px solid #f00", "2px dotted #00f"), (red, None));
        assert_eq!(edges("2px solid #f00", "2px solid #00f"), (red, None));
        assert_eq!(edges("2px hidden #f00", "4px solid #00f"), (None, None));
        assert_eq!(edges("4px solid #00f", "2px hidden #f00"), (None, None));
        assert_eq!(edges("2px solid #f00", "none"), (red, None));

        // Above beats below on a tie, and outer edges have nothing to share.
        let stacked = |css: &str| {
            BoxTree::new(
                node(
                    "t",
                    vec![
                        node("r1", vec![node("a", vec![])]),
                        node("r2", vec![node("b", vec![])]),
                    ],
                ),
                &format!(
                    "#t {{ display:table; border-collapse:collapse; }} #r1, #r2 {{ display:table-row; }}
                     #a, #b {{ display:table-cell; }} {css}"
                ),
            )
            .unwrap()
        };
        let tree = stacked("#a { border:2px solid #f00; } #b { border:2px solid #00f; }");
        assert_eq!(tree.paints[2].border[2], red);
        assert_eq!(tree.paints[4].border[0], None);
        assert_eq!(tree.paints[2].border[0], red);
        assert_eq!(tree.paints[4].border[2], blue);
        // A cell that is not visible has no say in its neighbour's edge.
        let tree = stacked(
            "#a { border:2px solid #f00; } #b { border:9px solid #00f; visibility:hidden; }",
        );
        assert_eq!(tree.paints[2].border[2], red);
        // Every real cell draws its borders after the table's backgrounds.
        let steps = tree.paint_steps(&BTreeSet::new());
        let last_box = steps
            .iter()
            .rposition(|step| matches!(step, PaintStep::Box(_)))
            .unwrap();
        let first_border = steps
            .iter()
            .position(|step| matches!(step, PaintStep::CellBorder(_)))
            .unwrap();
        assert!(first_border > last_box, "{steps:?}");

        // In rtl the first column is on the right: the shared edge is its left side.
        let tree = BoxTree::new(
            node(
                "t",
                vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
            ),
            "#t { display:table; border-collapse:collapse; direction:rtl; }
             #r { display:table-row; }
             #a { display:table-cell; border:2px solid #f00; }
             #b { display:table-cell; border:2px solid #00f; }",
        )
        .unwrap();
        assert_eq!(
            tree.paints[2].border[1], red,
            "the table's right edge stays"
        );
        assert_eq!(
            tree.paints[3].border[3], blue,
            "the table's left edge stays"
        );
        assert_eq!(
            tree.paints[2].border[3], red,
            "the first column wins the tie"
        );
        assert_eq!(tree.paints[3].border[1], None);

        // A separated table keeps both.
        let tree = BoxTree::new(
            node(
                "t",
                vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
            ),
            "#t { display:table; } #r { display:table-row; }
             #a, #b { display:table-cell; border:2px solid #f00; }",
        )
        .unwrap();
        assert!(tree.paints[2].border[1].is_some() && tree.paints[3].border[3].is_some());
        assert!(tree.paints[2].collapsed_cell.is_none());

        // A wrapped box's borders are its own.
        let tree = BoxTree::new(
            node(
                "t",
                vec![node("r", vec![node("a", vec![]), node("x", vec![])])],
            ),
            "#t { display:table; border-collapse:collapse; } #r { display:table-row; }
             #a { display:table-cell; border:1px solid #f00; }
             #x { border:2px solid #00f; }",
        )
        .unwrap();
        assert_eq!(tree.paints[3].border[3], blue);
    }

    #[test]
    fn collapsed_edges_take_spans_and_order_from_the_layouts_grid() {
        // Huge and zero spans are clamped, not walked.
        let bound = nz(1000);
        let limits = AttributeLimits {
            entries: bound,
            retained_bytes: bound,
            work: bound,
        };
        for span in ["0", "1000000000"] {
            let tall = node("a", vec![])
                .style_attribute("rowspan", span, limits)
                .unwrap();
            let wide = node("b", vec![])
                .style_attribute("colspan", span, limits)
                .unwrap();
            BoxTree::new(
                node("t", vec![node("r", vec![tall, wide])]),
                "#t { display:table; border-collapse:collapse; } #r { display:table-row; }
                 #a, #b { display:table-cell; border:1px solid #f00; }",
            )
            .unwrap();
        }
        // A foot written before the body still comes after it.
        let tree = BoxTree::new(
            node(
                "t",
                vec![
                    node("f", vec![node("fr", vec![node("c", vec![])])]),
                    node("g", vec![node("gr", vec![node("d", vec![])])]),
                ],
            ),
            "#t { display:table; border-collapse:collapse; }
             #f { display:table-footer-group; } #g { display:table-row-group; }
             #fr, #gr { display:table-row; }
             #c, #d { display:table-cell; border:2px solid #f00; }",
        )
        .unwrap();
        let (foot, body) = (3, 6);
        assert!(
            tree.paints[foot].border[2].is_some(),
            "the table's bottom edge stays"
        );
        assert!(
            tree.paints[body].border[0].is_some(),
            "the table's top edge stays"
        );
    }

    #[test]
    fn collapsed_cells_draw_backgrounds_and_borders_in_separate_steps() {
        let tree = BoxTree::new(
            node("t", vec![node("r", vec![node("a", vec![])])]),
            "#t { display:table; border-collapse:collapse; } #r { display:table-row; }
             #a { display:table-cell; width:20px; height:20px; border:2px solid #f00;
                  background-color:#00f; }",
        )
        .unwrap();
        let geometry = tree.layout(100.0, None).unwrap();
        let drawn = |parts: BoxParts| {
            let mut renderer = Recording::default();
            draw_box(&tree, &geometry, &mut renderer, Point::ORIGIN, 2, parts);
            renderer.0.len()
        };
        assert_eq!(drawn(BoxParts::All), 1, "the background alone");
        assert_eq!(drawn(BoxParts::Background), 1);
        assert_eq!(drawn(BoxParts::Border), 0);
        assert_eq!(drawn(BoxParts::CellBorder), 4);
    }

    /// Every quad as drawn, with its border.
    #[derive(Default)]
    struct Quads(Vec<(Quad, Color)>);
    impl renderer::Renderer for Quads {
        fn start_layer(&mut self, _: Rectangle) {}
        fn end_layer(&mut self) {}
        fn start_transformation(&mut self, _: Transformation) {}
        fn end_transformation(&mut self) {}
        fn fill_quad(&mut self, quad: Quad, background: impl Into<Background>) {
            let Background::Color(color) = background.into() else {
                panic!("box paint is a solid color");
            };
            self.0.push((quad, color));
        }
        fn allocate_image(
            &mut self,
            _: &iced::advanced::image::Handle,
            _: impl FnOnce(Result<iced::advanced::image::Allocation, iced::advanced::image::Error>)
            + Send
            + 'static,
        ) {
        }
        fn reset(&mut self, _: Rectangle) {}
    }

    #[test]
    fn rounded_boxes_draw_a_curved_background_and_one_ring_for_a_uniform_border() {
        let drawn = |css: &str| {
            let tree = BoxTree::new(node("page", vec![node("k", vec![])]), css).unwrap();
            let geometry = tree.layout(400.0, None).unwrap();
            let mut renderer = Quads::default();
            draw_box(
                &tree,
                &geometry,
                &mut renderer,
                Point::ORIGIN,
                1,
                BoxParts::All,
            );
            renderer.0
        };
        let corners = |quad: &Quad| {
            let radius = quad.border.radius;
            [
                radius.top_left,
                radius.top_right,
                radius.bottom_right,
                radius.bottom_left,
            ]
        };
        let base = "#k { width:100px; height:60px; background-color:#0000ff; }";
        // No radius: square corners, as before.
        let quads = drawn(base);
        assert_eq!(quads.len(), 1);
        assert_eq!(corners(&quads[0].0), [0.0; 4]);
        // One value rounds every corner of the background.
        let quads = drawn(&format!("{base} #k {{ border-radius:10px; }}"));
        assert_eq!(quads.len(), 1);
        assert_eq!(corners(&quads[0].0), [10.0; 4]);
        // Four values, clockwise from the top left; a longhand over the shorthand.
        let quads = drawn(&format!(
            "{base} #k {{ border-radius:1px 2px 3px 4px; border-bottom-right-radius:9px; }}"
        ));
        assert_eq!(corners(&quads[0].0), [1.0, 2.0, 9.0, 4.0]);
        // A percentage is of the shorter side; radii too large for a side shrink to fit.
        let quads = drawn(&format!("{base} #k {{ border-radius:50%; }}"));
        assert_eq!(corners(&quads[0].0), [30.0; 4]);
        let quads = drawn(&format!("{base} #k {{ border-radius:100px; }}"));
        assert!(
            corners(&quads[0].0)
                .iter()
                .all(|corner| (corner - 30.0).abs() < 0.001)
        );
        // A uniform solid border is one ring on the same curve, over the background.
        let quads = drawn(&format!(
            "{base} #k {{ border:4px solid #ff0000; border-radius:12px; }}"
        ));
        assert_eq!(quads.len(), 2);
        let (ring, fill) = &quads[1];
        assert_eq!(*fill, Color::TRANSPARENT);
        assert_eq!(ring.border.width, 4.0);
        assert_eq!(ring.border.color, Color::from_rgb(1.0, 0.0, 0.0));
        assert_eq!(corners(ring), [12.0; 4]);
        assert_eq!(
            ring.bounds,
            Rectangle::new(Point::ORIGIN, Size::new(108.0, 68.0))
        );
        // Sides that differ stay square bands; the background is still rounded.
        let quads = drawn(&format!(
            "{base} #k {{ border:4px solid #ff0000; border-left-color:#00ff00; border-radius:12px; }}"
        ));
        assert_eq!(quads.len(), 5);
        assert_eq!(corners(&quads[0].0), [12.0; 4]);
        assert!(quads[1..].iter().all(|(quad, _)| corners(quad) == [0.0; 4]));
        // `opacity` multiplies through ancestors, onto the background and the borders.
        let faded = |css: &str| {
            let tree = BoxTree::new(node("page", vec![node("k", vec![])]), css).unwrap();
            let geometry = tree.layout(400.0, None).unwrap();
            let mut renderer = Quads::default();
            draw_box(
                &tree,
                &geometry,
                &mut renderer,
                Point::ORIGIN,
                1,
                BoxParts::All,
            );
            renderer
                .0
                .iter()
                .map(|(_, color)| color.a)
                .collect::<Vec<_>>()
        };
        let bordered = format!("{base} #k {{ border:2px solid #ff0000; }}");
        assert_eq!(faded(&bordered), [1.0; 5]);
        assert_eq!(
            faded(&format!("{bordered} #k {{ opacity:0.5; }}")),
            [0.5; 5]
        );
        assert_eq!(
            faded(&format!(
                "{bordered} #page {{ opacity:0.5; }} #k {{ opacity:0.5; }}"
            )),
            [0.25; 5]
        );
        // Out of range clips; `inherit` takes the parent's own, not its product.
        assert_eq!(faded(&format!("{bordered} #k {{ opacity:7; }}")), [1.0; 5]);
        assert_eq!(faded(&format!("{bordered} #k {{ opacity:-1; }}")), [0.0; 5]);
        assert_eq!(
            faded(&format!(
                "{bordered} #page {{ opacity:0.5; }} #k {{ opacity:inherit; }}"
            )),
            [0.25; 5]
        );
        for bad in ["50%", "1 2", "half", "0.5px"] {
            assert_eq!(
                faded(&format!("{bordered} #k {{ opacity:0.5; opacity:{bad}; }}")),
                [0.5; 5],
                "{bad}"
            );
        }
        // The second radius of a corner, after a slash, is not read.
        // A shadow rides on the background's quad: offset, blur and color.
        let quads = drawn(&format!(
            "{base} #k {{ box-shadow:2px 3px 5px rgba(255, 0, 0, 0.5); border-radius:6px; }}"
        ));
        assert_eq!(quads.len(), 1);
        let shadow = quads[0].0.shadow;
        assert_eq!(shadow.offset, iced::Vector::new(2.0, 3.0));
        assert_eq!(shadow.blur_radius, 5.0);
        assert!((shadow.color.a - 0.5).abs() < 0.01);
        assert_eq!(
            (shadow.color.r, shadow.color.g, shadow.color.b),
            (1.0, 0.0, 0.0)
        );
        assert_eq!(corners(&quads[0].0), [6.0; 4]);
        // The first outer shadow of a list; its spread is not drawn; no color is `color`.
        let quads = drawn(&format!(
            "{base} #k {{ color:#00ff00; box-shadow:inset 1px 1px #000000, 0 4px 8px 2px, 9px 9px #ff0000; }}"
        ));
        let shadow = quads[0].0.shadow;
        assert_eq!(shadow.offset, iced::Vector::new(0.0, 4.0));
        assert_eq!(shadow.blur_radius, 8.0);
        assert_eq!(shadow.color, Color::from_rgb(0.0, 1.0, 0.0));
        // Without a background color the shadow is still cast.
        let quads = drawn("#k { width:100px; height:60px; box-shadow:1px 1px 2px #000000; }");
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].1, Color::TRANSPARENT);
        assert_eq!(quads[0].0.shadow.blur_radius, 2.0);
        // `none`, and invalid lists, cast nothing.
        for none in [
            "none",
            "1px",
            "1px 2px -3px",
            "1px 2px red blue",
            "1px 2px, ",
            "inset inset 1px 1px",
        ] {
            let quads = drawn(&format!("{base} #k {{ box-shadow:{none}; }}"));
            assert_eq!(quads[0].0.shadow, iced::Shadow::default(), "{none}");
        }
        let quads = drawn(&format!("{base} #k {{ border-radius:10px / 20px; }}"));
        assert_eq!(corners(&quads[0].0), [10.0; 4]);
        // Invalid values drop the declaration.
        for bad in ["-1px", "1px 2px 3px 4px 5px", "1px / ", "/ 1px", "red"] {
            let quads = drawn(&format!(
                "{base} #k {{ border-radius:7px; border-radius:{bad}; }}"
            ));
            assert_eq!(corners(&quads[0].0), [7.0; 4], "{bad}");
        }
    }

    #[test]
    fn cells_meeting_along_part_of_an_edge_keep_their_own_bands() {
        let bound = nz(1000);
        let limits = AttributeLimits {
            entries: bound,
            retained_bytes: bound,
            work: bound,
        };
        let tall = node("a", vec![])
            .style_attribute("rowspan", "2", limits)
            .unwrap();
        let tree = BoxTree::new(
            node(
                "t",
                vec![
                    node("r1", vec![tall, node("b", vec![])]),
                    node("r2", vec![node("c", vec![])]),
                ],
            ),
            "#t { display:table; border-collapse:collapse; } #r1, #r2 { display:table-row; }
             #a, #b, #c { display:table-cell; border:2px solid #f00; }",
        )
        .unwrap();
        assert!(
            tree.paints[3].border[3].is_some(),
            "#b beside the taller #a"
        );
        assert!(
            tree.paints[2].border[1].is_some(),
            "#a beside the shorter #b"
        );

        let wide = node("a", vec![])
            .style_attribute("colspan", "2", limits)
            .unwrap();
        let tree = BoxTree::new(
            node(
                "t",
                vec![
                    node("r1", vec![wide]),
                    node("r2", vec![node("b", vec![]), node("c", vec![])]),
                ],
            ),
            "#t { display:table; border-collapse:collapse; } #r1, #r2 { display:table-row; }
             #a, #b, #c { display:table-cell; border:2px solid #f00; }",
        )
        .unwrap();
        assert!(tree.paints[4].border[0].is_some(), "#b below the wider #a");
        assert!(
            tree.paints[2].border[2].is_some(),
            "#a above the narrower #b"
        );
    }

    #[test]
    fn a_band_too_thin_for_its_pattern_is_a_plain_line() {
        let band = Rectangle::new(Point::ORIGIN, Size::new(100.0, 0.4));
        assert_eq!(band_pieces(band, 0, "dotted", [0.5; 3]).len(), 1);
        assert_eq!(band_pieces(band, 0, "dashed", [0.5; 3]).len(), 1);
    }

    #[test]
    fn a_collapsed_loser_clears_the_winners_band_and_children_paint_under_borders() {
        let tree = BoxTree::new(
            node(
                "t",
                vec![node(
                    "r",
                    vec![node("a", vec![]), node("b", vec![node("kid", vec![])])],
                )],
            ),
            "#t { display:table; border-collapse:collapse; } #r { display:table-row; }
             #a, #b { display:table-cell; width:40px; border:1px solid #000; }
             #a { border-right:10px solid #f00; }
             #kid { height:9px; background-color:#fff; }",
        )
        .unwrap();
        let geometry = tree.layout(800.0, None).unwrap();
        let (a, b, kid) = (
            geometry[2].as_ref().unwrap(),
            geometry[3].as_ref().unwrap(),
            geometry[4].as_ref().unwrap(),
        );
        assert!(kid.border.left >= a.border.right, "{kid:?} {a:?} {b:?}");
        let steps = tree.paint_steps(&BTreeSet::new());
        let kid_box = steps
            .iter()
            .position(|step| *step == PaintStep::Box(4))
            .unwrap();
        let first_border = steps
            .iter()
            .position(|step| matches!(step, PaintStep::CellBorder(_)))
            .unwrap();
        assert!(first_border > kid_box, "{steps:?}");
    }

    #[test]
    fn positioned_content_in_a_cell_stays_above_the_grid() {
        let tree = BoxTree::new(
            node(
                "t",
                vec![node("r", vec![node("a", vec![node("pop", vec![])]), node("b", vec![])])],
            ),
            "#t { display:table; border-collapse:collapse; } #r { display:table-row; }
             #a, #b { display:table-cell; width:40px; border:1px solid #000; }
             #pop { position:absolute; z-index:10; width:200px; height:200px; background-color:#fff; }",
        )
        .unwrap();
        let steps = tree.paint_steps(&BTreeSet::new());
        let above = |steps: &[PaintStep], at: usize| {
            let pop = steps
                .iter()
                .position(|step| *step == PaintStep::Box(at))
                .unwrap();
            let last = steps
                .iter()
                .rposition(|step| matches!(step, PaintStep::CellBorder(_)))
                .unwrap();
            last < pop
        };
        assert!(above(&steps, 3));
        // A positioned table there is content too, not a part of this one.
        let nested = BoxTree::new(
            node(
                "t",
                vec![node(
                    "r",
                    vec![node("a", vec![node("pop", vec![])]), node("b", vec![])],
                )],
            ),
            "#t { display:table; border-collapse:collapse; } #r { display:table-row; }
             #a, #b { display:table-cell; width:40px; border:1px solid #000; }
             #pop { display:table; position:absolute; z-index:10; width:200px; height:200px;
                    background-color:#fff; }",
        )
        .unwrap();
        assert!(above(&nested.paint_steps(&BTreeSet::new()), 3));
        let pop = steps
            .iter()
            .position(|step| *step == PaintStep::Box(3))
            .unwrap();
        let last_border = steps
            .iter()
            .rposition(|step| matches!(step, PaintStep::CellBorder(_)))
            .unwrap();
        assert!(last_border < pop, "{steps:?}");
    }

    #[test]
    fn a_relative_cell_or_row_keeps_its_children_under_the_grid() {
        for css in ["#b { position:relative; }", "#r { position:relative; }"] {
            let tree = BoxTree::new(
                node(
                    "t",
                    vec![node(
                        "r",
                        vec![node("a", vec![]), node("b", vec![node("kid", vec![])])],
                    )],
                ),
                &format!(
                    "#t {{ display:table; border-collapse:collapse; }} #r {{ display:table-row; }}
                     #a, #b {{ display:table-cell; width:40px; border:1px solid #000; }}
                     #kid {{ height:9px; background-color:#fff; }} {css}"
                ),
            )
            .unwrap();
            let steps = tree.paint_steps(&BTreeSet::new());
            let kid = steps
                .iter()
                .position(|step| *step == PaintStep::Box(4))
                .unwrap();
            let first_border = steps
                .iter()
                .position(|step| matches!(step, PaintStep::CellBorder(_)))
                .unwrap();
            assert!(first_border > kid, "{css}: {steps:?}");
        }
    }

    #[test]
    fn a_hidden_shared_edge_takes_no_room() {
        let tree = BoxTree::new(
            node(
                "t",
                vec![node("r", vec![node("a", vec![]), node("b", vec![])])],
            ),
            "#t { display:table; border-collapse:collapse; } #r { display:table-row; }
             #a, #b { display:table-cell; width:20px; height:20px; }
             #a { border-right:4px solid #00f; } #b { border-left:2px hidden #f00; }",
        )
        .unwrap();
        let geometry = tree.layout(800.0, None).unwrap();
        let a = geometry[2].as_ref().unwrap();
        assert_eq!(a.border.right, a.padding.right, "{a:?}");
    }

    #[test]
    fn a_column_group_paints_once_before_its_columns() {
        let tree = BoxTree::new(
            node(
                "t",
                vec![
                    node("c0", vec![]),
                    node("g", vec![node("c1", vec![]), node("c2", vec![])]),
                    node("h", vec![node("c3", vec![])]),
                    node("r", vec![node("a", vec![])]),
                ],
            ),
            "#t { display:table; } #g, #h { display:table-column-group; }
             #c0, #c1, #c2, #c3 { display:table-column; }
             #r { display:table-row; } #a { display:table-cell; }",
        )
        .unwrap();
        let steps: Vec<PaintStep> = tree
            .paint_steps(&BTreeSet::new())
            .into_iter()
            .filter(|step| matches!(step, PaintStep::ColumnGroup(_) | PaintStep::Column(_)))
            .collect();
        assert_eq!(
            steps,
            vec![
                PaintStep::ColumnGroup(1),
                PaintStep::ColumnGroup(3),
                PaintStep::Column(0),
                PaintStep::Column(1),
                PaintStep::Column(2),
                PaintStep::Column(3),
            ]
        );
    }

    #[test]
    fn rows_and_groups_paint_under_every_cell() {
        let bound = nz(1000);
        let limits = AttributeLimits {
            entries: bound,
            retained_bytes: bound,
            work: bound,
        };
        let tall = node("a", vec![])
            .style_attribute("rowspan", "2", limits)
            .unwrap();
        let tree = BoxTree::new(
            node(
                "t",
                vec![node(
                    "g",
                    vec![
                        node("r1", vec![tall, node("b", vec![])]),
                        node("r2", vec![node("c", vec![])]),
                    ],
                )],
            ),
            "#t { display:table; } #g { display:table-row-group; }
             #r1, #r2 { display:table-row; } #a, #b, #c { display:table-cell; }
             #r2 { background-color:#fff; }",
        )
        .unwrap();
        let order = tree.paint_order();
        // t, g, r1, r2 first; then the cells a, b, c.
        assert_eq!(order, vec![0, 1, 2, 5, 3, 4, 6]);
    }
}
