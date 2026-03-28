use prefetch_config::{paths, AppConfig};

pub fn show() -> anyhow::Result<()> {
    let config = AppConfig::load()?;
    let toml_str = toml::to_string_pretty(&config)?;
    println!("{toml_str}");
    Ok(())
}

pub fn example() -> anyhow::Result<()> {
    println!("{}", AppConfig::example_toml());
    Ok(())
}

pub fn path() -> anyhow::Result<()> {
    let path = paths::config_path();
    println!("{}", path.display());
    if path.exists() {
        println!("(exists)");
    } else {
        println!("(not created yet — run `prefetch config example > {}`)", path.display());
    }
    Ok(())
}
