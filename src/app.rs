use crate::provider::{Message, Provider, StreamEvent};
use crate::tools;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{collections::HashSet, io, sync::Arc};
use tokio::sync::mpsc;

#[derive(Debug, Default, Clone)]
pub struct TokenUsage {
    pub input: u32,
    pub output: u32,
    pub max: u32,
}

impl TokenUsage {
    pub fn new() -> Self {
        Self { input: 0, output: 0, max: 200_000 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Ready,
    Streaming,
    AwaitingPermission,
    Executing,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct PendingCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub result: Option<String>,
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
    stream_task: Option<tokio::task::JoinHandle<()>>,
}

impl App {
    pub fn new(provider: Arc<dyn Provider>) -> Self {
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
            always_allowed: HashSet::new(),
            stream_task: None,
        }
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

        loop {
            terminal.draw(|f| crate::ui::render(f, self))?;

            tokio::select! {
                Some(Ok(event)) = events.next() => {
                    if self.handle_event(event, stream_tx.clone()).await? {
                        break;
                    }
                }
                Some(ev) = stream_rx.recv() => {
                    self.handle_stream(ev, stream_tx.clone());
                }
            }
        }

        Ok(())
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
                KeyCode::Char('u') if self.status == Status::Ready => {
                    self.input.clear();
                    self.cursor_pos = 0;
                }
                _ => {}
            }
            return Ok(false);
        }

        match &self.status {
            Status::AwaitingPermission => {
                match key.code {
                    KeyCode::Char('y') => self.resolve_permission(false, stream_tx),
                    KeyCode::Char('a') => self.resolve_permission(true, stream_tx),
                    KeyCode::Char('n') => self.deny_permission(stream_tx),
                    _ => {}
                }
            }
            Status::Streaming | Status::Executing => {}
            _ => {
                match key.code {
                    KeyCode::Enter => self.submit(stream_tx),
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
                    KeyCode::Left  => { self.cursor_pos = self.cursor_pos.saturating_sub(1); }
                    KeyCode::Right => { self.cursor_pos = (self.cursor_pos + 1).min(self.input.len()); }
                    KeyCode::Home  => { self.cursor_pos = 0; }
                    KeyCode::End   => { self.cursor_pos = self.input.len(); }
                    KeyCode::Up    => {
                        self.auto_scroll = false;
                        self.scroll = self.scroll.saturating_sub(3);
                    }
                    KeyCode::Down  => {
                        self.scroll = self.scroll.saturating_add(3);
                        if self.scroll >= u16::MAX - 3 {
                            self.auto_scroll = true;
                            self.scroll = u16::MAX;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(false)
    }

    fn submit(&mut self, tx: mpsc::Sender<StreamEvent>) {
        let input = self.input.trim().to_string();
        if input.is_empty() { return; }

        self.messages.push(Message::user(&input));
        self.input.clear();
        self.cursor_pos = 0;
        self.streaming_text.clear();
        self.status = Status::Streaming;
        self.auto_scroll = true;
        self.scroll = u16::MAX;

        self.spawn_stream(tx);
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
        let call = &self.pending_calls[self.current_call_idx];
        if always {
            self.always_allowed.insert(call.name.clone());
        }
        self.status = Status::Executing;
        let call = self.pending_calls[self.current_call_idx].clone();
        let tx2 = tx.clone();

        tokio::spawn(async move {
            let output = tokio::task::spawn_blocking(move || {
                tools::execute(call.name.as_str(), call.input.clone())
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
            let next = &self.pending_calls[self.current_call_idx];
            if self.always_allowed.contains(&next.name) {
                self.status = Status::Executing;
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
                let name = first.name.clone();
                let display = format_input(&first.input);
                self.diff_content = format!("tool: {name}\n\n{display}");

                if self.always_allowed.contains(&name) {
                    self.status = Status::Executing;
                    self.resolve_permission(false, tx);
                } else {
                    self.status = Status::AwaitingPermission;
                }
            }

            StreamEvent::ToolResult { id, output } => {
                if let Some(call) = self.pending_calls.iter_mut().find(|c| c.id == id) {
                    call.result = Some(output.clone());
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
                    self.messages.push(Message::assistant(text));
                }
                self.tokens.input = input_tokens;
                self.tokens.output = output_tokens;
                self.status = Status::Ready;
                self.stream_task = None;
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

fn format_input(input: &serde_json::Value) -> String {
    if let Some(obj) = input.as_object() {
        obj.iter()
            .map(|(k, v)| {
                let owned = v.to_string();
                let val = v.as_str().unwrap_or(&owned);
                format!("{k}: {val}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        input.to_string()
    }
}
