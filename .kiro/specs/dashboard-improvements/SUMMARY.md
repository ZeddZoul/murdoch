# Dashboard Improvements Specification - Summary

**Last Updated**: 2026-01-27  
**Status**: Ready for Implementation  
**Deployment Platform**: Shuttle.rs

## Executive Summary

This specification defines a production-ready dashboard improvement plan leveraging Rust's zero-cost abstractions for sub-200ms response times and 10,000+ concurrent user support. The implementation prioritizes critical bug fixes (empty state handling, missing user context) before adding real-time features, RBAC, and operational tooling.

## Key Technical Decisions

### ✅ Deployment: Shuttle.rs (Confirmed)

- **Why**: Built for Rust, zero-config, built-in SQLite, simpler than Railway
- **Benefits**: Native async support, automatic migrations, no Docker needed
- **Cost**: More economical than Railway for Rust workloads

### ✅ Caching: Moka (Not Redis)

- **Why**: 10x faster (no network), simpler deployment, lock-free
- **Performance**: <1μs cache hit vs ~1ms for Redis
- **Trade-off**: Single-instance only (acceptable for current scale)

### ✅ WebSocket: tokio-tungstenite with broadcast channels

- **Why**: Lock-free MPMC, sub-100μs broadcast latency, 10K+ connections
- **Trade-off**: In-memory only, no horizontal scaling (add Redis Pub/Sub later if needed)

### ✅ RBAC: Compile-Time Type State Pattern

- **Why**: Zero runtime overhead, impossible to bypass, refactoring-safe
- **Innovation**: `Authenticated<Owner>` ensures only owners can delete - enforced by compiler!

## Architecture at a Glance

```
┌─────────────────────────────────────────────────────────────────┐
│                        Axum Web Server                          │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │   HTTP Routes (/api/*)   │   WebSocket (/ws)            │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │          Moka Cache (lock-free, TTL-based)               │   │
│  │   Metrics: 5min  │  Users: 1hr  │  Config: 10min        │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │        Business Logic (Type-Safe RBAC, Services)         │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                SQLite (via sqlx)                         │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘

Real-Time Events: tokio::sync::broadcast (lock-free MPMC)
Concurrent State: Arc<DashMap> (lock-free HashMap)
```

## Performance Targets

| Metric                | Target  | Method                                 |
| --------------------- | ------- | -------------------------------------- |
| API Response (p95)    | <200ms  | Moka cache + indexed queries           |
| Cache Hit Rate        | >80%    | 5min TTL for metrics, 1hr for users    |
| WebSocket Latency     | <500ms  | Direct broadcast via Arc<WsEvent>      |
| Concurrent Users      | 10,000+ | Lock-free DashMap + broadcast channels |
| Memory per Connection | ~1KB    | Minimal state, Arc-based sharing       |
| Cache Lookup          | <1μs    | Moka in-memory, no serialization       |

## Implementation Phases

### Phase 1: Critical Fixes (Week 1) - P0

**Goal**: Dashboard displays real data correctly

1. Add dependencies (moka, dashmap, tokio-tungstenite)
2. Database migration (5 new tables, 12 indexes)
3. Implement Moka cache layer (sub-μs lookups)
4. Fix empty state handling (`#[derive(Default)]`)
5. Add user service with 3-tier caching
6. Enhance violation endpoints with user info

**Outcome**: Dashboard works with accurate data, sub-200ms response times

### Phase 2: Real-Time (Week 2) - P0

**Goal**: Live updates without page refresh

1. Implement WebSocket manager (broadcast channels)
2. Integrate WebSocket events (violations, metrics, config)
3. Update frontend client (auto-reconnect, smooth updates)

**Outcome**: Events appear in <500ms, 1000+ concurrent connections

### Phase 3: RBAC & Exports (Week 3) - P1

**Goal**: Production-ready security and reporting

1. Implement type-safe RBAC (compile-time checks)
2. Add export service (CSV/JSON, 30-day retention)

**Outcome**: Zero-runtime RBAC, comprehensive exports

