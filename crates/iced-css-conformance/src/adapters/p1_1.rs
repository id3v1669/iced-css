use iced_css_core::source::Metadata;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    scenario: String,
    ancestor_lang: Option<String>,
    child_lang: Option<String>,
    document_lang: Option<String>,
}

/// Controlled two-node projection of the section 47 source-language contract.
pub fn run(input: &Value) -> crate::CaseResult {
    let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    if input.scenario != "language_inheritance" {
        return Err("unknown source-language scenario".into());
    }
    // This seed qualifies explicit empty source attributes, not the document
    // metadata ingestion boundary. Do not silently choose that boundary's rule.
    if input.document_lang.as_deref() == Some("") {
        return Err("empty document language is outside this adapter's qualified input".into());
    }
    let mut ancestor = Metadata::default();
    let mut child = Metadata::default();
    if let Some(lang) = &input.ancestor_lang {
        ancestor.set_attribute("lang", lang);
    }
    if let Some(lang) = &input.child_lang {
        child.set_attribute("lang", lang);
    }
    let inherited = ancestor.language(input.document_lang.as_deref());
    let language = child.language(inherited);
    let from_document = language.is_some()
        && ancestor.attribute("lang").is_none()
        && child.attribute("lang").is_none();
    Ok(json!({"child_language":language, "inherits_document_language":from_document}))
}
