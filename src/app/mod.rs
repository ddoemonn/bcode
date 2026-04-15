mod state;
pub use state::{PendingCall, SetupStep, Status, TokenUsage};

use crate::config::Config;
use crate::provider::{
    anthropic::AnthropicProvider, ollama::OllamaProvider, openai::OpenAIProvider, Message,
    Provider, StreamEvent,
};
use crate::session::{self, Session, SessionMeta};
use crate::tools;
use async_trait::async_trait;
use chrono::Utc;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{collections::HashSet, io, sync::Arc};
use tokio::sync::mpsc;

pub const PROVIDERS: &[(&str, &str, &str, &str)] = &[
    ("anthropic", "Anthropic", "claude-sonnet-4-6", "console.anthropic.com"),
    ("openai", "OpenAI", "gpt-4o", "platform.openai.com/api-keys"),
    ("ollama", "Ollama", "llama3.2", "no key needed — runs locally"),
    ("gemini", "Gemini", "gemini-2.0-flash", "aistudio.google.com/apikey"),
];

struct NoProvider;

#[async_trait]
impl Provider for NoProvider {
    fn name(&self) -> &str { "none" }
    fn model(&self) -> &str { "none" }
    async fn stream(&self, _: &[Message], _: mpsc::Sender<StreamEvent>) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct App {
    pub messages: Vec<Message>,
    pub streaming_text: String,
    pub status: Status,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll: u16,
    pub auto_scroll: bool,
    pub tokens: TokenUsage,
    pub diff_content: String,
    pub provider: Arc<dyn Provider>,
    pub pending_calls: Vec<PendingCall>,
    pub current_call_idx: usize,
    pub always_allowed: HashSet<String>,
    pub session_id: String,
    pub session_list: Vec<SessionMeta>,
    pub session_selected: usize,
    pub setup_selected: usize,
    pub setup_provider: String,
    pub setup_api_key: String,
    pub input_history: Vec<String>,
    pub history_idx: Option<usize>,
    pub checkpoint_stack: Vec<usize>,
    pub redo_stack: Vec<usize>,
    pub turn_counter: usize,
    pub session_search: String,
    stream_task: Option<tokio::task::JoinHandle<()>>,
}

impl App {
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        let cfg = Config::load();
        let always_allowed: HashSet<String> =
            cfg.always_allowed_tools.into_iter().collect();
        Self {
            messages: Vec::new(),
            streaming_text: String::new(),
            status: Status::Ready,
            input: String::new(),
            cursor_pos: 0,
            scroll: u16::MAX,
            auto_scroll: true,
            tokens: TokenUsage::new(),
            diff_content: String::new(),
            provider,
            pending_calls: Vec::new(),
            current_call_idx: 0,
            always_allowed,
            session_id: session::new_id(),
            session_list: Vec::new(),
            session_selected: 0,
            setup_selected: 0,
            setup_provider: String::new(),
            setup_api_key: String::new(),
            input_history: Vec::new(),
            history_idx: None,
            checkpoint_stack: Vec::new(),
            redo_stack: Vec::new(),
            turn_counter: 0,
            session_search: String::new(),
            stream_task: None,
        }
    }

    pub fn needs_setup() -> Self {
        let mut app = Self::new(Arc::new(NoProvider));
        app.status = Status::Setup(SetupStep::ChooseProvider);
        app
    }

    pub fn with_session(mut self, s: Session) -> Self {
        self.messages = s.messages;
        self.session_id = s.id;
        self
    }

    pub fn prepend_system(&mut self, content: String) {
        use crate::provider::{Content, Role};
        self.messages.insert(0, Message { role: Role::System, content: Content::Text(content) });
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        let result = self.event_loop(&mut terminal).await;
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        result
    }

    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> anyhow::Result<()> {
        let (stream_tx, mut stream_rx) = mpsc::channel::<StreamEvent>(256);
        let mut events = EventStream::new();

        let shutdown_rx = spawn_shutdown_listener();

        loop {
            terminal.draw(|f| crate::ui::render(f, self))?;
            self.trim_message_history();

            tokio::select! {
                Some(Ok(event)) = events.next() => {
                    if self.handle_event(event, stream_tx.clone()).await? {
                        break;
                    }
                }
                Some(ev) = stream_rx.recv() => {
                    self.handle_stream(ev, stream_tx.clone());
                }
                _ = shutdown_rx.notified() => {
                    self.autosave();
                    break;
                }
            }
        }

        Ok(())
    }

