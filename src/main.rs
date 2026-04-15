mod app;
mod provider;
mod session;
mod tools;
mod ui;

use anyhow::Context;
use clap::{Parser, ValueEnum};
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
    #[arg(short, long, default_value = "anthropic")]
    provider: ProviderArg,

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

    let provider: Arc<dyn Provider> = match cli.provider {
        ProviderArg::Anthropic => {
            let key = cli
                .api_key
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .context("ANTHROPIC_API_KEY not set")?;
            let model = cli.model.unwrap_or_else(|| "claude-sonnet-4-6".to_string());
            Arc::new(AnthropicProvider::new(key, model))
        }
        ProviderArg::Openai => {
            let key = cli
                .api_key
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .context("OPENAI_API_KEY not set")?;
            let model = cli.model.unwrap_or_else(|| "gpt-4o".to_string());
            Arc::new(OpenAIProvider::new(key, model))
        }
        ProviderArg::Ollama => {
            let model = cli.model.unwrap_or_else(|| "llama3.2".to_string());
            Arc::new(OllamaProvider::new(model))
        }
    };

    let mut app = app::App::new(provider);

    if let Some(id) = cli.resume {
        let s = session::load(&id).context(format!("session {id} not found"))?;
        app = app.with_session(s);
    }

    app.run().await
}
