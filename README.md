# StreamFlow

StreamFlow is a memory-bounded video streaming proxy and web application built with Rust and Next.js. It implements HMAC-SHA256 signed URL verification, dynamic HLS manifest rewriting, bounded in-memory segment caching, SQLite metadata storage with full-text search, and a Next.js client interface.

---

## Architecture Overview

```
+-------------------------------------------------------------------------------+
|                                SYSTEM TOPOLOGY                                |
+-------------------------------------------------------------------------------+

  [ Web Browser ]
         |
         |  1. HTTP/HTTPS (Port 3000)
         v
  +-----------------------------+
  |      Next.js Frontend       |
  |  - Player & Telemetry HUD   |
  |  - Catalog & Search Views   |
  |  - Authentication Modals    |
  +-----------------------------+
         |
         |  2. REST API / Streaming Proxy (Port 8080)
         v
  +-----------------------------------------------------------------------------+
  |                          StreamFlow Rust Backend                            |
  |                                                                             |
  |  +-----------------------------------------------------------------------+  |
  |  |                         Rust Axum HTTP Server                         |  |
  |  |                                                                       |  |
  |  |  +-----------------------+                 +-----------------------+  |  |
  |  |  |   Auth & Catalog API  |                 | Stream Proxy Engine   |  |  |
  |  |  |   - Argon2id / JWT    |                 | - HMAC Token Signer   |  |  |
  |  |  |   - SQLite WAL / FTS5 |                 | - M3U8 Rewriter       |  |  |
  |  |  +-----------------------+                 | - Cache Controller    |  |  |
  |  |              |                             +-----------------------+  |  |
  |  +--------------|-----------------------------------------|--------------+  |
  |                 |                                         |                 |
  |                 v                                         v                 |
  |       +-------------------+                     +--------------------+      |
  |       | SQLite Database   |                     | In-Memory Cache    |      |
  |       | (WAL, 16MB cache) |                     | (Moka, 64MB cap)   |      |
  |       +-------------------+                     +--------------------+      |
  +-----------------------------------------------------------|-----------------+
                                                              |
                                                              | 3. Upstream Fetch
                                                              v
                                                   +---------------------+
                                                   | External Video CDNs |
                                                   | (Public HLS Streams)|
                                                   +---------------------+
```

---

## Streaming Lifecycle & Token Verification Workflow

```
Client (Browser / Hls.js)               StreamFlow Rust Core               Upstream CDN
         |                                       |                              |
         | 1. GET /api/stream/manifest/:id       |                              |
         |-------------------------------------->|                              |
         |                                       | 2. Fetch original manifest   |
         |                                       |----------------------------->|
         |                                       | 3. Return M3U8 text          |
         |                                       |<-----------------------------|
         |                                       |                              |
         |                                       | 4. Parse & Rewrite:          |
         |                                       |    Generate HMAC signature   |
         |                                       |    exp = now + TTL           |
         |                                       |    Rewrite sub-playlists &   |
         |                                       |    segments to proxy routes  |
         | 5. Return rewritten M3U8              |                              |
         |<--------------------------------------|                              |
         |                                       |                              |
         | 6. GET /api/stream/segment?           |                              |
         |    uri={target}&token={sig}&exp={t}   |                              |
         |-------------------------------------->|                              |
         |                                       | 7. Validate HMAC & expiry:   |
         |                                       |    sig == HMAC(uri:exp, key) |
         |                                       |    If expired -> 403         |
         |                                       |    If tampered -> 403        |
         |                                       |                              |
         |                                       | 8. Check 64MB Moka cache:    |
         |                                       |    If HIT -> return cached   |
         |                                       |    If MISS -> fetch upstream |
         |                                       |----------------------------->|
         |                                       | 9. Upstream segment bytes    |
         |                                       |<-----------------------------|
         |                                       | 10. Store in LRU cache       |
         | 11. Stream bytes (video/MP2T)         |                              |
         |<--------------------------------------|                              |
```

---

## Stack Component Details

### 1. Backend Core (Rust)
- **Language & Runtime:** Rust (2021 edition) using `tokio` (multi-threaded asynchronous event loop using non-blocking epoll/kqueue/IOCP).
- **HTTP Routing:** `axum` 0.7 with `tower-http` middleware for CORS, compression, tracing, and security headers (`nosniff`, `DENY`, `strict-origin-when-cross-origin`).
- **Cryptographic Security:**
  - Stream token verification via HMAC-SHA256 (`hmac` and `sha2` crates). Query parameters `uri`, `token`, and `exp` ensure signed URLs cannot be altered or replayed past expiration.
  - User password hashing via Argon2id (`argon2` crate) using randomized salts and standard memory/iteration parameters.
  - User authentication via stateless JSON Web Tokens (`jsonwebtoken` crate) using HMAC-SHA256 signatures.
- **In-Memory Cache:** `moka` asynchronous cache configured with:
  - Bounded capacity: 64 MB (`SEGMENT_CACHE_MB=64`), weighted by byte length.
  - TTL (Time To Live): 300 seconds for segments, 5 seconds for live playlists.
  - TTI (Time To Idle): 120 seconds for segments.
  - Immediate eviction endpoint: `POST /api/cache/clear`.
