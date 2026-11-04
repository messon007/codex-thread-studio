//! Native frontend entry point and reusable GTK document widgets.
#[cfg(all(debug_assertions, target_os = "linux"))]
mod acceptance_wait;
#[cfg(target_os = "linux")]
mod action_menu;
mod activity;
mod activity_details;
#[cfg(target_os = "linux")]
mod adaptive;
mod agents;
#[cfg(target_os = "linux")]
mod agents_ui;
mod annotations;
#[cfg(target_os = "linux")]
mod annotations_ui;
#[cfg(target_os = "linux")]
mod app;
mod appearance;
#[cfg(target_os = "linux")]
mod appearance_ui;
mod approval;
#[cfg(target_os = "linux")]
mod approval_ui;
mod attachments;
mod backend_lifecycle;
mod backend_recovery;
mod backend_settings;
#[cfg(target_os = "linux")]
mod backend_settings_ui;
#[allow(dead_code)]
#[path = "../../../src-tauri/src/browser_policy.rs"]
mod browser_policy;
mod browser_settings;
#[cfg(target_os = "linux")]
mod browser_settings_ui;
mod browser_tabs;
#[cfg(target_os = "linux")]
mod browser_ui;
mod byte_cache;
mod chat_content;
#[cfg(target_os = "linux")]
mod chat_diagrams;
#[cfg(target_os = "linux")]
mod chat_ui;
#[cfg(target_os = "linux")]
mod chat_visibility;
mod command_data;
#[cfg(target_os = "linux")]
mod command_palette;
mod command_tools;
#[cfg(target_os = "linux")]
mod compare_ui;
mod composer_refs;
#[cfg(target_os = "linux")]
mod composer_tools_ui;
mod connections;
#[cfg(target_os = "linux")]
mod connections_ui;
#[cfg(target_os = "linux")]
pub mod content_widgets;
mod conversation_cache;
#[cfg(target_os = "linux")]
mod conversation_context_ui;
mod conversation_focus;
mod conversation_images;
mod conversation_output;
#[cfg(target_os = "linux")]
mod conversation_output_ui;
mod conversation_pages;
#[cfg(target_os = "linux")]
mod conversation_pages_ui;
mod download_file;
mod environment;
mod epub;
mod epub_search;
#[cfg(target_os = "linux")]
mod epub_search_ui;
#[cfg(target_os = "linux")]
mod epub_ui;
mod favorites;
#[cfg(target_os = "linux")]
mod favorites_ui;
#[cfg(target_os = "linux")]
mod file_io;
#[cfg(target_os = "linux")]
mod file_picker;
#[cfg(target_os = "linux")]
mod files_ui;
mod files_watch;
mod gateway;
mod gateway_launch;
#[cfg(debug_assertions)]
mod gateway_test_fault;
mod git_review;
mod history_state;
mod html_preview;
#[cfg(target_os = "linux")]
mod html_ui;
#[cfg(target_os = "linux")]
mod i18n_ui;
mod image_animation;
#[cfg(target_os = "linux")]
mod image_playback;
mod images;
#[cfg(target_os = "linux")]
mod images_ui;
#[cfg(target_os = "linux")]
mod inline_math;
#[cfg(target_os = "linux")]
mod inline_math_shape;
mod interaction;
#[cfg(target_os = "linux")]
mod interaction_ui;
mod language;
#[cfg(target_os = "linux")]
mod language_ui;
#[cfg(target_os = "linux")]
mod list_navigation;
#[cfg(target_os = "linux")]
mod list_store;
#[cfg(target_os = "linux")]
mod logical_texture;
#[cfg(target_os = "linux")]
mod math_picture;
#[path = "../../../src-tauri/src/notification_ids.rs"]
#[allow(dead_code)]
mod notification_ids;
mod notifications;
#[cfg(target_os = "linux")]
mod notifications_ui;
mod pdf;
#[path = "../../../src-tauri/src/pdf_navigation.rs"]
#[allow(dead_code)]
mod pdf_navigation;
#[cfg(target_os = "linux")]
mod pdf_ui;
mod remote_images;
#[cfg(target_os = "linux")]
mod rich_paragraph;
#[path = "../../../src-tauri/src/session_directory_policy.rs"]
mod session_directory_policy;
mod session_hydration;
mod session_list;
mod session_map;
mod startup;
mod turn_navigation;
#[cfg(target_os = "linux")]
mod turn_navigation_ui;
mod viewer_resume;
mod session_map_types {
    pub use crate::session_map::types::*;
}
mod continuation;
#[allow(dead_code)]
#[path = "../../../src-tauri/src/continuation_text.rs"]
mod continuation_text;
mod message_flow;
#[cfg(target_os = "linux")]
mod message_flow_ui;
#[cfg(target_os = "linux")]
mod responsibility_ui;
mod session_info;
#[cfg(target_os = "linux")]
mod session_info_ui;
mod session_map_ai;
#[path = "../../../src-tauri/src/session_map_protocol.rs"]
mod session_map_protocol;
mod session_map_sync;
mod session_map_text;
#[cfg(target_os = "linux")]
mod session_map_ui;
mod session_resources;
#[cfg(target_os = "linux")]
mod session_resources_ui;
mod session_search;
#[cfg(target_os = "linux")]
mod session_search_ui;
mod session_visibility;
#[cfg(target_os = "linux")]
mod session_visibility_ui;
mod shared_directories;
#[cfg(target_os = "linux")]
mod shared_directories_ui;
#[cfg(all(target_os = "linux", debug_assertions))]
mod smoke_input;
#[cfg(all(target_os = "linux", debug_assertions))]
mod smoke_portal;
mod source_search;
mod speech;
#[cfg(target_os = "linux")]
mod texture_viewer;
mod translation;
mod translation_settings;
#[cfg(target_os = "linux")]
mod translation_settings_ui;
#[allow(dead_code)]
#[path = "../../../src-tauri/src/translation_text.rs"]
mod translation_text;
#[cfg(target_os = "linux")]
mod translation_ui;
mod workspace_navigation;
use studio_markdown_native::markup as markdown;
#[cfg(target_os = "linux")]
mod anchored_text_view;
#[cfg(target_os = "linux")]
mod code_gutter;
mod diagram;
mod diagram_queue;
mod diagram_style;
mod document;
mod engine_process;
mod flow_prepare;
#[cfg(target_os = "linux")]
mod git_ui;
mod links;
mod math;
mod mcp;
#[cfg(target_os = "linux")]
mod mcp_ui;
#[cfg(target_os = "linux")]
mod mermaid_fonts;
mod mermaid_math;
#[cfg(target_os = "linux")]
mod mermaid_prepared;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
mod mermaid_worker;
mod message_queue;
mod model;
mod opencode;
mod opencode_history;
#[cfg(target_os = "linux")]
mod preview_scheme;
mod protocol;
#[cfg(target_os = "linux")]
mod queue_ui;
#[cfg(target_os = "linux")]
mod reader;
mod router;
mod router_config;
#[cfg(target_os = "linux")]
mod router_config_ui;
mod router_notifications;
#[cfg(target_os = "linux")]
mod router_picker_ui;
#[cfg(target_os = "linux")]
mod router_ui;
#[cfg(target_os = "linux")]
mod scaled_picture;
#[cfg(target_os = "linux")]
mod selection_menu;
#[cfg(target_os = "linux")]
mod settings_ui;
mod source_reload;
mod source_surface;
mod streaming;
#[cfg(target_os = "linux")]
mod syntax_ui;
#[cfg(target_os = "linux")]
pub mod table_cell;
mod table_columns;
#[cfg(target_os = "linux")]
mod table_columns_ui;
mod tables;
#[cfg(target_os = "linux")]
mod tables_ui;
mod terminal;
#[cfg(target_os = "linux")]
mod text_edges;
#[cfg(target_os = "linux")]
pub mod text_flow;
#[cfg(target_os = "linux")]
mod text_links;
#[cfg(target_os = "linux")]
mod text_position;
mod text_reveal;
mod turn_options;
#[cfg(target_os = "linux")]
mod viewport;
#[cfg(all(target_os = "linux", debug_assertions))]
mod window_capture;
mod worker;
mod workspace;
#[cfg(target_os = "linux")]
mod workspace_ui;
#[cfg(target_os = "linux")]
mod write_feedback;

#[cfg(target_os = "linux")]
pub fn run() -> gtk::glib::ExitCode {
    run_math_worker_if_requested();
    app::run()
}

/// Worker dispatch must precede GTK, profile locks and application startup.
pub fn run_math_worker_if_requested() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--native-math-worker")) {
        std::process::exit(math::worker_stdio());
    }
}

mod supervision_state;
#[cfg(target_os = "linux")]
mod supervision_ui;
mod turn_supervision;
mod turn_supervision_host;
mod turn_supervision_io;
mod turn_supervision_runtime;
