//! Prepare an isolated native shelf, outside measured startup time.
use elmapa::{
    library::{AtlasRecord, Camera, Library},
    performance_fixtures::fixture,
};
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let root = std::path::PathBuf::from(
        args.get(1)
            .ok_or("Usage: prepare_performance_library ROOT COUNT WORKLOAD [SIZE] [--warm]")?,
    );
    let count: usize = args
        .get(2)
        .ok_or("Missing count")?
        .parse()
        .map_err(|_| "Invalid count")?;
    if ![1, 16, 128].contains(&count) {
        return Err("Use 1, 16, or 128 records.".into());
    }
    if root.exists()
        && std::fs::read_dir(&root)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err("Fixture destination must be empty; existing libraries are preserved.".into());
    }
    let name = args.get(3).map(String::as_str).unwrap_or("demo");
    let size = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1);
    let document = fixture(name, size)?;
    let mut library = Library::load(root)?;
    for i in 1..=count {
        let record = AtlasRecord {
            id: i as u64,
            name: format!("{} {i}", document.name),
            source: None,
            camera: Camera::for_document(&document),
        };
        library.checkpoint(record.clone(), &document)?;
        if args.iter().any(|s| s == "--warm") {
            library.preview(&record)?;
        }
    }
    println!("Prepared {count} isolated {name} atlases, size {size}.");
    Ok(())
}
