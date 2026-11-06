//! A stylesheet-driven iced widget for captured box trees: real cascade, general block
//! flow, background and border paint, and inline text: consecutive inline siblings share
//! one wrapped paragraph of styled runs. Runs carry size, color, weight, style, family,
//! underline, strikethrough and background. One line height and alignment apply per
//! paragraph. Inline boxes draw no border or padding. Native iced widgets can be placed
//! in leaf boxes by CSS id, where they receive input; boxes can publish a message on press;
//! `:hover` and `:active` restyle the tree. `overflow` other than visible clips, and
//! `auto` and `scroll` boxes scroll vertically with the wheel; there is no horizontal
//! scrolling or draggable bar. Captured payloads themselves are not drawn.
use crate::{
    animation::engine::ProgressLimits,
    capture::{CaptureLimits, CapturedElement, Construction, source::ConstructionState},
    evaluation::{
        clearance_box_facts::ClearanceBoxFactsLimits,
        display_rules::{BundleLimits, DisplayRuleBundle, GraphBinding, GraphOrigin},
        source_display::{SourceDisplayLimits, evaluate_source_display},
        tree_layout::{
            BoxPaint, GeneratedRun, PreparedTree, RunSource, TextLineHeight, TextStyle,
            TreeLayoutError, TreePaint, evaluate_tree_paint, prepare_captured_tree,
            prepare_tree_source_projection,
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
        block_flow::{
            FlowContent, FlowGeometry, FlowLimits, FlowRect, FlowViewport, RowAlign, TableRole,
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
    /// Restyles already cascaded. Pointerless, so nothing here keeps the source alive.
    variants: Mutex<BTreeMap<InteractionKey, Style>>,
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
    /// refused restyle keeps this styling: hovering must not take the view away.
    fn variant(&self, key: InteractionKey) -> Self {
        let cached = self
            .source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .cloned();
        if let Some(style) = cached {
            return Self::from_style(style, Arc::clone(&self.source));
        }
        let built = Self::styled(&self.source, key).unwrap_or_else(|_| self.clone());
        let mut variants = self
            .source
            .variants
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if variants.len() >= VARIANT_LIMIT {
            variants.clear();
        }
        variants.insert(key, built.style());
        drop(variants);
        built
    }

    fn style(&self) -> Style {
        Style {
            prepared: self.prepared.clone(),
            paints: self.paints.clone(),
            texts: self.texts.clone(),
            ids: self.ids.clone(),
            originals: self.originals.clone(),
            markers: self.markers.clone(),
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
            source,
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
        let css = source.css.as_str();
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
            vertical_aligns,
            captions_below,
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
        // Each slot's enclosing box owner: anonymous blocks answer to their block container.
        let mut container_of: Vec<usize> =
            prepared.positions.iter().map(|&(owner, _)| owner).collect();
        for (arena, value) in prepared.boxes.iter().enumerate() {
            for &child in &value.children {
                if prepared.anonymous[child] {
                    container_of[child] = prepared.positions[arena].0;
                }
            }
        }
        for (arena, value) in prepared.boxes.iter_mut().enumerate() {
            let owner = prepared.positions[arena].0;
            if let Some(TableRole::Table { spacing }) = &mut value.table {
                *spacing = spacings[owner];
            }
            value.cell_align = match vertical_aligns[owner] {
                Some("middle") => RowAlign::Center,
                Some("bottom") => RowAlign::Right,
                _ => RowAlign::Left,
            };
            value.caption_below = captions_below[owner];
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
                            let mut background = made.background;
                            while background.is_none() {
                                let Some(here) = owner else { break };
                                if prepared.slot[here].is_some_and(|slot| prepared.anonymous[slot])
                                {
                                    background = raw_paints[here].background;
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
                                background = raw_paints[here].background;
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
            markers: marker_leaves,
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

    /// Lays out by arena slot, with `measure(arena, width)` giving each leaf's height.
    fn layout_arena(
        &self,
        width: f32,
        height: Option<f32>,
        measure: &mut dyn FnMut(usize, f32) -> Option<f32>,
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
                PaintStep::Text(_) | PaintStep::Outline(_) | PaintStep::Marker(_) => None,
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
        // 18.4: outlines may be drawn above everything else.
        for (projected, paint) in self.paints.iter().enumerate() {
            let boxed = prepared.slot[projected].is_some_and(|slot| !prepared.anonymous[slot]);
            if paint.visible && paint.outline.is_some() && boxed {
                steps.push(PaintStep::Outline(projected));
            }
        }
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
                if self.markers.contains_key(&projected) {
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
    Renderer: text::Renderer<Font = iced::Font> + 'a,
{
    fn from(view: BoxTreeView<'a, Message, Theme, Renderer>) -> Self {
        let tree = view.tree;
        let (ids, elements): (Vec<_>, Vec<_>) = view.widgets.into_iter().unzip();
        Element::new(BoxTreeWidget {
            base: tree.clone(),
            tree,
            key: (None, None),
            measured: false,
            geometry: Vec::new(),
            slot_geometry: Vec::new(),
            scroll_limits: Vec::new(),
            clips: Vec::new(),
            paragraphs: BTreeMap::new(),
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
    /// Vertical scroll offsets by source node. They outlive restyles and view rebuilds.
    scrolled: BTreeMap<usize, f32>,
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
    let mut stack: Vec<(usize, bool, bool)> = prepared.boxes[root]
        .children
        .iter()
        .map(|child| (*child, holder, true))
        .collect();
    while let Some((arena, holder_inside, reaches)) = stack.pop() {
        let position = prepared.positions[arena].1;
        if position == Position::Fixed || (position == Position::Absolute && !holder_inside) {
            continue;
        }
        visit(arena, reaches);
        let inside = holder_inside || position != Position::Static;
        let further = reaches && !prepared.clipped[arena];
        stack.extend(
            prepared.boxes[arena]
                .children
                .iter()
                .map(|child| (*child, inside, further)),
        );
    }
}

/// How far each scrolling box can scroll, after moving its contents by its offset.
/// Parents come first in arena order, so nested scrollers measure moved contents.
fn apply_scrolling(
    prepared: &PreparedTree,
    geometry: &mut [FlowGeometry],
    scrolled: impl Fn(usize) -> f32,
    viewport_bottom: f32,
) -> Vec<f32> {
    let mut limits = vec![0.0_f32; prepared.boxes.len()];
    for arena in 0..geometry.len() {
        if !prepared.scrolls[arena] {
            continue;
        }
        let padding = geometry[arena].padding;
        // The root scrolls the viewport, however tall the root itself is.
        let bottom = if arena == 0 {
            viewport_bottom
        } else {
            padding.bottom
        };
        let mut reach = bottom;
        scrolling_descendants(prepared, arena, |child, reaches| {
            if reaches {
                reach = reach.max(geometry[child].border.bottom);
            }
        });
        // 11.1.1: the scrollable area includes the box's own end padding; at the root,
        // the box's whole bottom edge.
        let trailing = if arena == 0 {
            reach = reach.max(geometry[0].border.bottom);
            0.0
        } else {
            padding.bottom - geometry[arena].content.bottom
        };
        let limit = if reach > bottom {
            reach + trailing - bottom
        } else {
            0.0
        };
        limits[arena] = limit;
        let offset = scrolled(arena).clamp(0.0, limit);
        if offset == 0.0 {
            continue;
        }
        let mut moved = Vec::new();
        scrolling_descendants(prepared, arena, |child, _| moved.push(child));
        for child in moved {
            let target = &mut geometry[child];
            let band = target.leaf_band.as_mut();
            for rect in [&mut target.border, &mut target.padding, &mut target.content]
                .into_iter()
                .chain(band)
            {
                rect.top -= offset;
                rect.bottom -= offset;
            }
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
    /// How far each arena slot can scroll.
    scroll_limits: Vec<f32>,
    clips: Vec<Option<FlowRect>>,
    elements: Vec<Element<'a, Message, Theme, Renderer>>,
    /// The CSS id naming each element's box.
    ids: Vec<String>,
    /// Each element's arena slot in the current styling, while it has a box.
    slots: Vec<Option<usize>>,
    presses: Vec<(String, Message)>,
    /// Shaped text, its top-left corner relative to the layout, and its color, by arena.
    paragraphs: BTreeMap<usize, (Renderer::Paragraph, Point, Color)>,
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
        if !run.visible {
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
            if let Some(background) = run.background {
                quad(renderer, area, background);
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
    for band in border_bands(outer, inner) {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle {
                    x: band.x + origin.x,
                    y: band.y + origin.y,
                    ..band
                },
                ..Quad::default()
            },
            Color::from_rgb(color[0], color[1], color[2]),
        );
    }
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
    if !paint.visible {
        return;
    }
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
        for step in self.tree.paint_steps(&self.embedded()).into_iter().rev() {
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

    fn embedded(&self) -> BTreeSet<usize> {
        self.slots.iter().copied().flatten().collect()
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
        let interaction = tree.state.downcast_ref::<Interaction>().clone();
        // A rebuilt view keeps this state; `styled` ignores a node this tree lacks.
        let key = (interaction.hovered, interaction.active);
        if key != self.key {
            self.key = key;
            self.tree = self.base.variant(key);
            self.measured = false;
        }
        if !self.measured {
            self.measured = true;
            self.slots = self.ids.iter().map(|id| self.tree.leaf_slot(id)).collect();
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
            )
            .unwrap_or_default();
        let originals = &self.tree.originals;
        let positions = &self.tree.prepared.positions;
        self.scroll_limits = apply_scrolling(
            &self.tree.prepared,
            &mut self.slot_geometry,
            |arena| {
                let node = originals.get(positions[arena].0).copied();
                node.and_then(|node| interaction.scrolled.get(&node).copied())
                    .unwrap_or(0.0)
            },
            available.height,
        );
        self.geometry = self.tree.prepared.by_projected(&self.slot_geometry);
        self.clips = self.tree.clips(&self.geometry);
        self.paragraphs.clear();
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
        let Event::Mouse(mouse_event) = event else {
            return;
        };
        let bounds = layout.bounds();
        let hit = cursor.position_in(bounds).and_then(|point| self.hit(point));
        let state = tree.state.downcast_mut::<Interaction>();
        if let mouse::Event::WheelScrolled { delta } = mouse_event {
            if shell.is_event_captured() {
                return;
            }
            let amount = match delta {
                mouse::ScrollDelta::Lines { y, .. } => -y * WHEEL_LINE,
                mouse::ScrollDelta::Pixels { y, .. } => -y,
            };
            // The nearest box that can still move this way takes the wheel.
            let nodes = self.tree.source.construction.nodes();
            let mut at = hit;
            while let Some(node) = at {
                let arena = self
                    .tree
                    .originals
                    .iter()
                    .position(|value| *value == node)
                    .and_then(|projected| self.tree.prepared.slot[projected]);
                if let Some(limit) = arena.and_then(|arena| self.scroll_limits.get(arena)) {
                    let current = state
                        .scrolled
                        .get(&node)
                        .copied()
                        .unwrap_or(0.0)
                        .clamp(0.0, *limit);
                    let wanted = (current + amount).clamp(0.0, *limit);
                    if wanted != current {
                        state.scrolled.insert(node, wanted);
                        shell.capture_event();
                        shell.invalidate_layout();
                        shell.request_redraw();
                        return;
                    }
                }
                at = nodes[node].parent();
            }
            return;
        }
        let mut next = state.clone();
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
            let key = (next.hovered, next.active);
            *state = next;
            // Only states that a rule distinguishes need a new layout. Compare styles
            // before measuring: the current tree carries measured widths.
            if self.base.variant(key) != self.base.variant(self.key) {
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
        let top = cursor.position_in(bounds).and_then(|point| self.hit(point));
        let pressed = tree.state.downcast_ref::<Interaction>().active;
        let prepared = &self.tree.prepared;
        let clip_of = |step: &PaintStep| -> Rectangle {
            let arena = match *step {
                PaintStep::Box(index) | PaintStep::Outline(index) | PaintStep::Marker(index) => {
                    prepared.slot[index]
                }
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
        // 14.2: the root's background covers the whole canvas.
        if let Some(color) = prepared
            .positions
            .first()
            .and_then(|(root, _)| self.tree.paints[*root].background)
        {
            renderer.fill_quad(
                Quad {
                    bounds,
                    ..Quad::default()
                },
                Color::from_rgb(color[0], color[1], color[2]),
            );
        }
        // A layer draws its text over its quads, so anything painted after text needs
        // a layer of its own, as does any change of clip.
        let mut start = 0;
        while start < steps.len() {
            let clip = clip_of(&steps[start]);
            let mut end = start + 1;
            while end < steps.len()
                && clip_of(&steps[end]) == clip
                && !(matches!(steps[end - 1], PaintStep::Text(_) | PaintStep::Marker(_))
                    && !matches!(steps[end], PaintStep::Text(_) | PaintStep::Marker(_)))
            {
                end += 1;
            }
            renderer.with_layer(clip, |renderer| {
                for step in &steps[start..end] {
                    match *step {
                        PaintStep::Box(index) => {
                            draw_box(&self.tree, &self.geometry, renderer, origin, index);
                        }
                        PaintStep::Outline(index) => {
                            draw_outline(&self.tree, &self.geometry, renderer, origin, index);
                        }
                        PaintStep::Marker(index) => {
                            if let Some((shaped, corner)) = self.marker_paragraphs.get(&index) {
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
            let scrolled = &tree.state.downcast_ref::<Interaction>().scrolled;
            for (arena, limit) in self.scroll_limits.iter().enumerate() {
                let Some(geometry) = self.slot_geometry.get(arena).filter(|_| *limit > 0.0) else {
                    continue;
                };
                let projected = prepared.positions[arena].0;
                let area = geometry.padding;
                let end = if arena == 0 {
                    bounds.height
                } else {
                    area.bottom
                };
                let shown = (end - area.top).max(1.0);
                let thumb = (shown * shown / (shown + limit)).max(8.0);
                let offset = self
                    .tree
                    .originals
                    .get(projected)
                    .and_then(|node| scrolled.get(node))
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(0.0, *limit);
                let travel = (shown - thumb).max(0.0);
                let bar = Rectangle {
                    x: origin.x + area.right - 4.0,
                    y: origin.y + area.top + travel * offset / limit,
                    width: 4.0,
                    height: thumb,
                };
                // An ancestor's clip hides the bar along with the box.
                let visible = self
                    .clips
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
            draw_box(&tree, &geometry, &mut renderer, Point::new(5.0, 7.0), index);
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
}
