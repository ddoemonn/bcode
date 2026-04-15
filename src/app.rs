use crate::provider::{Message, Provider, StreamEvent};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::Arc};
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
    Error(String),
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
                    self.handle_stream(ev);
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
                    if self.status == Status::Streaming {
                        self.interrupt();
                        return Ok(false);
                    }
                    return Ok(true);
                }
                KeyCode::Char('u') => {
                    self.input.clear();
                    self.cursor_pos = 0;
                    return Ok(false);
                }
                _ => return Ok(false),
            }
        }

        if self.status == Status::Streaming {
            return Ok(false);
        }

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
            KeyCode::Left => { self.cursor_pos = self.cursor_pos.saturating_sub(1); }
            KeyCode::Right => { self.cursor_pos = (self.cursor_pos + 1).min(self.input.len()); }
            KeyCode::Home => { self.cursor_pos = 0; }
            KeyCode::End => { self.cursor_pos = self.input.len(); }
            KeyCode::Up => {
                self.auto_scroll = false;
                self.scroll = self.scroll.saturating_sub(3);
            }
            KeyCode::Down => {
                self.scroll = self.scroll.saturating_add(3);
                if self.scroll >= u16::MAX - 3 {
                    self.auto_scroll = true;
                    self.scroll = u16::MAX;
                }
            }
            _ => {}
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

        let provider = Arc::clone(&self.provider);
        let messages = self.messages.clone();

        self.stream_task = Some(tokio::spawn(async move {
            if let Err(e) = provider.stream(&messages, tx.clone()).await {
                let _ = tx.send(StreamEvent::Error(e.to_string())).await;
            }
        }));
    }

    fn interrupt(&mut self) {
        if let Some(task) = self.stream_task.take() {
            task.abort();
        }
        let text = std::mem::take(&mut self.streaming_text);
        if !text.is_empty() {
            self.messages.push(Message::assistant(text + " [interrupted]"));
        }
        self.status = Status::Ready;
    }

    fn handle_stream(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::TextDelta(text) => {
                self.streaming_text.push_str(&text);
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
