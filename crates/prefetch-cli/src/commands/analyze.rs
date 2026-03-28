use std::path::PathBuf;

use prefetch_core::providers::ProviderRegistry;

use crate::gguf_provider::GgufProvider;

fn resolve_path(model: &str) -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(model);
    if path.exists() { return Ok(path); }
    let models = prefetch_config::discovery::discover_ollama_models();
    if let Some(found) = models.iter().find(|m| m.name == model || m.name.starts_with(model)) {
        return Ok(found.path.clone());
    }
    anyhow::bail!("file not found: '{model}'")
}

pub fn run(model: &str) -> anyhow::Result<()> {
    let path = resolve_path(model)?;

    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(GgufProvider));

    match registry.analyze(&path) {
        Some(layout) => {
            println!("File:    {}", path.display());
            println!("Format:  {}", layout.format_name);
            println!("Size:    {:.1} GB ({} bytes)", layout.file_size as f64 / 1e9, layout.file_size);
            println!();

            if !layout.metadata.is_empty() {
                println!("Metadata:");
                let mut keys: Vec<_> = layout.metadata.keys().collect();
                keys.sort();
                for key in keys {
                    println!("  {}: {}", key, layout.metadata[key]);
                }
                println!();
            }

            println!("Segments ({}):", layout.segments.len());
            println!("  {:<25} {:>10}  {:>8}  {}", "NAME", "SIZE", "PRIORITY", "OFFSET");
            println!("  {}", "-".repeat(65));
            for seg in layout.ordered_segments() {
                println!(
                    "  {:<25} {:>8.1} MB  {:>8}  0x{:X}",
                    seg.name,
                    seg.length as f64 / (1024.0 * 1024.0),
                    seg.priority,
                    seg.offset,
                );
            }
            println!();
            println!(
                "Total segment data: {:.1} MB",
                layout.total_segment_bytes() as f64 / (1024.0 * 1024.0)
            );
        }
        None => {
            println!("No format provider recognized: {}", path.display());
            println!();
            println!("The file can still be warmed with sequential prefetching:");
            println!("  prefetch warm {} --strategy sequential --force", path.display());
            println!();
            println!("Or create a manifest file to define its structure:");
            println!("  {}.prefetch.toml", path.display());
        }
    }

    Ok(())
}