### Phase 4: Polish (Week 4) - P2

**Goal**: Professional UX and monitoring

1. Add theme support (dark/light)
2. Implement notification system (webhooks, in-app)
3. Add monitoring (Prometheus, health checks)

**Outcome**: Production-ready with full observability

### Phase 5: Testing & Docs (Week 5-6) - P2

**Goal**: Validated and documented

1. Comprehensive testing (property, integration, load)
2. Deployment documentation (Shuttle.rs guide)
3. Operational runbooks

**Outcome**: >80% test coverage, complete documentation

## Rust-Specific Best Practices Used

### Zero-Cost Abstractions

- `Arc<T>` for shared ownership (no cloning)
- `#[derive(Default)]` for empty states
- Phantom types for RBAC (zero runtime cost)
- `sqlx::query!` for compile-time SQL validation

### Fearless Concurrency

- `Arc<DashMap>` for lock-free shared state
- `tokio::sync::broadcast` for MPMC event distribution
- `moka::Cache` for concurrent cache access
- Structured concurrency with `tokio::spawn`

### Type Safety

- `Result<T, E>` everywhere, never panic
- `Option<T>` for nullable fields
- Type-state pattern for RBAC
- `#[serde(default)]` for backward compatibility

### Memory Efficiency

- `Arc<str>` for shared strings
- `Arc<[T]>` for shared slices
- Zero-copy WebSocket broadcasts
- TTL-based cache eviction (no memory leaks)

## Deployment Checklist

- [ ] Set Shuttle secrets: `DISCORD_TOKEN`, `GEMINI_API_KEY`, etc.
- [ ] Run database migrations: `sqlx migrate run`
- [ ] Deploy to Shuttle: `shuttle deploy`
- [ ] Verify health check: `curl https://murdoch.shuttleapp.rs/health`
- [ ] Check Prometheus metrics: `/metrics`
- [ ] Test WebSocket: Connect via browser DevTools
- [ ] Load test: `wrk -t4 -c100 -d30s` (expect >1000 req/s)

## Success Criteria

✅ All empty states display zeros, no errors  
✅ User info (username/avatar) in all lists  
✅ WebSocket updates <500ms  
✅ Cache hit rate >80%  
✅ API response <200ms (p95)  
✅ 1000+ concurrent WebSocket connections  
✅ Zero runtime RBAC overhead  
✅ Export all analytics (CSV/JSON)  
✅ Dark/light theme  
✅ Prometheus metrics  
✅ Health check <100ms  
✅ Test coverage >80%  
✅ Lighthouse score >90

## Risks & Mitigations

| Risk                         | Impact | Mitigation                                        |
| ---------------------------- | ------ | ------------------------------------------------- |
| Moka cache memory usage      | Medium | Set max_capacity, monitor via /metrics            |
| WebSocket connection storms  | Medium | Limit 5 per user, add rate limiting               |
| Discord API rate limits      | High   | Aggressive caching (1hr TTL), exponential backoff |
| SQLite performance at scale  | Medium | Add indexes, consider Shuttle Postgres later      |
| Compile-time RBAC complexity | Low    | Well-documented, property tests validate          |

## Next Steps

1. Review and approve this specification
2. Start Phase 1, Task 1.1 (Add Dependencies)
3. Update DEVLOG.md after each task completion
4. Run `cargo clippy` and `cargo test` continuously
5. Deploy to Shuttle after each phase for validation

## Questions?

- **Redis vs Moka**: Moka is faster, simpler, and sufficient for current scale. Add Redis Pub/Sub later only if horizontal scaling is needed.
- **Railway vs Shuttle**: Shuttle is better for Rust - native async, zero config, built-in SQLite. Railway is for polyglot teams.
- **Type-state RBAC**: Compile-time role checks mean zero runtime overhead and impossible to bypass. Worth the learning curve.
- **WebSocket vs SSE**: WebSocket for bidirectional (subscriptions), SSE is one-way only.

---

**Ready to implement!** Start with Phase 1, Task 1.1 and update DEVLOG.md after each completion.
