//! Controlled two-resource policy fixture; acquisition outcomes are supplied.
use super::{context, n, parse, publication_limits};
use iced_css::host::stylesheet::graph::RuntimeStylesheetGraph;
use iced_css_core::{
    stylesheet::{
        SheetEntryKind,
        imports::{ImportBinding, ImportResolution, LoadedGraphLimits, UnavailableImportPolicy},
        owned_graph::{OwnedGraphLimits, OwnedLoadedSheet},
        publication::SheetCompletion,
    },
    syntax::imports::{ImportLimits, ResourceId},
};
use serde_json::json;
use std::num::NonZeroU64;

fn limits() -> OwnedGraphLimits {
    OwnedGraphLimits {
        record_bytes: n(),
        work: n(),
        expansion: LoadedGraphLimits {
            unavailable: UnavailableImportPolicy::Record,
            entries: n(),
            record_bytes: n(),
            work: n(),
            tracking: ImportLimits {
                entries: n(),
                retained_bytes: n(),
                work: n(),
                occurrences: NonZeroU64::new(100).unwrap(),
            },
        },
    }
}
fn resources(root: &str, child: Option<&str>) -> Result<Vec<OwnedLoadedSheet>, String> {
    let root = parse(root)?;
    let imports: Vec<_> = root
        .sheet()
        .entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            matches!(&entry.kind, SheetEntryKind::Import(import) if import.applies_to_screen)
                .then_some(index)
        })
        .collect();
    if imports != [0] {
        return Err("fixture requires one applicable root import at entry zero".into());
    }
    let child = child.map(parse).transpose()?;
    if child.as_ref().is_some_and(|child| child.sheet().entries.iter().any(|entry| matches!(&entry.kind, SheetEntryKind::Import(import) if import.applies_to_screen))) {
        return Err("fixture child must have no applicable imports".into());
    }
    let resolution = if child.is_some() {
        ImportResolution::Sheet(1)
    } else {
        ImportResolution::Unavailable
    };
    let mut resources = vec![OwnedLoadedSheet {
        resource: ResourceId::new(NonZeroU64::MIN),
        candidate: root,
        imports: vec![ImportBinding {
            entry: 0,
            resolution,
        }],
    }];
    if let Some(child) = child {
        resources.push(OwnedLoadedSheet {
            resource: ResourceId::new(NonZeroU64::new(2).unwrap()),
            candidate: child,
            imports: vec![],
        });
    }
    Ok(resources)
}
pub(super) fn run(
    root_before: &str,
    import_before: &str,
    root_after: &str,
    import_after: Option<&str>,
) -> crate::CaseResult {
    let bytes = [
        root_before,
        import_before,
        root_after,
        import_after.unwrap_or(""),
    ]
    .iter()
    .try_fold(0usize, |sum, s| sum.checked_add(s.len()))
    .ok_or("fixture byte overflow")?;
    if bytes > 4096 {
        return Err("fixture exceeds 4096 CSS bytes".into());
    }
    let initial = resources(root_before, Some(import_before))?;
    let replacement = resources(root_after, import_after)?;
    let mut owner = RuntimeStylesheetGraph::new(publication_limits(), n());
    let first = owner
        .request(0, context(), limits())
        .map_err(|e| e.to_string())?;
    if !matches!(
        owner
            .finish(first.assemble(Ok::<_, String>(initial)))
            .map_err(|e| e.to_string())?,
        SheetCompletion::Published(_)
    ) {
        return Err("initial fixture graph did not publish".into());
    }
    let next = owner
        .request(0, context(), limits())
        .map_err(|e| e.to_string())?;
    let completion = match owner
        .finish(next.assemble(Ok::<_, String>(replacement)))
        .map_err(|e| e.to_string())?
    {
        SheetCompletion::Published(_) => "published",
        SheetCompletion::Failed(_) => "failed",
        SheetCompletion::Ignored => "ignored",
    };
    let active = owner.active().ok_or("fixture lost its initial graph")?;
    let root = active
        .resources()
        .first()
        .ok_or("fixture graph has no root")?;
    Ok(
        json!({"active_root":root.candidate.source().text(),"active_import":active.resources().get(1).map(|r|r.candidate.source().text()),"active_revision":owner.active_revision().map_or(0,|v|v.get()),"completion":completion}),
    )
}