    fn trim_message_history(&mut self) {
        let max = Config::load().max_messages.unwrap_or(usize::MAX);
        let non_system: usize = self
            .messages
            .iter()
            .filter(|m| !matches!(m.role, crate::provider::Role::System))
            .count();

        if non_system > max {
            let to_drop = non_system - max;
            let mut dropped = 0;
            self.messages.retain(|m| {
                if dropped < to_drop && !matches!(m.role, crate::provider::Role::System) {
                    dropped += 1;
                    false
                } else {
                    true
                }
            });
        }
    }

    async fn handle_event(
        &mut self,
        event: Event,
        stream_tx: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<bool> {
        let Event::Key(key) = event else { return Ok(false) };

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => {
                    match &self.status {
                        Status::Streaming | Status::AwaitingPermission | Status::Executing => {
                            self.interrupt();
                            return Ok(false);
                        }
                        _ => return Ok(true),
                    }
                }
                KeyCode::Char('r') if self.status == Status::Ready => {
                    self.open_session_browser();
                    return Ok(false);
                }
                KeyCode::Char('u') if matches!(self.status, Status::Ready | Status::Setup(_)) => {
                    self.input.clear();
                    self.cursor_pos = 0;
                }
                _ => {}
            }
            return Ok(false);
        }

        match self.status.clone() {
            Status::Setup(step) => self.handle_setup_key(key.code, step),
            Status::SessionBrowser => match key.code {
                KeyCode::Up => {
                    self.session_selected = self.session_selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    let max = self.session_list.len().saturating_sub(1);
                    self.session_selected = (self.session_selected + 1).min(max);
                }
                KeyCode::Enter => self.load_selected_session(),
                KeyCode::Esc => {
                    self.status = Status::Ready;
                }
                KeyCode::Char(c) => {
                    self.session_search.push(c);
                    self.session_list = session::list_filtered(&self.session_search);
                    self.session_selected = 0;
                }
                KeyCode::Backspace => {
                    self.session_search.pop();
                    self.session_list = session::list_filtered(&self.session_search);
                    self.session_selected = 0;
                }
                _ => {}
            },
            Status::AwaitingPermission => match key.code {
                KeyCode::Char('y') => self.resolve_permission(false, stream_tx),
                KeyCode::Char('a') => self.resolve_permission(true, stream_tx),
                KeyCode::Char('n') => self.deny_permission(stream_tx),
                _ => {}
            },
            Status::Streaming | Status::Executing => {}
            _ => match key.code {
                KeyCode::Enter => {
                    if self.input.trim().starts_with('/') {
                        self.handle_slash_command(stream_tx);
                    } else {
                        self.submit(stream_tx);
                    }
                }
                KeyCode::Char(c) => {
                    self.history_idx = None;
                    self.input.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                }
                KeyCode::Backspace => {
                    if self.cursor_pos > 0 {
                        self.history_idx = None;
                        self.input.remove(self.cursor_pos - 1);
                        self.cursor_pos -= 1;
                    }
                }
                KeyCode::Delete => {
                    if self.cursor_pos < self.input.len() {
                        self.input.remove(self.cursor_pos);
                    }
                }
                KeyCode::Left => {
                    self.cursor_pos = self.cursor_pos.saturating_sub(1);
                }
                KeyCode::Right => {
                    self.cursor_pos = (self.cursor_pos + 1).min(self.input.len());
                }
                KeyCode::Home => {
                    self.cursor_pos = 0;
                }
                KeyCode::End => {
                    self.cursor_pos = self.input.len();
                }
                KeyCode::Up => {
                    if self.input_history.is_empty() {
                        self.auto_scroll = false;
                        self.scroll = self.scroll.saturating_sub(3);
                    } else {
                        let next_idx = match self.history_idx {
                            None => self.input_history.len() - 1,
                            Some(0) => 0,
                            Some(i) => i - 1,
                        };
                        self.history_idx = Some(next_idx);
                        self.input = self.input_history[next_idx].clone();
                        self.cursor_pos = self.input.len();
                    }
                }
                KeyCode::Down => {
                    if let Some(idx) = self.history_idx {
                        if idx + 1 < self.input_history.len() {
                            self.history_idx = Some(idx + 1);
                            self.input = self.input_history[idx + 1].clone();
                            self.cursor_pos = self.input.len();
                        } else {
                            self.history_idx = None;
                            self.input.clear();
                            self.cursor_pos = 0;
                        }
                    } else {
                        self.scroll = self.scroll.saturating_add(3);
                        if self.scroll >= u16::MAX - 3 {
                            self.auto_scroll = true;
                            self.scroll = u16::MAX;
                        }
                    }
                }
                _ => {}
            },
        }

