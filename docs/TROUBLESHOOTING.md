# Troubleshooting Guide

This guide covers common issues and their solutions for the Murdoch Discord moderation bot and dashboard.

## Table of Contents

- [Quick Diagnostics](#quick-diagnostics)
- [Bot Issues](#bot-issues)
- [Dashboard Issues](#dashboard-issues)
- [Database Issues](#database-issues)
- [Performance Issues](#performance-issues)
- [WebSocket Issues](#websocket-issues)
- [Authentication Issues](#authentication-issues)
- [Deployment Issues](#deployment-issues)
- [Getting Help](#getting-help)

## Quick Diagnostics

### Health Check

First, verify the system health:

```bash
# Check health endpoint
curl https://your-app.shuttleapp.rs/health

# Expected response
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 12345,
  "database": "connected",
  "cache": "available",
  "discord_api": "reachable"
}
```

### View Logs

```bash
# Shuttle.rs
shuttle logs --follow

# Filter by level
shuttle logs --level error

# Last 100 lines
shuttle logs --tail 100
```

### Check Metrics

```bash
# View Prometheus metrics
curl https://your-app.shuttleapp.rs/metrics
```

---

## Bot Issues

### Bot Not Responding to Messages

**Symptoms**:
- Bot appears online but doesn't process messages
- No violations being recorded
- Dashboard shows no activity

**Possible Causes**:

1. **Missing Intents**:
```rust
// Verify intents in main.rs
let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::MESSAGE_CONTENT
    | GatewayIntents::GUILDS;
```

**Solution**: Ensure MESSAGE_CONTENT intent is enabled in Discord Developer Portal.

2. **Rate Limiting**:
```bash
# Check logs for rate limit errors
shuttle logs | grep "rate limit"
```

**Solution**: Implement exponential backoff (already included in code).

3. **Invalid Token**:
```bash
# Verify token is set
shuttle secrets list | grep DISCORD_TOKEN
```

**Solution**: Update token with `shuttle secrets set DISCORD_TOKEN="new_token"`.

---

### Bot Crashes on Startup

**Symptoms**:
- Bot starts then immediately crashes
- Error in logs about missing configuration

**Diagnostic Steps**:

1. **Check Required Secrets**:
```bash
shuttle secrets list
```

Required secrets:
- DISCORD_TOKEN
- GEMINI_API_KEY
- DISCORD_CLIENT_ID
- DISCORD_CLIENT_SECRET
- SESSION_SECRET

2. **Check Database**:
```bash
# Verify database exists
shuttle resource list
```

3. **Check Logs**:
```bash
shuttle logs --level error
```

**Common Errors**:

**Error**: `DISCORD_TOKEN not set`
```bash
# Solution
shuttle secrets set DISCORD_TOKEN="your_token_here"
shuttle deploy
```

**Error**: `Failed to connect to database`
```bash
# Solution: Reset database
shuttle resource delete database
shuttle deploy
```

**Error**: `Invalid Discord token`
```bash
# Solution: Generate new token from Discord Developer Portal
shuttle secrets set DISCORD_TOKEN="new_token"
shuttle deploy
```

---

### Violations Not Being Detected

**Symptoms**:
- Bot is online and responding
- Messages are being sent but no violations recorded
- Dashboard shows zero violations

**Diagnostic Steps**:

1. **Check Regex Patterns**:
```bash
# View configured patterns
shuttle secrets list | grep REGEX
```

2. **Test Pattern Matching**:
```rust
// Add debug logging in filter.rs
tracing::debug!("Testing message: {}", content);
```

3. **Check Gemini API**:
```bash
# Verify API key is valid
curl -H "x-goog-api-key: $GEMINI_API_KEY" \
  https://generativelanguage.googleapis.com/v1/models
```

**Solutions**:

- **Patterns too strict**: Adjust REGEX_SLURS to be less restrictive
- **Gemini API quota exceeded**: Check Google Cloud Console for quota limits
- **Buffer not flushing**: Reduce BUFFER_TIMEOUT_SECS to flush more frequently

---

### High False Positive Rate

**Symptoms**:
- Too many innocent messages flagged as violations
- Users complaining about over-moderation

**Solutions**:

1. **Adjust Regex Patterns**:
```bash
# Use word boundaries to avoid partial matches
REGEX_SLURS="\bbadword\b,\boffensive\b"
```

2. **Tune Gemini Threshold**:
```rust
// In analyzer.rs, adjust confidence threshold
if confidence > 0.8 { // Increase from 0.5 to 0.8
    // Flag as violation
}
```

3. **Review Violation History**:
```sql
-- Find most common false positives
SELECT reason, COUNT(*) as count
FROM violations
WHERE action_taken = 'none'
GROUP BY reason
ORDER BY count DESC
LIMIT 10;
```

---

## Dashboard Issues

### Dashboard Not Loading

**Symptoms**:
- Blank page or loading spinner
- 404 or 500 errors
- "Cannot connect to server" message

**Diagnostic Steps**:

1. **Check Server Status**:
```bash
curl https://your-app.shuttleapp.rs/health
```

2. **Check Browser Console**:
- Open DevTools (F12)
- Look for JavaScript errors
- Check Network tab for failed requests

3. **Verify OAuth Configuration**:
```bash
# Check OAuth secrets
shuttle secrets list | grep DISCORD_CLIENT
```

**Common Issues**:

**Issue**: 404 on all routes
```bash
# Solution: Verify static files are being served
# Check web/ directory exists in deployment
```

**Issue**: CORS errors
```rust
// Solution: Add CORS middleware in web.rs
use tower_http::cors::CorsLayer;

let app = Router::new()
    .layer(CorsLayer::permissive());
```

**Issue**: OAuth redirect mismatch
```bash
# Solution: Update redirect URI in Discord Developer Portal
# Must match: https://your-app.shuttleapp.rs/api/auth/callback
```

---

### Dashboard Shows Empty Data

**Symptoms**:
- Dashboard loads but shows no violations
- Metrics show zero values
- "No data available" messages

**Diagnostic Steps**:

1. **Verify Database Has Data**:
```bash
# Connect to database
shuttle resource get database > temp.db
sqlite3 temp.db "SELECT COUNT(*) FROM violations;"
```

2. **Check API Endpoints**:
```bash
# Test metrics endpoint
curl -H "Cookie: session=..." \
  https://your-app.shuttleapp.rs/api/servers/123/metrics
```

3. **Check Browser Network Tab**:
- Look for 401/403 errors (authentication)
- Look for 500 errors (server issues)

**Solutions**:

**No data in database**:
- Bot may not be processing messages
- See [Bot Issues](#bot-issues) section

**Authentication errors**:
- Clear cookies and re-login
- Check SESSION_SECRET hasn't changed

**Server errors**:
- Check logs: `shuttle logs --level error`
- Verify database schema is up to date

---

### Slow Dashboard Performance

**Symptoms**:
- Pages take > 5 seconds to load
- Metrics update slowly
- Browser becomes unresponsive

**Diagnostic Steps**:

1. **Check API Response Times**:
```bash
# Measure endpoint latency
time curl -H "Cookie: session=..." \
  https://your-app.shuttleapp.rs/api/servers/123/metrics
```

2. **Check Cache Hit Rate**:
```bash
# View metrics
curl https://your-app.shuttleapp.rs/metrics | grep cache_hit_rate
```

3. **Check Database Query Performance**:
```sql
-- Enable query logging
PRAGMA query_only = ON;

-- Check slow queries
SELECT * FROM violations WHERE guild_id = 123 ORDER BY timestamp DESC;
```

**Solutions**:

**Low cache hit rate (< 60%)**:
```bash
# Enable Redis caching
shuttle secrets set REDIS_ENABLED="true"
shuttle secrets set REDIS_URL="redis://your-redis:6379"
```

**Large database**:
```sql
-- Archive old data
DELETE FROM violations WHERE timestamp < datetime('now', '-90 days');
VACUUM;
```

**Missing indexes**:
```sql
-- Verify indexes exist
SELECT name FROM sqlite_master WHERE type='index';

-- Add missing indexes (see schema in design.md)
CREATE INDEX idx_violations_guild_timestamp 
ON violations(guild_id, timestamp DESC);
```

---

## Database Issues

### Database Locked Error

**Symptoms**:
- Error: "database is locked"
- Write operations fail
- Dashboard shows stale data

**Cause**: SQLite doesn't handle high concurrent writes well.

**Solutions**:

1. **Increase Timeout**:
```rust
// In database.rs
let pool = SqlitePoolOptions::new()
    .acquire_timeout(Duration::from_secs(10))
    .connect(&database_url)
    .await?;
```

2. **Enable WAL Mode**:
```sql
PRAGMA journal_mode=WAL;
```

3. **Migrate to PostgreSQL** (for high-traffic servers):
```bash
# See SCALING.md for migration guide
```

---

### Database Corruption

**Symptoms**:
- Error: "database disk image is malformed"
- Application crashes on database queries
- Data inconsistencies

**Recovery Steps**:

1. **Backup Current Database**:
```bash
shuttle resource get database > corrupted.db
```

2. **Attempt Repair**:
```bash
# Try SQLite recovery
sqlite3 corrupted.db ".recover" | sqlite3 recovered.db
```

3. **Restore from Backup**:
```bash
# Get latest backup
# Upload to Shuttle
shuttle resource set database < backup.db
```

4. **Verify Integrity**:
```bash
sqlite3 recovered.db "PRAGMA integrity_check;"
```

---

### Database Growing Too Large

**Symptoms**:
- Database file > 1 GB
- Slow query performance
- Running out of disk space

**Solutions**:

1. **Archive Old Data**:
```sql
-- Move violations older than 90 days to archive table
CREATE TABLE violations_archive AS
SELECT * FROM violations WHERE timestamp < datetime('now', '-90 days');

DELETE FROM violations WHERE timestamp < datetime('now', '-90 days');

VACUUM;
```

2. **Enable Auto-Vacuum**:
```sql
PRAGMA auto_vacuum = FULL;
VACUUM;
```

3. **Implement Data Retention Policy**:
```rust
// Add scheduled job to clean old data
async fn cleanup_old_data(db: &Database) {
    db.execute("DELETE FROM violations WHERE timestamp < datetime('now', '-90 days')")
        .await?;
}
```

---

## Performance Issues

### High CPU Usage

**Symptoms**:
- CPU usage > 80%
- Slow response times
- Application becomes unresponsive

**Diagnostic Steps**:

1. **Check Metrics**:
```bash
curl https://your-app.shuttleapp.rs/metrics | grep process_cpu
```

2. **Profile Application**:
```bash
# Enable CPU profiling
RUST_LOG=debug cargo run
```

**Common Causes**:

1. **Regex Compilation**:
```rust
// Solution: Compile regex patterns once at startup
lazy_static! {
    static ref SLUR_REGEX: Regex = Regex::new(r"\bbadword\b").unwrap();
}
```

2. **Excessive Logging**:
```bash
# Solution: Reduce log level
shuttle secrets set RUST_LOG="info"
```

3. **Inefficient Queries**:
```sql
-- Solution: Add indexes
CREATE INDEX idx_violations_guild_timestamp 
ON violations(guild_id, timestamp DESC);
```

---

### High Memory Usage

**Symptoms**:
- Memory usage > 90%
- Out of memory errors
- Application crashes

**Diagnostic Steps**:

1. **Check Memory Usage**:
```bash
curl https://your-app.shuttleapp.rs/metrics | grep process_resident_memory
```

2. **Check Cache Size**:
```rust
// In cache.rs, check cache statistics
let stats = cache.stats();
tracing::info!("Cache entries: {}", stats.entries);
```

**Solutions**:

1. **Reduce Cache Size**:
```rust
// In cache.rs
let cache = Cache::builder()
    .max_capacity(5_000) // Reduce from 10_000
    .build();
```

2. **Enable Cache Eviction**:
```rust
// Ensure TTL is set
let cache = Cache::builder()
    .time_to_live(Duration::from_secs(300))
    .build();
```

3. **Fix Memory Leaks**:
```rust
// Ensure WebSocket connections are cleaned up
impl Drop for WebSocketConnection {
    fn drop(&mut self) {
        // Clean up resources
    }
}
```

---

## WebSocket Issues

### WebSocket Connection Fails

**Symptoms**:
- "WebSocket connection failed" in browser console
- Real-time updates not working
- Connection status shows "disconnected"

**Diagnostic Steps**:

1. **Check Browser Console**:
```javascript
// Look for WebSocket errors
WebSocket connection to 'wss://...' failed: Error during WebSocket handshake
```

2. **Test WebSocket Endpoint**:
```bash
# Use wscat to test
npm install -g wscat
wscat -c wss://your-app.shuttleapp.rs/ws
```

3. **Check Authentication**:
```bash
# Verify session cookie is being sent
# Check browser DevTools > Network > WS > Headers
```

**Solutions**:

**Authentication failure**:
- Clear cookies and re-login
- Verify SESSION_SECRET is set correctly

**CORS issues**:
```rust
// Add WebSocket CORS headers in web.rs
.layer(CorsLayer::new()
    .allow_origin(Any)
    .allow_methods([Method::GET])
    .allow_headers([UPGRADE, CONNECTION]))
```

**Firewall blocking**:
- Ensure WebSocket port is open
- Check if corporate firewall blocks WebSocket

---

### WebSocket Disconnects Frequently

**Symptoms**:
- Connection drops every few minutes
- "Reconnecting..." message appears often
- Real-time updates are intermittent

**Diagnostic Steps**:

1. **Check Ping/Pong**:
```bash
# Monitor WebSocket frames in browser DevTools
# Look for ping/pong frames
```

2. **Check Server Logs**:
```bash
shuttle logs | grep "WebSocket"
```

**Solutions**:

1. **Adjust Keepalive Timeout**:
```rust
// In websocket.rs
let ping_interval = Duration::from_secs(30); // Increase if needed
```

2. **Implement Reconnection Logic**:
```javascript
// In web/js/websocket.js
const reconnectDelay = Math.min(1000 * Math.pow(2, attempts), 60000);
setTimeout(() => connect(), reconnectDelay);
```

3. **Check Load Balancer Timeout**:
- Ensure load balancer timeout > ping interval
- Configure sticky sessions

---

## Authentication Issues

### Cannot Login to Dashboard

**Symptoms**:
- OAuth redirect fails
- "Invalid state" error
- Stuck on login page

**Diagnostic Steps**:

1. **Check OAuth Configuration**:
```bash
shuttle secrets list | grep DISCORD_CLIENT
```

2. **Verify Redirect URI**:
- Discord Developer Portal > OAuth2 > Redirects
- Must match: `https://your-app.shuttleapp.rs/api/auth/callback`

3. **Check Logs**:
```bash
shuttle logs | grep "OAuth"
```

**Solutions**:

**Redirect URI mismatch**:
```bash
# Update DASHBOARD_URL
shuttle secrets set DASHBOARD_URL="https://your-app.shuttleapp.rs"
shuttle deploy
```

**Invalid client secret**:
```bash
# Reset in Discord Developer Portal
# Update secret
shuttle secrets set DISCORD_CLIENT_SECRET="new_secret"
shuttle deploy
```

**Session cookie issues**:
- Clear browser cookies
- Try incognito mode
- Check if cookies are being blocked

---

### Session Expires Too Quickly

**Symptoms**:
- Logged out after a few minutes
- Have to re-login frequently

**Solution**:

```rust
// In oauth.rs, increase session duration
let session = Session {
    expires_at: Utc::now() + Duration::hours(24), // Increase from 1 hour
    // ...
};
```

---

### Permission Denied Errors

**Symptoms**:
- 403 Forbidden errors
- "You don't have permission" messages
- Cannot access certain pages

**Diagnostic Steps**:

1. **Check User Role**:
```sql
SELECT role FROM role_assignments 
WHERE guild_id = 123 AND user_id = 456;
```

2. **Check Audit Logs**:
```sql
SELECT * FROM audit_log 
WHERE user_id = 456 
ORDER BY timestamp DESC 
LIMIT 10;
```

**Solutions**:

**No role assigned**:
```sql
-- Assign role (as server owner)
INSERT INTO role_assignments (guild_id, user_id, role, assigned_by)
VALUES (123, 456, 'moderator', 789);
```

**Wrong role**:
```sql
-- Update role
UPDATE role_assignments 
SET role = 'admin' 
WHERE guild_id = 123 AND user_id = 456;
```

---

## Deployment Issues

### Deployment Fails

**Symptoms**:
- `shuttle deploy` command fails
- Build errors
- Deployment stuck

**Diagnostic Steps**:

1. **Check Build Logs**:
```bash
shuttle logs --build
```

2. **Verify Cargo.toml**:
```bash
# Test build locally
cargo build --release
```

3. **Check Shuttle Status**:
```bash
shuttle status
```

**Common Errors**:

**Compilation error**:
```bash
# Solution: Fix code errors
cargo clippy --all --tests --all-features
cargo test
```

**Missing dependencies**:
```bash
# Solution: Update Cargo.toml
cargo update
```

**Timeout during build**:
```bash
# Solution: Increase build timeout or optimize build
# Remove unused dependencies
```

---

### Application Crashes After Deployment

**Symptoms**:
- Deployment succeeds but app crashes immediately
- Health check fails
- Cannot access endpoints

**Diagnostic Steps**:

1. **Check Logs**:
```bash
shuttle logs --level error --tail 50
```

2. **Verify Secrets**:
```bash
shuttle secrets list
```

3. **Check Resources**:
```bash
shuttle resource list
```

**Solutions**:

**Missing secrets**:
```bash
# Set all required secrets
shuttle secrets set DISCORD_TOKEN="..."
shuttle secrets set GEMINI_API_KEY="..."
# etc.
```

**Database migration needed**:
```bash
# Run migrations
shuttle deploy --force
```

**Resource limits exceeded**:
- Upgrade Shuttle tier
- Optimize resource usage

---

## Getting Help

### Before Asking for Help

1. **Check this guide** for your specific issue
2. **Review logs** for error messages
3. **Check health endpoint** for system status
4. **Verify configuration** is correct
5. **Try in staging** if possible

### Information to Provide

When asking for help, include:

1. **Error message** (full text)
2. **Logs** (relevant sections)
3. **Configuration** (sanitized, no secrets)
4. **Steps to reproduce**
5. **Expected vs actual behavior**
6. **Environment** (Shuttle tier, server size, etc.)

### Support Channels

- **GitHub Issues**: https://github.com/your-org/murdoch/issues
- **Discord**: https://discord.gg/your-server
- **Email**: support@example.com

### Emergency Contacts

For critical production issues:

1. **Check status page**: https://status.example.com
2. **Page on-call engineer**: Use PagerDuty
3. **Emergency hotline**: +1-XXX-XXX-XXXX

---

## Preventive Maintenance

### Regular Tasks

**Daily**:
- Review error logs
- Check health metrics
- Monitor disk space

**Weekly**:
- Review performance metrics
- Check for security updates
- Test backups

**Monthly**:
- Review and archive old data
- Update dependencies
- Review and update documentation
- Test disaster recovery procedures

### Health Checks

```bash
#!/bin/bash
# health-check.sh

# Check health endpoint
curl -f https://your-app.shuttleapp.rs/health || exit 1

# Check metrics endpoint
curl -f https://your-app.shuttleapp.rs/metrics || exit 1

# Check database size
DB_SIZE=$(shuttle resource get database --info | grep size)
echo "Database size: $DB_SIZE"

# Check error rate
ERROR_RATE=$(curl -s https://your-app.shuttleapp.rs/metrics | grep error_rate)
echo "Error rate: $ERROR_RATE"
```

---

## See Also

- [DEPLOYMENT.md](./DEPLOYMENT.md) - Deployment guide
- [CONFIGURATION.md](./CONFIGURATION.md) - Configuration reference
- [SCALING.md](./SCALING.md) - Scaling recommendations
- [MONITORING.md](./MONITORING.md) - Monitoring setup
- [RUNBOOK.md](./RUNBOOK.md) - Operational procedures
