# Configuration Guide

This document provides comprehensive documentation for all environment variables used by the Murdoch Discord moderation bot and dashboard.

## Table of Contents

- [Required Variables](#required-variables)
- [Discord Configuration](#discord-configuration)
- [API Keys](#api-keys)
- [Web Dashboard](#web-dashboard)
- [Database](#database)
- [Buffer Configuration](#buffer-configuration)
- [Regex Patterns](#regex-patterns)
- [Server Configuration](#server-configuration)
- [Caching](#caching)
- [Monitoring](#monitoring)
- [Notifications](#notifications)
- [Security](#security)

## Required Variables

These variables MUST be set for the application to start.

### DISCORD_TOKEN

**Description**: Discord bot token for authentication with Discord API.

**Type**: String

**Required**: Yes

**Example**: `DISCORD_TOKEN="MTIzNDU2Nzg5MDEyMzQ1Njc4.GhIjKl.MnOpQrStUvWxYzAbCdEfGhIjKlMnOpQrStUvWx"`

**How to obtain**:
1. Go to [Discord Developer Portal](https://discord.com/developers/applications)
2. Select your application
3. Navigate to "Bot" section
4. Click "Reset Token" or "Copy" to get the token
5. Keep this secret and never commit it to version control

**Security**: This token grants full access to your bot. Rotate regularly and never share publicly.

---

### GEMINI_API_KEY

**Description**: Google Gemini API key for semantic content analysis.

**Type**: String

**Required**: Yes

**Example**: `GEMINI_API_KEY="AIzaSyAbCdEfGhIjKlMnOpQrStUvWxYz1234567"`

**How to obtain**:
1. Go to [Google AI Studio](https://aistudio.google.com/app/apikey)
2. Sign in with your Google account
3. Click "Create API Key"
4. Copy the generated key

**Security**: This key is tied to your Google Cloud billing. Monitor usage to avoid unexpected charges.

---

### DISCORD_CLIENT_ID

**Description**: Discord OAuth2 client ID for web dashboard authentication.

**Type**: String (numeric)

**Required**: Yes (for web dashboard)

**Example**: `DISCORD_CLIENT_ID="1234567890123456789"`

**How to obtain**:
1. Go to [Discord Developer Portal](https://discord.com/developers/applications)
2. Select your application
3. Navigate to "OAuth2" section
4. Copy the "Client ID"

---

### DISCORD_CLIENT_SECRET

**Description**: Discord OAuth2 client secret for web dashboard authentication.

**Type**: String

**Required**: Yes (for web dashboard)

**Example**: `DISCORD_CLIENT_SECRET="AbCdEfGhIjKlMnOpQrStUvWxYz123456"`

**How to obtain**:
1. Go to [Discord Developer Portal](https://discord.com/developers/applications)
2. Select your application
3. Navigate to "OAuth2" section
4. Click "Reset Secret" or copy existing secret

**Security**: Keep this secret secure. It's used to verify OAuth callbacks.

---

### SESSION_SECRET

**Description**: Secret key for encrypting session cookies in the web dashboard.

**Type**: String (32+ characters recommended)

**Required**: Yes (for web dashboard)

**Example**: `SESSION_SECRET="$(openssl rand -base64 32)"`

**How to generate**:
```bash
# Linux/macOS
openssl rand -base64 32

# Or use any random string generator
```

**Security**: Use a cryptographically secure random string. Changing this will invalidate all active sessions.

---

## Discord Configuration

### MOD_ROLE_ID

**Description**: Discord role ID for moderator mentions on high-severity violations.

**Type**: String (numeric)

**Required**: No

**Default**: None (no mentions)

**Example**: `MOD_ROLE_ID="987654321098765432"`

**How to obtain**:
1. Enable Developer Mode in Discord (User Settings > Advanced > Developer Mode)
2. Right-click the role in Server Settings > Roles
3. Click "Copy ID"

**Usage**: When set, high-severity violations will mention this role in the moderation channel.

---

## API Keys

### GEMINI_API_KEY

See [Required Variables](#gemini_api_key) section above.

---

## Web Dashboard

### DASHBOARD_URL

**Description**: Public URL where the dashboard is hosted. Used for OAuth redirect URIs.

**Type**: String (URL)

**Required**: Yes (for web dashboard)

**Default**: `http://localhost:3000`

**Examples**:
- Development: `DASHBOARD_URL="http://localhost:3000"`
- Production: `DASHBOARD_URL="https://dashboard.example.com"`
- Shuttle: `DASHBOARD_URL="https://murdoch.shuttleapp.rs"`

**Important**: 
- Must match the redirect URI configured in Discord Developer Portal
- Should NOT include trailing slash
- Must use HTTPS in production

---

### WEB_PORT

**Description**: Port for the web dashboard HTTP server.

**Type**: Integer

**Required**: No

**Default**: `8081`

**Example**: `WEB_PORT="8081"`

**Valid Range**: 1024-65535 (avoid privileged ports < 1024)

**Note**: On Shuttle.rs, this is managed automatically.

---

## Database

### DATABASE_PATH

**Description**: Path to the SQLite database file.

**Type**: String (file path)

**Required**: No

**Default**: `murdoch.db`

**Examples**:
- Local: `DATABASE_PATH="murdoch.db"`
- Custom path: `DATABASE_PATH="/data/murdoch.db"`
- In-memory (testing): `DATABASE_PATH=":memory:"`

**Note**: Ensure the directory is writable and has sufficient disk space.

---

## Buffer Configuration

### BUFFER_FLUSH_THRESHOLD

**Description**: Number of violations to accumulate before flushing to database.

**Type**: Integer

**Required**: No

**Default**: `10`

**Example**: `BUFFER_FLUSH_THRESHOLD="10"`

**Valid Range**: 1-1000

**Tuning**:
- Lower values (1-5): More real-time, higher database load
- Medium values (10-50): Balanced performance
- Higher values (50-1000): Better throughput, less real-time

**Recommendation**: Use default (10) for most servers. Increase for high-traffic servers (> 10,000 members).

---

### BUFFER_TIMEOUT_SECS

**Description**: Maximum seconds to wait before flushing buffer, even if threshold not reached.

**Type**: Integer

**Required**: No

**Default**: `30`

**Example**: `BUFFER_TIMEOUT_SECS="30"`

**Valid Range**: 1-300 (5 minutes)

**Tuning**:
- Lower values (1-10): More real-time updates
- Medium values (30-60): Balanced
- Higher values (60-300): Better batching, less frequent writes

**Recommendation**: Use default (30) for most servers.

---

## Regex Patterns

### REGEX_SLURS

**Description**: Comma-separated list of regex patterns for detecting slurs and offensive language.

**Type**: String (comma-separated regex patterns)

**Required**: No

**Default**: Built-in patterns

**Example**: `REGEX_SLURS="badword1,badword2,offensive.*phrase"`

**Usage**: Supplements built-in patterns. Use `\b` for word boundaries.

**Example patterns**:
```bash
REGEX_SLURS="\bn[i1]gg[ae]r\b,\bf[a@]gg[o0]t\b,\bretard(ed)?\b"
```

**Note**: Patterns are case-insensitive. Test thoroughly to avoid false positives.

---

### REGEX_INVITE_LINKS

**Description**: Comma-separated list of regex patterns for detecting Discord invite links.

**Type**: String (comma-separated regex patterns)

**Required**: No

**Default**: Built-in patterns

**Example**: `REGEX_INVITE_LINKS="discord\.gg/[a-zA-Z0-9]+,discord\.com/invite/[a-zA-Z0-9]+"`

**Usage**: Detects unauthorized server invites.

**Default patterns**:
- `discord\.gg/[a-zA-Z0-9]+`
- `discord\.com/invite/[a-zA-Z0-9]+`
- `discordapp\.com/invite/[a-zA-Z0-9]+`

---

### REGEX_PHISHING_URLS

**Description**: Comma-separated list of regex patterns for detecting phishing URLs.

**Type**: String (comma-separated regex patterns)

**Required**: No

**Default**: Built-in patterns

**Example**: `REGEX_PHISHING_URLS="free-nitro\.com,discord-gift\.ru,steam-community\.ru"`

**Usage**: Detects known phishing domains.

**Common phishing patterns**:
```bash
REGEX_PHISHING_URLS="free-nitro\.(com|net|org),discord.*gift\.(com|ru),steam.*community\.(ru|tk)"
```

**Note**: Update regularly as new phishing domains emerge.

---

### REGEX_PATTERNS_PATH

**Description**: Path to a JSON file containing regex patterns (alternative to environment variables).

**Type**: String (file path)

**Required**: No

**Default**: None (use environment variables)

**Example**: `REGEX_PATTERNS_PATH="/config/patterns.json"`

**File format**:
```json
{
  "slurs": ["pattern1", "pattern2"],
  "invite_links": ["pattern1", "pattern2"],
  "phishing_urls": ["pattern1", "pattern2"]
}
```

**Usage**: Useful for managing complex pattern sets. Takes precedence over individual environment variables.

---

## Server Configuration

### HEALTH_PORT

**Description**: Port for the health check HTTP server.

**Type**: Integer

**Required**: No

**Default**: `8080`

**Example**: `HEALTH_PORT="8080"`

**Valid Range**: 1024-65535

**Endpoints**:
- `GET /health` - Health check with component status
- `GET /metrics` - Prometheus metrics

**Note**: On Shuttle.rs, this is managed automatically.

---

## Caching

### REDIS_URL

**Description**: Redis connection URL for distributed caching (optional).

**Type**: String (URL)

**Required**: No

**Default**: None (uses in-memory cache)

**Examples**:
- Local: `REDIS_URL="redis://localhost:6379"`
- With auth: `REDIS_URL="redis://:password@localhost:6379"`
- TLS: `REDIS_URL="rediss://localhost:6380"`

**Usage**: Enables distributed caching across multiple instances. Falls back to in-memory if unavailable.

---

### REDIS_ENABLED

**Description**: Enable or disable Redis caching.

**Type**: Boolean

**Required**: No

**Default**: `false`

**Example**: `REDIS_ENABLED="true"`

**Valid Values**: `true`, `false`, `1`, `0`, `yes`, `no`

**Note**: Requires REDIS_URL to be set. Application will start without Redis if disabled or unavailable.

---

## Monitoring

### PROMETHEUS_ENABLED

**Description**: Enable Prometheus metrics endpoint.

**Type**: Boolean

**Required**: No

**Default**: `true`

**Example**: `PROMETHEUS_ENABLED="true"`

**Valid Values**: `true`, `false`, `1`, `0`, `yes`, `no`

**Metrics endpoint**: `GET /metrics`

---

### PROMETHEUS_PORT

**Description**: Port for Prometheus metrics endpoint (if separate from health port).

**Type**: Integer

**Required**: No

**Default**: Same as HEALTH_PORT

**Example**: `PROMETHEUS_PORT="9090"`

**Valid Range**: 1024-65535

---

## Notifications

### SMTP_HOST

**Description**: SMTP server hostname for email notifications.

**Type**: String

**Required**: No (for email notifications)

**Example**: `SMTP_HOST="smtp.gmail.com"`

---

### SMTP_PORT

**Description**: SMTP server port.

**Type**: Integer

**Required**: No

**Default**: `587`

**Example**: `SMTP_PORT="587"`

**Common ports**:
- `25` - Unencrypted (not recommended)
- `587` - STARTTLS (recommended)
- `465` - SSL/TLS

---

### SMTP_USERNAME

**Description**: SMTP authentication username.

**Type**: String

**Required**: No (for email notifications)

**Example**: `SMTP_USERNAME="notifications@example.com"`

---

### SMTP_PASSWORD

**Description**: SMTP authentication password.

**Type**: String

**Required**: No (for email notifications)

**Example**: `SMTP_PASSWORD="your_app_password"`

**Security**: Use app-specific passwords when available (e.g., Gmail App Passwords).

---

## Security

### RUST_LOG

**Description**: Logging level and filter configuration.

**Type**: String

**Required**: No

**Default**: `info`

**Examples**:
- Basic: `RUST_LOG="info"`
- Debug: `RUST_LOG="debug"`
- Specific module: `RUST_LOG="murdoch=debug,sqlx=warn"`
- Multiple modules: `RUST_LOG="murdoch=debug,serenity=info,sqlx=warn"`

**Valid Levels**: `trace`, `debug`, `info`, `warn`, `error`

**Recommendation**: Use `info` in production, `debug` for troubleshooting.

---

## Configuration File Example

Complete `.env` file for production:

```bash
# Required - Discord
DISCORD_TOKEN="your_bot_token_here"
DISCORD_CLIENT_ID="1234567890123456789"
DISCORD_CLIENT_SECRET="your_client_secret_here"

# Required - API Keys
GEMINI_API_KEY="your_gemini_api_key_here"

# Required - Web Dashboard
DASHBOARD_URL="https://dashboard.example.com"
SESSION_SECRET="$(openssl rand -base64 32)"

# Optional - Discord
MOD_ROLE_ID="987654321098765432"

# Optional - Database
DATABASE_PATH="murdoch.db"

# Optional - Buffer
BUFFER_FLUSH_THRESHOLD="10"
BUFFER_TIMEOUT_SECS="30"

# Optional - Server
HEALTH_PORT="8080"
WEB_PORT="8081"

# Optional - Caching
REDIS_URL="redis://localhost:6379"
REDIS_ENABLED="true"

# Optional - Monitoring
PROMETHEUS_ENABLED="true"
RUST_LOG="info"

# Optional - Custom Patterns
REGEX_SLURS="custom_pattern1,custom_pattern2"
REGEX_INVITE_LINKS="discord\.gg/[a-zA-Z0-9]+"
REGEX_PHISHING_URLS="free-nitro\.com,discord-gift\.ru"
```

## Validation

To validate your configuration:

```bash
# Check required variables are set
cargo run --bin check-config

# Or manually verify
echo $DISCORD_TOKEN
echo $GEMINI_API_KEY
echo $DISCORD_CLIENT_ID
echo $DISCORD_CLIENT_SECRET
```

## Environment-Specific Configurations

### Development

```bash
DASHBOARD_URL="http://localhost:3000"
WEB_PORT="8081"
DATABASE_PATH="murdoch_dev.db"
RUST_LOG="debug"
REDIS_ENABLED="false"
```

### Staging

```bash
DASHBOARD_URL="https://staging.example.com"
WEB_PORT="8081"
DATABASE_PATH="/data/murdoch_staging.db"
RUST_LOG="info"
REDIS_ENABLED="true"
REDIS_URL="redis://staging-redis:6379"
```

### Production

```bash
DASHBOARD_URL="https://dashboard.example.com"
WEB_PORT="8081"
DATABASE_PATH="/data/murdoch.db"
RUST_LOG="info"
REDIS_ENABLED="true"
REDIS_URL="redis://prod-redis:6379"
PROMETHEUS_ENABLED="true"
```

## Troubleshooting

### Missing Required Variables

**Error**: `DISCORD_TOKEN not set`

**Solution**: Ensure all required variables are set in your environment or `.env` file.

### Invalid Values

**Error**: `Failed to parse BUFFER_FLUSH_THRESHOLD`

**Solution**: Ensure numeric variables contain valid integers.

### OAuth Redirect Mismatch

**Error**: `redirect_uri_mismatch`

**Solution**: Ensure DASHBOARD_URL matches the redirect URI in Discord Developer Portal exactly.

### Database Permission Issues

**Error**: `unable to open database file`

**Solution**: Ensure the directory for DATABASE_PATH exists and is writable.

## Security Best Practices

1. **Never commit secrets**: Use `.env` files and add them to `.gitignore`
2. **Rotate tokens regularly**: Update DISCORD_TOKEN and API keys every 90 days
3. **Use strong session secrets**: Generate with `openssl rand -base64 32`
4. **Restrict file permissions**: `chmod 600 .env` to prevent unauthorized access
5. **Use environment-specific configs**: Separate dev/staging/prod configurations
6. **Monitor for leaks**: Use tools like `git-secrets` to prevent accidental commits

## See Also

- [DEPLOYMENT.md](./DEPLOYMENT.md) - Deployment instructions
- [SCALING.md](./SCALING.md) - Scaling recommendations
- [MONITORING.md](./MONITORING.md) - Monitoring and observability
