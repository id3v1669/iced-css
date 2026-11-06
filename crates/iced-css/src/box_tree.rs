//! A stylesheet-driven iced widget for captured box trees: real cascade, general block
//! flow, background and border paint, and inline text: consecutive inline siblings share
//! one wrapped paragraph of styled runs. Runs carry size, color, weight, style, family,
//! underline, strikethrough and background. One line height and alignment apply per
//! paragraph. Inline boxes draw no border or padding. Native iced widgets can be placed
//! in leaf boxes by CSS id, where they receive input; boxes can publish a message on press;
//! `:hover` and `:active` restyle the tree. `overflow` other than visible clips, without
//! scrolling. Captured payloads themselves are not drawn.
use crate::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, Construction, source::ConstructionState},
    evaluation::{
        clearance_box_facts::ClearanceBoxFactsLimits,
        display_rules::{BundleLimits, DisplayRuleBundle, GraphBinding, GraphOrigin},
        source_display::{SourceDisplayLimits, evaluate_source_display},
        tree_layout::{
            BoxPaint, PreparedTree, TextLineHeight, TextStyle, TreeLayoutError,
            evaluate_tree_paint, prepare_captured_tree, prepare_tree_source_projection,
        },
    },
    host::source_publication::{InitialSourceHost, PublicationLimits},
};
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, overlay,
    renderer::{self, Quad},
    text::{self, Paragraph as _},
    widget::{Operation, Tree, tree},
};
use iced::{Color, Element, Event, Length, Pixels, Point, Rectangle, Size, Vector, mouse};
use iced_css_core::{
    cascade::{
        rule::{RuleBatchLimits, RuleCascadeLimits, RuleContext},
        sheets::{AuthorSheet, AuthorSheets, Membership, SheetId, SheetLimits},
        ua_display::{UaDisplayCatalog, UaDisplayLimits},
    },
    formatting::{
        block_flow::{FlowContent, FlowGeometry, FlowLimits, FlowRect, FlowViewport},
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
    sync::{Arc, Mutex, PoisonError},
};

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
        work: nz(WORK),
    }
}
fn per_target() -> RuleBatchLimits {
    RuleBatchLimits {
        rules: big(),
        per_rule: RuleCascadeLimits {
            declarations: big(),
            matching: MatchLimits {
                work: nz(10_000),
                retained_bytes: big(),
            },
        },
    }
}
fn facts() -> ClearanceBoxFactsLimits {
    ClearanceBoxFactsLimits {
        work: nz(WORK),
        rule_visits: nz(WORK),
        matching_work: nz(2_000_000_000),
        declaration_offers: nz(WORK),
        membership_work: nz(WORK),
        numeric_work: nz(WORK),
        selectors: selectors(),
        per_target: per_target(),
        numeric: numeric(),
    }
}
fn parse(css: &str) -> Result<OwnedStylesheetGraph, BoxTreeError> {
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
                    entries: big(),
                    retained_bytes: nz(BYTES),
                    work: nz(WORK),
                },
                charset_work: big(),
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
            first_order: 100_000,
            first_declaration: NonZeroU64::new(100_000).expect("positive"),
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

/// One styled run of inline text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub size: f32,
    pub color: Option<[f32; 3]>,
    pub background: Option<[f32; 3]>,
    pub weight: u16,
    pub italic: bool,
    pub family: Option<String>,
    pub underline: bool,
    pub line_through: bool,
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

/// The hovered and active source nodes a tree is styled for.
type InteractionKey = (Option<usize>, Option<usize>);

