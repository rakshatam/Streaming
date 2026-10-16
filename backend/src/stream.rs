use bytes::Bytes;
use moka::future::Cache;
use reqwest::header::{HeaderValue, RANGE};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::info;
use url::Url;

use crate::security::{sign_stream_token, verify_stream_token};

#[derive(Clone)]
pub struct StreamEngine {
    client: reqwest::Client,
    segment_cache: Cache<String, Bytes>,
    manifest_cache: Cache<String, String>,
    hmac_secret: String,
    token_validity_secs: u64,
    // Metrics counters
    pub total_requests: Arc<AtomicU64>,
    pub cache_hits: Arc<AtomicU64>,
    pub cache_misses: Arc<AtomicU64>,
    pub bytes_streamed: Arc<AtomicU64>,
}

impl StreamEngine {
    pub fn new(hmac_secret: String, cache_mb: u64, token_validity_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(100)
            .pool_idle_timeout(Duration::from_secs(120))
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(30))
            .timeout(Duration::from_secs(10))
            .danger_accept_invalid_certs(true) // Resilience against varied upstream CDNs
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to build HTTP streaming client");

        let max_capacity_bytes = cache_mb * 1024 * 1024;
        let segment_cache = Cache::builder()
            .max_capacity(max_capacity_bytes)
            .weigher(|_key, value: &Bytes| value.len() as u32)
            .time_to_live(Duration::from_secs(300)) // 5 minutes TTL for segments
            .time_to_idle(Duration::from_secs(120))
            .build();

        // High-speed manifest cache for sub-millisecond playlist delivery
        let manifest_cache = Cache::builder()
            .max_capacity(2000)
            .time_to_live(Duration::from_secs(5)) // 5 seconds default TTL (avoids stale live windows)
            .time_to_idle(Duration::from_secs(10))
            .build();

        info!("StreamEngine initialized with {} MB LRU cache and Manifest Accelerator.", cache_mb);

        StreamEngine {
            client,
            segment_cache,
            manifest_cache,
            hmac_secret,
            token_validity_secs,
            total_requests: Arc::new(AtomicU64::new(0)),
            cache_hits: Arc::new(AtomicU64::new(0)),
            cache_misses: Arc::new(AtomicU64::new(0)),
            bytes_streamed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Generates an HMAC-signed token for a target URI
    pub fn create_token(&self, target_uri: &str) -> (String, u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = now + self.token_validity_secs;
        let token = sign_stream_token(target_uri, expires_at, &self.hmac_secret);
        (token, expires_at)
    }

    /// Validates an incoming stream token
    pub fn validate_token(&self, target_uri: &str, token: &str, expires_at: u64) -> bool {
        verify_stream_token(target_uri, expires_at, token, &self.hmac_secret)
    }

    /// Fetches upstream M3U8 manifest and rewrites sub-playlists and segments with signed gateway URLs
    pub async fn rewrite_manifest(&self, upstream_url: &str) -> Result<String, String> {
        // Check manifest cache for instant sub-millisecond response
        if let Some(cached_manifest) = self.manifest_cache.get(upstream_url).await {
            self.total_requests.fetch_add(1, Ordering::Relaxed);
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(cached_manifest);
        }

        let base_url = Url::parse(upstream_url)
            .map_err(|e| format!("Invalid upstream URL: {}", e))?;

        let response = self
            .client
            .get(upstream_url)
            .header("Referer", upstream_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch upstream manifest: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Upstream returned HTTP status {}", response.status()));
        }

        let raw_m3u8 = response
            .text()
            .await
            .map_err(|e| format!("Failed to read upstream manifest body: {}", e))?;

        let mut rewritten = String::with_capacity(raw_m3u8.len() + 4096);

        for line in raw_m3u8.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                rewritten.push('\n');
                continue;
            }

            if trimmed.starts_with('#') {
                // Check for URI tags like #EXT-X-STREAM-INF, #EXT-X-MEDIA, #EXT-X-KEY
                if trimmed.starts_with("#EXT-X-MEDIA:") && trimmed.contains("URI=\"") {
                    rewritten.push_str(&self.rewrite_attribute_uri(trimmed, &base_url, "URI=\"", true));
                } else if trimmed.starts_with("#EXT-X-KEY:") && trimmed.contains("URI=\"") {
                    rewritten.push_str(&self.rewrite_attribute_uri(trimmed, &base_url, "URI=\"", false));
                } else {
                    rewritten.push_str(trimmed);
                }
                rewritten.push('\n');
            } else {
                // This is a playlist or segment URI line
                let resolved_url = match base_url.join(trimmed) {
                    Ok(u) => u.to_string(),
                    Err(_) => trimmed.to_string(),
                };

                let is_playlist = resolved_url.contains(".m3u8") || trimmed.contains(".m3u8");
                let endpoint = if is_playlist {
                    "/api/stream/playlist"
                } else {
                    "/api/stream/segment"
                };

                let (token, expires_at) = self.create_token(&resolved_url);
                let encoded_uri = urlencoding_simple(&resolved_url);

                rewritten.push_str(&format!(
                    "{}?uri={}&token={}&exp={}\n",
                    endpoint, encoded_uri, token, expires_at
                ));
            }
        }

