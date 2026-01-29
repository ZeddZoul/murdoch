# Deployment Guide

This guide covers deploying the Murdoch Discord moderation bot and dashboard to production using Shuttle.rs.

## Prerequisites

- Rust toolchain (1.70+)
- [Shuttle CLI](https://docs.shuttle.rs/getting-started/installation) installed
- Discord bot token and OAuth credentials
- Gemini API key

## Shuttle.rs Deployment

### Initial Setup

1. **Install Shuttle CLI**:

```bash
cargo install cargo-shuttle
```

2. **Login to Shuttle**:

```bash
shuttle login
```

3. **Initialize Project** (if not already done):

```bash
shuttle init --from .
```

### Secrets Configuration

Shuttle uses a secrets management system for sensitive configuration. Set secrets using the CLI:

#### Required Secrets

```bash
# Discord bot token (from Discord Developer Portal > Bot > Token)
shuttle secrets set DISCORD_TOKEN="your_bot_token_here"

# Gemini API key (from https://aistudio.google.com/app/apikey)
shuttle secrets set GEMINI_API_KEY="your_gemini_api_key_here"

# Discord OAuth credentials (from Discord Developer Portal > OAuth2)
shuttle secrets set DISCORD_CLIENT_ID="your_client_id_here"
shuttle secrets set DISCORD_CLIENT_SECRET="your_client_secret_here"

# Dashboard URL (will be your Shuttle deployment URL)
shuttle secrets set DASHBOARD_URL="https://murdoch.shuttleapp.rs"

# Session secret for web dashboard (generate a random 32+ character string)
shuttle secrets set SESSION_SECRET="$(openssl rand -base64 32)"
```

#### Optional Secrets

```bash
# Moderator role ID for @mentions on high-severity violations
shuttle secrets set MOD_ROLE_ID="123456789012345678"

# Buffer configuration
shuttle secrets set BUFFER_FLUSH_THRESHOLD="10"
shuttle secrets set BUFFER_TIMEOUT_SECS="30"

# Health check port (default: 8080)
shuttle secrets set HEALTH_PORT="8080"

# Web dashboard port (default: 8081)
shuttle secrets set WEB_PORT="8081"

# Database path (default: murdoch.db)
shuttle secrets set DATABASE_PATH="murdoch.db"

# Custom regex patterns (comma-separated)
shuttle secrets set REGEX_SLURS="badword1,badword2"
shuttle secrets set REGEX_INVITE_LINKS="discord\.gg/[a-zA-Z0-9]+"
shuttle secrets set REGEX_PHISHING_URLS="free-nitro\.com"
```

### Deployment Commands

#### Deploy to Production

```bash
# Deploy the application
shuttle deploy

# View deployment status
shuttle status

# View logs
shuttle logs

# View recent logs with follow
shuttle logs --follow
```

#### Local Development

```bash
# Run locally with Shuttle runtime
shuttle run

# This will:
# - Load secrets from Shuttle
# - Start the bot and web server
# - Enable hot reload on code changes
```

### Post-Deployment Configuration

1. **Update Discord OAuth Redirect URI**:
   - Go to Discord Developer Portal > OAuth2
   - Add redirect URI: `https://your-app.shuttleapp.rs/api/auth/callback`

2. **Verify Health Check**:

```bash
curl https://your-app.shuttleapp.rs/health
```

Expected response:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 123,
  "database": "connected",
  "cache": "available",
  "discord_api": "reachable"
}
```

3. **Access Dashboard**:
   - Navigate to `https://your-app.shuttleapp.rs`
   - Login with Discord OAuth

### Managing Secrets

#### View Current Secrets

```bash
# List all secret keys (values are hidden)
shuttle secrets list
```

#### Update a Secret

```bash
# Update an existing secret
shuttle secrets set DISCORD_TOKEN="new_token_here"

# Restart the service to apply changes
shuttle deploy
```

#### Delete a Secret

```bash
shuttle secrets unset SECRET_NAME
```

### Database Management

Shuttle provides persistent storage for SQLite databases:

#### Backup Database

```bash
# Download database file
shuttle resource get database > murdoch_backup.db
```

#### Restore Database

```bash
# Upload database file
shuttle resource set database < murdoch_backup.db
```

#### View Database Size

```bash
shuttle resource list
```

### Monitoring and Logs

#### View Real-Time Logs

```bash
# Follow logs in real-time
shuttle logs --follow

# Filter by log level
shuttle logs --level error

# View last N lines
shuttle logs --tail 100
```

#### Prometheus Metrics

Access metrics at: `https://your-app.shuttleapp.rs/metrics`

Metrics include:
- Request count and latency
- Cache hit rate
- WebSocket connection count
- Database query performance
- Error rates

### Scaling

Shuttle automatically handles scaling based on load. For custom scaling:

```bash
# View current resource allocation
shuttle resource list

# Request more resources (contact Shuttle support)
```

### Troubleshooting

#### Deployment Fails

```bash
# Check build logs
shuttle logs --build

# Verify secrets are set
shuttle secrets list

# Check Shuttle.toml configuration
cat Shuttle.toml
```

#### Application Crashes

```bash
# View error logs
shuttle logs --level error

# Check health endpoint
curl https://your-app.shuttleapp.rs/health

# Restart the service
shuttle deploy --force
```

#### Database Issues

```bash
# Verify database exists
shuttle resource list

# Check database size and limits
shuttle resource get database --info

# Backup and restore if corrupted
shuttle resource get database > backup.db
# Fix database locally
shuttle resource set database < fixed.db
```

#### WebSocket Connection Issues

1. Verify DASHBOARD_URL is set correctly
2. Check browser console for connection errors
3. Ensure firewall allows WebSocket connections
4. Review logs for authentication failures

### Security Best Practices

1. **Rotate Secrets Regularly**:
   - Update DISCORD_TOKEN every 90 days
   - Rotate SESSION_SECRET monthly
   - Update API keys on suspected compromise

2. **Monitor Access Logs**:
   - Review authentication failures
   - Check for unusual API usage patterns
   - Monitor permission denial logs

3. **Database Backups**:
   - Automated daily backups (see docs/BACKUP.md)
   - Test restore procedures monthly
   - Store backups securely off-platform

4. **Rate Limiting**:
   - Shuttle provides DDoS protection
   - Application-level rate limiting is enabled
   - Monitor for abuse patterns

### Cost Optimization

Shuttle pricing is based on resource usage:

- **Free Tier**: Suitable for small servers (< 1000 members)
- **Pro Tier**: Recommended for medium servers (1000-10000 members)
- **Enterprise**: Required for large servers (> 10000 members)

Optimization tips:
- Enable caching to reduce database queries
- Use WebSocket for real-time updates (more efficient than polling)
- Archive old violations to reduce database size
- Monitor metrics to identify bottlenecks

### Support and Resources

- [Shuttle Documentation](https://docs.shuttle.rs)
- [Shuttle Discord](https://discord.gg/shuttle)
- [Murdoch GitHub Issues](https://github.com/your-org/murdoch/issues)

### Next Steps

- Review [CONFIGURATION.md](./CONFIGURATION.md) for detailed environment variable documentation
- Read [SCALING.md](./SCALING.md) for scaling recommendations
- Check [MONITORING.md](./MONITORING.md) for observability setup
- See [RUNBOOK.md](./RUNBOOK.md) for operational procedures
