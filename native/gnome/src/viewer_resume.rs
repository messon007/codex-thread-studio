//! Recreate read-only viewer leases on workers, pinned to the original snapshot.
use crate::{epub, gateway::Api, pdf, tables};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Target {
    root: String,
    path: String,
    hash: String,
}
impl Target {
    fn validate(&self) -> Result<(), String> {
        if self.hash.is_empty()
            || self.hash.len() > 128
            || self.root.len() > 8192
            || self.path.len() > 8192
            || !std::path::Path::new(&self.path).is_absolute()
            || self.path.contains('\0')
        {
            return Err("The original viewer snapshot cannot be verified".into());
        }
        Ok(())
    }
    fn matches(&self, hash: &str, path: &str) -> Result<(), String> {
        if self.hash != hash || self.path != path {
            Err("The file changed while disconnected; reopen it to review the new contents".into())
        } else {
            Ok(())
        }
    }
}
fn decode<T: serde::de::DeserializeOwned>(body: Value) -> Result<T, String> {
    serde_json::from_value(body).map_err(|_| "Invalid viewer recovery request".into())
}

#[derive(Deserialize)]
struct Table {
    #[serde(flatten)]
    target: Target,
    delimiter: Option<String>,
    sheet: usize,
    sql: Option<String>,
    offset: usize,
}
pub async fn table(api: &Api, body: Value) -> Result<tables::Reply, String> {
    let request: Table = decode(body)?;
    request.target.validate()?;
    if request.sheet >= 64
        || request.offset > 10000
        || request.sql.as_ref().is_some_and(|s| s.len() > 65536)
    {
        return Err("Table recovery exceeds its presentation budget".into());
    }
    let tables::Reply::Opened(mut opened)=Box::pin(tables::execute(api,"open",json!({"operation":"open","root":request.target.root,"path":request.target.path,"delimiter":request.delimiter}))).await? else { unreachable!() };
    let id = opened.id.clone();
    let result=async {
        request.target.matches(&opened.hash,&opened.path)?;
        if request.sheet>=opened.sheets.len() { return Err("Table worksheet changed while disconnected".into()); }
        let mut page=if let Some(sql)=request.sql {
            let tables::Reply::Page(page)=Box::pin(tables::execute(api,"query",json!({"operation":"query","id":id,"sheet":request.sheet,"sql":sql}))).await? else { unreachable!() };
            page
        } else if request.sheet>0 {
            let tables::Reply::Page(page)=Box::pin(tables::execute(api,"page",json!({"operation":"page","id":id,"sheet":request.sheet,"offset":0}))).await? else { unreachable!() };
            page
        } else { std::sync::Arc::new(opened.page) };
        let offset=request.offset.min(page.total.saturating_sub(1))/page.page_size*page.page_size;
        if page.offset!=offset {
            let tables::Reply::Page(next)=Box::pin(tables::execute(api,"page",json!({"operation":"page","id":id,"sheet":request.sheet,"query":page.query_id,"offset":offset}))).await? else { unreachable!() };
            page=next;
        }
        if page.document_id!=id || page.sheet!=request.sheet { return Err("Recovered table page belongs to another view".into()); }
        opened.page=std::sync::Arc::try_unwrap(page).map_err(|_|"Recovery page is still in use".to_owned())?;
        Ok(tables::Reply::Opened(opened))
    }.await;
    if result.is_err() {
        let _ = Box::pin(tables::execute(
            api,
            "close",
            json!({"operation":"close","id":id}),
        ))
        .await;
    }
    result
}

