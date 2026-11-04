//! Settle requests that never reached a service; never retry their work.
use super::*;
pub(super) fn command(command: Command, error: &str) -> Option<Event> {
    let error = error.to_owned();
    Some(match command {
        Command::Continue { ticket, .. } => Event::Continuation {
            ticket,
            result: Err(error),
        },
        Command::MessageFlow { serial, .. } => Event::MessageFlow {
            serial,
            result: Err(error),
        },
        Command::SupervisionOptions { serial, .. } => Event::Http {
            tag: format!("native-supervision-options:{serial}"),
            result: Err(error),
        },
        Command::Supervision {
            backend,
            generation,
            request,
        } => Event::Supervision {
            backend,
            generation,
            update: crate::turn_supervision_host::Update::Rejected {
                request: request.request(),
                thread: request.thread().map(str::to_owned),
                message: error,
            },
        },
        Command::Appearance { dialog, serial, .. } => Event::Appearance {
            dialog,
            serial,
            result: Err(error),
        },
        Command::Annotations { dialog, serial, .. } => Event::Annotations {
            dialog,
            serial,
            result: Err(error),
        },
        Command::BackendConfig { dialog, serial, .. } => Event::BackendConfig {
            dialog,
            serial,
            result: Err(error),
        },
        Command::BrowserSettings { dialog, serial, .. } => Event::BrowserSettings {
            dialog,
            serial,
            result: Err(error),
        },
        Command::TranslationSettings { dialog, serial, .. } => Event::TranslationSettings {
            dialog,
            serial,
            result: Err(error),
        },
        Command::SessionVisibility { dialog, serial, .. } => Event::SessionVisibility {
            dialog,
            serial,
            result: Err(error),
        },
        Command::SharedDirectories { dialog, serial, .. } => Event::SharedDirectories {
            dialog,
            serial,
            result: Err(error),
        },
        Command::RouterConfig { dialog, serial, .. } => Event::RouterConfig {
            dialog,
            serial,
            result: Err(error),
        },
        Command::Favorites { dialog, serial, .. } => Event::Favorites {
            dialog,
            serial,
            result: Err(error),
        },
        Command::Router { serial, .. } => Event::Router {
            serial,
            result: Err(error),
        },
        Command::Language { serial, .. } => Event::Language {
            serial,
            result: Err(error),
        },
        Command::NotificationSettings { serial, .. } => Event::NotificationSettings {
            serial,
            result: Err(error),
        },
        Command::BrowserTabs { serial, .. } => Event::BrowserTabs {
            serial,
            result: Err(error),
        },
        Command::FilterTools { serial, .. } => Event::FilterTools {
            serial,
            result: Err(error),
        },
        Command::NativeImage { id, .. } => Event::NativeImage {
            id,
            result: Err(error),
        },
        Command::ImageExport { id, .. } => Event::ImageExport {
            id,
            result: Err(error),
        },
        Command::Speech { id, .. } => Event::Speech {
            id,
            result: Err(error),
        },
        Command::Epub {
            generation,
            serial,
            kind,
            ..
        } => Event::Epub(crate::epub::Response {
            generation,
            serial,
            kind,
            result: Err(error),
        }),
        Command::Pdf {
            generation,
            serial,
            kind,
            ..
        } => Event::Pdf(crate::pdf::Response {
            generation,
            serial,
            kind,
            result: Err(error),
        }),
        Command::Table {
            generation,
            serial,
            kind,
            ..
        } => Event::Table(crate::tables::Response {
            generation,
            serial,
            kind,
            result: Err(error),
        }),

        Command::Rpc {
            backend,
            generation,
            id,
            ..
        } => Event::Wire {
            backend,
            generation,
            payload: json!({"id":id,"error":{"message":error}}),
            prepared: Prepared::default(),
        },
        Command::Reply {
            backend,
            generation,
            ..
        }
        | Command::Connect {
            backend: Backend { id: backend, .. },
            generation,
        } => Event::BackendError {
            backend,
            generation,
            error,
        },
        Command::Http { tag, .. }
            if tag.starts_with("settings:") && !tag.ends_with(":backends") =>
        {
            Event::Environment {
                tag,
                result: Err(error),
            }
        }
        Command::Http { tag, .. } => Event::Http {
            tag,
            result: Err(error),
        },
        Command::Preferences(_) => settings_event("native-preferences-save".into(), Err(error)),
        Command::SessionOptions {
            request, options, ..
        } => settings_event(
            if options.is_some() {
                "turn-options-save".into()
            } else {
                format!("turn-options-load:{request}")
            },
            Err(error),
        ),
        Command::Queue { key, serial, .. } => Event::Queue {
            key,
            serial,
            result: Err(error),
        },
        Command::CommandPalette { dialog, serial, .. } => Event::CommandPalette {
            dialog,
            serial,
            result: Err(error),
        },
        Command::AgentTree {
            dialog, revision, ..
        } => Event::AgentTree {
            dialog,
            revision,
            result: Err(error),
        },
        Command::Images { key, serial, .. } => Event::Images {
            key,
            serial,
            result: Err(error),
        },
        Command::OutputExport { dialog, serial, .. } => Event::OutputExport {
            dialog,
            serial,
            result: Err(error),
        },
        Command::Translate { ticket, .. } => Event::Translation {
            ticket,
            update: crate::translation::Update::Done(Err(error)),
        },
        Command::SessionMap { serial, .. } => Event::SessionMap {
            serial,
            result: Err(crate::session_map::Failure {
                message: error,
                conflict: false,
                uncertain: false,
            }),
        },
        Command::Workspace {
            panel,
            generation,
            serial,
            request,
        } => Event::Workspace(crate::workspace::Response {
            panel,
            generation,
            serial,
            kind: request.kind(),
            result: Err(error),
        }),
        Command::ConversationFocus(Some(r)) => {
            Event::ConversationFocus(crate::conversation_focus::Response {
                context: r.context,
                serial: r.serial,
                result: Err(error),
            })
        }
        Command::SessionSearch(Some(r)) => Event::SessionSearch(crate::session_search::Response {
            context: r.context,
            serial: r.serial,
            result: Err(error),
            scope: Default::default(),
        }),
        Command::SessionResources(Some(r)) => {
            Event::SessionResources(crate::session_resources::Response {
                context: r.context,
                serial: r.serial,
                result: Err(error),
            })
        }
        Command::Connections { dialog, serial, .. } => Event::Connections {
            dialog,
            serial,
            result: Err(error),
        },
        Command::SessionInfo { dialog, serial, .. } => Event::SessionInfo {
            dialog,
            serial,
            result: Err(crate::session_info::Failure {
                message: error,
                reload_required: false,
                http_status: None,
            }),
        },
        Command::ConversationPage(Some(r)) => {
            Event::ConversationPage(crate::conversation_pages::Response {
                source: r.source,
                serial: r.serial,
                result: Err(error),
            })
        }
        Command::ConversationOutput(Some(r)) => {
            Event::ConversationOutput(crate::conversation_output::Response {
                dialog: r.dialog,
                serial: r.serial,
                result: Err(error),
            })
        }
        Command::Streaming(Some(request)) => Event::Streaming(crate::streaming::Response {
            request,
            result: Err(error),
        }),
        Command::FilesWatch(Some(r)) => Event::FilesWatch {
            generation: r.generation,
            update: crate::files_watch::Update::Error { message: error },
        },
        Command::Terminal(
            crate::terminal::Control::Open { generation, .. }
            | crate::terminal::Control::Input { generation, .. }
            | crate::terminal::Control::Resize { generation, .. },
        ) => Event::Terminal {
            generation,
            update: crate::terminal::Update::Error(error),
        },
        Command::MapDeleted { .. }
        | Command::RouterConfigCancel { .. }
        | Command::Shutdown
        | Command::Discover { .. }
        | Command::FilesWatch(None)
        | Command::Terminal(crate::terminal::Control::Close)
        | Command::Streaming(None)
        | Command::ConversationFocus(None)
        | Command::SessionSearch(None)
        | Command::SessionResources(None)
        | Command::ConversationPage(None)
        | Command::ConversationOutput(None) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_mutation_retains_backend_request_and_generation_without_acknowledgement() {
        let event = command(
            Command::Rpc {
                backend: "private-backend".into(),
                generation: 17,
                id: 91,
                method: "turn/start".into(),
                params: json!({"input":[{"text":"must not send"}]}),
                root: None,
                images: vec![],
            },
            "stopped <literal>",
        )
        .unwrap();
        let Event::Wire {
            backend,
            generation,
            payload,
            ..
        } = event
        else {
            panic!("Wrong rejection scope")
        };
        assert_eq!((backend.as_str(), generation), ("private-backend", 17));
        assert_eq!(payload["id"], 91);
        assert_eq!(payload["error"]["message"], "stopped <literal>");
        assert!(payload.get("result").is_none());
    }
    #[test]
    fn queued_export_settles_exact_receipt_without_writing_destination() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("unsent-export.txt");
        let source = crate::conversation_output::retain("Frozen export 中文😀".into());
        let event = command(
            Command::OutputExport {
                dialog: 3,
                serial: 22,
                source,
                path: path.clone(),
            },
            "gateway exited",
        )
        .unwrap();
        assert!(matches!(
            event,
            Event::OutputExport {
                dialog: 3,
                serial: 22,
                result: Err(_)
            }
        ));
        assert!(!path.exists());
    }
    #[test]
    fn queued_settings_and_options_use_existing_error_routes() {
        let event = command(
            Command::Http {
                tag: "settings:7:profile".into(),
                method: reqwest::Method::PATCH,
                path: "/studio/environment".into(),
                body: Some(json!({"value":"must not write"})),
                document: None,
            },
            "stopped",
        )
        .unwrap();
        assert!(matches!(event,Event::Environment{tag,result:Err(_)} if tag=="settings:7:profile"));
        assert!(
            matches!(command(Command::SessionOptions{key:"codex:target".into(),request:13,options:None},"stopped"),Some(Event::Http{tag,result:Err(_)} ) if tag=="turn-options-load:13")
        );
        assert!(
            matches!(command(Command::Queue{key:"codex:target".into(),serial:6,request:crate::message_queue::Request::Prepare},"stopped"),Some(Event::Queue{key,serial:6,result:Err(_)} ) if key=="codex:target")
        );
    }
}
