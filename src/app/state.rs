#[derive(Debug, Clone, PartialEq)]
pub enum SetupStep {
    ChooseProvider,
    EnterApiKey,
    EnterModel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Setup(SetupStep),
    Ready,
    Streaming,
    AwaitingPermission,
    Executing,
    SessionBrowser,
    Error(String),
}

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

#[derive(Debug, Clone)]
pub struct PendingCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub result: Option<String>,
}
