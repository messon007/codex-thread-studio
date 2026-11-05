//! A complete single-file consumer with no HTTP, GUI or AI account.
//! cargo run --locked --no-default-features --example reader-host -- public.csv
use marknoria::reader::{FileGrant, Options, Reader, worker_entry};
use std::{io, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = worker_entry() {
        return result.map_err(Into::into);
    }
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("Pass one explicitly authorized document"))?;
    let mut reader = Reader::open(FileGrant::read_only(path)?, Options::default())?;
    let metadata = reader.descriptor();
    println!("{}", serde_json::to_string(&metadata)?);
    let first = reader.window(&metadata.revision, 0, 1)?;
    println!(
        "{}",
        serde_json::json!({"firstWindowPages": first.pages.len(), "totalPages": first.total_pages})
    );
    if metadata.capabilities.sql {
        let operation = reader.operations().begin()?;
        let schema = reader.table_schema(&metadata.revision, 0)?;
        let page = reader.sql(&metadata.revision, 0, &schema.default_sql, &operation)?;
        println!(
            "{}",
            serde_json::json!({"queryPageRows": page.rows.len(), "totalResultRows": page.total_rows, "datasetScope": "complete-input"})
        );
    }
    if metadata.capabilities.jq {
        let operation = reader.operations().begin()?;
        let page = reader.jq(&metadata.revision, ".", false, &operation)?;
        println!(
            "{}",
            serde_json::json!({"queryType":"jq", "totalValues":page.total_values, "resultPages":page.pages, "firstPageBytes":page.text.len()})
        );
    }
    reader.close()?;
    Ok(())
}
