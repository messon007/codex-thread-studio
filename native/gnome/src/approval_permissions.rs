//! Select exact requested grants without resolving target-native paths or
//! weakening deny entries / traversal constraints. No GTK or filesystem access.
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Location {
    Network,
    Entry(usize),
    Read(usize),
    Write(usize),
}
#[derive(Debug, PartialEq, Eq, Hash)]
enum Target {
    Literal(String),
    Glob(String),
    Special {
        kind: String,
        path: Option<String>,
        subpath: Option<String>,
    },
}
#[derive(Debug, PartialEq, Eq, Hash)]
enum Key {
    Network,
    File { access: String, target: Target },
}
fn entry_key(entry: &Value) -> Key {
    let path = &entry["path"];
    let target = match path["type"].as_str().unwrap() {
        "path" => Target::Literal(path["path"].as_str().unwrap().into()),
        "glob_pattern" => Target::Glob(path["pattern"].as_str().unwrap().into()),
        _ => Target::Special {
            kind: path["value"]["kind"].as_str().unwrap().into(),
            path: path["value"]["path"].as_str().map(str::to_owned),
            subpath: path["value"]["subpath"].as_str().map(str::to_owned),
        },
    };
    Key::File {
        access: entry["access"].as_str().unwrap().into(),
        target,
    }
}
#[derive(Debug)]
pub struct Grant {
    pub label: String,
    pub description: String,
    locations: Vec<Location>,
}
#[derive(Debug)]
pub struct Profile {
    original: Value,
    pub grants: Vec<Grant>,
    pub restrictions: Vec<String>,
}
fn keys(value: &Value, allowed: &[&str]) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("Invalid requested permission object")?;
    if object.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(
            "This permission profile uses unsupported fields; no grant is available".into(),
        );
    }
    Ok(())
}
fn path_text(v: &Value) -> Result<&str, String> {
    v.as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
        .ok_or_else(|| "Invalid or oversized requested permission path".into())
}
fn path(v: &Value) -> Result<String, String> {
    match v["type"].as_str() {
        Some("path") => {
            keys(v, &["type", "path"])?;
            Ok(serde_json::to_string(path_text(&v["path"])?).unwrap())
        }
        Some("glob_pattern") => {
            keys(v, &["type", "pattern"])?;
            Ok(format!(
                "Pattern: {}",
                serde_json::to_string(path_text(&v["pattern"])?).unwrap()
            ))
        }
        Some("special") => {
            keys(v, &["type", "value"])?;
            let special = &v["value"];
            match special["kind"].as_str() {
                Some("root" | "minimal" | "tmpdir" | "slash_tmp") => keys(special, &["kind"])?,
                Some("project_roots") => {
                    keys(special, &["kind", "subpath"])?;
                    if !special["subpath"].is_null() {
                        path_text(&special["subpath"])?;
                    }
                }
                Some("unknown") => {
                    keys(special, &["kind", "path", "subpath"])?;
                    path_text(&special["path"])?;
                    if !special["subpath"].is_null() {
                        path_text(&special["subpath"])?;
                    }
                }
                _ => return Err("Unsupported special permission path".into()),
            }
            let label = match special["kind"].as_str().unwrap() {
                "root" => "Filesystem root (all paths)".to_owned(),
                "minimal" => "Minimal system paths".to_owned(),
                "tmpdir" => "Runtime temporary directory".to_owned(),
                "slash_tmp" => "/tmp".to_owned(),
                "project_roots" => "Project folders".to_owned(),
                _ => format!(
                    "Target-specific path: {}",
                    serde_json::to_string(&special["path"]).unwrap()
                ),
            };
            Ok(if special["subpath"].is_null() {
                label
            } else {
                format!(
                    "{label}\nSubpath: {}",
                    serde_json::to_string(&special["subpath"]).unwrap()
                )
            })
        }
        _ => Err("Unsupported permission path type".into()),
    }
}
fn list<'a>(v: &'a Value) -> Result<&'a [Value], String> {
    if v.is_null() {
        return Ok(&[]);
    }
    v.as_array()
        .filter(|v| v.len() <= 128)
        .map(Vec::as_slice)
        .ok_or_else(|| "Permission list exceeds 128 entries or is invalid".into())
}
impl Profile {
    pub fn parse(value: &Value) -> Result<Self, String> {
        keys(value, &["network", "fileSystem"])?;
        if serde_json::to_vec(value).map_err(|e| e.to_string())?.len() > 65536 {
            return Err("Permission profile exceeds 64 KiB".into());
        }
        let mut profile = Self {
            original: value.clone(),
            grants: Vec::new(),
            restrictions: Vec::new(),
        };
        let mut aliases = HashMap::<Key, usize>::new();
        let mut add = |key: Key,
                       label: &str,
                       description: String,
                       location: Location|
         -> Result<(), String> {
            if let Some(index) = aliases.get(&key) {
                profile.grants[*index].locations.push(location);
            } else {
                if profile.grants.len() >= 128 {
                    return Err("Permission profile exceeds 128 selectable grants".into());
                }
                aliases.insert(key, profile.grants.len());
                profile.grants.push(Grant {
                    label: label.into(),
                    description,
                    locations: vec![location],
                });
            }
            Ok(())
        };
        if !value["network"].is_null() {
            keys(&value["network"], &["enabled"])?;
            match &value["network"]["enabled"] {
                Value::Bool(true) => add(
                    Key::Network,
                    "Network access",
                    "Enable network access as requested".into(),
                    Location::Network,
                )?,
                Value::Bool(false) => profile
                    .restrictions
                    .push("Network access remains disabled".into()),
                Value::Null => {}
                _ => return Err("Invalid requested network permission".into()),
            }
        }
        let fs = &value["fileSystem"];
        if !fs.is_null() {
            keys(fs, &["entries", "read", "write", "globScanMaxDepth"])?;
            if !fs["globScanMaxDepth"].is_null() {
                let depth = fs["globScanMaxDepth"]
                    .as_u64()
                    .filter(|v| *v >= 1)
                    .ok_or("Invalid permission scan-depth limit")?;
                profile
                    .restrictions
                    .push(format!("Glob scan depth remains limited to {depth}"));
            }
            for (index, entry) in list(&fs["entries"])?.iter().enumerate() {
                keys(entry, &["path", "access"])?;
                let description = path(&entry["path"])?;
                let label = match entry["access"].as_str() {
                    Some("read") => "Read files",
                    Some("write") => "Write files",
                    Some("deny") => {
                        profile
                            .restrictions
                            .push(format!("Deny file access: {description}"));
                        continue;
                    }
                    _ => return Err("Invalid requested filesystem access mode".into()),
                };
                add(entry_key(entry), label, description, Location::Entry(index))?;
            }
            for (field, label) in [("read", "Read files"), ("write", "Write files")] {
                for (index, item) in list(&fs[field])?.iter().enumerate() {
                    let p = path_text(item)?;
                    let key = Key::File {
                        access: field.into(),
                        target: Target::Literal(p.into()),
                    };
                    add(
                        key,
                        label,
                        serde_json::to_string(p).unwrap(),
                        if field == "read" {
                            Location::Read(index)
                        } else {
                            Location::Write(index)
                        },
                    )?;
                }
            }
        }
        Ok(profile)
    }
    pub fn selected(&self, selected: &[usize]) -> Result<Value, String> {
        let indices: HashSet<usize> = selected.iter().copied().collect();
        if indices.len() != selected.len() || indices.iter().any(|i| *i >= self.grants.len()) {
            return Err(
                "Permission selection changed or is invalid; review the request again".into(),
            );
        }
        if indices.len() == self.grants.len() {
            return Ok(self.original.clone());
        }
        if indices.is_empty() {
            return Ok(json!({}));
        }
        let chosen: HashSet<_> = indices
            .iter()
            .flat_map(|i| self.grants[*i].locations.iter().cloned())
            .collect();
        let mut result = self.original.as_object().unwrap().clone();
        if self.original["network"]["enabled"] == true && !chosen.contains(&Location::Network) {
            result.remove("network");
        }
        if let Some(fs) = result.get_mut("fileSystem").and_then(Value::as_object_mut) {
            for field in ["entries", "read", "write"] {
                if let Some(items) = fs.get_mut(field).and_then(Value::as_array_mut) {
                    let mut index = 0;
                    items.retain(|entry| {
                        let keep = if field == "entries" {
                            entry["access"] == "deny" || chosen.contains(&Location::Entry(index))
                        } else if field == "read" {
                            chosen.contains(&Location::Read(index))
                        } else {
                            chosen.contains(&Location::Write(index))
                        };
                        index += 1;
                        keep
                    });
                }
            }
            let has_entries = ["entries", "read", "write"].iter().any(|f| {
                fs.get(*f)
                    .and_then(Value::as_array)
                    .is_some_and(|a| !a.is_empty())
            });
            if !has_entries {
                result.remove("fileSystem");
            }
        }
        Ok(Value::Object(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Value {
        json!({"network":{"enabled":true},"fileSystem":{"entries":[
        {"access":"write","path":{"type":"path","path":"/project 中文/a.rs"}},
        {"access":"read","path":{"type":"glob_pattern","pattern":"/project/**/*.md"}},
        {"access":"deny","path":{"type":"path","path":"/project/secrets"}},
        {"access":"write","path":{"type":"special","value":{"kind":"project_roots","subpath":"out"}}}
    ],"write":["/project 中文/a.rs","C:\\project\\b.rs"],"read":["/outside/notes"],"globScanMaxDepth":4}})
    }
    #[test]
    fn aliases_share_one_choice_and_full_selection_keeps_exact_profile() {
        let value = sample();
        let p = Profile::parse(&value).unwrap();
        assert_eq!(p.grants.len(), 6);
        assert_eq!(
            p.grants[1].locations,
            vec![Location::Entry(0), Location::Write(0)]
        );
        assert_eq!(
            p.selected(&(0..p.grants.len()).collect::<Vec<_>>())
                .unwrap(),
            value
        );
        assert!(p.restrictions.iter().any(|s| s.contains("secrets")));
        assert!(p.restrictions.iter().any(|s| s.contains("4")));
    }
    #[test]
    fn subsets_cannot_leave_an_unchecked_alias_or_drop_constraints() {
        let p = Profile::parse(&sample()).unwrap();
        let result = p.selected(&[2]).unwrap();
        assert!(result.get("network").is_none());
        assert_eq!(result["fileSystem"]["globScanMaxDepth"], 4);
        assert_eq!(result["fileSystem"]["entries"].as_array().unwrap().len(), 2);
        assert_eq!(result["fileSystem"]["entries"][0]["access"], "read");
        assert_eq!(result["fileSystem"]["entries"][1]["access"], "deny");
        assert_eq!(result["fileSystem"]["write"], json!([]));
        let network = p.selected(&[0]).unwrap();
        assert_eq!(network["network"]["enabled"], true);
        assert_eq!(
            network["fileSystem"]["entries"].as_array().unwrap().len(),
            1
        );
        assert_eq!(network["fileSystem"]["entries"][0]["access"], "deny");
        assert_eq!(p.selected(&[]).unwrap(), json!({}));
        assert!(p.selected(&[0, 0]).is_err());
        assert!(p.selected(&[6]).is_err());
    }
    #[test]
    fn paths_are_target_native_and_are_never_resolved_or_rewritten() {
        let value = json!({"fileSystem":{"read":["relative/path","C:\\work\\a","/unicode 中文/line\nbreak"],"entries":[{"path":{"type":"special","value":{"kind":"unknown","path":"target-native","subpath":"segment"}},"access":"read"}]}});
        let p = Profile::parse(&value).unwrap();
        let result = p.selected(&[3]).unwrap();
        assert_eq!(
            result["fileSystem"]["read"],
            json!(["/unicode 中文/line\nbreak"])
        );
        assert!(p.grants[3].description.contains("\\n"));
    }
    #[test]
    fn only_applicable_constraints_are_kept_and_empty_grants_do_not_expand() {
        let p = Profile::parse(
            &json!({"network":{"enabled":true},"fileSystem":{"write":["/a"],"globScanMaxDepth":2}}),
        )
        .unwrap();
        assert_eq!(
            p.selected(&[0]).unwrap(),
            json!({"network":{"enabled":true}})
        );
        let p = Profile::parse(
            &json!({"network":{"enabled":false},"fileSystem":{"write":["/a","/b"]}}),
        )
        .unwrap();
        assert_eq!(
            p.selected(&[1]).unwrap(),
            json!({"network":{"enabled":false},"fileSystem":{"write":["/b"]}})
        );
        let v =
            json!({"network":null,"fileSystem":{"entries":[],"write":null,"globScanMaxDepth":2}});
        assert_eq!(Profile::parse(&v).unwrap().selected(&[]).unwrap(), v);
    }
    #[test]
    fn malformed_unknown_and_oversized_profiles_have_no_grant_projection() {
        for value in [
            json!({"future":true}),
            json!({"network":{"enabled":"true"}}),
            json!({"fileSystem":{"write":["bad\0path"]}}),
            json!({"fileSystem":{"entries":[{"access":"future","path":{"type":"path","path":"/a"}}]}}),
            json!({"fileSystem":{"entries":[{"access":"read","path":{"type":"future","path":"/a"}}]}}),
            json!({"fileSystem":{"globScanMaxDepth":0}}),
            json!({"fileSystem":{"read":vec!["/a";129]}}),
        ] {
            assert!(Profile::parse(&value).is_err(), "{value}");
        }
    }
}
