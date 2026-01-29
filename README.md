# Murdoch

**High-efficiency semantic moderation for Discord**

Murdoch is an AI-powered Discord moderation bot built in Rust. It combines instant regex pattern matching with Google's Gemini AI for semantic content analysis, catching both obvious violations and nuanced toxic behavior that traditional bots miss.

## Why Murdoch?

Traditional Discord moderation bots rely on keyword blacklists. They catch "bad word" but miss:

- **Creative spelling**: "b4d w0rd", "b.a.d w.o.r.d"
- **Context-dependent toxicity**: Sarcasm, dog-whistles, coded language
- **Escalating harassment**: Patterns that are fine individually but toxic together
- **Social engineering**: Phishing attempts disguised as legitimate requests

Murdoch solves this with a three-layer pipeline that balances speed, accuracy, and cost.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Discord Message                          │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│  LAYER 1: Regex Filter                                          │
│  • Instant pattern matching (~1ms)                              │
│  • Slurs, invite links, phishing URLs                           │
│  • Blocks obvious violations immediately                        │
└─────────────────────────────────────────────────────────────────┘
                                 │
                          (if passed)
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│  LAYER 2: Message Buffer                                        │
│  • Collects messages for batch processing                       │
│  • Double-buffering for non-blocking operation                  │
│  • Flushes on: time interval, count threshold, or urgency       │
└─────────────────────────────────────────────────────────────────┘
                                 │
                          (on flush)
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│  LAYER 3: Gemini Analyzer                                       │
│  • Semantic content analysis via Gemini 3 Flash Preview         │
│  • Context-aware (sees conversation history)                    │
│  • Server-specific rules integration                            │
│  • Hardened prompts prevent prompt injection                    │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│  ACTION: Discord Client                                         │
│  • Delete message, warn user, timeout, ban                      │
│  • Log to mod channel with violation details                    │
│  • Progressive escalation based on warning count                │
└─────────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

| Decision                 | Why                                                                                       |
| ------------------------ | ----------------------------------------------------------------------------------------- |
| **Rust**                 | Zero-cost abstractions, memory safety without GC pauses, excellent async ecosystem        |
| **Three-layer pipeline** | Regex catches 90% of violations instantly; AI handles the remaining 10% with full context |
| **Double buffering**     | Collect messages while previous batch is being analyzed—no blocking                       |
| **Graceful degradation** | If Gemini API is down, regex filter continues working                                     |
| **Hardened prompts**     | OPSEC constraints prevent prompt injection and logic leakage                              |

## Features

### Moderation

- 🛡️ **Instant regex filtering** for slurs, invite links, phishing URLs
- 🧠 **Semantic AI analysis** catches context-dependent toxicity
- 📊 **Progressive warnings** with configurable escalation (warn → timeout → ban)
- 🔄 **Raid detection** identifies coordinated attacks
- 📝 **Appeal system** for users to contest moderation actions

### Dashboard

- 📈 Real-time violation metrics
- 🔍 Searchable violation history
- ⚙️ Per-server rule configuration
- 🌓 Dark/light theme support
- 📱 Mobile-responsive design

### Infrastructure

- 💾 SQLite database with connection pooling
- 🔐 OAuth2 authentication for dashboard
- 🚀 Fly.io deployment ready

## Getting Started

### Prerequisites

