# StreamFlow

StreamFlow is a memory-bounded video streaming proxy and web application built with Rust and Next.js. It implements HMAC-SHA256 signed URL verification, dynamic HLS manifest rewriting, bounded in-memory segment caching, SQLite metadata storage with full-text search, and a static Next.js client interface.

The service is configured to operate within restricted environments, including Render free tier containers (512 MB RAM, 0.1 vCPU) and GitHub Pages static hosting.

---

## Architecture Overview

```
+-------------------------------------------------------------------------------+
|                                DEPLOYMENT TOPOLOGY                            |
+-------------------------------------------------------------------------------+

  [ Web Browser ]
         |
         |  1. HTTPS (Static HTML / JS / CSS bundle)
         v
  +-----------------------------+
  |        GitHub Pages         |
  |  (Next.js Static Export)    |
  +-----------------------------+
         |
         |  2. API Requests / HLS Streams (CORS enabled)
         v
  +-----------------------------------------------------------------------------+
  |                          Render Free Tier Container                         |
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
- **Static Export:** Configured with `output: 'export'` and `images: { unoptimized: true }` in `next.config.mjs` to produce a static site deployable on GitHub Pages.
- **Dynamic API Routing:** `frontend/lib/api.ts` dynamically points to `NEXT_PUBLIC_API_URL` when deployed to static hosts, falling back to relative paths for local development.
- **Player & Telemetry:**
  - Integrated `hls.js` video player supporting adaptive bitrate streaming, resolution ladder switching, and buffer health tracking.
  - Telemetry HUD reporting real-time resolution, bitrate, buffer status, and in-player cache purge controls.
- **Styling & Assets:** Tailwind CSS with Lucide icons.

### 3. Containerization & Deployment
- **Docker Multi-Stage Build:** `Dockerfile.backend` compiles in `rust:1.80-slim-bookworm` and copies the stripped release binary to a minimal `debian:bookworm-slim` image (~85 MB total container footprint).
- **GitHub Actions Workflow:** `.github/workflows/deploy-pages.yml` automates static bundle generation and deployment to GitHub Pages.
- **Render Blueprint:** `render.yaml` defines the web service configuration with environment variables, health checks, and resource constraints for Render free tier.

---

## Render Free Tier Resource Benchmarks (512 MB RAM, 0.1 vCPU)

The backend was tested using `benchmark_render.py` on the compiled release binary under a simulated workload of 20 concurrent workers executing 300 requests across 115 stream and catalog endpoints.

| Metric | Measured Value | Render Free Tier Cap | Status |
| :--- | :--- | :--- | :--- |
| **Idle Working Set (RAM)** | 9.79 MB | 512 MB | 98.1% free headroom |
| **Peak Working Set Under Load** | 21.21 MB | 512 MB | 95.9% free headroom |
| **Private Memory Bytes** | 11.52 MB | 512 MB | Within threshold |
| **CPU Time Consumed** | 0.360 s (over 2.35 s elapsed) | 0.1 vCPU limit | Non-blocking async I/O |
| **Throughput** | 127.9 req/s | — | 100% success rate (0 errors) |
| **Median Response Latency** | 2.81 ms | — | In-memory cache hit |
| **Cache Clear Result** | Cache entries reset to 0 | — | Immediate memory release |

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
The frontend starts on `http://localhost:3000` and proxies `/api/*` to `http://127.0.0.1:8080`.

### 3. Run Automated Tests
```bash
cd backend
cargo test
```

---

## Deployment Instructions

### 1. Backend on Render Free Tier
1. Create an account on [Render.com](https://render.com).
2. Click **New +** > **Blueprint** and connect your GitHub repository.
3. Render reads `render.yaml` automatically:
   - Sets runtime to Docker (`Dockerfile.backend`).
   - Applies the Free plan (512 MB RAM, 0.1 vCPU).
   - Configures `SEGMENT_CACHE_MB=64`.
   - Generates unique secrets for `HMAC_SECRET` and `JWT_SECRET`.
4. Click **Apply**. Once built, note your backend URL: `https://<service-name>.onrender.com`.

### 2. Frontend on GitHub Pages
1. On GitHub, navigate to your repository > **Settings** > **Pages**.
2. Under **Build and deployment** > **Source**, choose **GitHub Actions**.
3. Navigate to **Settings** > **Secrets and variables** > **Actions** > **Variables** tab.
4. Add a variable:
   - **Name:** `NEXT_PUBLIC_API_URL`
   - **Value:** `https://<service-name>.onrender.com`
5. Push to `main` or `master`. The workflow in `.github/workflows/deploy-pages.yml` builds the static export and deploys to `https://<username>.github.io/<repo-name>/`.

---

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
