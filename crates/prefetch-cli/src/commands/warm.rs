use std::path::PathBuf;

use prefetch_core::prefetch::engine::PrefetchEngine;
use prefetch_core::prefetch::strategy::PrefetchStrategy;

/// Resolve a model argument to a file path.
/// Accepts either a direct file path or an Ollama model name.
fn resolve_model_path(model: &str) -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(model);
    if path.exists() {
        return Ok(path);
    }

    // Try to resolve as Ollama model name
    let models = prefetch_config::discovery::discover_ollama_models();
    if let Some(found) = models.iter().find(|m| m.name == model) {
        return Ok(found.path.clone());
    }

    // Try partial match
    if let Some(found) = models.iter().find(|m| m.name.starts_with(model)) {
        tracing::info!(resolved = %found.name, "matched Ollama model");
        return Ok(found.path.clone());
    }

    anyhow::bail!(
        "model not found: '{model}'. Provide a file path or Ollama model name.\n\
         Discovered Ollama models: {}",
        if models.is_empty() {
            "none".to_string()
        } else {
            models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")
        }
    )
}

pub fn run(model: &str, strategy: &str, layers: Option<u32>, low_priority: bool, force: bool) -> anyhow::Result<()> {
    use prefetch_core::prefetch::strategy::MemoryBudget;

    let path = resolve_model_path(model)?;
    let strategy = PrefetchStrategy::from_str_with_layers(strategy, layers)?;

    let file_size = std::fs::metadata(&path)?.len();
    println!(
        "🔥 Warming: {} ({:.1} GB)",
        path.display(),
        file_size as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!("   Strategy: {strategy}");
    if force {
        println!("   Force: skipping memory budget checks");
    }

    let budget = MemoryBudget {
        force,
        ..MemoryBudget::default()
    };
    let mut engine = PrefetchEngine::with_config(budget, 64);
    engine.register_provider(Box::new(crate::gguf_provider::GgufProvider));

    if low_priority {
        if let Err(e) = engine.set_low_priority() {
            tracing::debug!(error = %e, "could not set low IO priority");
        }
    }

    let mut last_percent = 0u32;
    let result = engine.prefetch_model(&path, &strategy, |progress| {
        let percent = progress.percent() as u32;
        if percent > last_percent {
            last_percent = percent;
            print!(
                "\r   Progress: {:>3}% | {:.0} MB/s | {}",
                percent,
                progress.throughput_mbps(),
                progress.current_layer,
            );
            // Flush without newline
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    })?;

    println!(); // End the progress line

    // Print cache status
    let status = &result.cache_status;
    println!();
    println!("   Cache status: {:.1}% resident ({:.1} MB / {:.1} MB)",
        status.cached_percent(),
        status.cached_bytes() as f64 / (1024.0 * 1024.0),
        status.file_size as f64 / (1024.0 * 1024.0),
    );

    if !status.layer_status.is_empty() {
        println!();
        println!("   Per-layer breakdown:");
        for layer in &status.layer_status {
            let bar = make_bar(layer.cached_percent(), 20);
            println!(
                "     {:<20} {bar} {:>5.1}%  ({:.1} MB)",
                layer.layer_name,
                layer.cached_percent(),
                layer.total_bytes as f64 / (1024.0 * 1024.0),
            );
        }
    }

    if result.budget_limited {
        println!();
        println!("   ⚠ Prefetch stopped early due to memory budget limit");
    }

    let elapsed = result.progress.elapsed;
    println!();
    println!(
        "   Completed in {:.2}s ({:.0} MB/s)",
        elapsed.as_secs_f64(),
        result.progress.throughput_mbps(),
    );

    Ok(())
}

/// Create a simple ASCII progress bar.
fn make_bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}
