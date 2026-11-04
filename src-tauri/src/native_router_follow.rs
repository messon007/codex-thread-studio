//! Record only an already accepted supervised source turn. This endpoint never
//! starts a turn, retries a mutation, or guesses a receipt from assistant text.
use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Follow {
    backend: String,
    thread_id: String,
    cwd: String,
    previous_turn_id: String,
    turn_id: String,
    prompt: String,
}
impl Follow {
    fn validate(&self) -> Result<String, String> {
        let key = format!("{}:{}", self.backend, self.thread_id);
        let id = |s: &str| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control);
        if !crate::valid_router_session_key(&key)
            || !id(&self.previous_turn_id)
            || !id(&self.turn_id)
            || self.previous_turn_id == self.turn_id
            || self.cwd.is_empty()
            || self.cwd.len() > 4096
            || self.cwd.contains('\0')
            || !self.prompt.starts_with("[Studio turn supervisor]\n")
            || self.prompt.len() > 16 * 1024 + 64
            || self.prompt.contains('\0')
            || self
                .prompt
                .strip_prefix("[Studio turn supervisor]\n")
                .is_none_or(|s| s.trim().is_empty() || s.encode_utf16().count() > 4000)
        {
            return Err("Invalid supervised source receipt".into());
        }
        Ok(key)
    }
    fn same(&self, job: &Job, key: &str) -> bool {
        job.target.as_ref().is_some_and(|target| {
            target.session_key == key
                && format!("{}:{}", target.backend, target.id) == key
                && target.cwd == self.cwd
        })
    }
    fn recorded(&self, job: &Job) -> bool {
        job.target_turn_id == self.turn_id
            && job.supervision.as_ref().is_some_and(|s| {
                s.previous_turn_id == self.previous_turn_id && s.prompt == self.prompt
            })
    }
}
fn receipt(value: Value, request: &Follow) -> Result<(), String> {
    let turns = value["data"]
        .as_array()
        .ok_or("Backend returned no source receipt window")?;
    if turns.len() > 4 {
        return Err("Backend exceeded its source receipt window".into());
    }
    let index = turns
        .iter()
        .position(|turn| turn["id"] == request.turn_id)
        .ok_or("The acknowledged source turn is outside the receipt window")?;
    let next = &turns[index];
    let previous = turns
        .get(index + 1)
        .ok_or("The preceding supervised source turn is unavailable")?;
    if previous["id"] != request.previous_turn_id
        || previous["status"] != "completed"
        || !previous["error"].is_null()
        || !matches!(
            next["status"].as_str(),
            Some("inProgress" | "completed" | "failed" | "interrupted")
        )
    {
        return Err("The supervised source turn chain changed".into());
    }
    let user = next["items"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["type"] == "userMessage"))
        .ok_or("The accepted source input is unavailable")?;
    let text = if let Some(text) = user["text"].as_str() {
        if text.len() > 16 * 1024 + 64 {
            return Err("The accepted source input exceeds its budget".into());
        }
        text.to_owned()
    } else {
        let parts = user["content"]
            .as_array()
            .ok_or("The accepted source input has no text")?;
        if parts.len() > 8192 {
            return Err("The accepted source input has too many parts".into());
        }
        let mut text = String::new();
        for (i, part) in parts.iter().enumerate() {
            let s = part["text"].as_str().unwrap_or("");
            if text.len() + s.len() + usize::from(i != 0) > 16 * 1024 + 64 {
                return Err("The accepted source input exceeds its budget".into());
            }
            if i != 0 {
                text.push('\n');
            }
            text.push_str(s);
        }
        text
    };
    if text != request.prompt {
        return Err("The acknowledged source input does not match the continuation".into());
    }
    Ok(())
}
fn response(job: &Job) -> Value {
    json!({"followed":true,"id":job.id,"controller":job.controller,"targetTurnId":job.target_turn_id})
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preparation {
    backend: String,
    thread_id: String,
    cwd: String,
    turn_id: String,
}
impl Preparation {
    fn key(&self, state: &GatewayState) -> Result<String, String> {
        let key = format!("{}:{}", self.backend, self.thread_id);
        if !crate::valid_router_session_key(&key)
            || self.cwd.is_empty()
            || self.cwd.len() > 4096
            || self.cwd.contains('\0')
            || self.turn_id.is_empty()
            || self.turn_id.len() > 256
            || self.turn_id.chars().any(char::is_control)
            || !state.codex_backends.contains_key(&self.backend)
        {
            return Err("Invalid supervised source preparation scope".into());
        }
        Ok(key)
    }
}
/// Read ownership without provider RPC, recovery, Map waiting or a send intent.
pub async fn post_scope(
    State(state): State<GatewayState>,
    body: Result<Json<Preparation>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        _ => return json_error(StatusCode::BAD_REQUEST, "Invalid supervision source scope"),
    };
    let key = match request.key(&state) {
        Ok(key) => key,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, &error),
    };
    let lookup = request.clone();
    match disk(&state, move |state| {
        store::source_scope(&state.studio_path, &key, &lookup.turn_id, &lookup.cwd)
    })
    .await
    {
        Ok(scope) => json_response(
            StatusCode::OK,
            &json!({"backend":request.backend,"threadId":request.thread_id,"turnId":request.turn_id,"cwd":request.cwd,"route":scope}),
        ),
        Err(error) => json_error(StatusCode::CONFLICT, &error),
    }
}
pub async fn post_prepare(
    State(state): State<GatewayState>,
    body: Result<Json<Preparation>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        _ => {
            return json_error(
                StatusCode::BAD_REQUEST,
                "Invalid supervised source preparation",
            )
        }
    };
    let key = match request.key(&state) {
        Ok(key) => key,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, &error),
    };
    if let Err(error) = recover(&state).await {
        return json_error(StatusCode::CONFLICT, &error);
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let source = key.clone();
        let turn = request.turn_id.clone();
        let cwd = request.cwd.clone();
        match disk(&state, move |state| {
            store::source_ready(&state.studio_path, &source, &turn, &cwd)
        })
        .await
        {
            Ok((routed, true)) => {
                return json_response(StatusCode::OK, &json!({"ready":true,"routed":routed}))
            }
            Ok((_, false)) => {}
            Err(error) => return json_error(StatusCode::CONFLICT, &error),
        }
        if tokio::time::Instant::now() >= deadline {
            return json_error(
                StatusCode::CONFLICT,
                "Previous routed Map update has not settled; no continuation was sent",
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
pub async fn post_follow(
    State(state): State<GatewayState>,
    body: Result<Json<Follow>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        _ => return json_error(StatusCode::BAD_REQUEST, "Invalid supervised source handoff"),
    };
    let key = match request.validate() {
        Ok(key) => key,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, &error),
    };
    if !state.codex_backends.contains_key(&request.backend) {
        return json_error(
            StatusCode::BAD_REQUEST,
            "Supervised handoff requires a configured Codex-compatible backend",
        );
    }
    if let Err(error) = recover(&state).await {
        return json_error(StatusCode::CONFLICT, &error);
    }
    let lookup = request.clone();
    let source = key.clone();
    let candidate = match disk(&state, move |state| {
        store::source_job(
            &state.studio_path,
            &source,
            &lookup.previous_turn_id,
            &lookup.turn_id,
        )
    })
    .await
    {
        Ok(Some(job)) => job,
        Ok(None) => return json_response(StatusCode::OK, &json!({"followed":false})),
        Err(error) => return json_error(StatusCode::CONFLICT, &error),
    };
    if !request.same(&candidate, &key) {
        return json_error(
            StatusCode::CONFLICT,
            "Router source identity or project changed",
        );
    }
    if request.recorded(&candidate) {
        return json_response(StatusCode::OK, &response(&candidate));
    }
    let session = match Session::new(&key, &request.cwd) {
        Ok(s) => s,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, &error),
    };
    if let Err(error) = session.summary(&state).await {
        return json_error(StatusCode::CONFLICT, &error);
    }
    let verified = async {
        let server = session
            .codex(&state)?
            .ok_or("Supervised handoff requires Codex")?;
        let value = crate::native_router_backend::read(
            server,
            "thread/turns/list",
            json!({"threadId":session.id,"limit":4,"sortDirection":"desc","itemsView":"full"}),
        )
        .await?;
        let checked = request.clone();
        tokio::task::spawn_blocking(move || receipt(value, &checked))
            .await
            .map_err(|_| "Source receipt worker stopped")?
    }
    .await;
    if let Err(error) = verified {
        return json_error(StatusCode::CONFLICT, &error);
    }
    let _transition = state.router_runtime.transitions.lock().await;
    let lookup = request.clone();
    let source = key.clone();
    let current = match disk(&state, move |state| {
        store::source_job(
            &state.studio_path,
            &source,
            &lookup.previous_turn_id,
            &lookup.turn_id,
        )
    })
    .await
    {
        Ok(Some(job)) => job,
        Ok(None) => {
            return json_error(
                StatusCode::CONFLICT,
                "The routed source changed before handoff",
            )
        }
        Err(error) => return json_error(StatusCode::CONFLICT, &error),
    };
    if !request.same(&current, &key) {
        return json_error(
            StatusCode::CONFLICT,
            "Router source identity or project changed",
        );
    }
    if request.recorded(&current) {
        return json_response(StatusCode::OK, &response(&current));
    }
    let (active, stopped) = match state.router_runtime.stops.lock() {
        Ok(stops) => stops
            .get(&current.id)
            .map(|s| (true, *s.sender.borrow()))
            .unwrap_or((false, false)),
        Err(_) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Router control lock unavailable",
            )
        }
    };
    let permit = if active {
        None
    } else {
        match state.router_runtime.slots.clone().try_acquire_owned() {
            Ok(permit) => Some(permit),
            Err(_) => {
                return json_error(
                    StatusCode::CONFLICT,
                    "Router monitoring workers are busy; inspect the accepted source turn",
                )
            }
        }
    };
    let id = current.id.clone();
    let owner = state.router_runtime.owner.clone();
    let update = request.clone();
    let job = match disk(&state, move |state| {
        store::follow(
            &state.studio_path,
            &owner,
            &id,
            &key,
            &update.cwd,
            &update.previous_turn_id,
            &update.turn_id,
            &update.prompt,
            stopped,
        )
    })
    .await
    {
        Ok(job) => job,
        Err(error) => return json_error(StatusCode::CONFLICT, &error),
    };
    if let Some(permit) = permit {
        if let Err(error) = spawn(state.clone(), job.clone(), None, permit) {
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error);
        }
    }
    json_response(StatusCode::OK, &response(&job))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Follow {
        Follow {
            backend: "codex".into(),
            thread_id: "target".into(),
            cwd: "/project".into(),
            previous_turn_id: "previous".into(),
            turn_id: "next".into(),
            prompt: "[Studio turn supervisor]\nFinish B 中文😀".into(),
        }
    }
    fn window() -> Value {
        json!({"data":[{"id":"next","status":"inProgress","items":[{"type":"userMessage","content":[{"type":"text","text":"[Studio turn supervisor]\nFinish B 中文😀"}]}]},{"id":"previous","status":"completed"}]})
    }
    #[test]
    fn receipt_requires_the_exact_adjacent_turn_chain_and_accepted_user_input() {
        let r = request();
        assert!(r.validate().is_ok());
        assert!(receipt(window(), &r).is_ok());
        for field in ["previous", "status", "input"] {
            let mut v = window();
            match field {
                "previous" => v["data"][1]["id"] = "unrelated".into(),
                "status" => v["data"][1]["status"] = "interrupted".into(),
                _ => v["data"][0]["items"][0]["content"][0]["text"] = "Different input".into(),
            }
            assert!(receipt(v, &r).is_err());
        }
    }
    #[test]
    fn later_human_turns_do_not_erase_a_real_receipt_and_an_immediate_failure_still_binds() {
        let r = request();
        let mut v = window();
        v["data"][0]["status"] = "failed".into();
        v["data"].as_array_mut().unwrap().insert(
            0,
            json!({"id":"later-human","status":"inProgress","items":[]}),
        );
        assert!(receipt(v, &r).is_ok());
        let mut invalid = r;
        invalid.prompt = "[Studio turn supervisor]\n".into();
        assert!(invalid.validate().is_err());
    }
}
