mod app;
mod config;
mod provider;
mod session;
mod tools;
mod ui;

use app::App;
use clap::{Parser, ValueEnum};
use config::Config;
use provider::{
    anthropic::AnthropicProvider, ollama::OllamaProvider, openai::OpenAIProvider, Provider,
};
use std::sync::Arc;

#[derive(Debug, Clone, ValueEnum)]
enum ProviderArg {
    Anthropic,
    Openai,
    Ollama,
}

#[derive(Parser, Debug)]
#[command(name = "bcode", about = "Terminal AI coding agent")]
struct Cli {
    #[arg(short, long)]
    provider: Option<ProviderArg>,

    #[arg(short, long)]
    model: Option<String>,

    #[arg(short, long)]
    api_key: Option<String>,

    #[arg(long, help = "Resume a saved session by ID")]
    resume: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load();

    let provider = try_build_provider(&cli, &config);

    let mut app = match provider {
        Some(p) => App::new(p),
        None    => App::needs_setup(),
    };

    if let Some(id) = cli.resume {
        if let Ok(s) = session::load(&id) {
            app = app.with_session(s);
        }
    }

    app.run().await
}

fn try_build_provider(cli: &Cli, config: &Config) -> Option<Arc<dyn Provider>> {
    let provider_name = cli.provider.as_ref()
        .map(|p| match p { ProviderArg::Anthropic => "anthropic", ProviderArg::Openai => "openai", ProviderArg::Ollama => "ollama" })
        .or_else(|| config.provider.as_deref())
        .unwrap_or("anthropic");

    let model = cli.model.clone()
        .or_else(|| config.model.clone())
        .unwrap_or_else(|| default_model(provider_name).to_string());

    match provider_name {
        "openai" => {
            let key = cli.api_key.clone()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .or_else(|| config.api_keys.get("openai").cloned())?;
            Some(Arc::new(OpenAIProvider::new(key, model)))
        }
        "ollama" => {
            Some(Arc::new(OllamaProvider::new(model)))
        }
        _ => {
            let key = cli.api_key.clone()
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .or_else(|| config.api_keys.get("anthropic").cloned())?;
            Some(Arc::new(AnthropicProvider::new(key, model)))
        }
    }
}

fn default_model(provider: &str) -> &'static str {
    match provider {
        "openai" => "gpt-4o",
        "ollama" => "llama3.2",
        _        => "claude-sonnet-4-6",
    }
}
