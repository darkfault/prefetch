use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;
mod gguf_provider;

#[derive(Parser)]
#[command(
    name = "prefetch",
    about = "Intelligent file prefetching with format-aware byte range ordering",
    version,
    long_about = "prefetch pre-warms files into the OS page cache by advising the kernel \
                  which byte ranges will be needed. It understands file formats (GGUF, custom manifests) \
                  and loads segments in optimal order. Works with LLM models, databases, game assets, \
                  or any large file."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, global = true, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Warm a file into the page cache
    Warm {
        /// File path or Ollama model name (e.g., "llama3:latest")
        model: String,

        /// Prefetch strategy: inference-order, sequential, first-n-layers
        #[arg(long, default_value = "inference-order")]
        strategy: String,

        /// Number of segments for first-n-layers strategy
        #[arg(long)]
        layers: Option<u32>,

        /// Use low IO priority (recommended for background warming)
        #[arg(long, default_value_t = true)]
        low_priority: bool,

        /// Force prefetch even if free memory is low
        #[arg(long)]
        force: bool,
    },

    /// Show page cache status of files
    Status {
        /// File path or Ollama model name.
        /// If omitted, shows all discovered Ollama models.
        model: Option<String>,
    },

    /// Analyze a file's internal structure and segment layout
    Analyze {
        /// File path or Ollama model name
        model: String,
    },

    /// Run as background daemon
    Daemon {
        /// Run in foreground instead of daemonizing
        #[arg(long)]
        foreground: bool,
    },

    /// Show or generate configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Discover available Ollama models
    Discover,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print the current configuration
    Show,
    /// Print an example configuration file
    Example,
    /// Show the config file path
    Path,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let filter = EnvFilter::try_new(&cli.log_level)
        .unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Warm {
            model, strategy, layers, low_priority, force,
        } => commands::warm::run(&model, &strategy, layers, low_priority, force),

        Commands::Status { model } => commands::status::run(model.as_deref()),

        Commands::Analyze { model } => commands::analyze::run(&model),

        Commands::Daemon { foreground: _ } => {
            let config = prefetch_config::AppConfig::load()?;
            prefetch_daemon::run_daemon(config).await
        }

        Commands::Config { action } => match action {
            ConfigAction::Show => commands::config::show(),
            ConfigAction::Example => commands::config::example(),
            ConfigAction::Path => commands::config::path(),
        },

        Commands::Discover => commands::discover::run(),
    }
}
