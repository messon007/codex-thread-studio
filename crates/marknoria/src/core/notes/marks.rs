//! Bounded, revision-owned rendered selectors and an indexed lightweight view.
//! Rendered offsets are hints for the matching snapshot, never source authority.
use super::{Anchor, error, sql_error, is_busy};
use crate::core::{Index, reader::Location};
use rusqlite::{Connection, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, io};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum TextScope {
    Projection { page: usize },
    Cell { table: usize, row: usize, column: usize },
    QueryCell { query_id: String, row: usize, column: usize },
}
impl TextScope {
    pub fn key(&self) -> String {
        match self {
            Self::Projection { page } => format!("p:{page}"),
            Self::Cell { table, row, column } => format!("c:{table}:{row}:{column}"),
            Self::QueryCell { query_id, row, column } => format!("q:{query_id}:{row}:{column}"),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextSegment {
    pub scope: TextScope,
    pub start_utf16: usize,
    pub end_utf16: usize,
    pub text_hash: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextRange {
    pub version: u8,
    pub segments: Vec<TextSegment>,
}
impl TextRange {
    pub fn validate(&self, index: &Index, anchor: &Anchor) -> io::Result<()> {
        if self.version != 1 || self.segments.is_empty() || self.segments.len() > 32
            || serde_json::to_vec(self)?.len() > 32768 {
            return Err(error("Invalid bounded comment text range"));
        }
        let mut unique = BTreeSet::new();
        for segment in &self.segments {
            if segment.start_utf16 >= segment.end_utf16 || segment.end_utf16 > 262144
                || segment.text_hash.len() != 64 || !segment.text_hash.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                || !unique.insert(segment.scope.key()) {
                return Err(error("Invalid comment text range endpoints or fingerprint"));
            }
            let valid = match &segment.scope {
                TextScope::Projection { page } => anchor.query.is_none() && index.pages.get(*page).is_some_and(|p| p.table.is_none()),
                TextScope::Cell { table, row, column } => anchor.query.is_none()
                    && index.tables.get(*table).is_some_and(|t| *row < t.rows && *column < t.columns.len())
                    && matches!(&anchor.location, Some(Location::Cell { table: t, row: r, column: c }) if (t,r,c)==(table,row,column)),
                TextScope::QueryCell { query_id, row, column } => query_id.len() <= 128 && !query_id.is_empty()
                    && query_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    && anchor.query.as_ref().is_some_and(|q| q.row == *row && q.column == *column),
            };
            if !valid { return Err(error("Comment text scope is outside its owning reference")); }
        }
        Ok(())
    }
}

pub fn keys(index: &Index, anchor: &Anchor) -> io::Result<Vec<String>> {
    if let Some(range) = &anchor.text_range {
        range.validate(index, anchor)?;
        return Ok(range.segments.iter().map(|s| s.scope.key()).collect());
    }
    Ok(match &anchor.location {
        Some(Location::Source { start_byte, end_byte }) if end_byte > start_byte && end_byte - start_byte <= 128 * 1024 =>
            (*start_byte / 16384..=(*end_byte - 1) / 16384).map(|p| format!("s:{p}")).collect(),
        Some(Location::Pdf { pages }) => pages.iter().map(|p| format!("pdf:{}", p.page)).collect::<BTreeSet<_>>().into_iter().collect(),
        Some(Location::Epub { href: Some(href), .. }) => vec![format!("epub:{href}")],
        _ => Vec::new(),
    })
}
pub fn register(db: &Connection, document: &str, revision: &str, id: &str, keys: &[String]) -> io::Result<()> {
    for key in keys {
        db.execute("INSERT OR IGNORE INTO comment_scopes(document,revision,scope,note_id) VALUES(?,?,?,?)", params![document, revision, key, id]).map_err(sql_error)?;
    }
    Ok(())
}
fn ensure_index(db: &Connection, document: &str, index: &Index) -> io::Result<()> {
    // Existing indexes are a pure read, even while another connection saves.
    let migrated: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM comment_scope_documents WHERE document=?)", [document], |row| row.get(0)).map_err(sql_error)?;
    if migrated { return Ok(()); }
    // Reserve the writer before taking a snapshot. A deferred read-to-write
    // upgrade can fail immediately with SQLITE_BUSY despite the busy timeout.
    let transaction = match Transaction::new_unchecked(db, TransactionBehavior::Immediate) {
        Ok(transaction) => transaction,
        Err(e) if is_busy(&e) => {
            // A concurrent first save/migration may have initialized the index
            // while this connection waited. That case is now a pure read.
            let ready: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM comment_scope_documents WHERE document=?)", [document], |row| row.get(0)).map_err(sql_error)?;
            if ready { return Ok(()); }
            return Err(sql_error(e));
        }
        Err(e) => return Err(sql_error(e)),
    };
    let migrated: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM comment_scope_documents WHERE document=?)", [document], |row| row.get(0)).map_err(sql_error)?;
    if !migrated {
        // One bounded migration per notebook identity. No comment bodies loaded.
        let mut statement = transaction.prepare("SELECT id,revision,anchor FROM notes WHERE document=? AND kind='comment' LIMIT 10001").map_err(sql_error)?;
        let rows = statement.query_map([document], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?))).map_err(sql_error)?;
        for (position, row) in rows.enumerate() {
            if position >= 10000 { return Err(error("Comment scope migration exceeds its item budget")); }
            let (id, revision, encoded) = row.map_err(sql_error)?;
            if let Ok(anchor) = serde_json::from_str::<Anchor>(&encoded)
                && let Ok(scopes) = keys(index, &anchor) {
                register(&transaction, document, &revision, &id, &scopes)?;
            }
        }
        drop(statement);
        transaction.execute("INSERT OR IGNORE INTO comment_scope_documents(document) VALUES(?)", [document]).map_err(sql_error)?;
    }
    transaction.commit().map_err(sql_error)
}
pub fn read(db: &Connection, document: &str, index: &Index, revision: &str, scopes: Vec<String>, offset: usize) -> io::Result<Value> {
    if revision != index.revision { return Err(error("Comment marks revision changed")); }
    if scopes.len() > 128 || offset > 10000 || scopes.iter().any(|s| s.is_empty() || s.len()>8192 || s.contains('\0') || !["p:","c:","q:","s:","pdf:","epub:"].iter().any(|p| s.starts_with(p)))
        || serde_json::to_vec(&scopes)?.len() > 32768 {
        return Err(error("Invalid bounded comment scopes"));
    }
    ensure_index(db, document, index)?;
    if scopes.is_empty() { return Ok(json!({"revision":revision,"items":[],"total":0,"nextOffset":offset,"complete":true})); }
    let encoded = serde_json::to_string(&scopes)?;
    let predicate = " FROM notes n JOIN comment_scopes s ON s.note_id=n.id WHERE s.document=? AND s.revision=? AND s.scope IN(SELECT value FROM json_each(?)) AND n.kind='comment'";
    let total: usize = db.query_row(&format!("SELECT COUNT(DISTINCT n.id){predicate}"), params![document,revision,encoded], |r| r.get(0)).map_err(sql_error)?;
    let mut statement = db.prepare(&format!("SELECT DISTINCT n.id,n.quote,n.anchor{predicate} ORDER BY n.created DESC,n.id LIMIT 64 OFFSET ?")).map_err(sql_error)?;
    let rows = statement.query_map(params![document,revision,encoded,offset], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).map_err(sql_error)?;
    let mut items = Vec::new(); let mut bytes = 256;
    for row in rows {
        let (id,quote,anchor) = row.map_err(sql_error)?;
        let item = json!({"id":id,"kind":"comment","quote":quote,"anchor":serde_json::from_str::<Value>(&anchor)?,"revision":revision});
        let size = serde_json::to_vec(&item)?.len();
        if size+256 > 128*1024 { return Err(error("A comment mark exceeds the transport budget")); }
        if bytes+size > 128*1024 { break; }
        bytes += size; items.push(item);
    }
    let next = offset + items.len();
    Ok(json!({"revision":revision,"items":items,"total":total,"nextOffset":next,"complete":next>=total}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{notes::{execute_for_identity, Request}, Page};
    fn index() -> Index {
        Index { revision: "r1".into(), source_lines: 20, source_bytes: 50000,
            pages: vec![Page { start:0,end:100,line:1,offset:0,bytes:100,rows:0,row_start:0,columns:0,table:None }], ..Default::default() }
    }
    fn anchor() -> Anchor {
        Anchor { location: Some(Location::Line { line: 1 }), text_range: Some(TextRange { version:1, segments:vec![TextSegment {
            scope:TextScope::Projection { page:0 }, start_utf16:5,end_utf16:10,text_hash:"a".repeat(64)
        }] }), ..Default::default() }
    }
    fn run(dir: &std::path::Path, doc: &str, index: &Index, value: Value) -> io::Result<Value> {
        execute_for_identity(dir,doc,index,serde_json::from_value::<Request>(value)?)
    }
    #[test]
    fn scoped_persistence_overlap_filters_and_cascading_delete() {
        let dir = tempfile::tempdir().unwrap(); let i = index();
        let add = |kind: &str| json!({"operation":"add","kind":kind,"quote":"again","comment":"private body","anchor":anchor(),"revision":"r1"});
        let a = run(dir.path(),"doc",&i,add("comment")).unwrap()["id"].as_str().unwrap().to_owned();
        let b = run(dir.path(),"doc",&i,add("comment")).unwrap()["id"].as_str().unwrap().to_owned();
        run(dir.path(),"doc",&i,add("favorite")).unwrap();
        run(dir.path(),"other",&i,add("comment")).unwrap();
        let marks = || run(dir.path(),"doc",&i,json!({"operation":"marks","revision":"r1","scopes":["p:0","p:0"]})).unwrap();
        let value = marks(); assert_eq!(value["total"],2); assert_eq!(value["items"].as_array().unwrap().len(),2);
        assert!(value["items"][0].get("comment").is_none(), "Marker view must not transport comment bodies");
        assert_eq!(value["items"][0]["anchor"]["textRange"]["segments"][0]["startUtf16"],5);
        assert_eq!(run(dir.path(),"doc",&i,json!({"operation":"marks","revision":"r1","scopes":["p:1"]})).unwrap()["total"],0);
        assert!(run(dir.path(),"other",&i,json!({"operation":"get","id":a})).is_err());
        assert_eq!(run(dir.path(),"doc",&i,json!({"operation":"get","id":a})).unwrap()["comment"],"private body");
        run(dir.path(),"doc",&i,json!({"operation":"clear","kind":"favorite"})).unwrap(); assert_eq!(marks()["total"],2);
        run(dir.path(),"doc",&i,json!({"operation":"delete","id":a})).unwrap(); assert_eq!(marks()["total"],1);
        assert_eq!(marks()["items"][0]["id"],b);
        run(dir.path(),"doc",&i,json!({"operation":"clear","kind":"comment"})).unwrap(); assert_eq!(marks()["total"],0);
        assert_eq!(super::super::db(dir.path()).unwrap().query_row("SELECT COUNT(*) FROM comment_scopes WHERE document='doc'",[],|r|r.get::<_,usize>(0)).unwrap(),0);
    }
    #[test]
    fn revision_budgets_and_legacy_precise_migration() {
        let dir = tempfile::tempdir().unwrap(); let i = index(); let connection = super::super::db(dir.path()).unwrap();
        let document = "legacy";
        for (n, anchor) in [Anchor { location:Some(Location::Source { start_byte:16000,end_byte:17000 }), ..Default::default() }, Anchor { line:Some(1), ..Default::default() }].into_iter().enumerate() {
            let id = uuid::Uuid::new_v4().to_string();
            connection.execute("INSERT INTO notes VALUES(?,?,?,?,?,?,?,?,?)",params![id,document,"r1","comment","repeat","body",serde_json::to_string(&anchor).unwrap(),n,id]).unwrap();
        }
        let marks=read(&connection,document,&i,"r1",vec!["s:0".into(),"s:1".into()],0).unwrap();
        assert_eq!(marks["total"],1,"Ambiguous old line anchors must never be guessed");
        assert!(read(&connection,document,&i,"changed",vec!["s:0".into()],0).is_err());
        assert!(read(&connection,document,&i,"r1",vec!["p:0".into();129],0).is_err());
        assert!(read(&connection,document,&i,"r1",vec!["p:0".into()],10001).is_err());
        let changed=Index { revision:"changed".into(), ..index() };
        assert_eq!(read(&connection,document,&changed,"changed",vec!["s:0".into()],0).unwrap()["total"],0);
    }
    #[test]
    fn selector_shape_and_owning_scope_are_strict() {
        let i = index(); let a=anchor(); a.text_range.as_ref().unwrap().validate(&i,&a).unwrap();
        for mutate in [0,1,2,3,4,5] {
            let mut a=anchor(); let range=a.text_range.as_mut().unwrap();
            match mutate { 0=>range.version=2,1=>range.segments[0].end_utf16=262145,2=>range.segments[0].start_utf16=10,
                3=>range.segments[0].text_hash="G".repeat(64),4=>range.segments[0].scope=TextScope::Projection { page:1 },_=>range.segments.push(range.segments[0].clone()) }
            assert!(a.text_range.as_ref().unwrap().validate(&i,&a).is_err());
        }
        let mut a=anchor(); a.text_range.as_mut().unwrap().segments[0].scope=TextScope::Cell { table:0,row:0,column:0 };
        assert!(a.text_range.as_ref().unwrap().validate(&i,&a).is_err());
        assert!(serde_json::from_value::<TextRange>(json!({"version":1,"segments":[],"unexpected":true})).is_err());
    }

    #[test]
    fn first_migration_waits_for_writer_and_migrated_marks_remain_read_only() {
        use std::{sync::mpsc, thread, time::Duration};
        let dir = tempfile::tempdir().unwrap();
        let writer = super::super::db(dir.path()).unwrap();
        let reader = super::super::db(dir.path()).unwrap();
        let transaction = Transaction::new_unchecked(&writer, TransactionBehavior::Immediate).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        transaction.execute("INSERT INTO notes VALUES(?,?,?,?,?,?,?,?,?)", params![id,"doc","r1","comment","again","body",serde_json::to_string(&anchor()).unwrap(),1,id]).unwrap();
        let (started, waiting) = mpsc::channel();
        let migration = thread::spawn(move || {
            started.send(()).unwrap();
            read(&reader,"doc",&index(),"r1",vec!["p:0".into()],0)
        });
        waiting.recv().unwrap();
        // A writer is deliberately held across the old deferred transaction's
        // snapshot/upgrade window. The new Immediate migration waits safely.
        thread::sleep(Duration::from_millis(100));
        transaction.commit().unwrap();
        assert_eq!(migration.join().unwrap().unwrap()["total"],1);

        let reader = super::super::db(dir.path()).unwrap();
        let transaction = Transaction::new_unchecked(&writer, TransactionBehavior::Immediate).unwrap();
        transaction.execute("UPDATE notes SET comment='pending' WHERE id=?",[id]).unwrap();
        let (send, receive) = mpsc::channel();
        let query = thread::spawn(move || {
            let value=read(&reader,"doc",&index(),"r1",vec!["p:0".into()],0);
            let _=send.send(value);
        });
        let value=receive.recv_timeout(Duration::from_secs(1));
        transaction.commit().unwrap();
        query.join().unwrap();
        assert_eq!(value.expect("Already migrated marks must not wait for a writer").unwrap()["total"],1);
    }

    #[test]
    fn fresh_notebook_readers_and_saves_initialize_and_migrate_concurrently() {
        use std::{sync::{Arc, Barrier}, thread};
        let dir=Arc::new(tempfile::tempdir().unwrap());
        let start=Arc::new(Barrier::new(4));
        let workers:Vec<_>=(0..4).map(|worker| {
            let dir=dir.clone();let start=start.clone();
            thread::spawn(move || {
                start.wait();let i=index();
                for number in 0..12 {
                    let request=if worker%2==0 {
                        json!({"operation":"add","kind":"comment","quote":"again","comment":format!("{worker}:{number}"),"anchor":anchor(),"revision":"r1"})
                    } else { json!({"operation":"marks","revision":"r1","scopes":["p:0"]}) };
                    // Mirror the transport's bounded retry of transient BUSY.
                    // InvalidInput (the old 400) must never be silently retried.
                    for attempt in 0..4 {
                        match run(dir.path(),"doc",&i,request.clone()) {
                            Ok(_) => break,
                            Err(e) if e.kind()==io::ErrorKind::WouldBlock && attempt<3 => thread::sleep(std::time::Duration::from_millis(10)),
                            Err(e) => panic!("Concurrent notebook operation failed: {e:?}"),
                        }
                    }
                }
            })
        }).collect();
        for worker in workers { worker.join().unwrap(); }
        assert_eq!(run(dir.path(),"doc",&index(),json!({"operation":"marks","revision":"r1","scopes":["p:0"]})).unwrap()["total"],24);
    }

    #[test]
    fn simultaneous_saves_cannot_exceed_the_global_notebook_limit() {
        use std::{sync::{Arc, Barrier}, thread};
        let dir=Arc::new(tempfile::tempdir().unwrap());let connection=super::super::db(dir.path()).unwrap();
        let transaction=Transaction::new_unchecked(&connection,TransactionBehavior::Immediate).unwrap();
        {
            let mut insert=transaction.prepare("INSERT INTO notes VALUES(?,?,?,?,?,?,?,?,?)").unwrap();
            for number in 0..9999 {
                let id=uuid::Uuid::new_v4().to_string();
                insert.execute(params![id,"doc","r1","favorite","quote","","{}",number,id]).unwrap();
            }
        }
        transaction.commit().unwrap();drop(connection);
        let start=Arc::new(Barrier::new(2));
        let workers:Vec<_>=(0..2).map(|_|{let dir=dir.clone();let start=start.clone();thread::spawn(move||{
            start.wait();run(dir.path(),"doc",&index(),json!({"operation":"add","kind":"comment","quote":"again","anchor":anchor(),"revision":"r1"}))
        })}).collect();
        let mut saved=0;let mut denied=0;
        for worker in workers {match worker.join().unwrap(){Ok(_)=>saved+=1,Err(e)=>{assert!(e.to_string().contains("10,000"),"{e:?}");denied+=1}}}
        assert_eq!((saved,denied),(1,1));
        assert_eq!(run(dir.path(),"doc",&index(),json!({"operation":"list","offset":0})).unwrap()["total"],10000);
    }
}
