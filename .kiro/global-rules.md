# Murdoch Global Rules

## Project Overview

Murdoch is an AI-powered Discord moderation bot built in Rust. It uses a three-layer pipeline:

1. **Regex Filter** - Instant pattern matching for slurs, phishing URLs, invite links
2. **Message Buffer** - Batches messages for efficient AI processing
3. **Gemini Analyzer** - Semantic content analysis using Google's Gemini 3 Flash Preview

## Core Principles

### Code Quality

- **Zero Panics**: No `unwrap()` or `expect()` in production code - use `thiserror` for explicit error handling
- **Property Testing**: Every significant logic path must have property tests (using `proptest`)
- **Type Safety**: Use strong types (enums, newtypes) over string-based logic
- **No Dead Code**: Ruthlessly delete unused code, parameters, and imports

### Architecture

- **Graceful Degradation**: If Gemini API fails, regex filter continues to work
- **Rate Limiting**: Built-in governor-based rate limiting for API calls
- **Double Buffering**: Non-blocking message collection during analysis

### Security

- **Hardened Prompts**: AI prompts include OPSEC constraints to prevent prompt injection
- **Blind Processing**: AI never exposes internal rule logic, IDs, or detection methods
- **PII Scrubbing**: User IDs, channel IDs, and role IDs are sanitized in outputs

### Testing

- **No Mocks**: Use real implementations or in-memory alternatives
- **Property-Based**: Use proptest for invariant testing
- **259 Tests**: Comprehensive test suite covering all modules

## File Conventions

| Path       | Purpose                            |
| ---------- | ---------------------------------- |
| `src/*.rs` | Core Rust modules                  |
| `web/`     | Static web dashboard (HTML/CSS/JS) |
| `docs/`    | Documentation                      |
| `.kiro/`   | Kiro specs and steering            |

## Deployment

- **Target**: Fly.io (single machine, SJC region)
- **Database**: SQLite (persistent volume)
- **Ports**: 8080 (web), 8081 (health check)
