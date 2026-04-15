# bcode

**Terminal AI coding agent — Rust, Ratatui, zero bloat.**

A fast, model-agnostic terminal agent built with Ratatui. Works with Anthropic, OpenAI, Gemini, Ollama, and any OpenAI-compatible endpoint. Ships as a single static binary.

---

## Why bcode

| | Claude Code | OpenCode | **bcode** |
|---|---|---|---|
| Memory footprint | ~300 MB+ | 1–30 GB (leaks) | **< 100 MB** |
| TUI flicker | Yes | Yes | **None** |
| Model-agnostic | No | Yes | **Yes** |
| Tool calling | Yes | Yes | **Yes (all providers)** |
| Air-gapped mode | No | Leaks to cloud | **Zero extra calls** |
| Binary | npm install | npm install | **Single binary** |
| Undo/redo | No | Yes | **Yes (file snapshots)** |
| Context meter | No | No | **Yes** |
| Open source | No | Yes | **Yes (MIT)** |

---

## Install

**Homebrew (macOS / Linux):**
```bash
brew install ddoemonn/tap/bcode
```

**One-line installer:**
```bash
curl -fsSL https://raw.githubusercontent.com/ddoemonn/bcode/main/install.sh | bash
```

**Cargo:**
```bash
cargo install bcode
```

**Download binary:** see [Releases](https://github.com/ddoemonn/bcode/releases).

---

## Quick start

```bash
# First run — interactive setup wizard
bcode

# Explicit provider
bcode --provider anthropic --api-key sk-ant-...
bcode --provider openai --api-key sk-...
bcode --provider gemini --api-key AIza...
bcode --provider ollama --model llama3.2

# OpenAI-compatible endpoint (LM Studio, vLLM, Groq, Together, etc.)
bcode --provider openai --base-url http://localhost:1234 --model local-model

# Resume a session
bcode --resume <session-id>
```

---

## TUI layout

```
┌ chat ────────────────────────────┐┌ diff / tool ────────────────────┐
│                                   ││                                  │
│  you  refactor auth.rs            ││  write_file        [write]       │
│                                   ││                                  │
│  ai   I'll start by reading the   ││  path  src/auth.rs               │
│       file, then extract the      ││                                  │
│       JWT logic into its own      ││  ──────────────────────────────  │
│       module.                     ││  [y] allow  [n] deny  [a] always │
│                                   ││                                  │
└───────────────────────────────────┘└──────────────────────────────────┘
 anthropic/claude-sonnet-4-6  │  ● ready  ↩ 2   ││████████░░ 42%
┌────────────────────────────────────────────────────────────────────────┐
│ > _                                                                     │
└────────────────────────────────────────────────────────────────────────┘
```

---

## Slash commands

| Command | Description |
|---|---|
| `/help` | Show all commands |
| `/clear` | Clear conversation |
| `/model <name>` | Switch model mid-session |
| `/compact` | Compress context via model summary |
| `/undo` | Revert last file-writing operation |
| `/redo` | Re-apply reverted operation |
| `/sessions` | Open session browser |
| `/tag <name>` | Tag the current session |
| `/cost` | Show token usage breakdown |
| `/config` | Show current configuration |

---

## Keybindings

| Key | Action |
|---|---|
| `Enter` | Send message / execute command |
| `Ctrl+C` | Interrupt streaming / quit |
| `Ctrl+R` | Open session browser |
| `Ctrl+U` | Clear input |
| `↑ / ↓` | Input history (or scroll chat) |
| `y / n / a` | Allow once / deny / always allow (permission prompt) |
| `Esc` | Close session browser |

---

## Project context — BCODE.md

Create a `BCODE.md` in your project root. `bcode` finds it automatically (walks up from cwd) and prepends it as a system message:

```markdown
# My Project

This is a TypeScript monorepo with packages in `apps/` and `libs/`.
Always use `pnpm` not `npm`. Tests live in `__tests__/` sibling to source.
```

User-level preferences: `~/.bcode/user.md`.

---

## Configuration

Config is stored at `~/.config/bcode/config.json` (macOS/Linux) or `%APPDATA%\bcode\config.json` (Windows).

```json
{
  "provider": "anthropic",
  "model": "claude-sonnet-4-6",
  "api_keys": {
    "anthropic": "sk-ant-...",
    "openai": "sk-..."
  },
  "base_urls": {
    "openai": "http://localhost:1234"
  },
  "always_allowed_tools": ["read_file", "list_dir", "glob"],
  "max_messages": 100
}
```

---

## Tools

| Tool | Description | Risk |
|---|---|---|
| `read_file` | Read file contents | read |
| `write_file` | Write/create a file | write |
| `replace_in_file` | Exact-string replacement (fails on ambiguity) | write |
| `list_dir` | List directory entries | read |
| `glob` | Find files by pattern | read |
| `search_in_files` | Search text across files | read |
| `bash` | Run shell command (30s timeout) | shell |

---

## Providers

| Provider | Tool calling | Streaming | Notes |
|---|---|---|---|
| Anthropic | Yes | Yes | Default |
| OpenAI | Yes | Yes | Compatible with any OpenAI-format endpoint |
| Gemini | Yes | Yes | Google AI Studio key |
| Ollama | Yes | Yes | Local, no key needed |

---

## Security

- No unauthenticated local HTTP servers
- No telemetry, no cloud calls beyond the configured inference endpoint
- No prompt sentiment scanning
- Config never fetched from web URLs
- `always_allowed_tools` persisted locally, scoped to your machine
- All file operations go through the explicit permission UI

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). PRs welcome — especially for new provider integrations, tool additions, and TUI improvements.

---

## License

MIT — see [LICENSE](LICENSE).