- **Database Engine:** `sqlx` with embedded SQLite:
  - Write-Ahead Logging (`PRAGMA journal_mode = WAL;`) for concurrent reads during writes.
  - Synchronization: `PRAGMA synchronous = NORMAL;`.
  - Memory page cache: `PRAGMA cache_size = -16000;` (16 MB maximum).
  - Memory-mapped I/O: `PRAGMA mmap_size = 67108864;` (64 MB).
  - Connection pool bounded to 20 connections to minimize memory allocations.
  - Full-Text Search index via SQLite FTS5 for video title, description, and tag queries.

### 2. Frontend Application (Next.js & TypeScript)
- **Framework:** Next.js 14 (App Router) with React 18 and TypeScript.
- **Dynamic API Routing:** `frontend/lib/api.ts` resolves backend endpoints, defaulting to relative `/api/*` paths proxied by Next.js rewrites in local development.
- **Player & Telemetry:**
  - Integrated `hls.js` video player supporting adaptive bitrate streaming, resolution ladder switching, and buffer health tracking.
  - Telemetry HUD reporting real-time resolution, bitrate, buffer status, and in-player cache purge controls.
- **Styling & Assets:** Tailwind CSS with Lucide icons.

### 3. Containerization & CI/CD
- **Docker Multi-Stage Build:** `Dockerfile.backend` compiles in `rust:1.80-slim-bookworm` and copies the stripped release binary to a minimal `debian:bookworm-slim` image (~85 MB total container footprint).
- **Docker Compose:** `docker-compose.yml` orchestrates backend and frontend containers with isolated bridge networking and volume persistence.
- **GitHub Actions Workflow:** `.github/workflows/ci.yml` runs Rust check, unit/integration tests, Next.js build, throughput benchmark, and multi-stage Docker build validation on every push.

---

## Throughput & Memory Benchmark Results

Automated stress testing executed via `scripts/throughput_benchmark.py` and `benchmark_render.py`:

| Test Suite | Concurrency | Requests | Requests/Sec (RPS) | Avg Latency | P95 Latency | Success Rate |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **API Health & Telemetry** | 20 | 200 | **956.0 RPS** | 15.40 ms | 25.86 ms | **100.0%** |
| **Catalog Listing (SQLite WAL)** | 15 | 150 | **1005.0 RPS** | 13.55 ms | 20.22 ms | **100.0%** |
| **FTS5 Full-Text Search ('space')** | 10 | 100 | **1022.5 RPS** | 8.76 ms | 13.19 ms | **100.0%** |
| **M3U8 Manifest Rewriter & HMAC Signer** | 5 | 50 | **45.6 RPS** | 108.97 ms | 1016.99 ms | **100.0%** |

### Memory Footprint Metrics
- **Idle Working Set (RAM):** 9.79 MB
- **Peak Working Set (20 concurrent threads):** 21.21 MB
- **Private Memory:** 11.52 MB
- **Cache Purge:** Calling `POST /api/cache/clear` invalidates all cached segments and manifests immediately.

---

## API Specification

| Method | Endpoint | Auth | Description |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/health` | No | Service status and version check |
| `GET` | `/api/metrics` | No | Cache hit ratio, total requests, streamed megabytes, and active entries |
| `POST` | `/api/cache/clear` | No | Invalidate all segment and manifest caches and reset counters |
| `POST` | `/api/auth/register` | No | Register user account with Argon2id password hashing |
| `POST` | `/api/auth/login` | No | Authenticate username/password and return JWT |
| `GET` | `/api/auth/me` | Bearer JWT | Return authenticated user details |
| `GET` | `/api/videos` | No | List video catalog (filter by category, live, featured) |
| `GET` | `/api/videos/search?q={term}` | No | Full-text search across titles, descriptions, and tags |
| `GET` | `/api/videos/:id` | No | Retrieve single video metadata |
| `GET` | `/api/videos/categories` | No | List distinct categories with stream counts |
| `GET` | `/api/videos/:id/interactions` | Optional | Retrieve like status, like count, and user/average ratings |
| `POST` | `/api/videos/:id/like` | Bearer JWT | Toggle like status for the authenticated user |
| `POST` | `/api/videos/:id/rate` | Bearer JWT | Submit 1-5 star rating |
| `GET` | `/api/stream/manifest/:id` | No | Return rewritten M3U8 manifest with signed gateway URLs |
| `GET` | `/api/stream/playlist` | HMAC Token | Proxy quality ladder playlists |
| `GET` | `/api/stream/segment` | HMAC Token | Proxy and cache `.ts`/`.m4s` video segments with byte-range support |

---

## Local Development Setup

### Prerequisites
- Rust 1.80+ (`cargo`, `rustc`)
- Node.js 20+ (`npm`)
- Docker (optional, for containerized run)

### 1. Run Backend
```bash
cd backend
cargo run --release
```
The backend initializes the SQLite database (`streamflow.db`), applies migrations, runs search seeders, and listens on `http://0.0.0.0:8080`.

### 2. Run Frontend
```bash
cd frontend
npm install
npm run dev
```
The frontend starts on `http://localhost:3000` and proxies `/api/*` requests to `http://127.0.0.1:8080`.

### 3. Run Automated Tests
```bash
cd backend
cargo test
```

### 4. Run with Docker Compose
```bash
docker compose up --build
```
This starts both the Rust backend (`http://localhost:8080`) and the Next.js frontend (`http://localhost:3000`) in isolated containers.

---

## Storage & Maintenance

For details on managing compiler build artifacts, reclaiming local disk space, and reducing debug symbol overhead, see [STORAGE_AND_MAINTENANCE.md](STORAGE_AND_MAINTENANCE.md).

---

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
