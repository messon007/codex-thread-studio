//! Rebind a page to a fresh gateway without replaying a save or rebuilding edits.
use super::*;
use std::time::Duration;

pub(super) async fn run(
    api: &Api,
    sessions: &mut HashMap<u64, Session>,
    message: &Message,
    revision: &str,
    text: Option<&str>,
) -> Result<Reply, String> {
    let old = sessions
        .get_mut(&message.key)
        .ok_or("Open the document again")?;
    // Advance the local FIFO generation even on conflict so further offline
    // typing remains tracked and an explicit retry can verify the same buffer.
    old.generation = message.generation;
    old.valid = false;
    let expected = old
        .editor
        .as_ref()
        .map(|e| e.page.revision.as_str())
        .or_else(|| old.index.as_ref().map(|i| i.revision.as_str()));
    if revision.len() != 64
        || !revision.bytes().all(|b| b.is_ascii_hexdigit())
        || expected != Some(revision)
    {
        return Err(
            "The retained document snapshot cannot be verified; edits remain in memory".into(),
        );
    }
    if old.poisoned.is_some()
        || match (&old.editor, text) {
            (Some(editor), Some(text)) => editor.buffer.text() != text,
            (None, None) => false,
            _ => true,
        }
    {
        return Err(
            "The retained edit tracking differs from the visible page; edits remain in memory"
                .into(),
        );
    }
    let origin = old.origin.clone();
    let old_page = old.editor.as_ref().map(|e| e.page.clone());
    let mut candidate = HashMap::new();
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        let make = |request| Message {
            key: message.key,
            generation: message.generation,
            serial: message.serial,
            request,
        };
        let opened = Box::pin(handle(
            api,
            &mut candidate,
            &make(Request::Open {
                root: origin.root.clone(),
                path: origin.path.clone(),
                read_only: origin.read_only,
            }),
        ))
        .await?;
        let Reply::Open {
            read_only,
            reader_id,
        } = opened
        else {
            unreachable!()
        };
        if read_only != origin.read_only {
            return Err(
                "Document access changed while disconnected; edits remain in memory".to_owned(),
            );
        }
        if candidate.get(&message.key).map(|s| &s.origin) != Some(&origin) {
            return Err("The canonical document location changed while disconnected; edits remain in memory".into());
        }
        let index = loop {
            match Box::pin(handle(api, &mut candidate, &make(Request::Index))).await? {
                Reply::Index(Some(index)) => break index,
                Reply::Index(None) => tokio::time::sleep(Duration::from_millis(100)).await,
                _ => unreachable!(),
            }
        };
        if index.revision != revision {
            return Err(
                "The file changed while disconnected; edits remain in memory and were not saved"
                    .into(),
            );
        }
        let editor = if let Some(previous) = old_page {
            let Reply::Edit(current) = Box::pin(handle(
                api,
                &mut candidate,
                &make(Request::Edit {
                    line: previous.window.line,
                    offset: Some(previous.window.start),
                    previous: false,
                }),
            ))
            .await?
            else {
                unreachable!()
            };
            if current.revision != revision || !same_page(&previous, &current) {
                return Err(
                    "The edit page changed while disconnected; edits remain in memory".into(),
                );
            }
            Some(current)
        } else {
            None
        };
        Ok((reader_id, editor))
    })
    .await
    .unwrap_or_else(|_| Err("Document revalidation timed out; edits remain in memory".into()));
    let (reader_id, editor) = match result {
        Ok(result) => result,
        Err(error) => {
            if let Some(opened) = candidate.remove(&message.key) {
                let _ = tokio::time::timeout(
                    Duration::from_secs(3),
                    api.json(
                        Method::POST,
                        "/studio/markdown/close",
                        Some(&json!({"id":opened.reader})),
                    ),
                )
                .await;
            }
            return Err(error);
        }
    };
    let mut verified = candidate
        .remove(&message.key)
        .ok_or("Revalidation lost its candidate")?;
    // Preserve token identities and all offline Insert/Delete operations. GTK's
    // buffer, caret and undo history are also retained by the scoped UI reply.
    if let Some(previous) = sessions.get_mut(&message.key).and_then(|s| s.editor.take()) {
        verified.editor = editor.as_ref().map(|page| Editor {
            page: page.clone(),
            buffer: previous.buffer,
        });
    }
    sessions.insert(message.key, verified);
    Ok(Reply::Resumed { reader_id, editor })
}

fn same_page(old: &EditPage, new: &EditPage) -> bool {
    old.source_bytes == new.source_bytes
        && old.source_lines == new.source_lines
        && old.window.start == new.window.start
        && old.window.end == new.window.end
        && old.window.line == new.window.line
        && old.window.end_line == new.window.end_line
        && old.window.text == new.window.text
}

#[cfg(test)]
#[path = "document_resume_tests.rs"]
mod tests;
