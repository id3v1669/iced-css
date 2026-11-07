//! A stylesheet-driven iced widget for captured box trees.
//!
//! ```
//! use iced_css::{
//!     box_tree::{BoxTree, attribute_limits, capture_limits},
//!     capture::adapters,
//! };
//!
//! # fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! // Capture a tree. `style_id` names each box for the stylesheet.
//! let card = adapters::column_from_vec::<u8, iced::Theme, iced::Renderer>(vec![], capture_limits())?
//!     .style_id("card", attribute_limits())?
//!     .erase();
//! let root = adapters::column_from_vec(vec![card], capture_limits())?
//!     .style_id("page", attribute_limits())?
//!     .erase();
//!
//! // Cascade once; the tree lays out again at any size.
//! let tree = BoxTree::new(root, "#page { padding:16px; } #card { height:40px; }")?;
//! let boxes = tree.layout(800.0, None)?;
//! assert_eq!(boxes[1].as_ref().map(|card| card.border.top), Some(16.0));
//!
//! // In a view: host iced widgets in leaf boxes, and press boxes by id.
//! let _view: iced::Element<'_, u8> = tree.interactive().on_press("card", 1).into();
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
use crate::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, Construction, source::ConstructionState},
    evaluation::{
        clearance_box_facts::ClearanceBoxFactsLimits,
        display_rules::{BundleLimits, DisplayRuleBundle, GraphBinding, GraphOrigin},
        source_display::{SourceDisplayLimits, evaluate_source_display},
        tree_layout::{
            BackgroundImage, BackgroundOffset, BoxPaint, GeneratedRun, InlineBoxSides,
            PreparedTree, RunSource, TextLineHeight, TextStyle, TreeLayoutError, TreePaint,
            evaluate_tree_paint, prepare_captured_tree, prepare_tree_source_projection,
        },
    },
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, image, layout, overlay,
    renderer::{self, Quad},
    text::{self, Paragraph as _},
    widget::{Operation, Tree, tree},
};
use iced::{
    Color, Element, Event, Length, Pixels, Point, Rectangle, Size, Vector, keyboard, mouse,
};
use iced_css_core::source::AttributeLimits;
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
        block_flow::{
            Dim, FlowContent, FlowGeometry, FlowLimits, FlowRect, FlowViewport, RowAlign, TableRole,
        },
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
    },
    values::{NumericLimits, color::ColorLimits},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU64, NonZeroUsize},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
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
/// One selector against one box. Descendant combinators backtrack, so `section div div div a`
/// spends several units per C(depth, 3): ordinary deep pages must fit, and only a selector
/// built to explode should not. Nothing meters the total, so sheets are trusted input.
const MATCH_WORK: usize = 1_000_000;

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
    OwnedStylesheetGraph::new(
        vec![OwnedLoadedSheet {
            resource: ResourceId::new(NonZeroU64::MIN),
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
    .map_err(failed)
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
    pub background: Option<[f32; 3]>,
    pub weight: u16,
    pub italic: bool,
    pub family: Option<String>,
    pub underline: bool,
    pub line_through: bool,
    /// 11.2: hidden text still takes up its room.
    pub visible: bool,
    /// The source node this run belongs to, for hit testing.
    pub owner: usize,
}

/// The inline content of one block: its runs, after white space and text-transform.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLeaf {
    pub runs: Vec<TextRun>,
    pub align: &'static str,
    pub wraps: bool,
    pub line_height: TextLineHeight,
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

fn transformed(text: &str, transform: &str) -> String {
    match transform {
        "uppercase" => text.to_uppercase(),
        "lowercase" => text.to_lowercase(),
        "capitalize" => {
            let mut out = String::with_capacity(text.len());
            let mut boundary = true;
            for character in text.chars() {
                if boundary {
                    out.extend(character.to_uppercase());
                } else {
                    out.push(character);
                }
                boundary = !character.is_alphanumeric();
            }
            out
        }
        _ => text.to_owned(),
    }
}

/// 4.1.3: only these collapse; U+00A0 and U+3000 are ordinary characters.
fn is_css_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n' | '\r' | '\x0C')
}

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
    for (index, (text, mode)) in texts.iter_mut().enumerate() {
        if matches!(*mode, "pre" | "pre-wrap") {
            after_space = text.ends_with(is_css_space);
            last_collapsing = None;
            continue;
        }
        let keep_newlines = *mode == "pre-line";
        let mut out = String::with_capacity(text.len());
        for character in text.chars() {
            if keep_newlines && character == '\n' {
                while out.ends_with(' ') {
                    out.pop();
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
                let start = before
                    .rfind(|character: char| {
                        character.is_whitespace() || ",>+~{};/".contains(character)
                    })
                    .map_or(0, |found| found + 1);
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
    /// Restyles already cascaded. Pointerless, so nothing here keeps the source alive.
    variants: Mutex<BTreeMap<InteractionKey, Arc<Style>>>,
    /// Keys whose restyle was refused: asking again would only cascade again, per frame.
    refused: Mutex<BTreeSet<InteractionKey>>,
}

static NEXT_CAPTURE: AtomicU64 = AtomicU64::new(0);

impl Source {
    fn reach_of(sheets: &[String]) -> [PseudoReach; 3] {
        [":hover", ":active", ":focus"].map(|pseudo| PseudoReach::of(sheets, pseudo))
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

/// Interaction states the cache holds before starting over: each is a whole style.
const VARIANT_LIMIT: usize = 64;

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
    marker_images: BTreeMap<usize, image::Handle>,
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
    /// Images that replace outside markers, by projected index of their item.
    marker_images: BTreeMap<usize, image::Handle>,
    /// Temporary values of running animations, laid over every interaction styling.
    presentation: Option<Arc<Vec<animation::Presented>>>,
}

impl PartialEq for BoxTree {
    fn eq(&self, other: &Self) -> bool {
        self.prepared == other.prepared && self.paints == other.paints && self.texts == other.texts
    }
}

impl BoxTree {
    /// Cascades `css` over a captured tree. Only the capture evidence is kept.
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
    /// load: a `url()` with a scheme, an absolute path or `..` draws nothing. A missing or
    /// undecodable file draws nothing too, and the box shows its background color.
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
            variants: Mutex::new(BTreeMap::new()),
            refused: Mutex::new(BTreeSet::new()),
        });
        Self::styled(&source, (None, None, None))
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
            variants: Mutex::new(BTreeMap::new()),
            refused: Mutex::new(BTreeSet::new()),
        });
        Self::styled(&source, (None, None, None))
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
        let key = self.source.normal_key(key);
        let cached = self
            .source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .cloned();
        if let Some(style) = cached {
            return Self::from_style((*style).clone(), Arc::clone(&self.source));
        }
        if self.is_refused(key) {
            return self.clone();
        }
        let built = match Self::styled(&self.source, key) {
            Ok(built) => built,
            // This tree may carry a frame's presented values; those are not a style to keep.
            Err(_) => {
                self.refuse(key);
                return self.clone();
            }
        };
        let mut variants = self
            .source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if variants.len() >= VARIANT_LIMIT {
            variants.clear();
        }
        variants.insert(key, Arc::new(built.style()));
        drop(variants);
        built
    }

    fn is_refused(&self, key: InteractionKey) -> bool {
        self.source
            .refused
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&key)
    }

    fn refuse(&self, key: InteractionKey) {
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
            .get(&key)
            .cloned();
        if cached.is_some() {
            return cached;
        }
        if self.is_refused(key) {
            return None;
        }
        let Ok(built) = Self::styled(&self.source, key) else {
            self.refuse(key);
            return None;
        };
        let style = Arc::new(built.style());
        let mut variants = self
            .source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if variants.len() >= VARIANT_LIMIT {
            variants.clear();
        }
        variants.insert(key, Arc::clone(&style));
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
        match (self.cached_style(first), self.cached_style(second)) {
            (Some(one), Some(other)) => {
                Arc::ptr_eq(&one, &other)
                    || (one.prepared == other.prepared
                        && one.paints == other.paints
                        && one.texts == other.texts
                        && one.markers == other.markers
                        && one.backgrounds == other.backgrounds
                        && one.marker_images == other.marker_images)
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
            marker_images: self.marker_images.clone(),
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
            marker_images: style.marker_images,
            presentation: None,
        }
    }

    /// The slot a widget for this CSS id occupies, in this styling: the leaf box itself,
    /// or the anonymous block of generated text it replaces when the box has only that.
    /// Display none has none.
    fn leaf_slot(&self, id: &str) -> Option<usize> {
        let prepared = &self.prepared;
        self.ids.iter().enumerate().find_map(|(arena, value)| {
            if value.as_deref() != Some(id) || prepared.anonymous[arena] {
                return None;
            }
            let children = &prepared.boxes[arena].children;
            match children.first() {
                None => Some(arena),
                Some(first) if children.iter().all(|child| prepared.anonymous[*child]) => {
                    Some(*first)
                }
                Some(_) => None,
            }
        })
    }

    fn styled(source: &Arc<Source>, key: InteractionKey) -> Result<Self, BoxTreeError> {
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
            .map(|(css, order)| parse(css, order))
            .collect::<Result<Vec<_>, _>>()?;
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
            // This bound is itself charged per membership query, so keep it small.
            SheetLimits {
                entries: big(),
                retained_bytes: nz(BYTES),
                work: nz(100 + 10 * ids.len()),
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
            &catalog,
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
            marker_images,
            outside_items,
        } = evaluate_tree_paint(&projection, &display, facts(), &sized.font_sizes)
            .map_err(failed)?;
        let mut prepared = prepare_captured_tree(
            &projection,
            &display,
            facts(),
            Some(16.0),
            |_| None,
            &generated,
        )
        .map_err(failed)?;
        let mut collapse_spacing: Vec<Option<[f32; 2]>> = vec![None; prepared.boxes.len()];
        // 17.6.2: collapsed borders share one line: neighbouring cells overlap by the widest
        // cell border on that axis, and the table's edge is the outer cells'. The width is
        // not per edge; `resolve_collapsed_edges` picks what each shared edge draws.
        for table in 0..prepared.boxes.len() {
            if !matches!(prepared.boxes[table].table, Some(TableRole::Table { .. }))
                || !collapses[prepared.positions[table].0]
            {
                continue;
            }
            resolve_collapsed_edges(&mut prepared, table, &mut paints);
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
                *spacing = collapse_spacing[arena].unwrap_or(spacings[owner]);
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
            value.caption_below = captions_below[owner];
            value.fixed_table = paints[owner].fixed_table && !prepared.anonymous[arena];
            if value.inline_row.is_some() {
                // 16.2: a line's alignment comes from its block container.
                value.inline_row = Some(match styles[container_of[arena]].align {
                    "center" => RowAlign::Center,
                    "right" => RowAlign::Right,
                    _ => RowAlign::Left,
                });
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
            let first = *runs.first()?;
            let style_of = |run: RunSource| -> &TextStyle {
                match (run, generated_of(run)) {
                    (_, Some(made)) => &made.style,
                    (RunSource::Node(at) | RunSource::Before(at) | RunSource::After(at), None) => {
                        &styles[at]
                    }
                }
            };
            let mut texts: Vec<(String, &'static str)> = runs
                .iter()
                .map(|&run| {
                    let content = match (run, generated_of(run)) {
                        (_, Some(made)) => made.text.as_str(),
                        (RunSource::Node(at), None) => projection
                            .original_index(at)
                            .and_then(|original| nodes[original].text())
                            .unwrap_or_default(),
                        _ => "",
                    };
                    let style = style_of(run);
                    (
                        transformed(content, style.text_transform),
                        style.white_space,
                    )
                })
                .collect();
            collapse(&mut texts);
            let runs = runs
                .iter()
                .zip(texts)
                .map(|(&run, (text, _))| {
                    let style = style_of(run);
                    let (size, color, background, visible) = match (run, generated_of(run)) {
                        (run_source, Some(made)) => {
                            // Generated text keeps an enclosing inline's background.
                            let (RunSource::Node(at)
                            | RunSource::Before(at)
                            | RunSource::After(at)) = run_source;
                            let mut owner = Some(at);
                            let mut background = made.background.filter(|_| made.visible);
                            while background.is_none() {
                                let Some(here) = owner else { break };
                                if prepared.slot[here].is_some_and(|slot| prepared.anonymous[slot])
                                {
                                    background = raw_paints[here]
                                        .background
                                        .filter(|_| raw_paints[here].visible);
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
                            let mut owner = Some(at);
                            let mut background = None;
                            while let Some(here) = owner {
                                background = raw_paints[here]
                                    .background
                                    .filter(|_| raw_paints[here].visible);
                                if background.is_some() {
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
                        background,
                        weight: style.weight,
                        italic: style.italic,
                        family: style.family.clone(),
                        underline: style.underline,
                        line_through: style.line_through,
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
            Some(TextLeaf {
                runs,
                // 16.2: alignment belongs to the block container of the lines.
                align: if prepared.anonymous[arena] {
                    styles[container_of[arena]].align
                } else {
                    lead.align
                },
                wraps: !matches!(lead.white_space, "pre" | "nowrap"),
                line_height: lead.line_height,
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
                    paints[projected].background = None;
                    paints[projected].border = [None; 4];
                }
            } else if let Some(leaf) = &mut texts[arena] {
                for (run, source) in leaf.runs.iter_mut().zip(&prepared.runs[arena]) {
                    if matches!(source, RunSource::Node(_)) {
                        run.background = None;
                    }
                }
            }
        }
        // 15.6: small capitals are simulated by scaling capital letters. 8.6: an inline box's
        // sides open before its first run and close after its last, whatever lines it breaks
        // across; spaces take the room across, as for `text-indent`.
        // The inline boxes around a projected index, outermost first.
        let inline_chain = |mut at: Option<usize>| {
            let mut found = Vec::new();
            while let Some(here) = at {
                let sided = prepared.inline_boxes[here].is_some_and(|sides| {
                    [sides.margin, sides.border, sides.padding]
                        .iter()
                        .flatten()
                        .any(|side| *side != 0.0)
                });
                if sided {
                    found.push(here);
                }
                at = prepared.inline_parent[here];
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
                for element in inline_chain(Some(at)) {
                    last_leaf.insert(element, arena);
                }
            }
        }
        let mut opened = BTreeSet::new();
        for (arena, runs) in prepared.runs.iter().enumerate() {
            let Some(leaf) = &mut texts[arena] else {
                continue;
            };
            let boxed = !leaf.is_empty();
            let mut tagged: Vec<(TextRun, usize)> = Vec::with_capacity(leaf.runs.len());
            for (run, &source) in leaf.runs.drain(..).zip(runs) {
                let (RunSource::Node(at) | RunSource::Before(at) | RunSource::After(at)) = source;
                let style = match generated_of(source) {
                    Some(made) => &made.style,
                    None => &styles[at],
                };
                let small_caps = style.small_caps;
                let mut run = run;
                // 16.4: more room at each space, by the same stand-in spaces as an indent.
                if style.word_spacing > 0.0
                    && let Some(extra) = indent_run(&run, style.word_spacing)
                {
                    let wide = format!(" {}", extra.text);
                    let wide_fixed = format!("\u{a0}{}", extra.text);
                    run.text = run.text.replace(' ', &wide).replace('\u{a0}', &wide_fixed);
                }
                if small_caps {
                    tagged.extend(small_capitals(&run).into_iter().map(|part| (part, at)));
                } else {
                    tagged.push((run, at));
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
                .map(|(run, at)| {
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
            for (index, (run, _)) in tagged.iter_mut().enumerate() {
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
                .map(|(_, at)| inline_chain(Some(*at)))
                .collect();
            let mut placed = Vec::with_capacity(tagged.len());
            for (index, (mut run, _)) in tagged.into_iter().enumerate() {
                let here = &chains[index];
                if !boxed {
                    placed.push(run);
                    continue;
                }
                let sides_of = |element: usize| {
                    prepared.inline_boxes[element].unwrap_or(InlineBoxSides {
                        margin: [0.0; 4],
                        border: [0.0; 4],
                        padding: [0.0; 4],
                    })
                };
                // 11.2: a hidden box paints nothing but keeps its room.
                let decorations = |elements: &[usize]| -> Vec<InlineDecoration> {
                    elements
                        .iter()
                        .filter(|element| raw_paints[**element].visible)
                        .map(|&element| {
                            let sides = sides_of(element);
                            let colors = raw_paints[element].border;
                            InlineDecoration {
                                padding: [sides.padding[0], sides.padding[2]],
                                border: [
                                    (sides.border[0], colors[0]),
                                    (sides.border[2], colors[2]),
                                ],
                                border_style: [
                                    raw_paints[element].border_style[0],
                                    raw_paints[element].border_style[2],
                                ],
                                background: raw_paints[element].background,
                                owner: projection.original_index(element).unwrap_or_default(),
                            }
                        })
                        .collect()
                };
                // A margin lies in the boxes around this one, and border and padding in this
                // one as well, as one space holding an exact border bar.
                let around = |mut at: Option<usize>| {
                    while let Some(here) = at {
                        if let Some(color) = raw_paints[here]
                            .background
                            .filter(|_| raw_paints[here].visible)
                        {
                            return Some(color);
                        }
                        at = prepared.inline_parent[here];
                    }
                    None
                };
                let side = |element: usize, across: usize, run: &TextRun, depth: usize| {
                    let sides = sides_of(element);
                    let owner_of = |at: Option<usize>| {
                        at.and_then(|at| projection.original_index(at))
                            .unwrap_or(run.owner)
                    };

                    let margin = (sides.margin[across] > 0.0)
                        .then(|| indent_run(run, sides.margin[across]))
                        .flatten()
                        .map(|spacer| TextRun {
                            background: around(prepared.inline_parent[element]),
                            inline_boxes: decorations(&here[..depth]),
                            // Nothing hidden answers input: the nearest box around that shows.
                            owner: owner_of(
                                std::iter::successors(prepared.inline_parent[element], |at| {
                                    prepared.inline_parent[*at]
                                })
                                .find(|at| raw_paints[*at].visible)
                                .or(Some(container_of[arena])),
                            ),
                            ..spacer
                        });
                    let bar = sides.border[across];
                    let room = bar + sides.padding[across];
                    let visible = raw_paints[element].visible;
                    let edge = raw_paints[element].border[across]
                        .filter(|_| bar > 0.0 && visible)
                        .map(|color| (across == 3, bar, color));
                    // Up to the next space that fits: a thin border must keep its bar.
                    let step = run.size.max(1.0) / 6.0;
                    let inner = (room > 0.0)
                        .then(|| indent_run(run, (room / step).ceil() * step * 1.0001))
                        .flatten()
                        .map(|spacer| TextRun {
                            // Its own background comes with its box, over its padding too.
                            background: if raw_paints[element].background.is_some() && visible {
                                None
                            } else {
                                around(prepared.inline_parent[element])
                            },
                            inline_boxes: decorations(&here[..=depth]),
                            edge,
                            edge_style: raw_paints[element].border_style[across],
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
        for arena in 0..prepared.boxes.len() {
            let owner = if prepared.anonymous[arena] {
                container_of[arena]
            } else {
                prepared.positions[arena].0
            };
            let indent = styles[owner].text_indent;
            let Some(leaf) = texts[arena]
                .as_mut()
                .filter(|leaf| indent > 0.0 && !leaf.is_empty())
            else {
                continue;
            };
            // An anonymous box must come first in its parent, and a text piece in its line.
            let leads = |at: usize| {
                !prepared.anonymous[at]
                    || parent_of[at].is_none_or(|parent: usize| {
                        prepared.boxes[parent].children.iter().find(|child| {
                            let child = &prepared.boxes[**child];
                            child.float.is_none() && !child.out_of_flow
                        }) == Some(&at)
                    })
            };
            // An inline-block with its own text has its own first line.
            let line_leads = parent_of[arena]
                .filter(|parent| {
                    prepared.anonymous[arena] && prepared.boxes[*parent].inline_row.is_some()
                })
                .is_none_or(leads);
            if !leads(arena) || !line_leads {
                continue;
            }
            let lead = leaf.runs.iter().find(|run| !run.text.is_empty());
            if let Some(mut spacer) = lead.and_then(|first| indent_run(first, indent)) {
                // Blank space of the block, not of whatever the first word sits in.
                spacer.owner = projection.original_index(owner).unwrap_or(spacer.owner);
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
            if let Some(first) = leaf.runs.iter_mut().find(|run| !run.text.is_empty()) {
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
                            background: None,
                            weight: style.weight,
                            italic: style.italic,
                            family: style.family.clone(),
                            underline: false,
                            line_through: false,
                            visible: raw_paints[projected].visible,
                            owner: projection.original_index(projected).unwrap_or_default(),
                        }],
                        align: "right",
                        wraps: false,
                        line_height: style.line_height,
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
            // The table decides, and an anonymous cell's owner is the box it wraps.
            let mut table = parent_of[arena];
            while let Some(at) = table {
                if matches!(prepared.boxes[at].table, Some(TableRole::Table { .. })) {
                    break;
                }
                table = parent_of[at];
            }
            let collapsed = table.is_some_and(|at| collapses[prepared.positions[at].0]);
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
            let collapsed = table.is_some_and(|at| collapses[prepared.positions[at].0]);
            paints[owner].hidden_empty = empty && !collapsed;
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
            marker_images: marker_image_handles,
            presentation: None,
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

    /// Lays out by arena slot, with `measure(arena, width)` giving each leaf's height and
    /// `split` its height when it widens past a float.
    fn layout_arena(
        &self,
        width: f32,
        height: Option<f32>,
        measure: &mut dyn FnMut(usize, f32) -> Option<f32>,
        split: &mut dyn FnMut(usize, f32, f32, f32) -> Option<f32>,
    ) -> Result<Vec<FlowGeometry>, TreeLayoutError> {
        self.prepared.layout_slots(
            FlowViewport {
                left: 0.0,
                top: 0.0,
                width,
                height,
            },
            FlowLimits {
                boxes: big(),
                depth: nz(2_000),
            },
            &mut |arena, used| {
                if self.texts[arena].as_ref().is_some_and(TextLeaf::is_empty) {
                    return Some(0.0);
                }
                measure(arena, used.ceil())
            },
            &mut |arena, narrow, until, wide| split(arena, narrow.ceil(), until, wide.ceil()),
        )
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
                | PaintStep::CellBorder(_) => None,
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
                        .filter(|child| prepared.positions[*child].1 == Position::Static),
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
        // 18.4: outlines may be drawn above everything else.
        for (projected, paint) in self.paints.iter().enumerate() {
            let boxed = prepared.slot[projected].is_some_and(|slot| !prepared.anonymous[slot]);
            if paint.visible && paint.outline.is_some() && boxed {
                steps.push(PaintStep::Outline(projected));
            }
        }
        steps
    }

    /// The steps that draw: a hidden empty cell keeps its place for input, and draws only
    /// around a hosted widget.
    fn drawn_steps(&self, embedded: &BTreeSet<usize>) -> Vec<PaintStep> {
        let mut steps = self.paint_steps(embedded);
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
        let meet = |clip: Option<FlowRect>, more: FlowRect| {
            Some(clip.map_or(more, |clip: FlowRect| FlowRect {
                left: clip.left.max(more.left),
                right: clip.right.min(more.right),
                top: clip.top.max(more.top),
                bottom: clip.bottom.min(more.bottom),
            }))
        };
        let mut clips: Vec<Option<FlowRect>> = vec![None; count];
        // `clip` alone, down through the box tree: a fixed box escapes `overflow`, not this.
        let mut cut: Vec<Option<FlowRect>> = vec![None; count];
        for arena in 0..count {
            let position = prepared.positions[arena].1;
            let owner = prepared.positions[arena].0;
            // 11.1.2: `clip` cuts an absolutely positioned box and what is inside it.
            let own = self.paints[owner]
                .clip
                .filter(|_| matches!(position, Position::Absolute | Position::Fixed))
                .zip(geometry.get(owner).copied().flatten())
                .map(|(sides, outer)| {
                    let border = outer.border;
                    FlowRect {
                        top: sides[0].map_or(border.top, |side| border.top + side),
                        right: sides[1].map_or(border.right, |side| border.left + side),
                        bottom: sides[2].map_or(border.bottom, |side| border.top + side),
                        left: sides[3].map_or(border.left, |side| border.left + side),
                    }
                });
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
    #[must_use]
    pub fn widget(
        mut self,
        id: &str,
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.widgets.push((id.to_owned(), element.into()));
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
            measured: false,
            geometry: Vec::new(),
            slot_geometry: Vec::new(),
            scroll_limits: Vec::new(),
            canvas_height: 0.0,
            canvas_width: 0.0,
            clips: Vec::new(),
            steps: Vec::new(),
            paragraphs: BTreeMap::new(),
            allocations: Vec::new(),
            loaded: BTreeSet::new(),
            loaded_markers: BTreeSet::new(),
            rest_paragraphs: BTreeMap::new(),
            marker_paragraphs: BTreeMap::new(),
            elements,
            ids,
            slots: Vec::new(),
            presses: view.presses,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaintStep {
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

#[derive(Debug, Clone, Default, PartialEq)]
struct Interaction {
    hovered: Option<usize>,
    active: Option<usize>,
    focused: Option<usize>,
    /// Vertical scroll offsets by source node. They outlive restyles and view rebuilds.
    scrolled: BTreeMap<usize, f32>,
    /// Horizontal scroll offsets by source node.
    scrolled_x: BTreeMap<usize, f32>,
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
    pub border: [(f32, Option<[f32; 3]>); 2],
    /// The `border-top-style` and `border-bottom-style`.
    pub border_style: [&'static str; 2],
    pub background: Option<[f32; 3]>,
    /// The source node of the box.
    pub owner: usize,
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

/// Invisible spaces about `indent` wide at `first`'s font, or `None` when too small to show.
fn indent_run(first: &TextRun, indent: f32) -> Option<TextRun> {
    // No line is this wide: more only costs shaping, and text past it is off the box anyway.
    let mut left = (indent / first.size.max(1.0)).min(512.0);
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
    (!text.is_empty()).then(|| TextRun {
        text,
        spacer: true,
        background: None,
        underline: false,
        line_through: false,
        inline_boxes: Vec::new(),
        edge: None,
        ..first.clone()
    })
}

/// 17.6.2.1: on an edge two cells of a collapsed table share, only the winner's border draws,
/// and a `hidden` one draws neither. A cell that is not visible has no say. Only cells
/// meeting along a whole edge are compared: a cell beside a taller one keeps its own part.
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
    let (placed, _) = table_grid(&prepared.boxes, &rows);
    let starts: BTreeMap<(usize, usize), usize> = placed
        .iter()
        .enumerate()
        .map(|(at, item)| ((item.row, item.column), at))
        .collect();
    // In an rtl table the first column is on the right, so its far side is its left.
    let rtl = prepared.boxes[table].direction == Direction::Rtl;
    let (near, far) = if rtl { (3, 1) } else { (1, 3) };
    let mut meetings = Vec::new();
    for item in &placed {
        let beside = starts
            .get(&(item.row, item.column + item.col_span))
            .map(|at| &placed[*at])
            .filter(|other| other.row_span == item.row_span);
        let below = starts
            .get(&(item.row + item.row_span, item.column))
            .map(|at| &placed[*at])
            .filter(|other| other.col_span == item.col_span);
        if let Some(other) = beside {
            meetings.push((item, near, other, far));
        }
        if let Some(other) = below {
            meetings.push((item, 2, other, 0));
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
    let mut losers = Vec::new();
    for (first, first_side, second, second_side) in meetings {
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
            BorderCandidate::new(
                style_of(paints[prepared.positions[item.cell].0].border_style[side]),
                prepared.boxes[item.cell].border[side],
                BorderOrigin::Cell,
                BorderOrder {
                    inline_start: item.column,
                    block_start: item.row,
                },
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
    }
}

/// A file under the base: no scheme, no absolute path, no `..`.
fn local_image(base: &Path, uri: &str) -> Option<image::Handle> {
    let inside = Path::new(uri)
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    let scheme = uri
        .split('/')
        .next()
        .is_some_and(|first| first.contains(':'));
    (inside && !scheme).then(|| image::Handle::from_path(base.join(uri)))
}

/// What Tab asks of the focusable widgets inside a hosted element.
enum HostFocus {
    Has(bool),
    IsFocused(bool),
    /// Focus the first one; the flag says it is still to do.
    Focus(bool),
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
            Self::Focus(pending) => {
                if *pending {
                    state.focus();
                    *pending = false;
                } else {
                    state.unfocus();
                }
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

/// How far each scrolling box can scroll down and across, after moving its contents by
/// its offsets. Parents come first in arena order, so nested scrollers measure moved
/// contents. Only rightward overflow scrolls: contents never reach left of an ltr box's
/// start in a way 11.1.1 lets anyone scroll to, and rtl scrollers stay put horizontally.
fn apply_scrolling(
    prepared: &PreparedTree,
    geometry: &mut [FlowGeometry],
    scrolled: impl Fn(usize) -> (f32, f32),
    text_size: impl Fn(usize, &FlowGeometry) -> Option<Size>,
    viewport: Size,
) -> Vec<(f32, f32)> {
    let mut limits = vec![(0.0_f32, 0.0_f32); prepared.boxes.len()];
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
        // A scroller's own text scrolls in it too.
        let own_text = text_size(arena, &geometry[arena]);
        if let Some(size) = own_text {
            let place = geometry[arena].leaf_band.unwrap_or(geometry[arena].content);
            reach = reach.max(place.top + size.height);
            reach_x = reach_x.max(place.left + size.width);
        }
        scrolling_descendants(prepared, arena, |child, reaches| {
            if reaches {
                let box_of = &geometry[child];
                reach = reach.max(box_of.border.bottom);
                reach_x = reach_x.max(box_of.border.right);
                // Text runs past a box too small for it, unless that box clips it.
                if !prepared.clipped[child]
                    && let Some(size) = text_size(child, box_of)
                {
                    let place = box_of.leaf_band.unwrap_or(box_of.content);
                    reach = reach.max(place.top + size.height);
                    reach_x = reach_x.max(place.left + size.width);
                }
            }
        });
        // 11.1.1: the scrollable area includes the box's own end padding; at the root,
        // the box's whole edge.
        let (trailing, trailing_x) = if arena == 0 {
            reach = reach.max(geometry[0].border.bottom);
            reach_x = reach_x.max(geometry[0].border.right);
            (0.0, 0.0)
        } else {
            (
                padding.bottom - geometry[arena].content.bottom,
                padding.right - geometry[arena].content.right,
            )
        };
        let limit = if reach > bottom {
            reach + trailing - bottom
        } else {
            0.0
        };
        let ltr = prepared.boxes[arena].direction == Direction::Ltr;
        let limit_x = if ltr && reach_x > right {
            reach_x + trailing_x - right
        } else {
            0.0
        };
        limits[arena] = (limit, limit_x);
        let (down, across) = scrolled(arena);
        let (offset, offset_x) = (down.clamp(0.0, limit), across.clamp(0.0, limit_x));
        if offset == 0.0 && offset_x == 0.0 {
            continue;
        }
        if own_text.is_some() {
            let own = &mut geometry[arena];
            let rectangles = [Some(&mut own.content), own.leaf_band.as_mut()];
            for place in rectangles.into_iter().flatten() {
                place.left -= offset_x;
                place.right -= offset_x;
                place.top -= offset;
                place.bottom -= offset;
            }
        }
        let mut moved = Vec::new();
        scrolling_descendants(prepared, arena, |child, _| moved.push(child));
        for child in moved {
            geometry[child].move_by(-offset_x, -offset);
        }
    }
    limits
}

struct BoxTreeWidget<'a, Message, Theme, Renderer: text::Renderer<Font = iced::Font>> {
    base: BoxTree,
    /// The base restyled for `key`.
    tree: BoxTree,
    key: InteractionKey,
    measured: bool,
    geometry: Vec<Option<FlowGeometry>>,
    /// The same layout by arena slot, including anonymous blocks.
    slot_geometry: Vec<FlowGeometry>,
    /// How far each arena slot can scroll, down and across.
    scroll_limits: Vec<(f32, f32)>,
    /// The widget's height at the last layout.
    canvas_height: f32,
    canvas_width: f32,
    clips: Vec<Option<FlowRect>>,
    /// Paint order at the last layout, front last; hit tests walk it on every pointer event.
    steps: Vec<PaintStep>,
    elements: Vec<Element<'a, Message, Theme, Renderer>>,
    /// The CSS id naming each element's box.
    ids: Vec<String>,
    /// Each element's arena slot in the current styling, while it has a box.
    slots: Vec<Option<usize>>,
    presses: Vec<(String, Message)>,
    /// Shaped text, its top-left corner relative to the layout, and its color, by arena.
    paragraphs: BTreeMap<usize, (Renderer::Paragraph, Point, Color)>,
    /// The text past a shorter float, its corner and its leaf, by arena slot.
    rest_paragraphs: BTreeMap<usize, (Renderer::Paragraph, Point, TextLeaf)>,
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
            if let Some(line_size) = run.line_size {
                let height = line_height_of(leaf).to_absolute(Pixels(line_size));
                span = span.line_height(text::LineHeight::Absolute(height));
            }
            if !run.visible {
                return span.color(Color::TRANSPARENT);
            }
            if let Some(c) = run.color {
                span = span.color(Color::from_rgb(c[0], c[1], c[2]));
            }
            span
        })
        .collect();
    let size = leaf.runs.first().map_or(16.0, |run| run.size);
    Renderer::Paragraph::with_spans(text::Text {
        content: spans.as_slice(),
        bounds: Size::new(width, f32::INFINITY),
        size: Pixels(size),
        line_height: line_height_of(leaf),
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
        wrapping: if leaf.wraps {
            text::Wrapping::Word
        } else {
            text::Wrapping::None
        },
    })
}

/// Inline backgrounds and text decorations. A paragraph only draws glyphs: these sit in
/// its run rectangles. `corner` is where the paragraph is drawn.
fn draw_run_decorations<Renderer: text::Renderer<Font = iced::Font>>(
    leaf: &TextLeaf,
    shaped: &Renderer::Paragraph,
    renderer: &mut Renderer,
    corner: Point,
) {
    let quad = |renderer: &mut Renderer, bounds: Rectangle, color: [f32; 3]| {
        renderer.fill_quad(
            Quad {
                bounds,
                ..Quad::default()
            },
            Color::from_rgb(color[0], color[1], color[2]),
        );
    };
    for (index, run) in leaf.runs.iter().enumerate() {
        // A hidden run keeps its room inside the visible boxes around it.
        if !run.visible && run.inline_boxes.is_empty() && run.background.is_none() {
            continue;
        }
        let color = run.color.unwrap_or([0.0; 3]);
        let thickness = (run.size / 14.0).max(1.0);
        for area in shaped.span_bounds(index) {
            let area = Rectangle {
                x: area.x + corner.x,
                y: area.y + corner.y,
                ..area
            };
            // 10.6.1: an inline box wraps the font's content area, not the taller line box.
            let sided = !run.inline_boxes.is_empty();
            let (content_top, content_height) = if sided {
                let height = area.height.min(run.line_size.unwrap_or(run.size) * 1.3);
                (area.y + (area.height - height) / 2.0, height)
            } else {
                (area.y, area.height)
            };
            let content_bottom = content_top + content_height;
            for sides in &run.inline_boxes {
                let top = content_top - sides.padding[0];
                let bottom = content_bottom + sides.padding[1];
                if let Some(background) = sides.background {
                    let bounds = Rectangle {
                        y: top,
                        height: bottom - top,
                        ..area
                    };
                    quad(renderer, bounds, background);
                }
                let edges = [
                    (top - sides.border[0].0, sides.border[0], 0),
                    (bottom, sides.border[1], 2),
                ];
                for (at, (height, edge), side) in edges {
                    if let Some(edge) = edge.filter(|_| height > 0.0) {
                        let bounds = Rectangle {
                            y: at,
                            height,
                            ..area
                        };
                        let style = sides.border_style[side / 2];
                        for (piece, shade) in band_pieces(bounds, side, style, edge) {
                            quad(renderer, piece, shade);
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
                quad(renderer, bounds, background);
            }
            if !run.visible {
                continue;
            }
            if let (Some((start, width, edge)), Some(sides)) = (run.edge, run.inline_boxes.last()) {
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
                    quad(renderer, piece, shade);
                }
            }
            // Line boxes may be taller than the text: measure from their centre, by size.
            let line = |offset: f32| Rectangle {
                x: area.x,
                y: area.y + area.height / 2.0 + offset,
                width: area.width,
                height: thickness,
            };
            if run.underline {
                quad(renderer, line(run.size * 0.4), color);
            }
            if run.line_through {
                quad(renderer, line(0.0), color);
            }
        }
    }
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
    // Forced breaks give several buffer lines, whose offsets do not add up. Bidirectional
    // text reorders per line and its second part would re-detect its base direction.
    if whole.chars().any(|character| {
        matches!(
            character,
            '\n' | '\r' | '\x0B' | '\x0C' | '\u{85}' | '\u{2028}' | '\u{2029}'
                | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}'
                | '\u{590}'..='\u{8FF}'
                | '\u{FB1D}'..='\u{FDFF}'
                | '\u{FE70}'..='\u{FEFF}'
                | '\u{10800}'..='\u{10FFF}'
                | '\u{1E800}'..='\u{1EFFF}'
        )
    }) {
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
    let first_height = paragraph::<Renderer>(renderer, &first, narrow)
        .min_bounds()
        .height;
    Some((first, second, first_height))
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
        for chunk in run.text.split_inclusive(char::is_whitespace) {
            let body = chunk.trim_end();
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
        PaintStep::Text(_) | PaintStep::Marker(_) => {
            !matches!(next, PaintStep::Text(_) | PaintStep::Marker(_))
        }
        PaintStep::Box(index) => tree.backgrounds.contains_key(&index),
        PaintStep::Outline(_) | PaintStep::CellBorder(_) => false,
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
    let Some((width, height)) = renderer.background_size(handle) else {
        return;
    };
    if width <= 0.0 || height <= 0.0 || area.width <= 0.0 || area.height <= 0.0 {
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
    for (side, band) in border_bands(outer, inner).into_iter().enumerate() {
        for (piece, shade) in band_pieces(band, side, style, color) {
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
    let Some(Some(geometry)) = geometry.get(index) else {
        return;
    };
    let quad = |renderer: &mut Renderer, bounds: Rectangle, color: [f32; 3]| {
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
                Color::from_rgb(color[0], color[1], color[2]),
            );
        }
    };
    let paint = tree.paints[index];
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
    if let Some(color) = paint.background.filter(|_| parts != BoxParts::Border) {
        // 14.2: the background extends under the border.
        renderer.fill_quad(
            Quad {
                bounds: {
                    let bounds = rectangle(border.left, border.right, border.top, border.bottom);
                    Rectangle {
                        x: bounds.x + origin.x,
                        y: bounds.y + origin.y,
                        ..bounds
                    }
                },
                ..Quad::default()
            },
            Color::from_rgba(
                color[0],
                color[1],
                color[2],
                paint.background_fade.unwrap_or(1.0),
            ),
        );
    }
    if parts != BoxParts::Background {
        let bands = border_bands(border, geometry.padding);
        for (side, (band, color)) in bands.into_iter().zip(paint.border).enumerate() {
            if let Some(color) = color {
                for (piece, shade) in band_pieces(band, side, paint.border_style[side], color) {
                    quad(renderer, piece, shade);
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
                && let Some((shaped, corner, _)) = self.paragraphs.get(&arena)
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
                    .map(|(shaped, corner, leaf)| (shaped, *corner, leaf));
                for (shaped, corner, leaf) in std::iter::once((shaped, *corner, leaf)).chain(rest) {
                    let local = Point::new(point.x - corner.x, point.y - corner.y);
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
            let moved = bring(scrolled_x, limit_x, area.left, end_x, left, right);
            left -= moved;
            right -= moved;
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
        let interaction = tree.state.downcast_ref::<Interaction>().clone();
        // A rebuilt view keeps this state; `styled` ignores a node this tree lacks.
        let key = (interaction.hovered, interaction.active, interaction.focused);
        if key != self.key {
            self.key = key;
            self.tree = self.base.variant(key);
            self.measured = false;
        }
        if !self.measured {
            self.measured = true;
            self.slots = self.ids.iter().map(|id| self.tree.leaf_slot(id)).collect();
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
        // A refused layout draws nothing; `BoxTree::layout_text` reports the cause.
        let (elements, slots, children) = (&mut self.elements, &self.slots, &mut tree.children);
        let current = &self.tree;
        self.slot_geometry = current
            .layout_arena(
                available.width,
                Some(available.height),
                &mut |arena, width| {
                    if let Some(index) = slots.iter().position(|slot| *slot == Some(arena)) {
                        // A Fill-height widget takes what the box has, and asks for nothing.
                        if elements[index].as_widget().size().height.is_fill() {
                            return Some(0.0);
                        }
                        let limits =
                            layout::Limits::new(Size::ZERO, Size::new(width, f32::INFINITY));
                        return Some(
                            elements[index]
                                .as_widget_mut()
                                .layout(&mut children[index], renderer, &limits)
                                .size()
                                .height,
                        );
                    }
                    current.texts[arena]
                        .as_ref()
                        .map(|leaf| paragraph(renderer, leaf, width).min_bounds().height)
                },
                &mut |arena, narrow, until, wide| {
                    let leaf = current.texts[arena].as_ref()?;
                    let (_, second, first_height) = split_leaf(renderer, leaf, narrow, until)?;
                    let rest: Renderer::Paragraph = paragraph(renderer, &second, wide);
                    Some(first_height + rest.min_bounds().height)
                },
            )
            .unwrap_or_default();
        self.canvas_height = available.height;
        self.canvas_width = available.width;
        let originals = &self.tree.originals;
        let positions = &self.tree.prepared.positions;
        let texts = &self.tree.texts;
        let boxes = &self.tree.prepared.boxes;
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
                if (geometry.leaf_after.is_some() && auto_height) || slots.contains(&Some(arena)) {
                    return None;
                }
                let place = geometry.leaf_band.unwrap_or(geometry.content);
                let width = (place.right - place.left).ceil();
                let size = paragraph(renderer, leaf, width).min_bounds();
                // Right lines overflow toward the start, where nothing scrolls; centered ones
                // spill half each way.
                Some(match leaf.align {
                    "right" => Size::new(0.0, size.height),
                    "center" => Size::new((width + size.width) / 2.0, size.height),
                    _ => size,
                })
            },
            available,
        );
        self.geometry = self.tree.prepared.by_projected(&self.slot_geometry);
        self.clips = self.tree.clips(&self.geometry);
        self.paragraphs.clear();
        self.rest_paragraphs.clear();
        for (arena, leaf) in self.tree.texts.iter().enumerate() {
            let Some(leaf) = leaf.as_ref().filter(|leaf| !leaf.is_empty()) else {
                continue;
            };
            let Some(geometry) = self.slot_geometry.get(arena) else {
                continue;
            };
            if self.slots.contains(&Some(arena)) {
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
                self.paragraphs
                    .insert(arena, (shaped_first, corner_first, Color::BLACK));
                self.rest_paragraphs
                    .insert(arena, (shaped_rest, corner_rest, second));
                continue;
            }
            // A paragraph aligns its lines within its widest line and draws from that
            // block's corner: place the block in the content box.
            let block = paragraph::<Renderer>(renderer, leaf, width)
                .min_bounds()
                .width;
            let corner = Point::new(
                match leaf.align {
                    "center" => place.left + (width - block) / 2.0,
                    "right" => place.left + width - block,
                    _ => place.left,
                },
                place.top,
            );
            self.paragraphs.insert(
                arena,
                (paragraph(renderer, leaf, width), corner, Color::BLACK),
            );
        }
        self.steps = self.tree.paint_steps(&self.embedded());
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
                None => layout::Node::new(Size::ZERO),
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
        operation.traverse(&mut |operation| {
            for ((element, child), child_layout) in self
                .elements
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                element
                    .as_widget_mut()
                    .operate(child, child_layout, renderer, operation);
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
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
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
            let next = match key.as_ref() {
                keyboard::Key::Named(keyboard::key::Named::Tab) => {
                    // A widget focused by a click counts as where Tab starts from.
                    hosts = self.focus_hosts(tree, layout, renderer);
                    for &(index, node) in &hosts {
                        let mut found = HostFocus::IsFocused(false);
                        self.operate_host(tree, layout, renderer, index, &mut found);
                        if matches!(found, HostFocus::IsFocused(true)) {
                            focused = Some(node);
                        }
                    }
                    let order = self.tab_order(&hosts);
                    if order.is_empty() {
                        return;
                    }
                    let at = focused.and_then(|node| order.iter().position(|item| *item == node));
                    let count = order.len();
                    let index = match (at, modifiers.shift()) {
                        (Some(index), false) => (index + 1) % count,
                        (Some(index), true) => (index + count - 1) % count,
                        (None, false) => 0,
                        (None, true) => count - 1,
                    };
                    Some(order[index])
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
                    if let Some(message) = self.press_message(focused) {
                        shell.publish(message);
                        shell.capture_event();
                    }
                    return;
                }
                _ => return,
            };
            // Every element, hidden ones too: their widgets keep their state.
            for index in 0..self.elements.len() {
                let target = hosts
                    .iter()
                    .any(|&(host, node)| host == index && next == Some(node));
                let mut change = if target {
                    HostFocus::Focus(true)
                } else {
                    HostFocus::Unfocus
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
            if moved || !self.base.same_variant(after, before) {
                shell.invalidate_layout();
                shell.request_redraw();
            }
            return;
        }
        let Event::Mouse(mouse_event) = event else {
            return;
        };
        let bounds = layout.bounds();
        let hit = cursor.position_in(bounds).and_then(|point| self.hit(point));
        let focus_after_press = self.focus_target(hit);
        let state = tree.state.downcast_mut::<Interaction>();
        if let mouse::Event::WheelScrolled { delta } = mouse_event {
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
                if position == Some(Position::Fixed) {
                    break;
                }
                let here = arena.map(|arena| self.tree.prepared.positions[arena].1);
                let skipped = position == Some(Position::Absolute)
                    && here == Some(Position::Static)
                    && arena != Some(0);
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
                    let wanted_x = (current_x + amount_x).clamp(0.0, limit_x);
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
                shell.capture_event();
                shell.invalidate_layout();
                shell.request_redraw();
            }
            return;
        }
        let mut next = state.clone();
        next.hovered = hit;
        match mouse_event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                next.active = hit;
                next.focused = focus_after_press;
                if !shell.is_event_captured()
                    && let Some(message) = self.press_message(hit)
                {
                    shell.publish(message);
                    shell.capture_event();
                }
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => next.active = None,
            _ => {}
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
        if inner == mouse::Interaction::default()
            && let Some(point) = cursor.position_in(layout.bounds())
        {
            let hit = self.hit(point);
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
        let origin = bounds.position();
        let embedded = self.embedded();
        let steps = self.tree.drawn_steps(&embedded);
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
            };
            let mut clip = arena.and_then(|arena| self.clips.get(arena).copied().flatten());
            // A box clips its own text as it clips its children.
            if let PaintStep::Text(arena) = *step
                && prepared.clipped[arena]
                && let Some(outer) = self.slot_geometry.get(arena)
            {
                let more = outer.padding;
                clip = Some(clip.map_or(more, |clip| FlowRect {
                    left: clip.left.max(more.left),
                    right: clip.right.min(more.right),
                    top: clip.top.max(more.top),
                    bottom: clip.bottom.min(more.bottom),
                }));
            }
            clip.map_or(bounds, |clip| {
                bounds
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
                // The canvas scrolls with the root, unlike any other scroller's background.
                let scrolled = tree
                    .state
                    .downcast_ref::<Interaction>()
                    .scrolled
                    .get(&self.tree.originals[prepared.positions[0].0])
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(
                        0.0,
                        self.scroll_limits.first().map_or(0.0, |limits| limits.0),
                    );
                let across = tree
                    .state
                    .downcast_ref::<Interaction>()
                    .scrolled_x
                    .get(&self.tree.originals[prepared.positions[0].0])
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(
                        0.0,
                        self.scroll_limits.first().map_or(0.0, |limits| limits.1),
                    );
                let mut placed = moved_rectangle(geometry.padding, origin);
                placed.y -= scrolled;
                placed.x -= across;
                placed
            };
            renderer.with_layer(bounds, |renderer| {
                draw_background_image(renderer, background, positioning, bounds);
            });
        }
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
                    match *step {
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
                                    origin,
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
                                    // border edge, under the border.
                                    let padding = moved_rectangle(geometry.padding, origin);
                                    let positioning = if background.0.fixed {
                                        viewport_box
                                    } else {
                                        padding
                                    };
                                    let area = moved_rectangle(geometry.border, origin)
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
                        PaintStep::CellBorder(index) => draw_box(
                            &self.tree,
                            &self.geometry,
                            renderer,
                            origin,
                            index,
                            BoxParts::CellBorder,
                        ),
                        PaintStep::Outline(index) => {
                            draw_outline(&self.tree, &self.geometry, renderer, origin, index);
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
                            if let Some((shaped, corner, color)) = self.paragraphs.get(&arena) {
                                let corner = Point::new(origin.x + corner.x, origin.y + corner.y);
                                if let Some(leaf) = self.tree.texts[arena].as_ref() {
                                    draw_run_decorations(leaf, shaped, renderer, corner);
                                }
                                renderer.fill_paragraph(shaped, corner, *color, clip);
                            }
                            if let Some((shaped, corner, leaf)) = self.rest_paragraphs.get(&arena) {
                                let corner = Point::new(origin.x + corner.x, origin.y + corner.y);
                                draw_run_decorations(leaf, shaped, renderer, corner);
                                renderer.fill_paragraph(shaped, corner, Color::BLACK, clip);
                            }
                            if let Some(index) =
                                self.slots.iter().position(|slot| *slot == Some(arena))
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
                                    viewport,
                                );
                            }
                        }
                    }
                }
            });
            start = end;
        }
        // A plain position indicator at each scrolled box's right padding edge.
        renderer.with_layer(bounds, |renderer| {
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
                    bars.push(Rectangle {
                        x: origin.x + area.left + along,
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
        overlay::from_children(
            &mut self.elements,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::adapters;
    use iced::{Background, Theme, Transformation};
    use iced_css_core::source::AttributeLimits;

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
            |arena, _| (arena == 1).then_some(Size::new(100.0, 10.0)),
            Size::new(800.0, 600.0),
        );
        assert_eq!(limits[1].1, 40.0);
        assert_eq!(geometry[1].content.left, left - 15.0);
        assert_eq!(geometry[1].padding, padding);
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
            tree.drawn_steps(embedded)
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
        // Deep enough that this selector's backtracking overruns its budget, only on hover.
        let mut deep = node("leaf", vec![]);
        for at in 0..200 {
            deep = node(&format!("n{at}"), vec![deep]);
        }
        let tree = BoxTree::new(
            deep,
            "#nope * * * * * *:hover { color:#ff0000; } #leaf { color:#00ff00; }",
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
}
