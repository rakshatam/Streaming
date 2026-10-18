use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use moka::future::Cache;
use reqwest::header::{HeaderValue, RANGE};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::{
    net::{Ipv4Addr, Ipv6Addr},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Semaphore;
use tracing::info;
use url::Url;

use crate::security::{sign_stream_token, verify_stream_token};

const MAX_MANIFEST_BYTES: usize = 5 * 1024 * 1024;
const MAX_REWRITTEN_MANIFEST_BYTES: usize = 6 * 1024 * 1024;
const MAX_MANIFEST_LINE_BYTES: usize = 64 * 1024;
const MAX_SEGMENT_BYTES: usize = 35 * 1024 * 1024;
const MAX_CONCURRENT_UPSTREAM_REQUESTS: usize = 4;

#[derive(Debug, Clone)]
pub struct SegmentData {
    pub bytes: Bytes,
    pub content_type: String,
    pub is_cached: bool,
    pub status: u16,
    pub content_range: Option<String>,
}

#[derive(Clone)]
pub struct StreamEngine {
    client: reqwest::Client,
    segment_cache: Cache<String, SegmentData>,
    manifest_cache: Cache<String, String>,
    hmac_secret: String,
    token_validity_secs: u64,
    upstream_requests: Arc<Semaphore>,
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
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(25))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 10 || is_blocked_upstream_url(attempt.url()) {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to build HTTP streaming client");

        let max_capacity_bytes = cache_mb.saturating_mul(1024 * 1024);
        let segment_cache = Cache::builder()
            .max_capacity(max_capacity_bytes)
            .weigher(|_key, value: &SegmentData| value.bytes.len() as u32)
            .time_to_live(Duration::from_secs(300)) // 5 minutes TTL for segments
            .time_to_idle(Duration::from_secs(120))
            .build();

        // In-memory cache for playlists
        let manifest_cache = Cache::builder()
            .max_capacity(8 * 1024 * 1024)
            .weigher(|key: &String, value: &String| (key.len() + value.len()) as u32)
            .time_to_live(Duration::from_secs(5)) // 5 seconds default TTL (avoids stale live windows)
            .time_to_idle(Duration::from_secs(10))
            .build();

        info!("StreamEngine initialized with {} MB cache.", cache_mb);

        StreamEngine {
            client,
            segment_cache,
            manifest_cache,
            hmac_secret,
            token_validity_secs,
            upstream_requests: Arc::new(Semaphore::new(MAX_CONCURRENT_UPSTREAM_REQUESTS)),
            total_requests: Arc::new(AtomicU64::new(0)),
            cache_hits: Arc::new(AtomicU64::new(0)),
            cache_misses: Arc::new(AtomicU64::new(0)),
            bytes_streamed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Clears in-memory manifest and segment caches, immediately releasing allocated memory
    pub async fn clear_cache(&self) {
        self.manifest_cache.invalidate_all();
        self.segment_cache.invalidate_all();
        self.manifest_cache.run_pending_tasks().await;
        self.segment_cache.run_pending_tasks().await;
    }

    /// Returns the current number of cached manifests and segments
    pub fn cache_entry_counts(&self) -> (u64, u64) {
        (
            self.manifest_cache.entry_count(),
            self.segment_cache.entry_count(),
        )
    }

    /// Directly inserts a segment for cache testing and runs eviction maintenance tasks
    pub async fn insert_segment_for_testing(&self, key: &str, data: SegmentData) {
        self.segment_cache.insert(key.to_string(), data).await;
        self.segment_cache.run_pending_tasks().await;
    }

    /// Resets cache hit/miss counters
    pub fn reset_cache_counters(&self) {
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
    }

    /// Generates an HMAC-signed token for a target URI
    pub fn create_token(&self, target_uri: &str) -> (String, u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expires_at = now.saturating_add(self.token_validity_secs);
        let token = sign_stream_token(target_uri, expires_at, &self.hmac_secret);
        (token, expires_at)
    }

    /// Validates an incoming stream token
    pub fn validate_token(&self, target_uri: &str, token: &str, expires_at: u64) -> bool {
        verify_stream_token(target_uri, expires_at, token, &self.hmac_secret)
    }

    /// Fetches upstream M3U8 manifest and rewrites sub-playlists and segments with signed gateway URLs
    pub async fn rewrite_manifest(&self, upstream_url: &str) -> Result<String, String> {
        let base_url =
            Url::parse(upstream_url).map_err(|e| format!("Invalid upstream URL: {e}"))?;
        validate_upstream_url(&base_url)?;

        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Check manifest cache for instant sub-millisecond response
        if let Some(cached_manifest) = self.manifest_cache.get(upstream_url).await {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(cached_manifest);
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        let _permit = self
            .upstream_requests
            .acquire()
            .await
            .map_err(|_| "Upstream request limiter is unavailable".to_string())?;

        let response = self
            .client
            .get(upstream_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch upstream manifest: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Upstream returned HTTP status {}",
                response.status()
            ));
        }

        let final_url = response.url().clone();
        validate_upstream_url(&final_url)?;
        let body = read_response_limited(response, MAX_MANIFEST_BYTES, "Manifest").await?;
        let raw_m3u8 = String::from_utf8(body.to_vec())
            .map_err(|_| "Upstream manifest is not valid UTF-8".to_string())?;

        let mut rewritten = String::with_capacity(raw_m3u8.len() + 4096);

        for line in raw_m3u8.lines() {
            if line.len() > MAX_MANIFEST_LINE_BYTES {
                return Err(
                    "Upstream manifest contains a line larger than the 64 KiB limit".into(),
                );
            }
            let trimmed = line.trim();

            if trimmed.is_empty() {
                rewritten.push('\n');
                continue;
            }

            if trimmed.starts_with('#') {
                // HLS URI attributes appear on media, key, map, iframe, and
                // steering tags. Preserve the tag and rewrite every URI value.
                if trimmed.starts_with("#EXT-X-") && trimmed.contains("URI=\"") {
                    let is_playlist = trimmed.starts_with("#EXT-X-MEDIA:")
                        || trimmed.starts_with("#EXT-X-I-FRAME-STREAM-INF:");
                    rewritten.push_str(&self.rewrite_attribute_uris(
                        trimmed,
                        &final_url,
                        is_playlist,
                    )?);
                } else {
                    rewritten.push_str(trimmed);
                }
                rewritten.push('\n');
            } else {
                // This is a playlist or segment URI line
                let resolved = final_url
                    .join(trimmed)
                    .map_err(|e| format!("Invalid HLS resource URI: {e}"))?;
                validate_upstream_url(&resolved)?;
                let resolved_url = resolved.to_string();
                let is_playlist = resolved.path().to_ascii_lowercase().ends_with(".m3u8");
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

            if rewritten.len() > MAX_REWRITTEN_MANIFEST_BYTES {
                return Err("Rewritten manifest exceeds the 6 MiB limit".into());
            }
        }

        // Cache rewritten manifest
        self.manifest_cache
            .insert(upstream_url.to_string(), rewritten.clone())
            .await;

        Ok(rewritten)
    }

    fn rewrite_attribute_uris(
        &self,
        line: &str,
        base_url: &Url,
        is_playlist: bool,
    ) -> Result<String, String> {
        let mut output = String::with_capacity(line.len() + 128);
        let mut cursor = 0;

        while let Some(relative_start) = line[cursor..].find("URI=\"") {
            let uri_start = cursor + relative_start + "URI=\"".len();
            let uri_end = line[uri_start..]
                .find('"')
                .map(|offset| uri_start + offset)
                .ok_or_else(|| "Malformed HLS URI attribute".to_string())?;
            let resolved = base_url
                .join(&line[uri_start..uri_end])
                .map_err(|e| format!("Invalid HLS URI attribute: {e}"))?;
            validate_upstream_url(&resolved)?;

            let endpoint = if is_playlist {
                "/api/stream/playlist"
            } else {
                "/api/stream/segment"
            };
            let resolved = resolved.to_string();
            let (token, expires_at) = self.create_token(&resolved);
            let encoded = urlencoding_simple(&resolved);
            let proxied_uri = format!("{endpoint}?uri={encoded}&token={token}&exp={expires_at}");

            output.push_str(&line[cursor..uri_start]);
            output.push_str(&proxied_uri);
            cursor = uri_end;
        }

        output.push_str(&line[cursor..]);
        Ok(output)
    }

    /// Fetches and caches video segments, handling byte-range requests for sub-millisecond seeking
    pub async fn fetch_segment(
        &self,
        segment_url: &str,
        client_range: Option<String>,
    ) -> Result<SegmentData, String> {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let cache_key = match &client_range {
            Some(range) => format!("{}:{}", segment_url, range),
            None => segment_url.to_string(),
        };

        // 1. Check in-memory cache
        if let Some(cached) = self.segment_cache.get(&cache_key).await {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
            self.bytes_streamed
                .fetch_add(cached.bytes.len() as u64, Ordering::Relaxed);
            return Ok(SegmentData {
                bytes: cached.bytes.clone(),
                content_type: cached.content_type.clone(),
                is_cached: true,
                status: cached.status,
                content_range: cached.content_range.clone(),
            });
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);

        let parsed_url =
            Url::parse(segment_url).map_err(|e| format!("Invalid segment URL: {e}"))?;
        validate_upstream_url(&parsed_url)?;
        let _permit = self
            .upstream_requests
            .acquire()
            .await
            .map_err(|_| "Upstream request limiter is unavailable".to_string())?;

        // 2. Fetch from upstream
        let mut req = self.client.get(segment_url);

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

        let status = resp.status().as_u16();
        let content_range = resp
            .headers()
            .get("content-range")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("video/MP2T")
            .to_string();

        validate_upstream_url(resp.url())?;
        let bytes = read_response_limited(resp, MAX_SEGMENT_BYTES, "Segment").await?;

        let byte_len = bytes.len();

        self.bytes_streamed
            .fetch_add(byte_len as u64, Ordering::Relaxed);

        let data = SegmentData {
            bytes: bytes.clone(),
            content_type,
            is_cached: false,
            status,
            content_range,
        };

        // Cache segments up to 10MB in memory
        if byte_len <= 10 * 1024 * 1024 {
            self.segment_cache.insert(cache_key, data.clone()).await;
        }

        Ok(data)
    }
}

async fn read_response_limited(
    response: reqwest::Response,
    max_bytes: usize,
    resource_name: &str,
) -> Result<Bytes, String> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(format!(
            "Upstream {resource_name} exceeds the {max_bytes}-byte limit"
        ));
    }

    let mut chunks = response.bytes_stream();
    let mut body = BytesMut::new();
    while let Some(chunk) = chunks.next().await {
        let chunk =
            chunk.map_err(|error| format!("Failed to read upstream {resource_name}: {error}"))?;
        if body
            .len()
            .checked_add(chunk.len())
            .map_or(true, |length| length > max_bytes)
        {
            return Err(format!(
                "Upstream {resource_name} exceeds the {max_bytes}-byte limit"
            ));
        }
        body.extend_from_slice(&chunk);
    }

    Ok(body.freeze())
}

