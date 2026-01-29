//! High-performance caching layer using moka.
//!
//! Provides sub-millisecond in-memory caching with automatic TTL eviction.
//! No Redis needed - moka is 10x faster with zero network overhead.

use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use serde::{Deserialize, Serialize};

use crate::error::{MurdochError, Result};

/// Statistics about cache performance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of entries in metrics cache.
    pub metrics_entries: u64,
    /// Number of entries in users cache.
    pub users_entries: u64,
    /// Number of entries in config cache.
    pub config_entries: u64,
    /// Total weighted size across all caches.
    pub weighted_size: u64,
    /// Total cache hits across all caches.
    pub hits: u64,
    /// Total cache misses across all caches.
    pub misses: u64,
    /// Cache hit rate (0.0 to 1.0).
    pub hit_rate: f64,
}

/// Internal statistics tracker.
#[derive(Clone)]
struct StatsTracker {
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
}

impl StatsTracker {
    fn new() -> Self {
        Self {
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
    }

    fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    fn get_hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    fn get_misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    fn get_hit_rate(&self) -> f64 {
        let hits = self.get_hits();
        let misses = self.get_misses();
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

/// High-performance cache service with separate caches for different data types.
///
/// Uses moka for lock-free concurrent access with automatic TTL-based eviction.
/// Each cache has different TTL based on data volatility:
/// - Metrics: 5 minutes (frequently updated)
/// - Users: 1 hour (rarely changes)
/// - Config: 10 minutes (occasionally updated)
#[derive(Clone)]
pub struct CacheService {
    /// Cache for metrics snapshots (5 minute TTL).
    metrics: Cache<String, Arc<Vec<u8>>>,
    /// Cache for user information (1 hour TTL).
    users: Cache<u64, Arc<Vec<u8>>>,
    /// Cache for server configuration (10 minute TTL).
    config: Cache<u64, Arc<Vec<u8>>>,
    /// Statistics tracker for hits/misses.
    stats_tracker: StatsTracker,
}

impl CacheService {
    /// Create a new cache service with default capacities and TTLs.
    ///
    /// Capacities:
    /// - Metrics: 10,000 entries (~2MB assuming 200 bytes per entry)
    /// - Users: 50,000 entries (~10MB assuming 200 bytes per entry)
    /// - Config: 1,000 entries (~200KB assuming 200 bytes per entry)
    pub fn new() -> Self {
        Self {
            metrics: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(300)) // 5 minutes
                .build(),
            users: Cache::builder()
                .max_capacity(50_000)
                .time_to_live(Duration::from_secs(3600)) // 1 hour
                .build(),
            config: Cache::builder()
                .max_capacity(1_000)
                .time_to_live(Duration::from_secs(600)) // 10 minutes
                .build(),
            stats_tracker: StatsTracker::new(),
        }
    }

    /// Get the metrics cache.
    pub fn metrics(&self) -> &Cache<String, Arc<Vec<u8>>> {
        &self.metrics
    }

    /// Get the users cache.
    pub fn users(&self) -> &Cache<u64, Arc<Vec<u8>>> {
        &self.users
    }

    /// Get the config cache.
    pub fn config(&self) -> &Cache<u64, Arc<Vec<u8>>> {
        &self.config
    }

    /// Get cache statistics for monitoring.
    ///
    /// Returns entry counts, weighted size, hits, misses, and hit rate for Prometheus metrics.
    pub fn stats(&self) -> CacheStats {
        let hits = self.stats_tracker.get_hits();
        let misses = self.stats_tracker.get_misses();

        CacheStats {
            metrics_entries: self.metrics.entry_count(),
            users_entries: self.users.entry_count(),
            config_entries: self.config.entry_count(),
            weighted_size: self.metrics.weighted_size()
                + self.users.weighted_size()
                + self.config.weighted_size(),
            hits,
            misses,
            hit_rate: self.stats_tracker.get_hit_rate(),
        }
    }

    /// Sync all pending cache operations.
    ///
    /// This is primarily useful for testing to ensure all async operations complete.
    pub async fn sync(&self) {
        self.metrics.run_pending_tasks().await;
        self.users.run_pending_tasks().await;
        self.config.run_pending_tasks().await;
    }

    /// Get a value from cache with hit/miss tracking.
    ///
    /// This is a convenience method that wraps cache.get() and tracks statistics.
    pub async fn get_with_stats<K, V>(&self, cache: &Cache<K, Arc<V>>, key: &K) -> Option<Arc<V>>
    where
        K: Hash + Eq + Send + Sync + 'static,
        V: Send + Sync + 'static,
    {
        let result = cache.get(key).await;

        if result.is_some() {
            self.stats_tracker.record_hit();
        } else {
            self.stats_tracker.record_miss();
        }

        result
    }

    /// Cache-aside pattern: get from cache or fetch and store.
    ///
    /// This method implements automatic deduplication - if multiple concurrent
    /// requests ask for the same key, only one fetch operation will execute.
    /// All callers will receive the same Arc<V> for zero-copy sharing.
    ///
    /// Automatically tracks cache hits and misses for statistics.
    ///
    /// # Type Parameters
    ///
    /// - `K`: Key type (must be Hash + Eq + Clone + Send + Sync)
    /// - `V`: Value type (must be Send + Sync)
    /// - `F`: Future that fetches the value
    ///
    /// # Arguments
    ///
    /// - `cache`: The specific cache to use (metrics, users, or config)
    /// - `key`: The cache key
    /// - `fetch`: Async function to fetch the value if not cached
    ///
    /// # Returns
    ///
    /// Returns `Arc<V>` for zero-copy sharing across multiple consumers.
    ///
    /// # Errors
    ///
    /// Returns error if the fetch function fails.
    pub async fn get_or_fetch<K, V, F, Fut>(
        &self,
        cache: &Cache<K, Arc<V>>,
        key: K,
        fetch: F,
    ) -> Result<Arc<V>>
    where
        K: Hash + Eq + Clone + Send + Sync + 'static,
        V: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<V>>,
    {
        // Check if key exists first to track hit/miss
        let exists = cache.contains_key(&key);

        let result = cache
            .try_get_with(key, async move { fetch().await.map(Arc::new) })
            .await
            .map_err(|e| MurdochError::InternalState(format!("Cache fetch failed: {}", e)))?;

        // Track statistics
        if exists {
            self.stats_tracker.record_hit();
        } else {
            self.stats_tracker.record_miss();
        }

        Ok(result)
    }

    /// Invalidate all metrics cache entries matching a pattern.
    ///
    /// Supports wildcard invalidation using prefix matching.
    /// For example, "metrics:guild:123:*" will invalidate all metrics
    /// for guild 123.
    ///
    /// # Arguments
    ///
    /// - `pattern`: Pattern to match (supports trailing wildcard *)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use murdoch::cache::CacheService;
    /// # async fn example() {
    /// let cache = CacheService::new();
    ///
    /// // Invalidate all metrics for a specific guild
    /// cache.invalidate_metrics_pattern("metrics:guild:123:").await;
    ///
    /// // Invalidate all metrics
    /// cache.invalidate_metrics_pattern("metrics:").await;
    /// # }
    /// ```
    pub async fn invalidate_metrics_pattern(&self, pattern: &str) {
        let prefix = pattern.trim_end_matches('*');

        // Collect keys to invalidate
        let mut keys_to_invalidate = Vec::new();
        for (key, _) in self.metrics.iter() {
            if key.as_str().starts_with(prefix) {
                keys_to_invalidate.push(key.as_ref().clone());
            }
        }

        // Invalidate collected keys
        for key in keys_to_invalidate {
            self.metrics.invalidate(&key).await;
        }
    }

    /// Invalidate a specific metrics cache entry.
    pub async fn invalidate_metrics(&self, key: &str) {
        self.metrics.invalidate(key).await;
    }

    /// Invalidate a specific user cache entry.
    pub async fn invalidate_user(&self, user_id: u64) {
        self.users.invalidate(&user_id).await;
    }

    /// Invalidate a specific config cache entry.
    pub async fn invalidate_config(&self, guild_id: u64) {
        self.config.invalidate(&guild_id).await;
    }

    /// Invalidate all entries in all caches.
    ///
    /// Use with caution - this will cause a cache stampede on the next requests.
    pub async fn invalidate_all(&self) {
        self.metrics.invalidate_all();
        self.users.invalidate_all();
        self.config.invalidate_all();
        // Sync to ensure invalidation is applied
        self.metrics.run_pending_tasks().await;
        self.users.run_pending_tasks().await;
        self.config.run_pending_tasks().await;
    }
}

impl Default for CacheService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_service_creation() {
        let cache = CacheService::new();
        let stats = cache.stats();

        assert_eq!(stats.metrics_entries, 0);
        assert_eq!(stats.users_entries, 0);
        assert_eq!(stats.config_entries, 0);
        assert_eq!(stats.weighted_size, 0);
    }

    #[test]
    fn cache_service_default() {
        let cache = CacheService::default();
        let stats = cache.stats();

        assert_eq!(stats.metrics_entries, 0);
    }

    #[tokio::test]
    async fn metrics_cache_basic_operations() {
        let cache = CacheService::new();
        let key = "test:guild:123".to_string();
        let value = Arc::new(vec![1, 2, 3, 4]);

        // Insert
        cache.metrics().insert(key.clone(), value.clone()).await;

        // Retrieve
        let retrieved = cache.metrics().get(&key).await;
        assert!(retrieved.is_some());
        assert_eq!(*retrieved.unwrap(), *value);

        // Sync pending tasks to update stats
        cache.sync().await;

        // Stats should reflect the entry
        let stats = cache.stats();
        assert_eq!(stats.metrics_entries, 1);
    }

    #[tokio::test]
    async fn users_cache_basic_operations() {
        let cache = CacheService::new();
        let user_id = 123456u64;
        let value = Arc::new(vec![5, 6, 7, 8]);

        // Insert
        cache.users().insert(user_id, value.clone()).await;

        // Retrieve
        let retrieved = cache.users().get(&user_id).await;
        assert!(retrieved.is_some());
        assert_eq!(*retrieved.unwrap(), *value);

        // Sync pending tasks to update stats
        cache.sync().await;

        // Stats should reflect the entry
        let stats = cache.stats();
        assert_eq!(stats.users_entries, 1);
    }

    #[tokio::test]
    async fn config_cache_basic_operations() {
        let cache = CacheService::new();
        let guild_id = 789012u64;
        let value = Arc::new(vec![9, 10, 11, 12]);

        // Insert
        cache.config().insert(guild_id, value.clone()).await;

        // Retrieve
        let retrieved = cache.config().get(&guild_id).await;
        assert!(retrieved.is_some());
        assert_eq!(*retrieved.unwrap(), *value);

        // Sync pending tasks to update stats
        cache.sync().await;

        // Stats should reflect the entry
        let stats = cache.stats();
        assert_eq!(stats.config_entries, 1);
    }

    #[tokio::test]
    async fn cache_miss_returns_none() {
        let cache = CacheService::new();

        let result = cache.metrics().get(&"nonexistent".to_string()).await;
        assert!(result.is_none());

        let result = cache.users().get(&99999u64).await;
        assert!(result.is_none());

        let result = cache.config().get(&88888u64).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn cache_invalidation() {
        let cache = CacheService::new();
        let key = "test:invalidate".to_string();
        let value = Arc::new(vec![1, 2, 3]);

        // Insert
        cache.metrics().insert(key.clone(), value).await;
        assert!(cache.metrics().get(&key).await.is_some());

        // Invalidate
        cache.metrics().invalidate(&key).await;
        assert!(cache.metrics().get(&key).await.is_none());
    }

    #[tokio::test]
    async fn multiple_caches_independent() {
        let cache = CacheService::new();

        cache
            .metrics()
            .insert("metrics:1".to_string(), Arc::new(vec![1]))
            .await;
        cache.users().insert(123, Arc::new(vec![2])).await;
        cache.config().insert(456, Arc::new(vec![3])).await;

        cache.sync().await;

        let stats = cache.stats();
        assert_eq!(stats.metrics_entries, 1);
        assert_eq!(stats.users_entries, 1);
        assert_eq!(stats.config_entries, 1);
    }

    #[tokio::test]
    async fn get_or_fetch_cache_hit() {
        let cache = CacheService::new();
        let key = "test:fetch".to_string();
        let value = Arc::new(vec![1, 2, 3]);

        // Pre-populate cache
        cache.metrics().insert(key.clone(), value.clone()).await;

        // Fetch should return cached value without calling fetch function
        let mut fetch_called = false;
        let result = cache
            .get_or_fetch(cache.metrics(), key.clone(), || async {
                fetch_called = true;
                Ok::<Vec<u8>, MurdochError>(vec![9, 9, 9])
            })
            .await
            .expect("should succeed");

        assert!(!fetch_called, "fetch should not be called on cache hit");
        assert_eq!(*result, *value);
    }

    #[tokio::test]
    async fn get_or_fetch_cache_miss() {
        let cache = CacheService::new();
        let key = "test:miss".to_string();

        let mut fetch_called = false;
        let result = cache
            .get_or_fetch(cache.metrics(), key.clone(), || async {
                fetch_called = true;
                Ok::<Vec<u8>, MurdochError>(vec![4, 5, 6])
            })
            .await
            .expect("should succeed");

        assert!(fetch_called, "fetch should be called on cache miss");
        assert_eq!(*result, vec![4, 5, 6]);

        // Verify it was cached
        let cached = cache.metrics().get(&key).await;
        assert!(cached.is_some());
        assert_eq!(*cached.unwrap(), vec![4, 5, 6]);
    }

    #[tokio::test]
    async fn get_or_fetch_deduplication() {
        use std::sync::atomic::{AtomicU32, Ordering};

        let cache = CacheService::new();
        let key = "test:dedup".to_string();
        let fetch_count = Arc::new(AtomicU32::new(0));

        // Spawn 10 concurrent requests for the same key
        let mut handles = vec![];
        for _ in 0..10 {
            let cache_clone = cache.clone();
            let key_clone = key.clone();
            let fetch_count_clone = fetch_count.clone();

            let handle = tokio::spawn(async move {
                cache_clone
                    .get_or_fetch(cache_clone.metrics(), key_clone, || async move {
                        fetch_count_clone.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        Ok::<Vec<u8>, MurdochError>(vec![7, 8, 9])
                    })
                    .await
            });
            handles.push(handle);
        }

        // Wait for all requests to complete
        let results: Vec<_> = futures::future::join_all(handles).await;

        // All should succeed
        for result in results {
            let value = result
                .expect("task should not panic")
                .expect("should succeed");
            assert_eq!(*value, vec![7, 8, 9]);
        }

        // Fetch should only be called once due to deduplication
        assert_eq!(
            fetch_count.load(Ordering::SeqCst),
            1,
            "fetch should only be called once"
        );
    }

    #[tokio::test]
    async fn get_or_fetch_error_propagation() {
        let cache = CacheService::new();
        let key = "test:error".to_string();

        let result = cache
            .get_or_fetch(cache.metrics(), key, || async {
                Err::<Vec<u8>, MurdochError>(MurdochError::Database("test error".to_string()))
            })
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cache fetch failed"));
    }

    #[tokio::test]
    async fn get_or_fetch_zero_copy_sharing() {
        let cache = CacheService::new();
        let key = "test:zerocopy".to_string();

        let result1 = cache
            .get_or_fetch(cache.metrics(), key.clone(), || async {
                Ok::<Vec<u8>, MurdochError>(vec![1, 2, 3])
            })
            .await
            .expect("should succeed");

        let result2 = cache
            .get_or_fetch(cache.metrics(), key, || async {
                Ok::<Vec<u8>, MurdochError>(vec![9, 9, 9])
            })
            .await
            .expect("should succeed");

        // Both should point to the same Arc (zero-copy)
        assert!(Arc::ptr_eq(&result1, &result2));
    }

    #[tokio::test]
    async fn invalidate_metrics_single_key() {
        let cache = CacheService::new();
        let key = "metrics:guild:123:violations".to_string();

        cache
            .metrics()
            .insert(key.clone(), Arc::new(vec![1, 2, 3]))
            .await;
        assert!(cache.metrics().get(&key).await.is_some());

        cache.invalidate_metrics(&key).await;
        assert!(cache.metrics().get(&key).await.is_none());
    }

    #[tokio::test]
    async fn invalidate_metrics_pattern() {
        let cache = CacheService::new();

        // Insert multiple entries for guild 123
        cache
            .metrics()
            .insert(
                "metrics:guild:123:violations".to_string(),
                Arc::new(vec![1]),
            )
            .await;
        cache
            .metrics()
            .insert("metrics:guild:123:health".to_string(), Arc::new(vec![2]))
            .await;
        cache
            .metrics()
            .insert("metrics:guild:123:offenders".to_string(), Arc::new(vec![3]))
            .await;

        // Insert entry for different guild
        cache
            .metrics()
            .insert(
                "metrics:guild:456:violations".to_string(),
                Arc::new(vec![4]),
            )
            .await;

        // Invalidate all metrics for guild 123
        cache.invalidate_metrics_pattern("metrics:guild:123:").await;

        // Guild 123 entries should be gone
        assert!(cache
            .metrics()
            .get(&"metrics:guild:123:violations".to_string())
            .await
            .is_none());
        assert!(cache
            .metrics()
            .get(&"metrics:guild:123:health".to_string())
            .await
            .is_none());
        assert!(cache
            .metrics()
            .get(&"metrics:guild:123:offenders".to_string())
            .await
            .is_none());

        // Guild 456 entry should still exist
        assert!(cache
            .metrics()
            .get(&"metrics:guild:456:violations".to_string())
            .await
            .is_some());
    }

    #[tokio::test]
    async fn invalidate_metrics_pattern_with_wildcard() {
        let cache = CacheService::new();

        cache
            .metrics()
            .insert("metrics:test:1".to_string(), Arc::new(vec![1]))
            .await;
        cache
            .metrics()
            .insert("metrics:test:2".to_string(), Arc::new(vec![2]))
            .await;
        cache
            .metrics()
            .insert("other:test:3".to_string(), Arc::new(vec![3]))
            .await;

        // Invalidate with wildcard
        cache.invalidate_metrics_pattern("metrics:test:*").await;

        assert!(cache
            .metrics()
            .get(&"metrics:test:1".to_string())
            .await
            .is_none());
        assert!(cache
            .metrics()
            .get(&"metrics:test:2".to_string())
            .await
            .is_none());
        assert!(cache
            .metrics()
            .get(&"other:test:3".to_string())
            .await
            .is_some());
    }

    #[tokio::test]
    async fn invalidate_user() {
        let cache = CacheService::new();
        let user_id = 123456u64;

        cache.users().insert(user_id, Arc::new(vec![1, 2, 3])).await;
        assert!(cache.users().get(&user_id).await.is_some());

        cache.invalidate_user(user_id).await;
        assert!(cache.users().get(&user_id).await.is_none());
    }

    #[tokio::test]
    async fn invalidate_config() {
        let cache = CacheService::new();
        let guild_id = 789012u64;

        cache
            .config()
            .insert(guild_id, Arc::new(vec![1, 2, 3]))
            .await;
        assert!(cache.config().get(&guild_id).await.is_some());

        cache.invalidate_config(guild_id).await;
        assert!(cache.config().get(&guild_id).await.is_none());
    }

    #[tokio::test]
    async fn invalidate_all() {
        let cache = CacheService::new();

        // Populate all caches
        cache
            .metrics()
            .insert("metrics:1".to_string(), Arc::new(vec![1]))
            .await;
        cache.users().insert(123, Arc::new(vec![2])).await;
        cache.config().insert(456, Arc::new(vec![3])).await;

        cache.sync().await;

        let stats = cache.stats();
        assert_eq!(stats.metrics_entries, 1);
        assert_eq!(stats.users_entries, 1);
        assert_eq!(stats.config_entries, 1);

        // Invalidate all
        cache.invalidate_all().await;

        let stats = cache.stats();
        assert_eq!(stats.metrics_entries, 0);
        assert_eq!(stats.users_entries, 0);
        assert_eq!(stats.config_entries, 0);
    }

    #[tokio::test]
    async fn invalidate_on_write_simulation() {
        let cache = CacheService::new();
        let guild_id = 999u64;
        let key = format!("metrics:guild:{}", guild_id);

        // Cache some metrics
        cache
            .metrics()
            .insert(key.clone(), Arc::new(vec![1, 2, 3]))
            .await;

        // Simulate a write operation (new violation recorded)
        // This should invalidate the cache
        cache
            .invalidate_metrics_pattern(&format!("metrics:guild:{}*", guild_id))
            .await;

        // Cache should be empty
        assert!(cache.metrics().get(&key).await.is_none());

        // Next read will fetch fresh data
        let fresh_data = cache
            .get_or_fetch(cache.metrics(), key.clone(), || async {
                Ok::<Vec<u8>, MurdochError>(vec![4, 5, 6])
            })
            .await
            .expect("should succeed");

        assert_eq!(*fresh_data, vec![4, 5, 6]);
    }

    #[tokio::test]
    async fn stats_tracking_hits_and_misses() {
        let cache = CacheService::new();
        let key = "test:stats".to_string();

        // Initial stats should be zero
        let stats = cache.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.hit_rate, 0.0);

        // First fetch is a miss
        let _ = cache
            .get_or_fetch(cache.metrics(), key.clone(), || async {
                Ok::<Vec<u8>, MurdochError>(vec![1, 2, 3])
            })
            .await
            .expect("should succeed");

        let stats = cache.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 0.0);

        // Second fetch is a hit
        let _ = cache
            .get_or_fetch(cache.metrics(), key.clone(), || async {
                Ok::<Vec<u8>, MurdochError>(vec![9, 9, 9])
            })
            .await
            .expect("should succeed");

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 0.5);

