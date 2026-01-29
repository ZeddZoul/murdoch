# Monitoring Guide

This guide explains how to set up monitoring for the Murdoch Discord bot using Prometheus and Grafana.

## Overview

Murdoch exposes metrics in Prometheus format via the `/metrics` endpoint on the health check server (default port 8080). These metrics can be scraped by Prometheus and visualized in Grafana.

## Metrics Exposed

### Cache Metrics

- `cache_entries{cache="metrics|users|config"}` - Number of entries in each cache
- `cache_weighted_size` - Total weighted size of all caches in bytes
- `cache_hits` - Total cache hits (counter)
- `cache_misses` - Total cache misses (counter)
- `cache_hit_rate` - Cache hit rate (0.0 to 1.0)

### Database Metrics

- `database_violations_total` - Total number of violations recorded
- `database_warnings_total` - Total number of warnings issued
- `database_guilds_total` - Total number of guilds using the bot
- `database_size_bytes` - Database file size in bytes

### HTTP Metrics (Placeholder)

- `http_requests_total` - Total HTTP requests
- `http_response_time_seconds` - HTTP response time histogram
- `http_errors_total` - Total HTTP errors

### Build Information

- `build_info{version,timestamp}` - Build version and timestamp

## Setup Instructions

### 1. Configure Prometheus

Add the following job to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'murdoch'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

### 2. Start Prometheus

```bash
prometheus --config.file=prometheus.yml
```

### 3. Import Grafana Dashboard

1. Open Grafana (default: http://localhost:3000)
2. Navigate to Dashboards → Import
3. Upload `docs/grafana-dashboard.json`
4. Select your Prometheus datasource
5. Click Import

## Dashboard Panels

The Grafana dashboard includes the following panels:

### Cache Performance

- **Cache Hit Rate** - Gauge showing current cache hit rate percentage
- **Cache Operations Rate** - Time series of cache hits and misses per second
- **Cache Entries by Type** - Stacked time series showing entries in each cache
- **Cache Memory Usage** - Time series of total cache memory consumption

### Database Statistics

- **Total Violations** - Current count of all violations
- **Total Warnings** - Current count of all warnings
- **Total Guilds** - Number of guilds using the bot
- **Database Size** - Current database file size

### HTTP Performance

- **HTTP Request Rate** - Requests per second
- **HTTP Response Time** - Average response time in seconds

## Health Check Endpoint

The `/health` endpoint provides detailed health information:

```bash
curl http://localhost:8080/health
```

Response format:

```json
{
  "status": "healthy",
  "version": {
    "version": "0.1.0",
    "build_timestamp": "2024-01-28T12:00:00Z"
  },
  "checks": {
    "database": {
      "status": "healthy",
      "message": "Database connection successful",
      "response_time_ms": 5
    },
    "cache": {
      "status": "healthy",
      "message": "Cache operational (hit rate: 85.2%)",
      "response_time_ms": 1
    },
    "discord_api": {
      "status": "healthy",
      "message": "Discord API reachable",
      "response_time_ms": 120
    }
  }
}
```

## Environment Variables

- `HEALTH_PORT` - Port for health check server (default: 8080)
- `DATABASE_PATH` - Path to SQLite database (default: murdoch.db)

## Alerting

You can configure Prometheus alerts based on these metrics. Example alert rules:

```yaml
groups:
  - name: murdoch
    rules:
      - alert: LowCacheHitRate
        expr: cache_hit_rate < 0.5
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Cache hit rate is below 50%"
          description: "Cache hit rate is {{ $value | humanizePercentage }}"

      - alert: HighDatabaseSize
        expr: database_size_bytes > 1073741824  # 1GB
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Database size exceeds 1GB"
          description: "Database size is {{ $value | humanize1024 }}"

      - alert: HealthCheckFailed
        expr: up{job="murdoch"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Murdoch health check failed"
          description: "Health check endpoint is not responding"
```

## Troubleshooting

### Metrics endpoint returns 404

- Verify the health server is running on the correct port
- Check `HEALTH_PORT` environment variable
- Ensure the `/metrics` route is accessible

### No data in Grafana

- Verify Prometheus is scraping the metrics endpoint
- Check Prometheus targets page: http://localhost:9090/targets
- Ensure the datasource is configured correctly in Grafana

### Cache hit rate is 0

- This is normal on startup when caches are empty
- Wait for the bot to process some requests
- Check that the cache service is initialized correctly

## Production Recommendations

1. **Use a dedicated monitoring server** - Don't run Prometheus/Grafana on the same host as the bot
2. **Set up alerting** - Configure alerts for critical metrics
3. **Enable authentication** - Secure Grafana with authentication
4. **Regular backups** - Back up Prometheus data and Grafana dashboards
5. **Retention policy** - Configure appropriate data retention in Prometheus
6. **Resource limits** - Set memory limits for Prometheus to prevent OOM

## Additional Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
