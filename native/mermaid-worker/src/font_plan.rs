//! Discover font requests from Merman's own semantic/config projection, not source heuristics.
use crate::protocol::{FontPlan, Request, VERSION};
use merman::{OperationControl, Renderer};
use serde_json::Value;
use std::{collections::BTreeSet, time::Duration};

pub fn plan(request: &Request) -> Result<FontPlan, String> {
    request.validate()?;
    let control = OperationControl::new().with_deadline(Duration::from_secs(3));
    let renderer = Renderer::new()
        .with_engine(
            merman::Engine::new()
                .with_site_config(merman::MermaidConfig::from_value(request.config())),
        )
        .with_resource_policy(
            merman::resources::InputResourcePolicy::default()
                .with_limit(
                    merman::resources::InputResourceLimitId::MaxSourceBytes,
                    crate::protocol::SOURCE_LIMIT,
                )
                .map_err(|e| e.to_string())?
                .with_limit(merman::resources::InputResourceLimitId::MaxModelItems, 8192)
                .map_err(|e| e.to_string())?,
        );
    let semantic = renderer
        .prepare_semantic(&request.source, control.clone())
        .map_err(|e| e.to_string())?
        .ok_or("No Mermaid diagram for font planning")?;
    let config = semantic.metadata().effective_config.as_value();
    let primary = config
        .get("fontFamily")
        .and_then(Value::as_str)
        .unwrap_or(&request.style.font_family)
        .to_owned();
    let mut families = BTreeSet::from([primary.clone(), request.style.font_family_mono.clone()]);
    fn collect(
        value: &Value,
        style: bool,
        families: &mut BTreeSet<String>,
        control: &OperationControl,
    ) -> Result<(), String> {
        control.checkpoint().map_err(|e| e.to_string())?;
        match value {
            Value::Object(fields) => {
                for (key, value) in fields {
                    if key.to_ascii_lowercase().ends_with("fontfamily") || key == "font-family" {
                        if let Some(value) = value.as_str() {
                            families.insert(value.to_owned());
                        }
                    } else {
                        collect(
                            value,
                            style
                                || matches!(
                                    key.as_str(),
                                    "style"
                                        | "styles"
                                        | "textStyles"
                                        | "cssStyle"
                                        | "cssStyles"
                                        | "cssCompiledStyles"
                                        | "labelStyle"
                                        | "classDefs"
                                ),
                            families,
                            control,
                        )?;
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    collect(value, style, families, control)?;
                }
            }
            Value::String(value) if style => {
                for declaration in simplecss::DeclarationTokenizer::from(value.as_str()) {
                    if declaration.name.eq_ignore_ascii_case("font-family") {
                        families.insert(declaration.value.to_owned());
                    }
                }
            }
            _ => {}
        }
        if families.len() > 16
            || families
                .iter()
                .any(|s| s.is_empty() || s.len() > 8192 || s.chars().any(char::is_control))
        {
            return Err("Mermaid font requests exceed sixteen bounded family stacks".into());
        }
        Ok(())
    }
    collect(config, false, &mut families, &control)?;
    let model = semantic.compatibility_json().map_err(|e| e.to_string())?;
    collect(model.as_value(), false, &mut families, &control)?;
    // The semantic model has already removed Mermaid comments. Do not demand
    // glyphs for arbitrary comment/config text that will never be painted.
    let mut characters = BTreeSet::new();
    fn labels(
        value: &Value,
        characters: &mut BTreeSet<u32>,
        families: &mut BTreeSet<String>,
        control: &OperationControl,
    ) -> Result<(), String> {
        control.checkpoint().map_err(|e| e.to_string())?;
        match value {
            Value::Object(fields) => {
                for (key, value) in fields {
                    if !matches!(
                        key.as_str(),
                        "style"
                            | "styles"
                            | "textStyles"
                            | "cssStyle"
                            | "cssStyles"
                            | "cssCompiledStyles"
                            | "labelStyle"
                            | "classDefs"
                            | "config"
                            | "source"
                            | "rawSource"
                            | "link"
                            | "href"
                            | "url"
                    ) && !key.to_ascii_lowercase().ends_with("fontfamily")
                    {
                        labels(value, characters, families, control)?;
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    labels(value, characters, families, control)?;
                }
            }
            Value::String(value) => {
                if value.contains('<') {
                    let (names, points) = html_content(value);
                    families.extend(names);
                    characters.extend(points);
                } else {
                    characters.extend(value.chars().filter(|c| !c.is_control()).map(u32::from));
                }
            }
            _ => {}
        }
        if characters.len() > 4096 || families.len() > 16 {
            return Err("Mermaid font plan exceeds its character/family budget".into());
        }
        Ok(())
    }
    labels(model.as_value(), &mut characters, &mut families, &control)?;
    if let Some(title) = &semantic.metadata().title {
        characters.extend(title.chars().map(u32::from));
    }
    if characters.len() > 4096 {
        return Err("Mermaid font coverage exceeds 4096 distinct characters".into());
    }
    control.checkpoint().map_err(|e| e.to_string())?;
    Ok(FontPlan {
        version: VERSION,
        primary,
        mono: request.style.font_family_mono.clone(),
        families: families.into_iter().collect(),
        characters: characters.into_iter().collect(),
    })
}

// Tokenize existing HTML label fragments without constructing a DOM or executing content.
fn html_content(text: &str) -> (BTreeSet<String>, BTreeSet<u32>) {
    use html5ever::tokenizer::{BufferQueue, Token, TokenSink, TokenSinkResult, Tokenizer};
    use std::cell::RefCell;
    struct Sink(RefCell<(BTreeSet<String>, BTreeSet<u32>)>);
    impl TokenSink for Sink {
        type Handle = ();
        fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
            if let Token::CharacterTokens(ref text) = token {
                self.0
                    .borrow_mut()
                    .1
                    .extend(text.chars().filter(|c| !c.is_control()).map(u32::from));
            }
            if let Token::TagToken(tag) = token {
                for attr in tag.attrs {
                    let name: &str = attr.name.local.as_ref();
                    if name == "style" {
                        for d in simplecss::DeclarationTokenizer::from(attr.value.as_ref()) {
                            if d.name.eq_ignore_ascii_case("font-family") {
                                self.0.borrow_mut().0.insert(d.value.to_owned());
                            }
                        }
                    }
                }
            }
            TokenSinkResult::Continue
        }
    }
    let input = BufferQueue::default();
    input.push_back(text.into());
    let tokenizer = Tokenizer::new(
        Sink(RefCell::new((BTreeSet::new(), BTreeSet::new()))),
        Default::default(),
    );
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.0.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(source: &str) -> Request {
        Request {
            version: VERSION,
            source: source.into(),
            dark: false,
            style: Default::default(),
            font_defaults: None,
        }
    }
    #[test]
    fn comments_and_html_attributes_do_not_request_unpainted_glyphs() {
        let p = plan(&request("flowchart LR\n%% 𓀀\nA[Hello] --> B[世界]")).unwrap();
        assert!(!p.characters.contains(&0x13000));
        assert!(p.characters.contains(&u32::from('世')));
        let (names, chars) =
            html_content("<span style=\"font-family: 'DejaVu Serif'\">Hello<!--𓀀--></span>");
        assert!(names.contains("'DejaVu Serif'"));
        assert!(!chars.contains(&0x13000));
    }
    #[test]
    fn source_configuration_and_class_fonts_are_parser_owned() {
        let p=plan(&request("%%{init: {\"fontFamily\":\"DejaVu Serif\"}}%%\nflowchart LR\nA[Hello]:::code --> B[World]\nclassDef code font-family:monospace")).unwrap();
        assert_eq!(p.primary, "DejaVu Serif");
        assert!(p.families.contains(&"monospace".into()));
    }
}
