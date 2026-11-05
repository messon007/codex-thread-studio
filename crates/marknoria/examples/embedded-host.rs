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
    let image_reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut host_images = false;
    if args.iter().any(|a| a == "--indexed") { options = options.projection(reader::ProjectionProfile::Indexed); }
    for argument in &args {
        if let Some(identity) = argument.strip_prefix("--resource-id=") { options = options.resource_id(identity.into()); }
        if let Some(policy) = argument.strip_prefix("--host-image-policy=") {
            let policy = PathBuf::from(policy);
            let reads = image_reads.clone();
            host_images = true;
            options = options.reference_image_loader(move |target,hint,limit,cancelled| {
                use std::io::Read;
                use std::sync::atomic::Ordering;
                let mut encoded=Vec::new();
                std::fs::File::open(&policy)?.take(128*1024+1).read_to_end(&mut encoded)?;
                if encoded.len()>128*1024 || cancelled.load(Ordering::Relaxed) { return Err(io::Error::other("Host policy cancelled or exceeds budget")); }
                let value:serde_json::Value=serde_json::from_slice(&encoded).map_err(io::Error::other)?;
                if value["allowed"]!=true || value["target"].as_str()!=Some(target) {
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied,"Host image grant was revoked"));
                }
                let path=PathBuf::from(value["path"].as_str().ok_or_else(||io::Error::other("Missing host image path"))?);
                let identity=value["identity"].as_str().ok_or_else(||io::Error::other("Missing host image identity"))?.to_owned();
                let metadata=std::fs::metadata(&path)?;
                if !metadata.is_file() || metadata.len()>limit { return Err(io::Error::other("Host image exceeds its byte budget")); }
                let bytes=if hint==Some(identity.as_str()) { None } else {
                    let mut bytes=Vec::new();
                    std::fs::File::open(&path)?.take(limit+1).read_to_end(&mut bytes)?;
                    if cancelled.load(Ordering::Relaxed) { return Err(io::Error::other("Host image cancelled")); }
                    reads.fetch_add(1,Ordering::Relaxed);Some(bytes)
                };
                Ok(marknoria::web::HostImage { name:value["name"].as_str().map(str::to_owned).unwrap_or_else(||path.file_name().unwrap_or_default().to_string_lossy().into()),identity,bytes })
            });
        }
        if let Some(origin) = argument.strip_prefix("--parent=") {
            options = options.parent_origin(origin)?;
        }
        if let Some(directory) = argument.strip_prefix("--reference-root=") { options = options.reference_root(directory)?; }
        if let Some(path) = argument.strip_prefix("--reference-roots-file=") {
            let path = PathBuf::from(path);
            options = options.reference_roots_provider(move || {
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(&path)?.take(128 * 1024 + 1).read_to_end(&mut bytes)?;
                if bytes.len() > 128 * 1024 { return Err(io::Error::new(io::ErrorKind::InvalidData,"Reference policy exceeds 128 KiB")); }
                serde_json::from_slice(&bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData,error))
            });
        }
        if let Some(directory) = argument.strip_prefix("--notes=") {
            options = options.notes_directory(PathBuf::from(directory));
        }
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let service = if args.iter().any(|argument| argument == "--materialized") {
        use std::io::Read;
        if write { return Err(io::Error::other("Materialized byte services are read-only")); }
        let name = grant.path().file_name().unwrap().to_string_lossy().into_owned();
        let mut bytes = Vec::new();
        std::fs::File::open(grant.path())?.take(128*1024*1024+1).read_to_end(&mut bytes)?;
        runtime.block_on(Service::from_bytes(name, bytes, options))?
    } else { runtime.block_on(Service::start_with_options(grant, options))? };
    println!("{}", service.url());
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    runtime.block_on(service.close())?;
    if host_images { eprintln!("HOST_IMAGE_SNAPSHOT_READS={}", image_reads.load(std::sync::atomic::Ordering::Relaxed)); }
    Ok(())
}
