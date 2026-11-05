//! A minimal owner of one file-scoped service; stdin newline closes it cleanly.
use marknoria::{
    reader,
    web::{DiagramMode, Service, ServiceOptions},
};
use std::{io, path::PathBuf};
fn main() -> io::Result<()> {
    if let Some(result) = reader::worker_entry() {
        return result;
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    let path = PathBuf::from(
        args.first()
            .ok_or_else(|| io::Error::other("Pass an authorized file"))?,
    );
    let write = args.iter().any(|a| a == "--write");
    let grant = if write {
        reader::FileGrant::read_write(path)?
    } else {
        reader::FileGrant::read_only(path)?
    };
    let mut options = ServiceOptions::default()
        .queries(!args.iter().any(|a| a == "--no-queries"))
        .diagrams(DiagramMode::Browser);
    if args.iter().any(|a| a == "--indexed") { options = options.projection(reader::ProjectionProfile::Indexed); }
    for argument in &args {
        if let Some(origin) = argument.strip_prefix("--parent=") {
            options = options.parent_origin(origin)?;
        }
        if let Some(directory) = argument.strip_prefix("--notes=") {
            options = options.notes_directory(PathBuf::from(directory));
        }
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let service = runtime.block_on(Service::start_with_options(grant, options))?;
    println!("{}", service.url());
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    runtime.block_on(service.close())
}
