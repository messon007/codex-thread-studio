//! Legacy OpenCode commands: bounded metadata, server expansion and server IDs.
use serde::{de, Deserialize};
use serde_json::{json, Value};
use std::{collections::HashSet, fmt};

pub const STOP_NOTICE: &str = "OpenCode has not returned this command's message identity yet. Open the target conversation to stop it; Router will not guess which message to abort.";

fn valid(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

/// Discard templates while decoding, rather than materializing them as Values.
pub fn inventory(bytes: &[u8]) -> Result<Value, String> {
    #[derive(Deserialize)]
    struct Command {
        name: String,
        source: Option<String>,
    }
    struct Commands;
    impl<'de> de::Visitor<'de> for Commands {
        type Value = Value;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("at most 512 OpenCode command entries")
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
            let mut rows = Vec::new();
            let mut names = HashSet::new();
            while let Some(command) = seq.next_element::<Command>()? {
                if rows.len() == 512
                    || !valid(&command.name, 128)
                    || command.source.as_ref().is_some_and(|s| !valid(s, 32))
                    || !names.insert(command.name.clone())
                {
                    return Err(de::Error::custom(
                        "Invalid or oversized OpenCode command inventory",
                    ));
                }
                rows.push(json!({"name":command.name,"source":command.source}));
            }
            Ok(json!(rows))
        }
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = de::Deserializer::deserialize_seq(&mut decoder, Commands)
        .map_err(|_| "Invalid or oversized OpenCode command inventory")?;
    decoder
        .end()
        .map_err(|_| "Invalid OpenCode command inventory")?;
    Ok(value)
}

pub fn contains(inventory: &Value, name: &str, path: &str) -> bool {
    name == path
        && inventory.as_array().is_some_and(|rows| {
            rows.iter()
                .any(|row| row["name"] == name && row["source"] == "skill")
        })
}

pub fn body(params: &Value) -> Result<Option<Value>, String> {
    let inputs = params["input"].as_array().ok_or("Missing routed prompt")?;
    let mut skills = inputs.iter().filter(|part| part["type"] == "skill");
    let Some(skill) = skills.next() else {
        return Ok(None);
    };
    if skills.next().is_some() {
        return Err("OpenCode accepts one skill command per request".into());
    }
    let name = skill["name"]
        .as_str()
        .filter(|name| valid(name, 128))
        .ok_or("Invalid OpenCode skill name")?;
    if skill["path"] != name {
        return Err("Choose an OpenCode skill from the target project's inventory".into());
    }
    let mut arguments = Vec::new();
    let mut attachments = Vec::new();
    for input in inputs {
        match input["type"].as_str() {
            Some("skill") => {}
            Some("text") => {
                arguments.push(input["text"].as_str().ok_or("Invalid command arguments")?)
            }
            Some("file" | "image") => {
                attachments.push(crate::native_router::image_input::opencode_part(input)?)
            }
            _ => {
                return Err("OpenCode skill commands do not accept Codex plugin references".into())
            }
        }
    }
    let arguments = arguments.join("\n");
    if arguments.len() > 64 * 1024 {
        return Err("OpenCode command arguments exceed 64 KiB".into());
    }
    let mut body = json!({"command":name,"arguments":arguments,"agent":"build"});
    if !attachments.is_empty() {
        body["parts"] = json!(attachments);
    }
    if let Some(model) = params["model"].as_str().filter(|model| !model.is_empty()) {
        if !valid(model, 256)
            || !model
                .split_once('/')
                .is_some_and(|(p, m)| !p.is_empty() && !m.is_empty())
        {
            return Err("Choose an OpenCode provider/model before routing".into());
        }
        body["model"] = model.into();
    }
    Ok(Some(body))
}

/// Only the synchronous command ACK can bind its expanded prompt to a user ID.
/// Parts may contain enormous templates or tool output; keep only identity.
pub fn acknowledgement(bytes: &[u8]) -> Result<Value, String> {
    #[derive(Deserialize)]
    struct Envelope {
        info: Info,
    }
    #[derive(Deserialize, serde::Serialize)]
    struct Info {
        role: String,
        id: String,
        #[serde(rename = "parentID")]
        parent_id: Option<String>,
        #[serde(rename = "sessionID")]
        session_id: String,
    }
    let value: Envelope = serde_json::from_slice(bytes).map_err(|_| {
        "OpenCode command response has no usable identity; inspect the target before retrying"
    })?;
    Ok(json!({"info":value.info}))
}

pub fn identity(value: &Value, session: &str, known: &HashSet<String>) -> Result<String, String> {
    let info = &value["info"];
    let id = match info["role"].as_str() {
        Some("user") => info["id"].as_str(),
        Some("assistant") => info["parentID"].as_str(),
        _ => None,
    }
    .filter(|id| valid(id, 256) && !known.contains(*id));
    if info["sessionID"] != session
        || !info["id"].as_str().is_some_and(|id| valid(id, 256))
        || id.is_none()
    {
        return Err("OpenCode accepted the command without a matching new message identity; inspect the target before retrying. The command was not resent.".into());
    }
    Ok(id.unwrap().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_keep_arguments_model_and_file_parts_without_client_identity() {
        let params = json!({"input":[{"type":"text","text":"  quoted 中文😀\n"},{"type":"skill","name":"build","path":"build"},{"type":"file","mime":"text/plain","url":"file:///project/a%20b.md","filename":"/project/a b.md"},{"type":"image","url":"data:image/gif;base64,R0lGODlhdGVzdA=="}],"model":"provider/model"});
        let command = body(&params).unwrap().unwrap();
        assert_eq!(command["arguments"], "  quoted 中文😀\n");
        assert_eq!(command["model"], "provider/model");
        assert_eq!(command["parts"][0], params["input"][2]);
        assert_eq!(command["parts"][1]["mime"], "image/gif");
        assert!(command.get("messageID").is_none());
        let mut bad = params.clone();
        bad["input"]
            .as_array_mut()
            .unwrap()
            .push(params["input"][1].clone());
        assert!(body(&bad).unwrap_err().contains("one skill"));
        bad = params.clone();
        bad["input"][1]["path"] = "/codex/SKILL.md".into();
        assert!(body(&bad).is_err());
        bad = params.clone();
        bad["input"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"mention"}));
        assert!(body(&bad).is_err());
    }
    #[test]
    fn inventory_discards_templates_rejects_collisions_and_bounds_count() {
        let rows = json!([{"name":"build","source":"skill","template":{"nested":["secret"]}},{"name":"review","source":"command","template":"secret"}]);
        let projected = inventory(&serde_json::to_vec(&rows).unwrap()).unwrap();
        assert!(contains(&projected, "build", "build"));
        assert!(!contains(&projected, "build", "/codex/SKILL.md"));
        assert!(!contains(&projected, "review", "review"));
        assert!(!projected.to_string().contains("secret"));
        assert!(inventory(br#"[{"name":"x"},{"name":"x"}]"#).is_err());
        let rows = (0..513)
            .map(|i| json!({"name":format!("skill-{i}"),"source":"skill"}))
            .collect::<Vec<_>>();
        assert!(inventory(&serde_json::to_vec(&rows).unwrap()).is_err());
    }
    #[test]
    fn ack_cannot_claim_another_session_or_existing_message() {
        let bytes=br#"{"info":{"id":"assistant","role":"assistant","sessionID":"target","parentID":"server-user"},"parts":[{"text":"private output"}]}"#;
        let value = acknowledgement(bytes).unwrap();
        assert_eq!(
            identity(&value, "target", &HashSet::new()).unwrap(),
            "server-user"
        );
        assert!(!value.to_string().contains("private output"));
        assert!(identity(&value, "other", &HashSet::new()).is_err());
        assert!(identity(&value, "target", &HashSet::from(["server-user".into()])).is_err());
        assert!(acknowledgement(br#"{"info":{"role":"assistant","id":"assistant"}}"#).is_err());
    }
}
