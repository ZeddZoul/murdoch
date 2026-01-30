# Design Document: Slash Command Fixes

## Overview

This design addresses two critical issues:
1. Slash command option parsing for `/murdoch clear` and `/murdoch warnings` commands
2. Integration of dashboard configuration into bot message processing

## Architecture

### Command Option Parsing

The issue stems from Discord's nested subcommand structure. When a command like `/murdoch clear @user` is invoked:

```
CommandInteraction {
  data: {
    options: [
      CommandDataOption {
        name: "clear",
        value: SubCommand([
          CommandDataOption {
            name: "user",
            value: User(UserId)
          }
        ])
      }
    ]
  }
}
```

The current code incorrectly tries to access the user directly from `options.first()`, which returns the subcommand itself, not the user parameter.

### Configuration Integration

The bot currently uses hardcoded defaults from environment variables. The dashboard allows users to configure per-server settings, but these aren't being applied during message processing.

## Components and Interfaces

### 1. SlashCommandHandler (src/commands.rs)

**Changes Required:**

- `handle_warnings()`: Extract user from nested subcommand options
- `handle_clear()`: Extract user from nested subcommand options

**Pattern to Apply:**

```rust
// Extract nested subcommand options
let subcommand_options = command.data.options.first().and_then(|o| match &o.value {
    serenity::all::CommandDataOptionValue::SubCommand(opts) => Some(opts),
    _ => None,
});

let empty_vec = vec![];
let options = subcommand_options.unwrap_or(&empty_vec);

// Find the user parameter
let user_option = options
    .iter()
    .find(|o| o.name == "user")
    .and_then(|o| o.value.as_user_id());
```

### 2. ModDirectorPipeline (src/pipeline.rs)

**Changes Required:**

- Load server configuration from database before processing messages
- Apply configuration to buffer and analyzer behavior

**New Method:**

```rust
async fn get_server_config(&self, guild_id: u64) -> Result<ServerConfig> {
    // Load from database
    // Cache for performance
}
```

**Integration Points:**

- `process_message()`: Load config at start
- Pass config to buffer and analyzer
- Use config.mod_role_id for notifications

### 3. MessageBuffer (src/buffer.rs)

**Changes Required:**

- Accept per-message configuration
- Use config.buffer_threshold and config.buffer_timeout_secs

**Method Signature Change:**

```rust
pub async fn add(&self, msg: BufferedMessage, config: &ServerConfig) -> Result<bool>
```

## Data Models

### ServerConfig (Already Exists)

```rust
pub struct ServerConfig {
    pub guild_id: u64,
    pub severity_threshold: f32,
    pub buffer_timeout_secs: u64,
    pub buffer_threshold: u32,
    pub mod_role_id: Option<u64>,
}
```

## Error Handling

- Database connection failures: Fall back to environment variable defaults
- Missing server config: Use default configuration
- Invalid config values: Log warning and use defaults

## Testing Strategy

### Unit Tests

- Test nested option extraction with mock CommandInteraction
- Test config loading and caching
- Test fallback to defaults on database errors

### Integration Tests

- Test `/murdoch clear @user` command end-to-end
- Test `/murdoch warnings @user` command end-to-end
- Test message processing with custom config
- Test config changes are applied immediately

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system.*

### Property 1: Subcommand Option Extraction

*For any* slash command with a user subcommand option, extracting the user parameter SHALL return the correct UserId when present and None when absent.

**Validates: Requirements 1.1, 1.2**

### Property 2: Configuration Application

*For any* server with custom configuration, message processing SHALL use the server's configured thresholds and timeouts instead of global defaults.

**Validates: Requirements 2.1, 2.2, 2.3, 2.4**

### Property 3: Configuration Fallback

*For any* server without custom configuration or when database is unavailable, message processing SHALL use environment variable defaults without errors.

**Validates: Requirements 2.1**
