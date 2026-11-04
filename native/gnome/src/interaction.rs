//! Worker-normalized user questions and the MCP flat primitive form schema.
//! No GTK objects, network calls, template expansion or answer persistence.
use regex::{Regex, RegexBuilder};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

#[derive(Debug)]
pub enum Request {
    Questions(Vec<Question>),
    OpenCodeQuestions(Vec<Question>),
    Form {
        server: String,
        message: String,
        fields: Vec<Field>,
    },
    Url {
        server: String,
        message: String,
        url: String,
        host: String,
    },
}
#[derive(Debug)]
pub struct Choice {
    pub value: String,
    pub label: String,
    pub description: String,
}
#[derive(Debug)]
pub struct Question {
    pub id: String,
    pub header: String,
    pub text: String,
    pub choices: Vec<Choice>,
    pub other: bool,
    pub secret: bool,
    pub multiple: bool,
}
#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub title: String,
    pub description: String,
    pub required: bool,
    pub kind: Kind,
    pub default: Input,
}
#[derive(Debug)]
pub enum Kind {
    Text {
        minimum: usize,
        maximum: usize,
        format: String,
        pattern: Option<Regex>,
    },
    Number {
        integer: bool,
        minimum: Option<f64>,
        maximum: Option<f64>,
    },
    Boolean,
    Select {
        choices: Vec<Choice>,
        multiple: bool,
        minimum: usize,
        maximum: usize,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    Text(String),
    Choices(Vec<usize>),
    Boolean(bool),
    Selected { choices: Vec<usize>, other: String },
}
#[derive(Debug)]
pub struct Invalid {
    pub field: usize,
    pub message: String,
}

fn text(value: &Value, limit: usize) -> Result<String, String> {
    match value {
        Value::Null => Ok(String::new()),
        Value::String(s) if s.len() <= limit && !s.contains('\0') => Ok(s.clone()),
        _ => Err("Input request contains invalid or oversized text".into()),
    }
}
fn flag(value: &Value) -> Result<bool, String> {
    match value {
        Value::Bool(b) => Ok(*b),
        Value::Null => Ok(false),
        _ => Err("Invalid input flag".into()),
    }
}
fn count(value: &Value, default: usize) -> Result<usize, String> {
    if value.is_null() {
        return Ok(default);
    }
    value
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or("Invalid form count constraint".into())
}
fn limit(value: &Value) -> Result<Option<f64>, String> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_f64()
        .filter(|n| n.is_finite())
        .map(Some)
        .ok_or("Invalid numeric constraint".into())
}
fn keys(value: &Value, allowed: &[&str]) -> Result<(), String> {
    let object = value.as_object().ok_or("Invalid form schema")?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(
            "This form uses schema features that the native input dialog cannot validate".into(),
        );
    }
    Ok(())
}
fn choices(schema: &Value, array: bool) -> Result<Vec<Choice>, String> {
    let rows = schema
        .get("enum")
        .or_else(|| schema.get(if array { "anyOf" } else { "oneOf" }))
        .and_then(Value::as_array)
        .ok_or("Form selection requires string choices")?;
    if rows.is_empty() || rows.len() > 64 {
        return Err("Form selections support 1–64 choices".into());
    }
    let labels = schema
        .get("enumNames")
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_array()
                .filter(|a| a.len() == rows.len())
                .ok_or("Invalid choice labels")
        })
        .transpose()?;
    let mut seen = HashSet::new();
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let raw = if row.is_object() {
                keys(row, &["const", "title"])?;
                &row["const"]
            } else {
                row
            };
            if !raw.is_string() {
                return Err("Form choices require string values".into());
            }
            let value = text(raw, 4096)?;
            if !seen.insert(value.clone()) {
                return Err("Invalid or repeated form choice".into());
            }
            let label = text(
                labels.map(|a| &a[index]).unwrap_or(if row.is_object() {
                    &row["title"]
                } else {
                    row
                }),
                4096,
            )?;
            Ok(Choice {
                value,
                label: if label.is_empty() {
                    "(empty value)".into()
                } else {
                    label
                },
                description: String::new(),
            })
        })
        .collect()
}
fn field(name: &str, schema: &Value, required: bool) -> Result<Field, String> {
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err("Invalid form field name".into());
    }
    keys(
        schema,
        &[
            "type",
            "title",
            "description",
            "default",
            "format",
            "minLength",
            "maxLength",
            "pattern",
            "minimum",
            "maximum",
            "enum",
            "enumNames",
            "oneOf",
            "items",
            "minItems",
            "maxItems",
        ],
    )?;
    let kind = match schema["type"].as_str() {
        Some("string") if schema.get("enum").is_some() || schema.get("oneOf").is_some() => {
            Kind::Select {
                choices: choices(schema, false)?,
                multiple: false,
                minimum: usize::from(required),
                maximum: 1,
            }
        }
        Some("string") => {
            let minimum = count(&schema["minLength"], 0)?;
            let maximum = count(&schema["maxLength"], 16384)?.min(16384);
            if minimum > maximum {
                return Err("Form string constraints exceed the input budget".into());
            }
            let format = text(&schema["format"], 32)?;
            if !matches!(format.as_str(), "" | "email" | "uri" | "date" | "date-time") {
                return Err("Unsupported form text format".into());
            }
            let pattern = if schema["pattern"].is_null() {
                None
            } else {
                let pattern = text(&schema["pattern"], 4096)?;
                Some(
                    RegexBuilder::new(&pattern)
                        .size_limit(256 * 1024)
                        .dfa_size_limit(256 * 1024)
                        .build()
                        .map_err(|_| "Unsupported or oversized form pattern")?,
                )
            };
            Kind::Text {
                minimum,
                maximum,
                format,
                pattern,
            }
        }
        Some("integer" | "number") => {
            let minimum = limit(&schema["minimum"])?;
            let maximum = limit(&schema["maximum"])?;
            if minimum.zip(maximum).is_some_and(|(a, b)| a > b) {
                return Err("Invalid numeric range".into());
            }
            Kind::Number {
                integer: schema["type"] == "integer",
                minimum,
                maximum,
            }
        }
        Some("boolean") => Kind::Boolean,
        Some("array") => {
            keys(&schema["items"], &["type", "enum", "anyOf", "enumNames"])?;
            let options = choices(&schema["items"], true)?;
            let minimum = count(&schema["minItems"], 0)?;
            let maximum = count(&schema["maxItems"], options.len())?.min(options.len());
            if minimum > maximum {
                return Err("Invalid form selection count".into());
            }
            Kind::Select {
                choices: options,
                multiple: true,
                minimum,
                maximum,
            }
        }
        _ => return Err("Only flat primitive MCP form fields are supported".into()),
    };
    let default = match &kind {
        Kind::Boolean => Input::Boolean(if schema["default"].is_null() {
            false
        } else {
            schema["default"]
                .as_bool()
                .ok_or("Invalid boolean default")?
        }),
        Kind::Select {
            choices, multiple, ..
        } => {
            let values = match &schema["default"] {
                Value::Null => Vec::new(),
                Value::Array(values) if *multiple => values
                    .iter()
                    .map(|v| v.as_str().ok_or("Invalid selection default"))
                    .collect::<Result<Vec<_>, _>>()?,
                Value::String(value) if !multiple => vec![value.as_str()],
                _ => return Err("Invalid selection default".into()),
            };
            let indices = values
                .into_iter()
                .map(|value| {
                    choices
                        .iter()
                        .position(|c| c.value == value)
                        .ok_or("Unknown default choice".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Input::Choices(indices)
        }
        Kind::Number { .. } => Input::Text(if schema["default"].is_null() {
            String::new()
        } else if schema["default"].is_number() {
            schema["default"].to_string()
        } else {
            return Err("Invalid numeric default".into());
        }),
        Kind::Text { .. } => Input::Text(text(&schema["default"], 16384)?),
    };
    let title = text(&schema["title"], 4096)?;
    Ok(Field {
        name: name.into(),
        title: if title.is_empty() { name.into() } else { title },
        description: text(&schema["description"], 4096)?,
        required,
        kind,
        default,
    })
}

pub fn parse(method: &str, params: &Value) -> Result<Request, String> {
    let opencode = method == "opencode/question";
    let result = if method == "item/tool/requestUserInput" || opencode {
        let rows = params["questions"]
            .as_array()
            .filter(|q| !q.is_empty() && q.len() <= 16)
            .ok_or("Input requests support 1–16 questions")?;
        let mut ids = HashSet::new();
        let mut total = 0;
        let questions = rows
            .iter()
            .enumerate()
            .map(|(index, q)| {
                let id = if opencode {
                    index.to_string()
                } else {
                    text(&q["id"], 256)?
                };
                if id.is_empty() || !ids.insert(id.clone()) {
                    return Err("Invalid or repeated question identity".into());
                }
                let rows = if q["options"].is_null() {
                    &[][..]
                } else {
                    q["options"]
                        .as_array()
                        .filter(|a| a.len() <= 64)
                        .ok_or("Question supports at most 64 options")?
                };
                total += rows.len();
                if total > 256 {
                    return Err("Question options exceed the input budget".into());
                }
                let mut labels = HashSet::new();
                let choices = rows
                    .iter()
                    .map(|option| {
                        let label = text(&option["label"], 4096)?;
                        if label.is_empty() || !labels.insert(label.clone()) {
                            return Err("Invalid or repeated question option".into());
                        }
                        Ok(Choice {
                            value: label.clone(),
                            label,
                            description: text(&option["description"], 4096)?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(Question {
                    id,
                    header: text(&q["header"], 256)?,
                    text: text(&q["question"], 8192)?,
                    choices,
                    other: if opencode {
                        if q["custom"].is_null() {
                            true
                        } else {
                            flag(&q["custom"])?
                        }
                    } else {
                        flag(&q["isOther"])?
                    },
                    secret: !opencode && flag(&q["isSecret"])?,
                    multiple: opencode && flag(&q["multiple"])?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if opencode {
            Request::OpenCodeQuestions(questions)
        } else {
            Request::Questions(questions)
        }
    } else if method == "mcpServer/elicitation/request" {
        let server = text(&params["serverName"], 256)?;
        let message = text(&params["message"], 8192)?;
        let mode = if params["mode"].is_null() {
            "form"
        } else {
            params["mode"].as_str().ok_or("Invalid elicitation mode")?
        };
        match mode {
            "url"=>{
                let raw=text(&params["url"],4096)?;
                let url=url::Url::parse(&raw).map_err(|_|"Invalid elicitation URL")?;
                if !matches!(url.scheme(),"https"|"http") || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {return Err("Elicitation links require HTTP(S), a host and no embedded credentials".into())}
                Request::Url{server,message,host:url.host_str().unwrap().into(),url:url.into()}
            },
            "form"=>{
                let schema=&params["requestedSchema"];
                keys(schema,&["$schema","type","properties","required","additionalProperties"])?;
                if schema["type"]!="object" {return Err("MCP form requires an object schema".into())}
                let properties=schema["properties"].as_object().filter(|p|p.len()<=32).ok_or("MCP form supports at most 32 fields")?;
                let required=if schema["required"].is_null(){Vec::new()}else{schema["required"].as_array().filter(|a|a.len()<=32).ok_or("Invalid required fields")?.iter().map(|v|v.as_str().filter(|key|properties.contains_key(*key)).ok_or("Unknown required field")).collect::<Result<Vec<_>,_>>()?};
                let fields=properties.iter().map(|(name,schema)|field(name,schema,required.contains(&name.as_str()))).collect::<Result<Vec<_>,_>>()?;
                let count=fields.iter().map(|f|if let Kind::Select{choices,..}=&f.kind{choices.len()}else{0}).sum::<usize>();
                if count>256 {return Err("MCP form options exceed the input budget".into())}
                Request::Form{server,message,fields}
            },
            _=>return Err("This elicitation mode requires a client capability that the native dialog does not provide".into()),
        }
    } else {
        return Err("Unsupported native input request".into());
    };
    // All strings above are individually bounded; this also bounds aggregate UI work.
    if format!("{result:?}").len() > 256 * 1024 {
        return Err("Input form exceeds its presentation budget".into());
    }
    Ok(result)
}

fn selection(choices: &[Choice], values: &[usize]) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    values
        .iter()
        .map(|index| {
            if !seen.insert(*index) {
                return Err("Choose each option once".into());
            }
            choices
                .get(*index)
                .map(|c| c.value.clone())
                .ok_or("Choose an available option".into())
        })
        .collect()
}
fn email(value: &str) -> bool {
    let Some((local, domain)) = value.rsplit_once('@') else {
        return false;
    };
    !local.is_empty()
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        && !domain.is_empty()
        && !domain
            .chars()
            .any(|c| c.is_whitespace() || "/@:\\?#".contains(c))
        && url::Url::parse(&format!("http://{domain}")).is_ok_and(|u| u.host_str().is_some())
}
fn value(field: &Field, input: &Input) -> Result<Option<Value>, String> {
    match (&field.kind, input) {
        (
            Kind::Text {
                minimum,
                maximum,
                format,
                pattern,
            },
            Input::Text(text),
        ) => {
            if text.is_empty() && !field.required {
                return Ok(None);
            }
            let len = text.chars().count();
            if (field.required && text.is_empty()) || len < *minimum || len > *maximum {
                return Err(format!(
                    "Enter {}–{} characters",
                    if field.required {
                        (*minimum).max(1)
                    } else {
                        *minimum
                    },
                    maximum
                ));
            }
            let valid = match format.as_str() {
                "email" => email(text),
                "uri" => url::Url::parse(text).is_ok(),
                "date" => {
                    text.len() == 10
                        && chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
                            .is_ok_and(|date| date.format("%Y-%m-%d").to_string() == *text)
                }
                "date-time" => chrono::DateTime::parse_from_rfc3339(text).is_ok(),
                _ => true,
            };
            if !valid {
                return Err(format!("Enter a valid {format}"));
            }
            if pattern.as_ref().is_some_and(|p| !p.is_match(text)) {
                return Err("The value does not match the requested pattern".into());
            }
            Ok(Some(json!(text)))
        }
        (
            Kind::Number {
                integer,
                minimum,
                maximum,
            },
            Input::Text(text),
        ) => {
            if text.is_empty() && !field.required {
                return Ok(None);
            }
            let mut value = serde_json::from_str::<serde_json::Number>(text.trim())
                .map_err(|_| "Enter a valid number")?;
            let n = value
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or("Number is outside the supported range")?;
            if *integer && value.as_i64().is_none() && value.as_u64().is_none() {
                if n.fract() != 0.0 {
                    return Err("Enter a whole number".into());
                }
                // MCP numeric defaults are doubles even for integer schemas.
                // Normalize exact small integral doubles (3.0, 1e1), but never
                // silently round a large integer that was entered as a float.
                if n.abs() > 9_007_199_254_740_991.0 {
                    return Err(
                        "Enter large whole numbers as full decimal digits within the 64-bit range"
                            .into(),
                    );
                }
                value = serde_json::Number::from(n as i64);
            }
            if minimum.is_some_and(|v| n < v) || maximum.is_some_and(|v| n > v) {
                return Err("Number is outside the requested range".into());
            }
            Ok(Some(Value::Number(value)))
        }
        (Kind::Boolean, Input::Boolean(value)) => Ok(Some(json!(value))),
        (
            Kind::Select {
                choices,
                multiple,
                minimum,
                maximum,
            },
            Input::Choices(indices),
        ) => {
            if indices.is_empty() && !field.required {
                return Ok(None);
            }
            if indices.len() < *minimum
                || indices.len() > *maximum
                || (field.required && indices.is_empty())
            {
                return Err(format!(
                    "Choose {}–{} options",
                    (*minimum).max(usize::from(field.required)),
                    maximum
                ));
            }
            let values = selection(choices, indices)?;
            Ok(Some(if *multiple {
                json!(values)
            } else {
                json!(values[0])
            }))
        }
        _ => Err("Input type does not match the requested field".into()),
    }
}
impl Request {
    pub fn answer(&self, inputs: &[Input]) -> Result<Value, Invalid> {
        let work = || -> Result<Value, (usize, String)> {
            let expected = match self {
                Self::Questions(q) | Self::OpenCodeQuestions(q) => q.len(),
                Self::Form { fields, .. } => fields.len(),
                Self::Url { .. } => 0,
            };
            if inputs.len() != expected {
                return Err((0, "The input form changed; reopen the request".into()));
            }
            if inputs
                .iter()
                .map(|i| match i {
                    Input::Text(s) => s.len(),
                    Input::Selected { other, .. } => other.len(),
                    _ => 0,
                })
                .sum::<usize>()
                > 64 * 1024
            {
                return Err((0, "Answers exceed 64 KiB".into()));
            }
            let mut result = Map::new();
            match self {
                Self::Questions(questions) => {
                    for (i, (q, input)) in questions.iter().zip(inputs).enumerate() {
                        let answer = match input {
                            Input::Text(s)
                                if (q.choices.is_empty() || q.other) && !s.trim().is_empty() =>
                            {
                                s.clone()
                            }
                            Input::Choices(indices) if indices.len() == 1 => {
                                selection(&q.choices, indices).map_err(|e| (i, e))?[0].clone()
                            }
                            _ => return Err((i, "Answer this question before submitting".into())),
                        };
                        result.insert(q.id.clone(), json!({"answers":[answer]}));
                    }
                    Ok(json!({"answers":result}))
                }
                Self::Form { fields, .. } => {
                    for (i, (field, input)) in fields.iter().zip(inputs).enumerate() {
                        if let Some(value) = value(field, input).map_err(|e| (i, e))? {
                            result.insert(field.name.clone(), value);
                        }
                    }
                    Ok(json!({"action":"accept","content":result}))
                }
                Self::OpenCodeQuestions(questions) => {
                    let mut answers = Vec::new();
                    for (i, (q, input)) in questions.iter().zip(inputs).enumerate() {
                        let answer = match input {
                            Input::Text(s) if q.other && !s.trim().is_empty() => vec![s.clone()],
                            Input::Choices(indices) if q.multiple || indices.len() == 1 => {
                                selection(&q.choices, indices).map_err(|e| (i, e))?
                            }
                            Input::Selected { choices, other } if q.multiple => {
                                let mut values =
                                    selection(&q.choices, choices).map_err(|e| (i, e))?;
                                if !other.trim().is_empty() {
                                    if !q.other {
                                        return Err((
                                            i,
                                            "This question does not allow custom answers".into(),
                                        ));
                                    }
                                    if !values.contains(other) {
                                        values.push(other.clone());
                                    }
                                }
                                values
                            }
                            _ => return Err((i, "Answer this question before submitting".into())),
                        };
                        answers.push(answer);
                    }
                    Ok(json!({"answers":answers}))
                }
                Self::Url { .. } => Ok(json!({"action":"accept"})),
            }
        };
        let value = work().map_err(|(field, message)| Invalid { field, message })?;
        if serde_json::to_vec(&value).map_or(true, |v| v.len() > 64 * 1024) {
            return Err(Invalid {
                field: 0,
                message: "Answers exceed 64 KiB".into(),
            });
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opencode_questions_keep_order_multiple_choices_and_exact_custom_answers() {
        let request=parse("opencode/question",&json!({"questions":[
            {"header":"Many","question":"Choose several","multiple":true,"options":[{"label":"A","description":"one"},{"label":"B","description":"two"}]},
            {"question":"Only choices","custom":false,"options":[{"label":"C","description":"three"}]}]})).unwrap();
        assert_eq!(
            request
                .answer(&[
                    Input::Selected {
                        choices: vec![0, 1],
                        other: "  中文😀 ".into()
                    },
                    Input::Choices(vec![0])
                ])
                .unwrap(),
            json!({"answers":[["A","B","  中文😀 "],["C"]]})
        );
        assert_eq!(
            request
                .answer(&[Input::Choices(vec![]), Input::Choices(vec![0])])
                .unwrap(),
            json!({"answers":[[],["C"]]})
        );
        assert!(request
            .answer(&[Input::Choices(vec![0]), Input::Text("not allowed".into())])
            .is_err());
        assert!(request
            .answer(&[
                Input::Selected {
                    choices: vec![0, 0],
                    other: String::new()
                },
                Input::Choices(vec![0])
            ])
            .is_err());
    }
    #[test]
    fn selected_labels_also_count_toward_the_answer_byte_budget() {
        let choices = (0..20)
            .map(|i| json!({"label":format!("{i}{}","x".repeat(3990)),"description":""}))
            .collect::<Vec<_>>();
        let request=parse("opencode/question",&json!({"questions":[{"question":"Many large labels","multiple":true,"options":choices}]})).unwrap();
        assert!(request
            .answer(&[Input::Choices((0..20).collect())])
            .unwrap_err()
            .message
            .contains("64 KiB"));
    }
    fn form(properties: Value, required: Value) -> Request {
        parse("mcpServer/elicitation/request",&json!({"serverName":"test-server","mode":"form","message":"Review before submitting","requestedSchema":{"type":"object","properties":properties,"required":required}})).unwrap()
    }
    #[test]
    fn questions_preserve_ids_choices_exact_custom_text_and_secret_flags() {
        let request=parse("item/tool/requestUserInput",&json!({"questions":[
            {"id":"pick","header":"Choice","question":"Which?","options":[{"label":"Native GTK4","description":"Description"},{"label":"Other stack","description":""}]},
            {"id":"custom","question":"Why?","isOther":true,"options":[{"label":"Default","description":""}]},
            {"id":"secret","question":"Secret?","isSecret":true}]})).unwrap();
        let result = request
            .answer(&[
                Input::Choices(vec![1]),
                Input::Text("  中文😀\n".into()),
                Input::Text("private memory only".into()),
            ])
            .unwrap();
        assert_eq!(result["answers"]["pick"]["answers"], json!(["Other stack"]));
        assert_eq!(
            result["answers"]["custom"]["answers"],
            json!(["  中文😀\n"])
        );
        assert!(matches!(&request,Request::Questions(q) if q[2].secret));
        assert_eq!(
            request
                .answer(&[
                    Input::Text("injected".into()),
                    Input::Choices(vec![0]),
                    Input::Text("answer".into())
                ])
                .unwrap_err()
                .field,
            0
        );
        assert_eq!(
            request
                .answer(&[
                    Input::Choices(vec![0]),
                    Input::Text("  ".into()),
                    Input::Text("answer".into())
                ])
                .unwrap_err()
                .field,
            1
        );
        assert!(parse(
            "item/tool/requestUserInput",
            &json!({"questions":[{"id":"same"},{"id":"same"}]})
        )
        .is_err());
    }
    #[test]
    fn forms_return_typed_values_and_omit_missing_optional_fields() {
        let request = form(
            json!({
            "enabled":{"type":"boolean","default":true},
            "level":{"type":"integer","minimum":1,"maximum":10,"default":3.0},
            "optional":{"type":"string"},
            "ratio":{"type":"number","default":0.5},
            "scope":{"type":"string","oneOf":[{"const":"read-only","title":"Read only"},{"const":"write","title":"Write"}],"default":"read-only"},
            "tags":{"type":"array","minItems":1,"maxItems":2,"items":{"anyOf":[{"const":"a","title":"Alpha"},{"const":"b","title":"Beta"}]},"default":["b"]}}),
            json!(["enabled", "level", "scope", "tags"]),
        );
        let Request::Form { fields, .. } = &request else {
            panic!()
        };
        let values = fields.iter().map(|f| f.default.clone()).collect::<Vec<_>>();
        let result = request.answer(&values).unwrap();
        assert_eq!(
            result,
            json!({"action":"accept","content":{"enabled":true,"level":3,"ratio":0.5,"scope":"read-only","tags":["b"]}})
        );
        let mut invalid = values.clone();
        invalid[1] = Input::Text("11".into());
        assert_eq!(request.answer(&invalid).unwrap_err().field, 1);
        invalid[1] = Input::Text("1.5".into());
        assert!(request.answer(&invalid).is_err());
        invalid = values;
        invalid[5] = Input::Choices(vec![0, 0]);
        assert!(request.answer(&invalid).is_err());
        let request = form(json!({"exact":{"type":"integer"}}), json!(["exact"]));
        assert_eq!(
            request
                .answer(&[Input::Text("18446744073709551615".into())])
                .unwrap()["content"]["exact"]
                .as_u64(),
            Some(u64::MAX)
        );
        assert_eq!(
            request.answer(&[Input::Text("1e1".into())]).unwrap()["content"]["exact"],
            json!(10)
        );
        assert!(request
            .answer(&[Input::Text("9007199254740993.0".into())])
            .is_err());
    }
    #[test]
    fn unicode_lengths_patterns_dates_and_formats_are_validated() {
        let request = form(
            json!({"text":{"type":"string","minLength":2,"maxLength":3,"pattern":"^中.*$"}}),
            json!(["text"]),
        );
        assert_eq!(
            request.answer(&[Input::Text("中文😀".into())]).unwrap()["content"]["text"],
            "中文😀"
        );
        for invalid in ["中", "中文😀x", "abc"] {
            assert!(request.answer(&[Input::Text(invalid.into())]).is_err());
        }
        for (format, valid, invalid) in [
            ("date", "2024-02-29", "2025-02-29"),
            ("date-time", "2026-10-25T15:30:00+08:00", "2026-10-25 15:30"),
            ("email", "a+b@example.com", "name@bad domain"),
            ("uri", "urn:example:test", "relative/path"),
        ] {
            let request = form(
                json!({"value":{"type":"string","format":format}}),
                json!(["value"]),
            );
            assert!(
                request.answer(&[Input::Text(valid.into())]).is_ok(),
                "{format}"
            );
            assert!(
                request.answer(&[Input::Text(invalid.into())]).is_err(),
                "{format}"
            );
        }
    }
    #[test]
    fn schema_errors_and_budget_failures_cannot_be_accepted() {
        for properties in [
            json!({"nested":{"type":"object"}}),
            json!({"x":{"type":"string","maxLength":1,"minLength":2}}),
            json!({"x":{"type":"string","pattern":"(?=unsupported)"}}),
            json!({"x":{"type":"array","items":{"type":"object"}}}),
            json!({"x":{"type":"string","enum":["a"],"default":"b"}}),
        ] {
            assert!(parse(
                "mcpServer/elicitation/request",
                &json!({"mode":"form","requestedSchema":{"type":"object","properties":properties}})
            )
            .is_err());
        }
        let properties = (0..33)
            .map(|i| (format!("field{i}"), json!({"type":"boolean"})))
            .collect::<Map<_, _>>();
        assert!(parse(
            "mcpServer/elicitation/request",
            &json!({"requestedSchema":{"type":"object","properties":properties}})
        )
        .is_err());
        let request = form(json!({"x":{"type":"string"}}), json!(["x"]));
        assert!(request.answer(&[Input::Text("x".repeat(65537))]).is_err());
    }
    #[test]
    fn url_mode_never_opens_a_link_and_responds_without_form_content() {
        let request=parse("mcpServer/elicitation/request",&json!({"mode":"url","serverName":"test","url":"https://example.com/oauth?state=private","message":"Continue in browser"})).unwrap();
        assert!(matches!(&request,Request::Url{host,..} if host=="example.com"));
        assert_eq!(request.answer(&[]).unwrap(), json!({"action":"accept"}));
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://user:password@example.com/",
        ] {
            assert!(parse(
                "mcpServer/elicitation/request",
                &json!({"mode":"url","url":url})
            )
            .is_err());
        }
        assert!(parse(
            "mcpServer/elicitation/request",
            &json!({"mode":"openai/userVerification"})
        )
        .is_err());
    }
}
