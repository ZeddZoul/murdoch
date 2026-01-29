# Scaling Guide

This guide provides recommendations for scaling the Murdoch Discord moderation bot and dashboard based on server size and traffic patterns.

## Table of Contents

- [Server Size Categories](#server-size-categories)
- [Small Servers (< 1,000 members)](#small-servers--1000-members)
- [Medium Servers (1,000-10,000 members)](#medium-servers-1000-10000-members)
- [Large Servers (> 10,000 members)](#large-servers--10000-members)
- [Performance Metrics](#performance-metrics)
- [Capacity Planning](#capacity-planning)
- [Optimization Strategies](#optimization-strategies)
- [Monitoring and Alerts](#monitoring-and-alerts)

## Server Size Categories

We categorize Discord servers into three tiers based on member count and expected message volume:

| Category | Members | Messages/Day | Violations/Day | Concurrent Users |
|----------|---------|--------------|----------------|------------------|
| Small    | < 1,000 | < 10,000     | < 100          | < 50             |
| Medium   | 1,000-10,000 | 10,000-100,000 | 100-1,000 | 50-500 |
| Large    | > 10,000 | > 100,000    | > 1,000        | > 500            |

## Small Servers (< 1,000 members)

### Recommended Configuration

**Infrastructure**:
- Single Shuttle.rs instance (Free or Starter tier)
- SQLite database (local file)
- In-memory caching (no Redis)
- Local file storage for exports

**Resource Requirements**:
- CPU: 0.5-1 vCPU
- Memory: 512 MB - 1 GB
- Storage: 1-5 GB
- Bandwidth: Minimal (< 1 GB/month)

**Environment Variables**:
```bash
# Use defaults for most settings
BUFFER_FLUSH_THRESHOLD="10"
BUFFER_TIMEOUT_SECS="30"
REDIS_ENABLED="false"
DATABASE_PATH="murdoch.db"
```

### Performance Expectations

- API response time: < 100ms (p99)
- WebSocket latency: < 200ms
- Dashboard load time: < 2 seconds
- Concurrent WebSocket connections: 50+
- Database size growth: ~10 MB/month

### Cost Estimate

- Shuttle.rs: $0-10/month (Free or Starter tier)
- Total: **$0-10/month**

### Scaling Triggers

Consider upgrading to Medium configuration when:
- Member count exceeds 800
- Message volume exceeds 8,000/day
- Dashboard response time exceeds 200ms
- Database size exceeds 500 MB

---

## Medium Servers (1,000-10,000 members)

### Recommended Configuration

**Infrastructure**:
- Single Shuttle.rs instance (Pro tier)
- SQLite or PostgreSQL database
- Redis cache (optional but recommended)
- S3 or equivalent for exports

**Resource Requirements**:
- CPU: 1-2 vCPU
- Memory: 2-4 GB
- Storage: 10-50 GB
- Bandwidth: 5-20 GB/month

**Environment Variables**:
```bash
# Optimized for medium traffic
BUFFER_FLUSH_THRESHOLD="25"
BUFFER_TIMEOUT_SECS="60"
REDIS_ENABLED="true"
REDIS_URL="redis://your-redis-instance:6379"
DATABASE_PATH="/data/murdoch.db"
```

### Database Considerations

**SQLite** (up to 5,000 members):
- Pros: Simple, no external dependencies, good performance
- Cons: Limited concurrent writes, single-server only
- Recommendation: Use for 1,000-5,000 members

**PostgreSQL** (5,000+ members):
- Pros: Better concurrent writes, supports read replicas, more scalable
- Cons: Requires external service, more complex setup
- Recommendation: Use for 5,000+ members

### Caching Strategy

**Redis Configuration**:
```bash
# Redis cache settings
REDIS_URL="redis://your-redis:6379"
REDIS_ENABLED="true"

# Cache TTLs (in seconds)
# Metrics: 5 minutes
# User info: 1 hour
# Config: 10 minutes
```

**Expected Cache Hit Rates**:
- Metrics: 80-90%
- User info: 90-95%
- Config: 95-99%

### Performance Expectations

- API response time: < 150ms (p99)
- WebSocket latency: < 300ms
- Dashboard load time: < 3 seconds
- Concurrent WebSocket connections: 500+
- Database size growth: ~50-100 MB/month

### Cost Estimate

- Shuttle.rs Pro: $20-50/month
- Redis (managed): $10-30/month
- S3 storage: $1-5/month
- Total: **$31-85/month**

### Optimization Tips

1. **Enable Redis caching**: Reduces database load by 70-80%
2. **Increase buffer thresholds**: Reduces write frequency
3. **Archive old violations**: Keep last 90 days in main table
4. **Use database indexes**: Ensure all indexes from schema are created
5. **Monitor cache hit rates**: Aim for > 80% hit rate

### Scaling Triggers

Consider upgrading to Large configuration when:
- Member count exceeds 8,000
- Message volume exceeds 80,000/day
- API response time exceeds 300ms
- Database size exceeds 5 GB
- Cache hit rate drops below 70%

---

## Large Servers (> 10,000 members)

### Recommended Configuration

**Infrastructure**:
- Multiple Shuttle.rs instances behind load balancer
- PostgreSQL with read replicas
- Redis cluster for distributed caching
- S3 for exports and backups
- Separate WebSocket server pool (optional)

**Resource Requirements**:
- CPU: 4-8 vCPU (per instance)
- Memory: 8-16 GB (per instance)
- Storage: 100-500 GB
- Bandwidth: 50-200 GB/month

**Environment Variables**:
```bash
# Optimized for high traffic
BUFFER_FLUSH_THRESHOLD="50"
BUFFER_TIMEOUT_SECS="120"
REDIS_ENABLED="true"
REDIS_URL="redis://redis-cluster:6379"
DATABASE_PATH="postgresql://user:pass@db-primary:5432/murdoch"
```

### Architecture

```
                    ┌─────────────────┐
                    │  Load Balancer  │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
         ┌────▼────┐    ┌────▼────┐   ┌────▼────┐
         │ App     │    │ App     │   │ App     │
         │ Instance│    │ Instance│   │ Instance│
         │ 1       │    │ 2       │   │ 3       │
         └────┬────┘    └────┬────┘   └────┬────┘
              │              │              │
              └──────────────┼──────────────┘
                             │
                    ┌────────▼────────┐
                    │  Redis Cluster  │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
         ┌────▼────┐    ┌────▼────┐   ┌────▼────┐
         │ Primary │    │ Read    │   │ Read    │
         │ DB      │───▶│ Replica │   │ Replica │
         └─────────┘    └─────────┘   └─────────┘
```

### Database Optimization

**PostgreSQL Configuration**:
```sql
-- Connection pooling
max_connections = 200
shared_buffers = 4GB
effective_cache_size = 12GB
work_mem = 64MB

-- Write optimization
wal_buffers = 16MB
checkpoint_completion_target = 0.9
max_wal_size = 4GB

-- Query optimization
random_page_cost = 1.1
effective_io_concurrency = 200
```

**Read Replicas**:
- Use for read-heavy operations (metrics, analytics)
- Route writes to primary, reads to replicas
- Configure automatic failover

**Partitioning Strategy**:
```sql
-- Partition violations table by month
CREATE TABLE violations_2024_01 PARTITION OF violations
    FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');

-- Automatically create partitions
CREATE EXTENSION pg_partman;
```

### Caching Strategy

**Redis Cluster Configuration**:
```bash
# 3-node cluster for high availability
redis-cli --cluster create \
  redis1:6379 redis2:6379 redis3:6379 \
  --cluster-replicas 1
```

**Cache Warming**:
- Pre-populate cache on startup
- Warm frequently accessed data
- Use background jobs for cache refresh

**Cache Eviction Policy**:
- Use LRU (Least Recently Used) for most caches
- Use TTL for time-sensitive data
- Monitor eviction rates

### WebSocket Scaling

**Dedicated WebSocket Servers** (optional for > 50,000 members):
- Separate WebSocket handling from API servers
- Use sticky sessions for connection persistence
- Scale horizontally based on connection count

**Connection Limits**:
- Limit 5 connections per user per server
- Monitor connection count per instance
- Auto-scale when connections exceed 80% capacity

### Performance Expectations

- API response time: < 200ms (p99)
- WebSocket latency: < 500ms
- Dashboard load time: < 4 seconds
- Concurrent WebSocket connections: 5,000+
- Database size growth: ~500 MB - 1 GB/month

### Cost Estimate

- Shuttle.rs Enterprise: $200-500/month (3 instances)
- PostgreSQL (managed): $100-300/month
- Redis Cluster: $50-150/month
- S3 storage: $10-50/month
- Load Balancer: $20-50/month
- Total: **$380-1,050/month**

### High Availability

**Multi-Region Deployment** (optional):
- Deploy to multiple regions for redundancy
- Use global load balancer
- Replicate database across regions
- Estimated additional cost: +50-100%

**Disaster Recovery**:
- Automated backups every 6 hours
- Point-in-time recovery (7 days)
- Cross-region backup replication
- Regular disaster recovery drills

---

## Performance Metrics

### Key Performance Indicators (KPIs)

| Metric | Small | Medium | Large | Critical Threshold |
|--------|-------|--------|-------|-------------------|
| API Response Time (p99) | < 100ms | < 150ms | < 200ms | > 500ms |
| WebSocket Latency | < 200ms | < 300ms | < 500ms | > 1000ms |
| Cache Hit Rate | > 70% | > 80% | > 85% | < 60% |
| Database Query Time | < 50ms | < 100ms | < 150ms | > 500ms |
| Error Rate | < 0.1% | < 0.1% | < 0.05% | > 1% |
| CPU Usage | < 50% | < 60% | < 70% | > 90% |
| Memory Usage | < 60% | < 70% | < 80% | > 95% |
| Disk I/O Wait | < 5% | < 10% | < 15% | > 30% |

### Monitoring Queries

```sql
-- Check database size
SELECT pg_size_pretty(pg_database_size('murdoch'));

-- Check table sizes
SELECT 
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;

-- Check slow queries
SELECT 
    query,
    calls,
    mean_exec_time,
    max_exec_time
FROM pg_stat_statements
WHERE mean_exec_time > 100
ORDER BY mean_exec_time DESC
LIMIT 10;
```

---

## Capacity Planning

### Growth Projections

**Small to Medium** (1,000 members):
- Expected growth: 100-200 members/month
- Time to Medium tier: 4-10 months
- Database growth: 10 MB/month
- Plan upgrade 2-3 months in advance

**Medium to Large** (10,000 members):
- Expected growth: 500-1,000 members/month
- Time to Large tier: 10-20 months
- Database growth: 100 MB/month
- Plan upgrade 3-6 months in advance

### Resource Planning Formula

```
Required CPU = (Messages/Day × 0.001) + (Members × 0.0001)
Required Memory (GB) = (Members / 1000) + (Database Size GB × 2)
Required Storage (GB) = (Violations/Day × 365 × 0.001) + (Exports × 0.1)
```

### Scaling Checklist

Before scaling up:
- [ ] Review current performance metrics
- [ ] Identify bottlenecks (CPU, memory, database, network)
- [ ] Estimate growth trajectory
- [ ] Calculate cost impact
- [ ] Plan migration strategy
- [ ] Schedule maintenance window
- [ ] Prepare rollback plan
- [ ] Update monitoring thresholds
- [ ] Test new configuration in staging
- [ ] Document changes

---

## Optimization Strategies

### Database Optimization

1. **Index Optimization**:
```sql
-- Analyze query patterns
EXPLAIN ANALYZE SELECT * FROM violations WHERE guild_id = 123;

-- Add missing indexes
CREATE INDEX CONCURRENTLY idx_violations_guild_timestamp 
ON violations(guild_id, timestamp DESC);

-- Remove unused indexes
DROP INDEX IF EXISTS unused_index;
```

2. **Query Optimization**:
- Use prepared statements
- Avoid N+1 queries
- Batch operations when possible
- Use connection pooling

3. **Data Archival**:
```sql
-- Archive old violations (> 90 days)
CREATE TABLE violations_archive AS
SELECT * FROM violations WHERE timestamp < NOW() - INTERVAL '90 days';

DELETE FROM violations WHERE timestamp < NOW() - INTERVAL '90 days';

-- Vacuum to reclaim space
VACUUM FULL violations;
```

### Caching Optimization

1. **Cache Key Design**:
```rust
// Good: Specific, versioned keys
format!("metrics:v2:{}:{}", guild_id, date)

// Bad: Generic keys
format!("metrics:{}", guild_id)
```

2. **Cache Warming**:
```rust
// Pre-populate frequently accessed data
async fn warm_cache(cache: &CacheService, guild_ids: Vec<u64>) {
    for guild_id in guild_ids {
        let _ = cache.get_metrics(guild_id).await;
    }
}
```

3. **Cache Invalidation**:
```rust
// Invalidate related caches on write
async fn record_violation(db: &Database, cache: &CacheService, violation: Violation) {
    db.insert_violation(&violation).await?;
    cache.invalidate_pattern(&format!("metrics:*:{}:*", violation.guild_id)).await;
}
```

### Application Optimization

1. **Async Batching**:
```rust
// Batch Discord API calls
let user_infos = futures::stream::iter(user_ids)
    .map(|id| fetch_user_info(id))
    .buffer_unordered(10)
    .collect::<Vec<_>>()
    .await;
```

2. **Connection Pooling**:
```rust
// Configure SQLx pool
let pool = SqlitePoolOptions::new()
    .max_connections(20)
    .acquire_timeout(Duration::from_secs(5))
    .connect(&database_url)
    .await?;
```

3. **Rate Limiting**:
```rust
// Implement per-user rate limits
let limiter = RateLimiter::keyed(Quota::per_minute(nonzero!(60u32)));
```

---

## Monitoring and Alerts

### Critical Alerts

Set up alerts for these conditions:

1. **High Error Rate** (> 1%):
```
alert: HighErrorRate
expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.01
severity: critical
```

2. **Slow Response Time** (p99 > 500ms):
```
alert: SlowResponseTime
expr: histogram_quantile(0.99, http_request_duration_seconds) > 0.5
severity: warning
```

3. **Low Cache Hit Rate** (< 60%):
```
alert: LowCacheHitRate
expr: cache_hits / (cache_hits + cache_misses) < 0.6
severity: warning
```

4. **High Memory Usage** (> 90%):
```
alert: HighMemoryUsage
expr: process_resident_memory_bytes / node_memory_MemTotal_bytes > 0.9
severity: critical
```

5. **Database Connection Pool Exhaustion**:
```
alert: DatabasePoolExhausted
expr: db_connections_active / db_connections_max > 0.9
severity: critical
```

### Dashboard Metrics

Monitor these metrics in Grafana:

- Request rate (requests/second)
- Response time (p50, p95, p99)
- Error rate (%)
- Cache hit rate (%)
- Database query time (ms)
- WebSocket connections (count)
- CPU usage (%)
- Memory usage (%)
- Disk I/O (MB/s)

### Capacity Alerts

Set up proactive alerts:

1. **Database Size** (> 80% capacity):
```
alert: DatabaseNearCapacity
expr: db_size_bytes / db_max_size_bytes > 0.8
severity: warning
```

2. **Connection Count** (> 80% limit):
```
alert: HighConnectionCount
expr: websocket_connections / websocket_max_connections > 0.8
severity: warning
```

3. **Growth Rate** (exceeding projections):
```
alert: HighGrowthRate
expr: rate(violations_total[7d]) > expected_rate * 1.5
severity: info
```

---

## Migration Procedures

### Small to Medium Migration

1. **Pre-migration**:
   - Backup database
   - Set up Redis instance
   - Update environment variables
   - Test in staging

2. **Migration**:
   - Enable maintenance mode
   - Deploy new configuration
   - Verify health checks
   - Disable maintenance mode

3. **Post-migration**:
   - Monitor performance for 24 hours
   - Verify cache hit rates
   - Check error logs
   - Update documentation

### Medium to Large Migration

1. **Pre-migration**:
   - Set up PostgreSQL with replicas
   - Configure Redis cluster
   - Set up load balancer
   - Migrate data from SQLite to PostgreSQL
   - Test thoroughly in staging

2. **Migration**:
   - Schedule maintenance window (2-4 hours)
   - Enable read-only mode
   - Sync final data changes
   - Switch DNS to load balancer
   - Verify all instances healthy
   - Enable write mode

3. **Post-migration**:
   - Monitor for 48 hours
   - Verify data consistency
   - Check replication lag
   - Test failover procedures
   - Update runbooks

---

## Summary

Choose your configuration based on current and projected needs:

- **Small servers**: Start simple with SQLite and in-memory caching
- **Medium servers**: Add Redis and consider PostgreSQL at 5,000+ members
- **Large servers**: Use PostgreSQL with replicas, Redis cluster, and multiple app instances

Monitor key metrics and scale proactively before hitting performance limits. Plan migrations 2-6 months in advance to ensure smooth transitions.

For questions or assistance with scaling, consult the [RUNBOOK.md](./RUNBOOK.md) or reach out to the development team.