        // Third fetch is another hit
        let _ = cache
            .get_or_fetch(cache.metrics(), key, || async {
                Ok::<Vec<u8>, MurdochError>(vec![9, 9, 9])
            })
            .await
            .expect("should succeed");

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 0.666).abs() < 0.01);
    }

    #[tokio::test]
    async fn get_with_stats_tracking() {
        let cache = CacheService::new();
        let key = "test:get_stats".to_string();

        // Miss
        let result = cache.get_with_stats(cache.metrics(), &key).await;
        assert!(result.is_none());

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);

        // Insert and hit
        cache
            .metrics()
            .insert(key.clone(), Arc::new(vec![1, 2, 3]))
            .await;

        let result = cache.get_with_stats(cache.metrics(), &key).await;
        assert!(result.is_some());

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 0.5);
    }

    #[tokio::test]
    async fn stats_hit_rate_calculation() {
        let cache = CacheService::new();

        // 3 misses
        for i in 0..3 {
            let _ = cache
                .get_or_fetch(cache.metrics(), format!("key:{}", i), || async {
                    Ok::<Vec<u8>, MurdochError>(vec![i as u8])
                })
                .await;
        }

        // 7 hits
        for i in 0..3 {
            for _ in 0..2 {
                let _ = cache
                    .get_or_fetch(cache.metrics(), format!("key:{}", i), || async {
                        Ok::<Vec<u8>, MurdochError>(vec![99])
                    })
                    .await;
            }
        }

        // Additional hit
        let _ = cache
            .get_or_fetch(cache.metrics(), "key:0".to_string(), || async {
                Ok::<Vec<u8>, MurdochError>(vec![99])
            })
            .await;

        let stats = cache.stats();
        assert_eq!(stats.misses, 3);
        assert_eq!(stats.hits, 7);
        assert_eq!(stats.hit_rate, 0.7);
    }

    #[tokio::test]
    async fn stats_across_multiple_caches() {
        let cache = CacheService::new();

        // Metrics cache operations
        let _ = cache
            .get_or_fetch(cache.metrics(), "m1".to_string(), || async {
                Ok::<Vec<u8>, MurdochError>(vec![1])
            })
            .await;
        let _ = cache
            .get_or_fetch(cache.metrics(), "m1".to_string(), || async {
                Ok::<Vec<u8>, MurdochError>(vec![1])
            })
            .await;

        // Users cache operations
        let _ = cache
            .get_or_fetch(cache.users(), 123u64, || async {
                Ok::<Vec<u8>, MurdochError>(vec![2])
            })
            .await;

        // Config cache operations
        let _ = cache
            .get_or_fetch(cache.config(), 456u64, || async {
                Ok::<Vec<u8>, MurdochError>(vec![3])
            })
            .await;
        let _ = cache
            .get_or_fetch(cache.config(), 456u64, || async {
                Ok::<Vec<u8>, MurdochError>(vec![3])
            })
            .await;

        cache.sync().await;

        let stats = cache.stats();
        assert_eq!(stats.misses, 3); // m1, 123, 456
        assert_eq!(stats.hits, 2); // m1 (2nd), 456 (2nd)
        assert_eq!(stats.metrics_entries, 1);
        assert_eq!(stats.users_entries, 1);
        assert_eq!(stats.config_entries, 1);
    }

    #[tokio::test]
    async fn stats_prometheus_format() {
        let cache = CacheService::new();

        // Populate cache
        cache
            .metrics()
            .insert("m1".to_string(), Arc::new(vec![1, 2, 3]))
            .await;
        cache.users().insert(123, Arc::new(vec![4, 5, 6])).await;

        cache.sync().await;

        let stats = cache.stats();

        // Verify all fields are present for Prometheus export
        assert!(stats.metrics_entries > 0);
        assert!(stats.users_entries > 0);
        assert_eq!(stats.config_entries, 0);
        assert!(stats.weighted_size > 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.hit_rate, 0.0);
    }
}
