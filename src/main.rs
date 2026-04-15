mod app;
mod checkpoint;
mod config;
mod markdown;
mod provider;
mod session;
mod tools;
mod ui;

use app::App;
use clap::{Parser, ValueEnum};
use config::Config;
use provider::{
    anthropic::AnthropicProvider,
    gemini::GeminiProvider,
    ollama::OllamaProvider,
    openai::OpenAIProvider,
    Provider,
};
use std::sync::Arc;

#[derive(Debug, Clone, ValueEnum)]
enum ProviderArg {
    Anthropic,
    Openai,
    Ollama,
    Gemini,
}

#[derive(Parser, Debug)]
#[command(name = "bcode", about = "Terminal AI coding agent", version)]
struct Cli {
    #[arg(short, long)]
    provider: Option<ProviderArg>,

    #[arg(short, long)]
    model: Option<String>,

    #[arg(short, long)]
    api_key: Option<String>,

    #[arg(long, help = "Base URL for OpenAI-compatible endpoints")]
    base_url: Option<String>,

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
        None => App::needs_setup(),
    };

    if let Some(id) = cli.resume {
        if let Ok(s) = session::load(&id) {
            app = app.with_session(s);
        }
    }

    for ctx in load_context_files() {
        app.prepend_system(ctx);
    }

    app.run().await
}

fn try_build_provider(cli: &Cli, config: &Config) -> Option<Arc<dyn Provider>> {
    let provider_name = cli
        .provider
        .as_ref()
        .map(|p| match p {
            ProviderArg::Anthropic => "anthropic",
            ProviderArg::Openai => "openai",
            ProviderArg::Ollama => "ollama",
            ProviderArg::Gemini => "gemini",
        })
        .or_else(|| config.provider.as_deref())
        .unwrap_or("anthropic");

    let model = cli
        .model
        .clone()
        .or_else(|| config.model.clone())
        .unwrap_or_else(|| default_model(provider_name).to_string());

    match provider_name {
        "openai" => {
            let key = cli
                .api_key
                .clone()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .or_else(|| config.api_keys.get("openai").cloned())?;
            let base = cli
                .base_url
                .clone()
                .or_else(|| config.base_urls.get("openai").cloned());
            Some(Arc::new(OpenAIProvider::new(key, model, base)))
        }
        "ollama" => Some(Arc::new(OllamaProvider::new(model))),
        "gemini" => {
            let key = cli
                .api_key
                .clone()
                .or_else(|| std::env::var("GEMINI_API_KEY").ok())
                .or_else(|| config.api_keys.get("gemini").cloned())?;
            Some(Arc::new(GeminiProvider::new(key, model)))
        }
        _ => {
            let key = cli
                .api_key
                .clone()
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
        "gemini" => "gemini-2.0-flash",
        _ => "claude-sonnet-4-6",
    }
}

fn load_context_files() -> Vec<String> {
    let mut contexts = Vec::new();

    let mut dir = std::env::current_dir().ok();
    while let Some(d) = dir {
        let bcode_md = d.join("BCODE.md");
        if bcode_md.exists() {
            if let Ok(content) = std::fs::read_to_string(&bcode_md) {
                contexts.push(format!("Project context from BCODE.md:\n\n{content}"));
            }
            break;
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }

    if let Some(home) = directories::BaseDirs::new() {
        let user_md = home.home_dir().join(".bcode").join("user.md");
        if user_md.exists() {
            if let Ok(content) = std::fs::read_to_string(&user_md) {
                contexts.push(format!("User preferences from ~/.bcode/user.md:\n\n{content}"));
            }
        }
    }

    contexts
}
