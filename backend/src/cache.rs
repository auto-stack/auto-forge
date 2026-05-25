/// Simple in-memory cache with get and set operations
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cache key type
pub type CacheKey = String;

/// Cache value type - supports any serializable data
#[derive(Debug, Clone)]
pub struct CacheValue {
    data: String,
}

impl CacheValue {
    /// Create a new cache value from a string
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
        }
    }

    /// Get the string data
    pub fn as_str(&self) -> &str {
        &self.data
    }

    /// Convert to owned String
    pub fn into_string(self) -> String {
        self.data
    }
}

impl From<String> for CacheValue {
    fn from(data: String) -> Self {
        Self::new(data)
    }
}

impl From<&str> for CacheValue {
    fn from(data: &str) -> Self {
        Self::new(data)
    }
}

/// Simple in-memory cache
#[derive(Debug, Clone)]
pub struct Cache {
    inner: Arc<RwLock<HashMap<CacheKey, CacheValue>>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a value from the cache
    ///
    /// # Arguments
    /// * `key` - The cache key to look up
    ///
    /// # Returns
    /// * `Some(value)` if the key exists
    /// * `None` if the key does not exist
    ///
    /// # Example
    /// ```ignore
    /// let cache = Cache::new();
    /// cache.set("my_key".to_string(), "my_value".into()).await;
    /// let value = cache.get("my_key".to_string()).await;
    /// assert_eq!(value.unwrap().as_str(), "my_value");
    /// ```
    pub async fn get(&self, key: CacheKey) -> Option<CacheValue> {
        let read_guard = self.inner.read().await;
        read_guard.get(&key).cloned()
    }

    /// Set a value in the cache
    ///
    /// # Arguments
    /// * `key` - The cache key to set
    /// * `value` - The value to store
    ///
    /// # Example
    /// ```ignore
    /// let cache = Cache::new();
    /// cache.set("my_key".to_string(), "my_value".into()).await;
    /// ```
    pub async fn set(&self, key: CacheKey, value: CacheValue) {
        let mut write_guard = self.inner.write().await;
        write_guard.insert(key, value);
    }

    /// Remove a value from the cache
    ///
    /// # Arguments
    /// * `key` - The cache key to remove
    ///
    /// # Returns
    /// * `Some(value)` if the key existed and was removed
    /// * `None` if the key did not exist
    pub async fn remove(&self, key: &CacheKey) -> Option<CacheValue> {
        let mut write_guard = self.inner.write().await;
        write_guard.remove(key)
    }

    /// Check if a key exists in the cache
    ///
    /// # Arguments
    /// * `key` - The cache key to check
    ///
    /// # Returns
    /// * `true` if the key exists
    /// * `false` if the key does not exist
    pub async fn contains_key(&self, key: &CacheKey) -> bool {
        let read_guard = self.inner.read().await;
        read_guard.contains_key(key)
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) {
        let mut write_guard = self.inner.write().await;
        write_guard.clear();
    }

    /// Get the number of entries in the cache
    pub async fn len(&self) -> usize {
        let read_guard = self.inner.read().await;
        read_guard.len()
    }

    /// Check if the cache is empty
    pub async fn is_empty(&self) -> bool {
        let read_guard = self.inner.read().await;
        read_guard.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get() {
        let cache = Cache::new();
        
        cache.set("key1".to_string(), "value1".into()).await;
        let value = cache.get("key1".to_string()).await;
        
        assert!(value.is_some());
        assert_eq!(value.unwrap().as_str(), "value1");
    }

    #[tokio::test]
    async fn test_get_nonexistent_key() {
        let cache = Cache::new();
        
        let value = cache.get("nonexistent".to_string()).await;
        
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_overwrite_value() {
        let cache = Cache::new();
        
        cache.set("key1".to_string(), "value1".into()).await;
        cache.set("key1".to_string(), "value2".into()).await;
        let value = cache.get("key1".to_string()).await;
        
        assert_eq!(value.unwrap().as_str(), "value2");
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = Cache::new();
        
        cache.set("key1".to_string(), "value1".into()).await;
        let removed = cache.remove(&"key1".to_string()).await;
        
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().as_str(), "value1");
        
        let value = cache.get("key1".to_string()).await;
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_contains_key() {
        let cache = Cache::new();
        
        assert!(!cache.contains_key(&"key1".to_string()).await);
        
        cache.set("key1".to_string(), "value1".into()).await;
        assert!(cache.contains_key(&"key1".to_string()).await);
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = Cache::new();
        
        cache.set("key1".to_string(), "value1".into()).await;
        cache.set("key2".to_string(), "value2".into()).await;
        
        assert_eq!(cache.len().await, 2);
        
        cache.clear().await;
        
        assert_eq!(cache.len().await, 0);
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_len_and_is_empty() {
        let cache = Cache::new();
        
        assert_eq!(cache.len().await, 0);
        assert!(cache.is_empty().await);
        
        cache.set("key1".to_string(), "value1".into()).await;
        
        assert_eq!(cache.len().await, 1);
        assert!(!cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_cache_value_conversions() {
        let value_from_str = CacheValue::from("test");
        let value_from_string = CacheValue::from("test".to_string());
        let value_new = CacheValue::new("test");
        
        assert_eq!(value_from_str.as_str(), "test");
        assert_eq!(value_from_string.as_str(), "test");
        assert_eq!(value_new.as_str(), "test");
        
        assert_eq!(value_from_str.into_string(), "test");
    }
}