#[derive(Deserialize)]
struct Pdf {
    #[serde(flatten)]
    target: Target,
    id: String,
    page: usize,
    password: Option<String>,
}
pub async fn pdf(api: &Api, body: Value) -> Result<pdf::Reply, String> {
    let request: Pdf = decode(body)?;
    request.target.validate()?;
    if request.id.is_empty()
        || request.id.len() > 64
        || request.page > i32::MAX as usize
        || request
            .password
            .as_ref()
            .is_some_and(|s| s.len() > 512 || s.contains('\0'))
    {
        return Err("Invalid PDF recovery target".into());
    }
    let id = request.id;
    let result=async {
        let pdf::Reply::Page{mut meta,image}=Box::pin(pdf::execute(api,"open",json!({"operation":"open","id":id,"root":request.target.root,"path":request.target.path,"password":request.password}))).await? else { unreachable!() };
        request.target.matches(&meta.hash,&meta.path)?;
        if meta.document_id!=id { return Err("Recovered PDF belongs to another view".into()); }
        if request.page>=meta.pages { return Err("PDF page changed while disconnected".into()); }
        if request.page==0 { return Ok(pdf::Reply::Page{meta,image}); }
        drop(image);
        let pdf::Reply::Page{meta:mut page,image}=Box::pin(pdf::execute(api,"page",json!({"operation":"page","id":id,"page":request.page}))).await? else { unreachable!() };
        if page.document_id!=id || page.page!=request.page { return Err("Recovered PDF page belongs to another view".into()); }
        // The page endpoint reuses immutable bytes held by this lease. Its
        // compact metadata omits the fingerprint/name/path from Open.
        page.hash=meta.hash;
        page.path=meta.path;
        page.name=meta.name;
        page.outline=std::mem::take(&mut meta.outline);
        page.outline_limited=meta.outline_limited;
        Ok(pdf::Reply::Page{meta:page,image})
    }.await;
    if result.is_err() {
        let _ = Box::pin(pdf::execute(
            api,
            "close",
            json!({"operation":"close","id":id}),
        ))
        .await;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn malformed_recovery_and_cross_book_reading_are_rejected_before_network_io() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = Api::from_access_url(&format!(
            "http://{}/#token=0123456789abcdef0123456789abcdef",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let target = json!({"root":"/private","path":"/private/file","hash":"a".repeat(64)});
        let mut table_request = target.clone();
        table_request["sheet"] = 64.into();
        table_request["offset"] = 0.into();
        assert!(table(&api, table_request)
            .await
            .unwrap_err()
            .contains("budget"));
        let mut pdf_request = target.clone();
        pdf_request["id"] = "new-document".into();
        pdf_request["page"] = 0.into();
        pdf_request["password"] = "x".repeat(513).into();
        assert!(pdf(&api, pdf_request)
            .await
            .unwrap_err()
            .contains("Invalid PDF"));
        let mut book_request = target;
        book_request["id"] = "new-book".into();
        book_request["reading"] = json!({"root":"/other","path":"/private/file","bookHash":"a".repeat(64),"cfi":"","chapterLabel":"","progress":0,"fontScale":1,"theme":"light","flow":"scrolled","tocOpen":false});
        assert!(epub(&api, book_request)
            .await
            .unwrap_err()
            .contains("another book"));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), listener.accept())
                .await
                .is_err()
        );
    }
    #[test]
    fn identical_contents_do_not_authorize_a_different_canonical_viewer_target() {
        let snapshot = Target {
            root: "/private".into(),
            path: "/private/original".into(),
            hash: "content".into(),
        };
        assert!(snapshot.matches("content", "/private/original").is_ok());
        assert!(snapshot.matches("content", "/private/retargeted").is_err());
        assert!(snapshot
            .matches("new-content", "/private/original")
            .is_err());
    }
}

#[derive(Deserialize)]
struct Epub {
    #[serde(flatten)]
    target: Target,
    id: String,
    reading: epub::Reading,
}
pub async fn epub(api: &Api, body: Value) -> Result<epub::Reply, String> {
    let request: Epub = decode(body)?;
    request.target.validate()?;
    if request.id.is_empty() || request.id.len() > 64 {
        return Err("Invalid EPUB recovery target".into());
    }
    if request.reading.path != request.target.path
        || request.reading.root != request.target.root
        || request.reading.book_hash != request.target.hash
    {
        return Err("Reading state belongs to another book".into());
    }
    let id = request.id;
    let result=async {
        let epub::Reply::Open(mut book)=Box::pin(epub::execute(api,"open",json!({"operation":"open","id":id,"root":request.target.root,"path":request.target.path}))).await? else { unreachable!() };
        request.target.matches(&book.hash,&book.path)?;
        if book.document_id!=id {return Err("Recovered EPUB belongs to another view".into());}
        if book.root!=request.target.root { return Err("Book root changed while disconnected".into()); }
        book.saved=Some(request.reading);
        book.validate()?;
        Ok(epub::Reply::Open(book))
    }.await;
    if result.is_err() {
        let _ = Box::pin(epub::execute(
            api,
            "close",
            json!({"operation":"close","id":id}),
        ))
        .await;
    }
    result
}
