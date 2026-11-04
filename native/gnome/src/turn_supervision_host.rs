//! Socket integration for native supervision. Owns bounded environment work;
//! the existing socket-owned Map runtime still prepares every source turn.
use crate::{
    gateway::Api,
    session_search::Context,
    turn_supervision::Follow,
    turn_supervision_runtime::{self as coordinator, Effect, Runtime, Service, Start, View},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug)]
pub enum Request {
    Start(Start),
    Stop {
        thread: String,
    },
    Prepared {
        id: u64,
        params: Value,
    },
    Rejected {
        id: u64,
        reason: String,
    },
    Options {
        thread: String,
        model: String,
        effort: String,
    },
    Invalidate {
        thread: String,
    },
}
impl Request {
    pub fn request(&self) -> Option<u64> {
        match self {
            Self::Start(start) => Some(start.request),
            _ => None,
        }
    }
    pub fn thread(&self) -> Option<&str> {
        match self {
            Self::Start(start) => Some(&start.context.thread),
            Self::Stop { thread } | Self::Options { thread, .. } | Self::Invalidate { thread } => {
                Some(thread)
            }
            _ => None,
        }
    }
}
#[derive(Debug)]
pub enum Update {
    Changed(View),
    Prepare {
        id: u64,
        request: u64,
        context: Context,
        root: String,
        text: String,
    },
    Follow {
        id: u64,
        receipt: Follow,
        prompt: String,
        routed: Result<bool, String>,
    },
    Rejected {
        request: Option<u64>,
        thread: Option<String>,
        message: String,
    },
}
#[derive(Debug)]
pub enum Action {
    Update(Update),
    Map { id: u64, params: Value },
    Ready { id: u64 },
    Abandon { id: u64, reason: String },
}
pub struct Host<S: Service = coordinator::Worker> {
    pub runtime: Runtime<S>,
    api: Api,
    backend: String,
    environments: tokio::task::JoinSet<(u64, Value, Result<bool, String>)>,
    routing: HashMap<u64, (String, bool)>,
    starting: HashSet<u64>,
    actions: VecDeque<Action>,
    process: Option<u64>,
    applied: crate::environment::Applied,
    handoffs: tokio::task::JoinSet<Result<bool, String>>,
    handoff_receipts: HashMap<tokio::task::Id, (u64, Follow, String)>,
}
impl<S: Service> Host<S> {
    pub fn new(
        api: Api,
        service: S,
        backend: String,
        generation: u64,
        applied: crate::environment::Applied,
    ) -> Self {
        Self {
            runtime: Runtime::new(service, backend.clone(), generation),
            api,
            backend,
            environments: tokio::task::JoinSet::new(),
            routing: HashMap::new(),
            starting: HashSet::new(),
            actions: VecDeque::new(),
            process: None,
            applied,
            handoffs: tokio::task::JoinSet::new(),
            handoff_receipts: HashMap::new(),
        }
    }
    pub fn has_work(&self) -> bool {
        self.runtime.has_work()
            || !self.environments.is_empty()
            || !self.actions.is_empty()
            || !self.handoffs.is_empty()
    }
    pub fn request(&mut self, request: Request) {
        let thread = request.thread().map(str::to_owned);
        let token = request.request();
        let result = match request {
            Request::Start(start) => self.runtime.start(start),
            Request::Stop { thread } => {
                self.runtime.stop(&thread, "Supervision stopped by user");
                Ok(())
            }
            Request::Options {
                thread,
                model,
                effort,
            } => self.runtime.options(&thread, model, effort),
            Request::Invalidate { thread } => {
                self.runtime.invalidate_preparation(&thread);
                Ok(())
            }
            Request::Rejected { id, reason } => {
                self.runtime.preparation_failed(id, reason);
                Ok(())
            }
            Request::Prepared { id, params } => {
                let Some((source, text)) = self.runtime.preparation(id) else {
                    return;
                };
                let Some(previous) = self.runtime.preparation_turn(id) else {
                    return;
                };
                if self.starting.contains(&id) {
                    return;
                }
                if params["threadId"].as_str() != Some(source.context.thread.as_str())
                    || params["input"] != serde_json::json!([{"type":"text","text":text}])
                    || params.to_string().len() > 32 * 1024
                {
                    self.runtime.preparation_failed(
                        id,
                        "Supervised source parameters changed; no continuation was sent".into(),
                    );
                    return;
                }
                if self.environments.len() >= 2 {
                    self.runtime.preparation_failed(
                        id,
                        "Source environment workers are busy; no continuation was sent".into(),
                    );
                    return;
                }
                self.starting.insert(id);
                let api = self.api.clone();
                let backend = self.backend.clone();
                let applied = self.applied.clone();
                self.environments.spawn(async move {
                    let result = async {
                    let readiness=api.json(reqwest::Method::POST,"/studio/native/router/prepare-supervised",Some(&serde_json::json!({"backend":backend,"threadId":source.context.thread,"cwd":source.directory,"turnId":previous}))).await?;
                    if readiness["ready"]!=true{return Err("Routed source preparation was not confirmed; no continuation was sent".into());}
                    let routed=readiness["routed"].as_bool().ok_or("Routed preparation returned no ownership decision")?;
                    crate::environment::prepare(
                        &api,
                        &backend,
                        &source.context.thread,
                        &source.directory,
                        &mut *applied.lock().await,
                    )
                    .await?;
                    Ok(routed)
                    }.await;
                    (id, params, result)
                });
                Ok(())
            }
        };
        if let Err(message) = result {
            self.actions.push_back(Action::Update(Update::Rejected {
                thread,
                request: token,
                message,
            }));
        }
    }
    pub fn observe(&mut self, payload: &Value, attention: bool) -> bool {
        if payload["method"] == "studio/appServer/status" {
            let generation = payload["params"]["generation"].as_u64();
            if (self.process.is_some() && generation.is_some() && self.process != generation)
                || matches!(
                    payload["params"]["state"].as_str(),
                    Some("starting" | "error" | "stopped")
                )
            {
                self.runtime.stop_all("Backend changed or disconnected");
            }
            if generation.is_some() {
                self.process = generation;
            }
        }
        self.runtime.observe(payload, attention)
    }
    pub fn map_ready(&mut self, id: u64, params: Value) -> bool {
        if !self.starting.remove(&id) {
            return false;
        }
        // A cancelled Map effect is consumed rather than becoming a normal send.
        self.runtime.prepared(id, params);
        true
    }
    pub fn failed(&mut self, id: u64, reason: String) -> bool {
        self.starting.remove(&id);
        self.routing.remove(&id);
        self.runtime.preparation_failed(id, reason)
    }
    pub async fn next(&mut self) -> Action {
        loop {
            if let Some(action) = self.actions.pop_front() {
                return action;
            }
            tokio::select! {
                result=self.handoffs.join_next_with_id(),if !self.handoffs.is_empty()=>{
                    let result=match result{Some(Ok((task,result)))=>Some((task,result)),Some(Err(error))=>Some((error.id(),Err("Supervised handoff worker failed; inspect the accepted source turn".into()))),None=>None};
                    if let Some((task,result))=result{if let Some((id,receipt,prompt))=self.handoff_receipts.remove(&task){
                        self.runtime.followed(id,result.as_ref().map(|_|()).map_err(Clone::clone));
                        return Action::Update(Update::Follow{id,receipt,prompt,routed:result});
                    }}
                }
                result=self.environments.join_next(),if !self.environments.is_empty()=>{
                    match result {
                        Some(Ok((id,params,result)))=>{
                            match result {
                                Ok(routed) if self.runtime.preparation(id).is_some()=>{self.routing.insert(id,(params["threadId"].as_str().unwrap_or("").to_owned(),routed));return Action::Map{id,params};},
                                Ok(_)=>{self.starting.remove(&id);},
                                Err(error)=>{self.starting.remove(&id);self.runtime.preparation_failed(id,error);}
                            }
                        }
                        Some(Err(_))=>{let ids=self.starting.iter().copied().collect::<Vec<_>>();for id in ids{self.failed(id,"Source environment worker failed; no continuation was sent".into());}},
                        None=>{}
                    }
                }
                effect=self.runtime.next(),if self.runtime.has_work()=>match effect {
                    Effect::Follow{id,receipt,prompt}=>{
                        let source=self.runtime.source(&receipt.context.thread);let api=self.api.clone();
                        let routing=self.routing.remove(&id).filter(|(thread,_)|thread==&receipt.context.thread).map(|(_,routed)|routed);
                        let retained=(id,receipt.clone(),prompt.clone());
                        let task=self.handoffs.spawn(async move {
                            let result=async {
                                let source=source.ok_or("Supervised source context was lost; inspect the accepted source turn")?;
                                let expected=routing.ok_or("Supervised routing ownership was lost; inspect the accepted source turn")?;
                                let value=api.json(reqwest::Method::POST,"/studio/native/router/follow",Some(&serde_json::json!({"backend":receipt.context.backend,"threadId":receipt.context.thread,"cwd":source.directory,"previousTurnId":receipt.previous_turn,"turnId":receipt.turn,"prompt":prompt}))).await?;
                                handoff_result(value,&receipt,expected)
                            }.await;
                            result
                        });
                        self.handoff_receipts.insert(task.id(),retained);
                    },
                    Effect::Changed(view)=>{if !view.holding {self.routing.retain(|_,(thread,_)|thread!=&view.context.thread);}return Action::Update(Update::Changed(view));},
                    Effect::Prepare{id,request,context,root,text}=>return Action::Update(Update::Prepare{id,request,context,root,text}),
                    Effect::Ready{id}=>return Action::Ready{id},
                    Effect::Abandon{id,reason}=>{self.starting.remove(&id);self.routing.remove(&id);return Action::Abandon{id,reason};},
                }
            }
        }
    }
    pub fn begin_shutdown(&mut self) {
        self.runtime.begin_shutdown();
        self.environments.abort_all();
    }
    pub async fn shutdown(&mut self) {
        self.begin_shutdown();
        self.environments.abort_all();
        while self.environments.join_next().await.is_some() {}
        let deadline = tokio::time::sleep(std::time::Duration::from_secs(45));
        tokio::pin!(deadline);
        while self.has_work() {
            tokio::select! {_=&mut deadline=>{eprintln!("Native supervision cleanup or accepted handoff was not confirmed before shutdown");break;},_=self.next()=>{}}
        }
    }
}
fn handoff_result(value: Value, receipt: &Follow, expected: bool) -> Result<bool, String> {
    let followed = value["followed"]
        .as_bool()
        .ok_or("Invalid supervised handoff response")?;
    if expected && !followed {
        return Err("The routed source was not rebound; inspect the accepted source turn".into());
    }
    if followed {
        let id = value["id"].as_str().unwrap_or("");
        let controller = value["controller"].as_str().unwrap_or("");
        if id.is_empty()
            || id.len() > 64
            || id.chars().any(char::is_control)
            || controller.len() > 320
            || controller.chars().any(char::is_control)
            || controller
                .split_once(':')
                .is_none_or(|(backend, thread)| backend.is_empty() || thread.is_empty())
            || value["targetTurnId"].as_str() != Some(receipt.turn.as_str())
        {
            return Err("Supervised handoff response changed its source identity".into());
        }
    }
    Ok(followed)
}