#[derive(Debug)]
struct Source {
    construction: Construction,
    css: String,
    /// Restyled trees, at most two per node. Each interaction state cascades once.
    variants: Mutex<BTreeMap<InteractionKey, BoxTree>>,
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
    source: Arc<Source>,
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
        let (_, construction) = root.into_parts();
        let source = Arc::new(Source {
            construction,
            css: css.to_owned(),
            variants: Mutex::new(BTreeMap::new()),
        });
        Self::styled(&source, (None, None))
    }

    /// This tree restyled for an interaction state, cascading it the first time. A
    /// refused restyle keeps the base styling: hovering must not take the view away.
    fn variant(&self, key: InteractionKey) -> Self {
        if let Some(found) = self
            .source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return found.clone();
        }
        let built = Self::styled(&self.source, key).unwrap_or_else(|_| self.clone());
        self.source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, built.clone());
        built
    }

    fn styled(source: &Arc<Source>, key: InteractionKey) -> Result<Self, BoxTreeError> {
        let construction = source.construction.clone();
        let css = source.css.as_str();
        let count = construction.nodes().len();
        // The selector source extends hover and active to ancestors (5.11.3).
        let mut states = vec![ConstructionState::default(); count];
        if let Some(node) = key.0 {
            states[node].hovered = true;
        }
        if let Some(node) = key.1 {
            states[node].active = true;
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
        let projection = prepare_tree_source_projection(&host, snapshot, big()).map_err(failed)?;
        let catalog = UaDisplayCatalog::new(UaDisplayLimits {
            rules: big(),
            retained_bytes: nz(BYTES),
            work: nz(WORK),
            values: colors(),
        })
        .map_err(failed)?;
        let graph = parse(css)?;
        let sheet = SheetId::new(NonZeroU64::MIN);
        let sheets = AuthorSheets::new(
            vec![AuthorSheet {
                id: sheet,
                enabled: true,
                membership: Membership::Persistent,
            }],
            vec![],
            true,
            None,
            // This bound is itself charged per membership query, so keep it small.
            SheetLimits {
                entries: big(),
                retained_bytes: nz(BYTES),
                work: nz(100),
            },
        )
        .map_err(failed)?;
        let bindings = [GraphBinding {
            graph: &graph,
            origin: GraphOrigin::Author(sheet),
        }];
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
                work: nz(WORK),
                rule_visits: nz(WORK),
                matching_work: nz(2_000_000_000),
                declaration_offers: nz(WORK),
                membership_work: nz(WORK),
                selectors: selectors(),
                per_target: per_target(),
            },
        )
        .map_err(failed)?;
        let prepared = prepare_captured_tree(&projection, &display, facts(), Some(16.0), |_| None)
            .map_err(failed)?;
        let (mut paints, styles) =
            evaluate_tree_paint(&projection, &display, facts(), &prepared.font_sizes)
                .map_err(failed)?;
        let nodes = snapshot.construction().nodes();
        let raw_paints = paints.clone();
        let build_leaf = |arena: usize| -> Option<TextLeaf> {
            let runs = &prepared.runs[arena];
            let first = *runs.first()?;
            let mut texts: Vec<(String, &'static str)> = runs
                .iter()
                .map(|&run| {
                    let content = projection
                        .original_index(run)
                        .and_then(|original| nodes[original].text())
                        .unwrap_or_default();
                    (
                        transformed(content, styles[run].text_transform),
                        styles[run].white_space,
                    )
                })
                .collect();
            collapse(&mut texts);
            let runs = runs
                .iter()
                .zip(texts)
                .map(|(&run, (text, _))| {
                    let style: &TextStyle = &styles[run];
                    // An inline background paints behind its own and its children's text.
                    let mut owner = Some(run);
                    let mut background = None;
                    while let Some(at) = owner {
                        background = raw_paints[at].background;
                        if background.is_some() {
                            break;
                        }
                        owner = prepared.inline_parent[at];
                    }
                    TextRun {
                        text,
                        size: prepared.font_sizes[run],
                        color: raw_paints[run].color,
                        background,
                        weight: style.weight,
                        italic: style.italic,
                        family: style.family.clone(),
                        underline: style.underline,
                        line_through: style.line_through,
                    }
                })
                .collect();
            let lead = &styles[first];
            Some(TextLeaf {
                runs,
                align: lead.align,
                wraps: !matches!(lead.white_space, "pre" | "nowrap"),
                line_height: lead.line_height,
            })
        };
        let texts: Vec<Option<TextLeaf>> = (0..prepared.boxes.len()).map(build_leaf).collect();
        // A block's own paint already drew its box: a block text leaf's run has no
        // highlight. An anonymous block has no paint of its own at all (9.2.1.1).
        let mut texts = texts;
        for (arena, &(projected, _)) in prepared.positions.iter().enumerate() {
            if prepared.anonymous[arena] {
                paints[projected].background = None;
                paints[projected].border = [None; 4];
            } else if let Some(leaf) = &mut texts[arena] {
                for run in &mut leaf.runs {
                    run.background = None;
                }
            }
        }
        let originals: Vec<usize> = (0..count)
            .map(|projected| projection.original_index(projected).unwrap_or_default())
            .collect();
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
            source: Arc::clone(source),
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

    /// Lays out with `measure(arena, width)` giving each leaf's height, or `None`.
    fn layout_arena(
        &self,
        width: f32,
        height: Option<f32>,
        measure: &mut dyn FnMut(usize, f32) -> Option<f32>,
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
                if self.texts[arena].as_ref().is_some_and(TextLeaf::is_empty) {
                    return Some(0.0);
                }
                measure(arena, used.ceil())
            },
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

    /// Projected box indices in 9.9 paint order: flow blocks, then floats, then
    /// positioned boxes in tree order, each with the descendants that belong to them.
    #[must_use]
    pub fn paint_order(&self) -> Vec<usize> {
        self.paint_steps(&BTreeSet::new())
            .into_iter()
            .filter_map(|step| match step {
                PaintStep::Box(index) => Some(index),
                PaintStep::Text(_) => None,
            })
            .collect()
    }

    fn paint_steps(&self, embedded: &BTreeSet<usize>) -> Vec<PaintStep> {
        let prepared = &self.prepared;
        let mut steps = Vec::with_capacity(prepared.boxes.len());
        if prepared.boxes.is_empty() {
            return steps;
        }
        let mut positioned = BTreeSet::new();
        self.paint_context(0, embedded, &mut positioned, &mut steps);
        // Arena order is tree order, so the lowest pending index is always next.
        while let Some(next) = positioned.pop_first() {
            self.paint_context(next, embedded, &mut positioned, &mut steps);
        }
        steps
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
        let mut stack = vec![root];
        while let Some(arena) = stack.pop() {
            steps.push(PaintStep::Box(prepared.positions[arena].0));
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
    }

    /// Wraps the tree as an iced element that fills the size it is given.
    pub fn view<'a, Message: Clone + 'a, Theme: 'a, Renderer>(
        self,
    ) -> Element<'a, Message, Theme, Renderer>
    where
        Renderer: text::Renderer<Font = iced::Font> + 'a,
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
        for arena in 0..count {
            let position = prepared.positions[arena].1;
            if position == Position::Fixed {
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
            let Some(holder) = holder else { continue };
            clips[arena] = clips[holder];
            if prepared.clipped[holder]
                && let Some(Some(outer)) = geometry.get(prepared.positions[holder].0)
            {
                clips[arena] = meet(clips[arena], outer.padding);
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
    /// says; a box with auto height takes the widget's height at the used width. An id
    /// matching no leaf box is ignored.
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
    Renderer: text::Renderer<Font = iced::Font> + 'a,
{
    fn from(view: BoxTreeView<'a, Message, Theme, Renderer>) -> Self {
        let tree = view.tree;
        let (mut elements, mut slots) = (Vec::new(), Vec::new());
        for (id, element) in view.widgets {
            let found = tree.ids.iter().enumerate().find(|(arena, value)| {
                value.as_deref() == Some(id.as_str())
                    && tree.prepared.boxes[*arena].children.is_empty()
            });
            if let Some((arena, _)) = found {
                slots.push(arena);
                elements.push(element);
            }
        }
        let presses = view
            .presses
            .into_iter()
            .filter_map(|(id, message)| {
                let arena = tree
                    .ids
                    .iter()
                    .position(|value| value.as_deref() == Some(id.as_str()))?;
                Some((arena, message))
            })
            .collect();
        Element::new(BoxTreeWidget {
            base: tree.clone(),
            tree,
            key: (None, None),
            measured: false,
            geometry: Vec::new(),
            clips: Vec::new(),
            paragraphs: BTreeMap::new(),
            elements,
            slots,
            presses,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaintStep {
    /// A box's background and border, by projected index.
    Box(usize),
    /// A text leaf, by arena slot.
    Text(usize),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Interaction {
    hovered: Option<usize>,
    active: Option<usize>,
}

struct BoxTreeWidget<'a, Message, Theme, Renderer: text::Renderer<Font = iced::Font>> {
    base: BoxTree,
    /// The base restyled for `key`.
    tree: BoxTree,
    key: InteractionKey,
    measured: bool,
    geometry: Vec<Option<FlowGeometry>>,
    clips: Vec<Option<FlowRect>>,
    elements: Vec<Element<'a, Message, Theme, Renderer>>,
    /// The arena slot of each element.
    slots: Vec<usize>,
    presses: Vec<(usize, Message)>,
    /// Shaped text, its top-left corner relative to the layout, and its color, by arena.
    paragraphs: BTreeMap<usize, (Renderer::Paragraph, Point, Color)>,
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
                .font(font_of(base, run))
                .underline(run.underline)
                .strikethrough(run.line_through);
            if let Some(c) = run.color {
                span = span.color(Color::from_rgb(c[0], c[1], c[2]));
            }
            if let Some(c) = run.background {
                span = span.background(Color::from_rgb(c[0], c[1], c[2]));
            }
            span
        })
        .collect();
    let size = leaf.runs.first().map_or(16.0, |run| run.size);
    Renderer::Paragraph::with_spans(text::Text {
        content: spans.as_slice(),
        bounds: Size::new(width, f32::INFINITY),
        size: Pixels(size),
        line_height: match leaf.line_height {
            TextLineHeight::Normal => text::LineHeight::default(),
            TextLineHeight::Multiplier(factor) => text::LineHeight::Relative(factor),
            TextLineHeight::Px(px) => text::LineHeight::Absolute(Pixels(px)),
        },
        font: base,
        align_x: match leaf.align {
            "center" => text::Alignment::Center,
            "right" => text::Alignment::Right,
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

/// The narrowest width a leaf can wrap to: its widest single word.
fn narrowest_width<Renderer: text::Renderer<Font = iced::Font>>(
    renderer: &Renderer,
    leaf: &TextLeaf,
) -> f32 {
    let mut widest = 0.0_f32;
    for run in &leaf.runs {
        for word in run.text.split_whitespace() {
            let single = TextLeaf {
                runs: vec![TextRun {
                    text: word.to_owned(),
                    ..run.clone()
                }],
                ..leaf.clone()
            };
            widest = widest.max(
                paragraph(renderer, &single, f32::INFINITY)
                    .min_bounds()
                    .width,
            );
        }
    }
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

/// One box's background and border bands, offset to the layout origin.
fn draw_box<Renderer: renderer::Renderer>(
    tree: &BoxTree,
    geometry: &[Option<FlowGeometry>],
    renderer: &mut Renderer,
    origin: Point,
    index: usize,
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
    let border = geometry.border;
    if let Some(color) = paint.background {
        // 14.2: the background extends under the border.
        quad(
            renderer,
            rectangle(border.left, border.right, border.top, border.bottom),
            color,
        );
    }
    let bands = border_bands(border, geometry.padding);
    for (band, color) in bands.into_iter().zip(paint.border) {
        if let Some(color) = color {
            quad(renderer, band, color);
        }
    }
}

impl<Message, Theme, Renderer> BoxTreeWidget<'_, Message, Theme, Renderer>
where
    Renderer: text::Renderer<Font = iced::Font>,
{
    /// The topmost box under a point relative to the layout, as a source node.
    fn hit(&self, point: Point) -> Option<usize> {
        for step in self.tree.paint_order().into_iter().rev() {
            let Some(Some(geometry)) = self.geometry.get(step) else {
                continue;
            };
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
            let projected = self.tree.originals.iter().position(|value| *value == node);
            let arena = projected.and_then(|projected| self.tree.prepared.slot[projected]);
            if let Some(found) = self.presses.iter().find(|(owner, _)| Some(*owner) == arena) {
                return Some(found.1.clone());
            }
            at = nodes[node].parent();
        }
        None
    }

    fn embedded(&self) -> BTreeSet<usize> {
        self.slots.iter().copied().collect()
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for BoxTreeWidget<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: text::Renderer<Font = iced::Font>,
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
        let interaction = *tree.state.downcast_ref::<Interaction>();
        let key = (interaction.hovered, interaction.active);
        if key != self.key {
            self.key = key;
            self.tree = self.base.variant(key);
            self.measured = false;
        }
        if !self.measured {
            self.measured = true;
            self.tree.set_text_widths(|leaf| {
                let longest = paragraph(renderer, leaf, f32::INFINITY).min_bounds().width;
                (narrowest_width(renderer, leaf), longest)
            });
            for (index, element) in self.elements.iter_mut().enumerate() {
                let unbounded =
                    layout::Limits::new(Size::ZERO, Size::new(f32::INFINITY, f32::INFINITY));
                let natural = element
                    .as_widget_mut()
                    .layout(&mut tree.children[index], renderer, &unbounded)
                    .size()
                    .width;
                // A Fill-width widget reports no natural width of its own.
                let natural = if natural.is_finite() { natural } else { 0.0 };
                self.tree.prepared.boxes[self.slots[index]].content = Some(FlowContent {
                    height: 0.0,
                    preferred_width: natural,
                    preferred_min_width: natural,
                });
            }
        }
        // A refused layout draws nothing; `BoxTree::layout_text` reports the cause.
        let (elements, slots, children) = (&mut self.elements, &self.slots, &mut tree.children);
        let current = &self.tree;
        self.geometry = current
            .layout_arena(
                available.width,
                Some(available.height),
                &mut |arena, width| {
                    if let Some(index) = slots.iter().position(|slot| *slot == arena) {
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
            )
            .unwrap_or_default();
        self.clips = self.tree.clips(&self.geometry);
        self.paragraphs.clear();
        for (arena, leaf) in self.tree.texts.iter().enumerate() {
            let Some(leaf) = leaf.as_ref().filter(|leaf| !leaf.is_empty()) else {
                continue;
            };
            let projected = self.tree.prepared.positions[arena].0;
            let Some(Some(geometry)) = self.geometry.get(projected) else {
                continue;
            };
            let place = geometry.leaf_band.unwrap_or(geometry.content);
            self.paragraphs.insert(
                arena,
                (
                    paragraph(renderer, leaf, (place.right - place.left).ceil()),
                    Point::new(place.left, place.top),
                    Color::BLACK,
                ),
            );
        }
        let mut nodes = Vec::with_capacity(self.elements.len());
        for (index, element) in self.elements.iter_mut().enumerate() {
            let projected = self.tree.prepared.positions[self.slots[index]].0;
            let place = self
                .geometry
                .get(projected)
                .copied()
                .flatten()
                .map(|geometry| geometry.leaf_band.unwrap_or(geometry.content));
            let node = match place {
                Some(place) => {
                    let width = (place.right - place.left).max(0.0);
                    let limits = layout::Limits::new(Size::ZERO, Size::new(width, f32::INFINITY));
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
        for ((element, child), child_layout) in self
            .elements
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            element.as_widget_mut().update(
                child,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
        let Event::Mouse(mouse_event) = event else {
            return;
        };
        let bounds = layout.bounds();
        let hit = cursor.position_in(bounds).and_then(|point| self.hit(point));
        let state = tree.state.downcast_mut::<Interaction>();
        let mut next = *state;
        next.hovered = hit;
        match mouse_event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                next.active = hit;
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
            *state = next;
            // Only states that a rule distinguishes need a new layout.
            if self.base.variant((next.hovered, next.active)) != self.tree {
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
        let inner = self
            .elements
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((element, child), child_layout)| {
                element.as_widget().mouse_interaction(
                    child,
                    child_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .max()
            .unwrap_or_default();
        if inner == mouse::Interaction::default()
            && let Some(point) = cursor.position_in(layout.bounds())
            && self.press_message(self.hit(point)).is_some()
        {
            return mouse::Interaction::Pointer;
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
        let steps = self.tree.paint_steps(&embedded);
        let prepared = &self.tree.prepared;
        let clip_of = |step: &PaintStep| -> Rectangle {
            let arena = match *step {
                PaintStep::Box(index) => prepared.slot[index],
                PaintStep::Text(arena) => Some(arena),
            };
            let clip = arena.and_then(|arena| self.clips.get(arena).copied().flatten());
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
        // A layer draws its text over its quads, so anything painted after text needs
        // a layer of its own, as does any change of clip.
        let mut start = 0;
        while start < steps.len() {
            let clip = clip_of(&steps[start]);
            let mut end = start + 1;
            while end < steps.len()
                && clip_of(&steps[end]) == clip
                && !(matches!(steps[end - 1], PaintStep::Text(_))
                    && matches!(steps[end], PaintStep::Box(_)))
            {
                end += 1;
            }
            renderer.with_layer(clip, |renderer| {
                for step in &steps[start..end] {
                    match *step {
                        PaintStep::Box(index) => {
                            draw_box(&self.tree, &self.geometry, renderer, origin, index);
                        }
                        PaintStep::Text(arena) => {
                            if let Some((shaped, corner, color)) = self.paragraphs.get(&arena) {
                                let position = Point::new(origin.x + corner.x, origin.y + corner.y);
                                renderer.fill_paragraph(shaped, position, *color, clip);
                            }
                            if let Some(index) = self.slots.iter().position(|slot| *slot == arena)
                                && let Some(child_layout) = layout.children().nth(index)
                            {
                                self.elements[index].as_widget().draw(
                                    &tree.children[index],
                                    renderer,
                                    theme,
                                    style,
                                    child_layout,
                                    cursor,
                                    viewport,
                                );
                            }
                        }
                    }
                }
            });
            start = end;
        }
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
            draw_box(&tree, &geometry, &mut renderer, Point::new(5.0, 7.0), index);
        }
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
}