- Rust 1.75+
- Discord bot token (from [Discord Developer Portal](https://discord.com/developers/applications))
- Gemini API key (from [Google AI Studio](https://aistudio.google.com/))

### Environment Variables

Create a `.env` file:

```env
DISCORD_TOKEN=your_discord_bot_token
GEMINI_API_KEY=your_gemini_api_key
DATABASE_URL=sqlite:murdoch.db
RUST_LOG=info
```

### Build & Run

```bash
# Clone the repository
git clone https://github.com/zeddzoul/murdoch.git
cd murdoch

# Build
cargo build --release

# Run
cargo run --release
```

### Docker

```bash
docker build -t murdoch .
docker run -d --env-file .env murdoch
```

## Testing

Murdoch has a comprehensive test suite with 259 tests covering all modules.

### Run All Tests

```bash
cargo test
```

### Run Specific Module Tests

```bash
# Test the regex filter
cargo test filter::

# Test the message buffer
cargo test buffer::

# Test the Gemini analyzer
cargo test analyzer::

# Test the full pipeline
cargo test pipeline::
```

### Property-Based Testing

Murdoch uses [proptest](https://github.com/proptest-rs/proptest) for property-based testing, ensuring invariants hold across random inputs:

```bash
# Run property tests
cargo test --lib
```

### Test Philosophy

- **No mocks**: Tests use real implementations or in-memory alternatives
- **Property-based**: Random input generation catches edge cases
- **Zero panics**: All error paths are explicit via `thiserror`

## Deployment

### Fly.io (Recommended)

```bash
# Install flyctl
brew install flyctl

# Login
flyctl auth login

# Deploy
flyctl deploy --remote-only
```

### Configuration

See [fly.toml](fly.toml) for Fly.io configuration including:

- Single machine deployment
- Persistent volume for SQLite
- Health check endpoints
- Environment variable secrets

## Project Structure

```
src/
├── analyzer.rs     # Layer 3: Gemini AI analysis
├── buffer.rs       # Layer 2: Message batching with double-buffer
├── filter.rs       # Layer 1: Regex pattern matching
├── pipeline.rs     # Orchestrates all layers
├── discord.rs      # Discord API client and actions
├── models.rs       # Core data types
├── database.rs     # SQLite persistence
├── warnings.rs     # Progressive warning system
├── appeals.rs      # User appeal handling
├── raid.rs         # Raid detection
├── rules.rs        # Server-specific rule engine
├── web.rs          # Dashboard HTTP server
├── websocket.rs    # Real-time updates
├── health.rs       # Health check endpoints
├── oauth.rs        # OAuth2 authentication
└── error.rs        # Error types (thiserror)

web/
├── index.html      # Dashboard SPA
├── css/styles.css  # Styling
└── js/             # Frontend JavaScript
```

## Configuration

### Server-Specific Rules

Murdoch supports custom rules per Discord server:

```json
{
  "rules": [
    {
      "id": "no-crypto",
      "description": "No cryptocurrency promotion",
      "enabled": true
    },
    {
      "id": "no-politics",
      "description": "Keep political discussions civil",
      "enabled": true
    }
  ]
}
```

### Warning Escalation

Configure how warnings escalate:

| Warning Count | Default Action    |
| ------------- | ----------------- |
| 1-2           | Warning message   |
| 3-4           | 10 minute timeout |
| 5-6           | 1 hour timeout    |
| 7+            | Ban               |

## API Endpoints

| Endpoint              | Method    | Description           |
| --------------------- | --------- | --------------------- |
| `/health`             | GET       | Health check          |
| `/api/violations`     | GET       | List violations       |
| `/api/violations/:id` | GET       | Get violation details |
| `/api/rules`          | GET/POST  | Manage rules          |
| `/api/appeals`        | GET/POST  | Manage appeals        |
| `/api/metrics`        | GET       | Moderation metrics    |
| `/ws`                 | WebSocket | Real-time updates     |

## Performance

| Metric                   | Value     |
| ------------------------ | --------- |
| Regex filter latency     | ~1ms      |
| Buffer flush interval    | 5 seconds |
| Gemini API latency       | 200-500ms |
| Messages/second capacity | 1000+     |
| Memory footprint         | ~50MB     |

## Security

- **Hardened prompts**: AI prompts include OPSEC constraints to prevent prompt injection
- **Blind processing**: AI never exposes internal rule logic or detection methods
- **PII scrubbing**: User IDs and channel IDs are sanitized in outputs
- **Rate limiting**: Governor-based rate limiting for API calls
- **No credential logging**: Tokens and keys are never logged

## License

MIT License. See [LICENSE](LICENSE) for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass (`cargo test`)
5. Submit a pull request

## Acknowledgments

- [Serenity](https://github.com/serenity-rs/serenity) - Discord API library
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [Google Gemini](https://ai.google.dev/) - AI content analysis
- [proptest](https://github.com/proptest-rs/proptest) - Property-based testing