        // Cache rewritten manifest
        self.manifest_cache.insert(upstream_url.to_string(), rewritten.clone()).await;

        Ok(rewritten)
    }

    fn rewrite_attribute_uri(&self, line: &str, base_url: &Url, attr_prefix: &str, is_playlist: bool) -> String {
        if let Some(start_idx) = line.find(attr_prefix) {
            let uri_start = start_idx + attr_prefix.len();
            if let Some(end_rel) = line[uri_start..].find('"') {
                let uri_end = uri_start + end_rel;
                let raw_uri = &line[uri_start..uri_end];
                let resolved = match base_url.join(raw_uri) {
                    Ok(u) => u.to_string(),
                    Err(_) => raw_uri.to_string(),
                };

                let endpoint = if is_playlist {
                    "/api/stream/playlist"
                } else {
                    "/api/stream/segment"
                };

                let (token, expires_at) = self.create_token(&resolved);
                let encoded = urlencoding_simple(&resolved);
                let proxied_uri = format!("{}?uri={}&token={}&exp={}", endpoint, encoded, token, expires_at);

                let mut out = String::with_capacity(line.len() + proxied_uri.len());
                out.push_str(&line[..uri_start]);
                out.push_str(&proxied_uri);
                out.push_str(&line[uri_end..]);
                return out;
            }
        }
        line.to_string()
    }

    /// Fetches and caches video segments, handling byte-range requests for sub-millisecond seeking
    pub async fn fetch_segment(
        &self,
        segment_url: &str,
        client_range: Option<String>,
    ) -> Result<(Bytes, String, bool), String> {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let cache_key = match &client_range {
            Some(range) => format!("{}:{}", segment_url, range),
            None => segment_url.to_string(),
        };

        // 1. Check in-memory cache
        if let Some(cached_bytes) = self.segment_cache.get(&cache_key).await {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            self.bytes_streamed.fetch_add(cached_bytes.len() as u64, Ordering::Relaxed);
            return Ok((cached_bytes, "video/MP2T".to_string(), true));
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);

        // 2. Fetch from upstream
        let mut req = self
            .client
            .get(segment_url)
            .header("Referer", segment_url);

        if let Some(ref range) = client_range {
            if let Ok(hdr) = HeaderValue::from_str(range) {
                req = req.header(RANGE, hdr);
            }
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("Failed to fetch upstream segment: {}", e))?;

        if !resp.status().is_success() && resp.status().as_u16() != 206 {
            return Err(format!("Upstream segment returned HTTP {}", resp.status()));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("video/MP2T")
            .to_string();

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read segment bytes: {}", e))?;

        let byte_len = bytes.len();
        self.bytes_streamed.fetch_add(byte_len as u64, Ordering::Relaxed);

        // Cache segments up to 10MB in memory
        if byte_len <= 10 * 1024 * 1024 {
            self.segment_cache.insert(cache_key, bytes.clone()).await;
        }

        Ok((bytes, content_type, false))
    }
}

fn urlencoding_simple(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() * 2);
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}