        Ok(false)
    }

    fn handle_setup_key(&mut self, code: KeyCode, step: SetupStep) {
        match step {
            SetupStep::ChooseProvider => match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.setup_selected = self.setup_selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.setup_selected = (self.setup_selected + 1).min(PROVIDERS.len() - 1);
                }
                KeyCode::Char('1') => {
                    self.setup_selected = 0;
                    self.advance_setup();
                }
                KeyCode::Char('2') => {
                    self.setup_selected = 1;
                    self.advance_setup();
                }
                KeyCode::Char('3') => {
                    self.setup_selected = 2;
                    self.advance_setup();
                }
                KeyCode::Char('4') => {
                    self.setup_selected = 3;
                    self.advance_setup();
                }
                KeyCode::Enter => self.advance_setup(),
                _ => {}
            },
            SetupStep::EnterApiKey | SetupStep::EnterModel => match code {
                KeyCode::Enter => self.advance_setup(),
                KeyCode::Char(c) => {
                    self.input.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                }
                KeyCode::Backspace => {
                    if self.cursor_pos > 0 {
                        self.input.remove(self.cursor_pos - 1);
                        self.cursor_pos -= 1;
                    }
                }
                KeyCode::Delete => {
                    if self.cursor_pos < self.input.len() {
                        self.input.remove(self.cursor_pos);
                    }
                }
                KeyCode::Left => {
                    self.cursor_pos = self.cursor_pos.saturating_sub(1);
                }
                KeyCode::Right => {
                    self.cursor_pos = (self.cursor_pos + 1).min(self.input.len());
                }
                KeyCode::Home => {
                    self.cursor_pos = 0;
                }
                KeyCode::End => {
                    self.cursor_pos = self.input.len();
                }
                _ => {}
            },
        }
    }

    fn advance_setup(&mut self) {
        let (pname, _, default_model, _) = PROVIDERS[self.setup_selected];

        match self.status.clone() {
            Status::Setup(SetupStep::ChooseProvider) => {
                self.setup_provider = pname.to_string();
                self.input.clear();
                self.cursor_pos = 0;
                if pname == "ollama" {
                    self.input = default_model.to_string();
                    self.cursor_pos = self.input.len();
                    self.status = Status::Setup(SetupStep::EnterModel);
                } else {
                    self.status = Status::Setup(SetupStep::EnterApiKey);
                }
            }
            Status::Setup(SetupStep::EnterApiKey) => {
                self.setup_api_key = self.input.trim().to_string();
                let dm = PROVIDERS
                    .iter()
                    .find(|(n, _, _, _)| *n == self.setup_provider)
                    .map(|(_, _, m, _)| *m)
                    .unwrap_or(default_model);
                self.input = dm.to_string();
                self.cursor_pos = self.input.len();
                self.status = Status::Setup(SetupStep::EnterModel);
            }
            Status::Setup(SetupStep::EnterModel) => {
                let model = if self.input.trim().is_empty() {
                    PROVIDERS
                        .iter()
                        .find(|(n, _, _, _)| *n == self.setup_provider)
                        .map(|(_, _, m, _)| m.to_string())
                        .unwrap_or_else(|| default_model.to_string())
                } else {
                    self.input.trim().to_string()
                };

                self.provider = make_provider(&self.setup_provider, &self.setup_api_key, &model);

                let mut config = Config::load();
                config.provider = Some(self.setup_provider.clone());
                config.model = Some(model);
                if !self.setup_api_key.is_empty() {
                    config.api_keys.insert(self.setup_provider.clone(), self.setup_api_key.clone());
                }
                let _ = config.save();

                self.input.clear();
                self.cursor_pos = 0;
                self.status = Status::Ready;
            }
            _ => {}
        }
    }

    pub fn open_session_browser(&mut self) {
        self.session_search.clear();
        self.session_list = session::list();
        self.session_selected = 0;
        self.status = Status::SessionBrowser;
    }

    fn load_selected_session(&mut self) {
        let Some(meta) = self.session_list.get(self.session_selected) else { return };
        if let Ok(s) = session::load(&meta.id) {
            self.messages = s.messages;
            self.session_id = s.id;
            self.streaming_text.clear();
            self.auto_scroll = true;
            self.scroll = u16::MAX;
        }
        self.status = Status::Ready;
    }

    pub fn submit(&mut self, tx: mpsc::Sender<StreamEvent>) {
        let input = self.input.trim().to_string();
        if input.is_empty() { return; }
        if !input.starts_with('/') {
            self.input_history.push(input.clone());
        }
        self.history_idx = None;
        self.messages.push(Message::user(&input));
        self.input.clear();
        self.cursor_pos = 0;
        self.streaming_text.clear();
        self.status = Status::Streaming;
        self.auto_scroll = true;
        self.scroll = u16::MAX;
        self.spawn_stream(tx);
    }

    fn handle_slash_command(&mut self, tx: mpsc::Sender<StreamEvent>) {
        let raw = self.input.trim().to_string();
        self.input.clear();
        self.cursor_pos = 0;
        self.history_idx = None;

        let parts: Vec<&str> = raw.splitn(2, ' ').collect();
        let cmd = parts[0];
        let arg = parts.get(1).copied().unwrap_or("").trim();

        match cmd {
            "/help" => {
                let help = "\
/help           show this message
/clear          clear conversation
/model <name>   switch model
/compact        compress context
/sessions       open session browser
/config         show current config
/cost           show token usage
/undo           revert last agent turn
/redo           re-apply reverted turn";
                self.messages.push(Message::assistant(help));
            }
            "/clear" => {
                self.messages.retain(|m| matches!(m.role, crate::provider::Role::System));
                self.streaming_text.clear();
                self.diff_content.clear();
                self.tokens = TokenUsage::new();
                self.session_id = session::new_id();
            }
            "/model" => {
                if arg.is_empty() {
                    self.messages.push(Message::assistant(
                        format!("current model: {}", self.provider.model()),
                    ));
                } else {
                    let name = self.provider.name().to_string();
                    let key = Config::load()
                        .api_keys
                        .get(&name)
                        .cloned()
                        .unwrap_or_default();
                    self.provider = make_provider(&name, &key, arg);
                    self.messages.push(Message::assistant(format!("switched to {arg}")));
                }
            }
            "/compact" => {
                self.compact_context(tx);
                return;
            }
            "/sessions" => {
                self.open_session_browser();
                return;
            }
            "/config" => {
                let cfg = Config::load();
                let text = format!(
                    "provider: {}\nmodel: {}\nkeys: {}",
                    cfg.provider.as_deref().unwrap_or("(none)"),
                    cfg.model.as_deref().unwrap_or("(none)"),
                    cfg.api_keys.keys().cloned().collect::<Vec<_>>().join(", ")
                );
                self.messages.push(Message::assistant(text));
            }
            "/cost" => {
                let total = self.tokens.input + self.tokens.output;
                let pct = if self.tokens.max > 0 {
                    total * 100 / self.tokens.max
                } else {
                    0
                };
                let text = format!(
                    "input tokens:   {}\noutput tokens:  {}\ntotal:          {} / {} ({}%)",
                    self.tokens.input,
                    self.tokens.output,
                    total,
                    self.tokens.max,
                    pct
                );
                self.messages.push(Message::assistant(text));
            }
            "/tag" => {
                if arg.is_empty() {
                    if let Some(meta) = self.session_list.first() {
                        self.messages.push(Message::assistant(format!(
                            "tags for current session: {}",
                            if meta.tags.is_empty() {
                                "(none)".to_string()
                            } else {
                                meta.tags.join(", ")
                            }
                        )));
                    } else {
                        self.messages.push(Message::assistant("usage: /tag <name>"));
                    }
                } else {
                    let _ = session::tag(&self.session_id, arg);
                    self.messages.push(Message::assistant(format!("tagged session as \"{arg}\"")));
                }
            }
            "/undo" => {
                if let Some(turn) = self.checkpoint_stack.pop() {
                    match crate::checkpoint::restore(&self.session_id, turn) {
                        Ok(_) => {
                            self.redo_stack.push(turn);
                            self.messages.push(Message::assistant(
                                format!("reverted to checkpoint {turn}"),
                            ));
                        }
                        Err(e) => {
                            self.checkpoint_stack.push(turn);
                            self.messages.push(Message::assistant(format!("undo failed: {e}")));
                        }
                    }
                } else {
                    self.messages.push(Message::assistant("no checkpoints to undo"));
                }
            }
            "/redo" => {
                if let Some(turn) = self.redo_stack.pop() {
                    match crate::checkpoint::restore(&self.session_id, turn) {
                        Ok(_) => {
                            self.checkpoint_stack.push(turn);
                            self.messages.push(Message::assistant(
                                format!("re-applied checkpoint {turn}"),
                            ));
                        }
                        Err(e) => {
                            self.redo_stack.push(turn);
                            self.messages.push(Message::assistant(format!("redo failed: {e}")));
                        }
                    }
                } else {
                    self.messages.push(Message::assistant("nothing to redo"));
                }
            }
            _ => {
                self.messages.push(Message::assistant(format!("unknown command: {cmd}  (try /help)")));
            }
        }
    }

    fn compact_context(&mut self, tx: mpsc::Sender<StreamEvent>) {
        let summary_prompt = "Summarize the conversation so far into a concise system context that preserves all important decisions, code changes, and outstanding tasks. Output only the summary, no preamble.";
        let mut msgs = self.messages.clone();
        msgs.push(Message::user(summary_prompt));
        self.status = Status::Streaming;
        self.streaming_text.clear();
        let provider = Arc::clone(&self.provider);
        let session_id = self.session_id.clone();
        let _ = session_id;
        self.stream_task = Some(tokio::spawn(async move {
            if let Err(e) = provider.stream(&msgs, tx.clone()).await {
                let _ = tx.send(StreamEvent::Error(e.to_string())).await;
            }
        }));
    }

    fn spawn_stream(&mut self, tx: mpsc::Sender<StreamEvent>) {
        let provider = Arc::clone(&self.provider);
        let messages = self.messages.clone();
        self.stream_task = Some(tokio::spawn(async move {
            if let Err(e) = provider.stream(&messages, tx.clone()).await {
                let _ = tx.send(StreamEvent::Error(e.to_string())).await;
            }
        }));
    }

    fn resolve_permission(&mut self, always: bool, tx: mpsc::Sender<StreamEvent>) {
        if always {
            let tool_name = self.pending_calls[self.current_call_idx].name.clone();
            self.always_allowed.insert(tool_name.clone());
            let mut cfg = Config::load();
            if !cfg.always_allowed_tools.contains(&tool_name) {
                cfg.always_allowed_tools.push(tool_name);
                let _ = cfg.save();
            }
        }
        self.status = Status::Executing;
        let call = self.pending_calls[self.current_call_idx].clone();
        let tx2 = tx.clone();

        let turn = self.turn_counter;
        let session_id = self.session_id.clone();
        let is_write_op = matches!(
            call.name.as_str(),
            "write_file" | "replace_in_file" | "bash"
        );

        if is_write_op {
            let paths: Vec<&str> = if call.name == "bash" {
                Vec::new()
            } else {
                call.input["path"].as_str().map(|p| vec![p]).unwrap_or_default()
            };
            if let Ok(cp) = crate::checkpoint::snapshot(&session_id, turn, &paths) {
                self.checkpoint_stack.push(cp.turn);
                self.redo_stack.clear();
                self.turn_counter += 1;
            }
        }

        tokio::spawn(async move {
            let output = tokio::task::spawn_blocking(move || {
                tools::execute(&call.name, call.input.clone())
                    .unwrap_or_else(|e| format!("error: {e}"))
            })
            .await
            .unwrap_or_else(|e| format!("task error: {e}"));
            let _ = tx2.send(StreamEvent::ToolResult { id: call.id, output }).await;
        });
    }

    fn deny_permission(&mut self, tx: mpsc::Sender<StreamEvent>) {
        let id = self.pending_calls[self.current_call_idx].id.clone();
        let _ = tx.try_send(StreamEvent::ToolResult {
            id,
            output: "user denied this tool call".to_string(),
        });
    }

    fn advance_tool_queue(&mut self, tx: mpsc::Sender<StreamEvent>) {
        self.current_call_idx += 1;
        if self.current_call_idx >= self.pending_calls.len() {
            let results: Vec<(String, String)> = self.pending_calls
                .iter()
                .map(|c| (c.id.clone(), c.result.clone().unwrap_or_default()))
                .collect();
            self.messages.push(Message::tool_results(&results));
            self.pending_calls.clear();
            self.current_call_idx = 0;
            self.status = Status::Streaming;
            self.streaming_text.clear();
            self.spawn_stream(tx);
        } else {
            let name = self.pending_calls[self.current_call_idx].name.clone();
            if self.always_allowed.contains(&name) {
                self.resolve_permission(false, tx);
            } else {
                self.status = Status::AwaitingPermission;
            }
        }
    }

    fn interrupt(&mut self) {
        if let Some(task) = self.stream_task.take() {
            task.abort();
        }
        let text = std::mem::take(&mut self.streaming_text);
        if !text.is_empty() {
            self.messages.push(Message::assistant(text + " [interrupted]"));
        }
        self.pending_calls.clear();
        self.current_call_idx = 0;
        self.status = Status::Ready;
    }

    fn autosave(&self) {
        let s = Session {
            id: self.session_id.clone(),
            title: session::derive_title(&self.messages),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            messages: self.messages.clone(),
            tags: Vec::new(),
        };
        let _ = session::save(&s);
    }

    fn handle_stream(&mut self, event: StreamEvent, tx: mpsc::Sender<StreamEvent>) {
        match event {
            StreamEvent::TextDelta(text) => {
                self.streaming_text.push_str(&text);
            }
            StreamEvent::ToolCalls { calls, accumulated_text, input_tokens, output_tokens } => {
                self.tokens.input = input_tokens;
                self.tokens.output = output_tokens;
                self.stream_task = None;
                self.messages.push(Message::assistant_with_tools(accumulated_text, &calls));
                self.streaming_text.clear();
                self.pending_calls = calls
                    .into_iter()
                    .map(|c| PendingCall { id: c.id, name: c.name, input: c.input, result: None })
                    .collect();
                self.current_call_idx = 0;
                let first = &self.pending_calls[0];
                self.diff_content = format!("tool: {}\n\n{}", first.name, fmt_input(&first.input));
                if self.always_allowed.contains(&first.name) {
                    self.status = Status::Executing;
                    self.resolve_permission(false, tx);
                } else {
                    self.status = Status::AwaitingPermission;
                }
            }
            StreamEvent::ToolResult { id, output } => {
                if let Some(c) = self.pending_calls.iter_mut().find(|c| c.id == id) {
                    c.result = Some(output.clone());
                }
                let name = self.pending_calls
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                self.diff_content = format!("tool: {name}\n\n{output}");
                self.advance_tool_queue(tx);
            }
            StreamEvent::Done { input_tokens, output_tokens } => {
                let text = std::mem::take(&mut self.streaming_text);
                if !text.is_empty() {
                    if matches!(self.status, Status::Streaming) {
                        self.messages.push(Message::assistant(text));
                    } else {
                        let sys_content = format!("Context summary:\n\n{text}");
                        self.messages.retain(|m| !matches!(m.role, crate::provider::Role::System));
                        self.messages.insert(0, Message::system(sys_content));
                    }
                }
                self.tokens.input = input_tokens;
                self.tokens.output = output_tokens;
                self.status = Status::Ready;
                self.stream_task = None;
                self.autosave();
            }
            StreamEvent::Error(e) => {
                let leftover = std::mem::take(&mut self.streaming_text);
                if !leftover.is_empty() {
                    self.messages.push(Message::assistant(leftover));
                }
                self.status = Status::Error(e);
                self.stream_task = None;
            }
        }
    }
}

fn spawn_shutdown_listener() -> Arc<tokio::sync::Notify> {
    let notify = Arc::new(tokio::sync::Notify::new());
    let n1 = Arc::clone(&notify);
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).unwrap();
            let mut sighup = signal(SignalKind::hangup()).unwrap();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
                _ = sighup.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        n1.notify_one();
    });
    notify
}

pub fn make_provider(name: &str, key: &str, model: &str) -> Arc<dyn Provider> {
    match name {
        "openai" => Arc::new(OpenAIProvider::new(key.to_string(), model.to_string(), None)),
        "ollama" => Arc::new(OllamaProvider::new(model.to_string())),
        "gemini" => Arc::new(crate::provider::gemini::GeminiProvider::new(
            key.to_string(),
            model.to_string(),
        )),
        _ => Arc::new(AnthropicProvider::new(key.to_string(), model.to_string())),
    }
}

pub fn fmt_input(input: &serde_json::Value) -> String {
    if let Some(obj) = input.as_object() {
        obj.iter()
            .map(|(k, v)| {
                let owned = v.to_string();
                format!("{k}: {}", v.as_str().unwrap_or(&owned))
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        input.to_string()
    }
}
