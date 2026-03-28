use prefetch_config::discovery;

pub fn run() -> anyhow::Result<()> {
    let models = discovery::discover_ollama_models();

    if models.is_empty() {
        println!("No Ollama models discovered.");
        println!();
        println!("Checked: {}", prefetch_config::paths::ollama_models_dir().display());
        println!();
        println!("To warm a model file directly:");
        println!("  prefetch warm /path/to/model.gguf");
        return Ok(());
    }

    println!("Discovered {} Ollama model(s):", models.len());
    println!();
    println!("  {:<30} {:>10}   {}", "NAME", "SIZE", "PATH");
    println!("  {}", "-".repeat(80));

    for model in &models {
        println!(
            "  {:<30} {:>8.1} GB   {}",
            model.name,
            model.size as f64 / (1024.0 * 1024.0 * 1024.0),
            model.path.display(),
        );
    }

    println!();
    println!("To warm a model:  prefetch warm <name>");
    println!("To check status:  prefetch status <name>");

    Ok(())
}