fn validate_upstream_url(url: &Url) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Upstream resources must use HTTP or HTTPS".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Credentials in upstream URLs are not allowed".into());
    }

    let host = url
        .host_str()
        .ok_or_else(|| "Upstream URL is missing a host".to_string())?;
    let normalized_host = host.trim_end_matches('.').to_ascii_lowercase();
    if normalized_host == "localhost"
        || normalized_host.ends_with(".localhost")
        || normalized_host.ends_with(".local")
        || normalized_host.ends_with(".internal")
        || normalized_host.ends_with(".test")
    {
        return Err("Local and reserved host names are not allowed as upstreams".into());
    }

    match url.host() {
        Some(url::Host::Ipv4(address)) if is_blocked_ipv4(address) => {
            Err("Private or reserved IP addresses are not allowed as upstreams".into())
        }
        Some(url::Host::Ipv6(address)) if is_blocked_ipv6(address) => {
            Err("Private or reserved IP addresses are not allowed as upstreams".into())
        }
        _ => Ok(()),
    }
}

fn is_blocked_upstream_url(url: &Url) -> bool {
    validate_upstream_url(url).is_err()
}

fn is_blocked_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_unspecified()
        || address.is_multicast()
        || octets[0] == 0
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 198 && (18..=19).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
        || octets[0] >= 240
}

fn is_blocked_ipv6(address: Ipv6Addr) -> bool {
    let octets = address.octets();
    let second_segment = u16::from_be_bytes([octets[2], octets[3]]);
    address.is_loopback()
        || address.is_unspecified()
        || (octets[0] & 0xfe) == 0xfc
        || (octets[0] == 0xfe && (octets[1] & 0xc0) == 0x80)
        || address.is_multicast()
        || (octets[0] & 0xe0) != 0x20 // Outside global unicast (2000::/3).
        || (octets[0] == 0x20 && octets[1] == 0x01 && second_segment <= 0x01ff)
        || (octets[0] == 0x20 && octets[1] == 0x01 && octets[2] == 0x0d && octets[3] == 0xb8)
        || (octets[0] == 0x20 && octets[1] == 0x02) // Deprecated 6to4.
        || address.to_ipv4_mapped().is_some_and(is_blocked_ipv4)
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
