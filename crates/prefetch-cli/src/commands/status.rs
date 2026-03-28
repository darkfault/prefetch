use std::path::PathBuf;

use prefetch_core::prefetch::engine::PrefetchEngine;

pub fn run(model: Option<&str>) -> anyhow::Result<()> {
    let mut engine = PrefetchEngine::new();
    engine.register_provider(Box::new(crate::gguf_provider::GgufProvider));

    let models: Vec<(String, PathBuf)> = if let Some(model) = model {
        let path = PathBuf::from(model);
        if path.exists() {
            vec![(path.display().to_string(), path)]
        } else {
            // Try Ollama resolution
            let discovered = prefetch_config::discovery::discover_ollama_models();
            match discovered.iter().find(|m| m.name == model || m.name.starts_with(model)) {
                Some(m) => vec![(m.name.clone(), m.path.clone())],
                None => anyhow::bail!("model not found: '{model}'"),
            }
        }
    } else {
        // Show all discovered Ollama models
        let discovered = prefetch_config::discovery::discover_ollama_models();
        if discovered.is_empty() {
            println!("No Ollama models found. Provide a model path:");
            println!("  prefetch status /path/to/model.gguf");
            return Ok(());
        }
        discovered.into_iter().map(|m| (m.name, m.path)).collect()
    };

    for (name, path) in &models {
        match engine.cache_status(path) {
            Ok(status) => {
                let bar = make_bar(status.cached_percent(), 30);
                println!(
                    "{:<30} {bar} {:>5.1}%  ({:.1} GB)",
                    name,
                    status.cached_percent(),
                    status.file_size as f64 / (1024.0 * 1024.0 * 1024.0),
                );

                // Show per-layer if only one model
                if models.len() == 1 && !status.layer_status.is_empty() {
                    println!();
                    for layer in &status.layer_status {
                        let bar = make_bar(layer.cached_percent(), 20);
                        println!(
                            "  {:<22} {bar} {:>5.1}%  ({:.1} MB)",
                            layer.layer_name,
                            layer.cached_percent(),
                            layer.total_bytes as f64 / (1024.0 * 1024.0),
                        );
                    }
                }
            }
            Err(e) => {
                println!("{:<30} error: {e}", name);
            }
        }
    }

    Ok(())
}

fn make_bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}
