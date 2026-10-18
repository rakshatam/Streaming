# StreamFlow

StreamFlow is a demo HLS video catalog and streaming proxy. The backend uses Rust, Axum, SQLite, and Moka; the browser interface uses Next.js, React, TypeScript, and hls.js.

The repository includes a seeded catalog of public sample and live feeds. Availability and playback rights depend on each upstream provider. This project does not host or store the upstream video files.

## Features

- Catalog browsing, category filtering, and SQLite FTS5 search.
- HLS master and media playlist rewriting, signed stream URLs, byte-range forwarding, and a bounded in-memory segment cache.
- Username/password registration and login, Argon2id password hashes, and signed JWT sessions.
- Video likes and ratings, plus backend endpoints for watch history and bookmarks.
- Health and cache metrics endpoints.
- Docker Compose deployment with a persistent SQLite volume.

## Architecture

```text
Browser
  └─ Next.js frontend :3000
       └─ /api/* rewrite ──> Rust/Axum backend :8080
                               ├─ SQLite catalog, accounts, and interactions
                               ├─ bounded Moka segment and manifest caches
                               └─ HTTPS/HTTP HLS upstreams
```

For local development, Next.js sends `/api/*` to `http://127.0.0.1:8080`. In Docker, the frontend rewrite targets `http://streamflow-backend:8080` over the Compose network.

## Requirements

- Rust toolchain and Cargo.
- Node.js 20 or later and npm.
- Python 3.10 or later for the benchmark scripts.
- Docker Compose (optional).
- OpenSSL or another secure random generator to create application secrets.

## Configure secrets

The backend refuses to start without separate `HMAC_SECRET` and `JWT_SECRET` values, each at least 32 bytes long. Create a local environment file and replace both placeholders with different random values:

```powershell
Copy-Item backend/.env.example backend/.env
openssl rand -hex 32
```

Run the OpenSSL command separately for each secret and paste the results into `backend/.env`. Keep that file private; it is excluded from Git and Docker build contexts. `CACHE_ADMIN_TOKEN` is optional. Set it to a third random value only if you need the cache-clear endpoint.

Set `CORS_ALLOWED_ORIGINS` to the exact browser-facing frontend origin or comma-separated origins. The default allows local development at `localhost:3000` and `127.0.0.1:3000`.

## Run locally

Start the backend from the repository root with the command below, or run `cargo run --release` from the `backend` directory. The configuration loader reads `backend/.env` from either location.

```powershell
cargo run --release --manifest-path backend/Cargo.toml
```

The backend listens on `127.0.0.1:8080` by default, creates the SQLite database if needed, initializes the schema and FTS5 index, and synchronizes the seeded catalog.

In a second terminal:

```powershell
Set-Location frontend
npm ci
npm run dev
```

Open <http://localhost:3000>. The development rewrite sends API calls to the local backend. To use a different API server directly from the browser, set `NEXT_PUBLIC_API_URL` when building the frontend and include the frontend origin in the backend CORS allowlist.

## Run with Docker Compose

After creating and filling in `backend/.env`, run:

```powershell
docker compose --env-file backend/.env up --build
```

The frontend is available at <http://localhost:3000>. The backend is bound to `127.0.0.1:8080`; SQLite data persists in the `streamflow_data` named volume. Compose passes secrets from `backend/.env` and builds the frontend with the backend service URL.

For a separately built Next.js server, set `BACKEND_URL` at build time to the backend API origin. `NEXT_PUBLIC_API_URL` is used by browser code for direct API access; `BACKEND_URL` is used by the Next.js server rewrite. A static export requires `STATIC_EXPORT=true` and a public `NEXT_PUBLIC_API_URL`; no static-site deployment workflow is included.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SERVER_HOST` | `127.0.0.1` | Backend bind address. Use `0.0.0.0` inside a container. |
| `PORT` / `SERVER_PORT` | `8080` | Backend listening port. `PORT` takes precedence when set. |
| `DATABASE_URL` | `sqlite://streamflow.db?mode=rwc` | SQLx SQLite connection URL. Relative paths use the process working directory. |
| `HMAC_SECRET` | Required | Secret for signing stream resource URLs; minimum 32 bytes. |
| `JWT_SECRET` | Required | Separate secret for JWT signatures; minimum 32 bytes. |
| `CACHE_ADMIN_TOKEN` | Unset | Optional bearer token for `POST /api/cache/clear`. |
| `CORS_ALLOWED_ORIGINS` | Local frontend origins | Comma-separated HTTP(S) origins, without paths or wildcards. |
| `SEGMENT_CACHE_MB` | `64` | Segment cache capacity from 1 to 128 MiB. |
| `TOKEN_VALIDITY_SECS` | `3600` | Stream URL lifetime from 60 to 86,400 seconds. |
| `BACKEND_URL` | Local in development; Compose service in Docker | Build-time Next.js rewrite target. |
| `NEXT_PUBLIC_API_URL` | Empty | Optional browser API origin; empty uses relative `/api` paths. |
| `NEXT_PUBLIC_BASE_PATH` | Empty | Optional Next.js base path. |
| `STATIC_EXPORT` | `false` | Set to `true` to enable Next.js static export. |

Upstream TLS certificates are verified. The proxy accepts only HTTP(S) resource URLs, rejects local and reserved literal addresses and common local hostnames, and limits upstream concurrency and response sizes. Keep the catalog's upstream URLs under trusted control.

## API

All paths are served by the Rust backend. `:id` denotes a video ID.

| Method | Endpoint | Authentication | Description |
| --- | --- | --- | --- |
| `GET` | `/api/health` | None | Liveness response. |
| `GET` | `/api/metrics` | None | Stream request, cache, and byte counters. |
| `POST` | `/api/cache/clear` | `Authorization: Bearer <CACHE_ADMIN_TOKEN>` | Clear both caches and reset cache hit/miss counters. Returns 404 when no admin token is configured. |
| `POST` | `/api/auth/register` | None | Register with a username and 8–128-byte password. |
| `POST` | `/api/auth/login` | None | Authenticate and receive a JWT. |
| `GET` | `/api/auth/me` | Bearer JWT | Return the current account profile. |
| `GET` | `/api/videos` | None | List videos; supports `category`, `is_live`, `is_featured`, `limit`, and `offset`. |
| `GET` | `/api/videos/search?q={term}` | None | Search title, description, category, and tags. |
| `GET` | `/api/videos/categories` | None | List catalog categories and counts. |
| `GET` | `/api/videos/:id` | None | Return video metadata. |
| `GET` | `/api/videos/:id/interactions` | Optional bearer JWT | Return likes and rating aggregates, with the signed-in user's state when available. |
| `POST` | `/api/videos/:id/like` | Bearer JWT | Toggle the signed-in user's like. |
| `POST` | `/api/videos/:id/rate` | Bearer JWT | Set a 1–5 star rating. |
| `GET`, `POST` | `/api/user/history` | Bearer JWT | Read history or update playback progress. |
| `GET`, `POST` | `/api/user/bookmarks` | Bearer JWT | Read bookmarks or toggle a bookmark. |
| `GET` | `/api/stream/manifest/:id` | None | Fetch and rewrite a catalog HLS manifest. |
| `GET` | `/api/stream/playlist` | Signed stream URL | Fetch and rewrite a nested playlist. |
| `GET` | `/api/stream/segment` | Signed stream URL | Fetch a segment, forward an optional byte range, and cache eligible responses. |

Stream tokens bind a URL and expiration time. The proxy uses a four-request upstream concurrency limit, a 5 MiB playlist limit, and a 35 MiB segment limit; rewritten playlists are capped at 6 MiB, and only segments up to 10 MiB are cached. The configured segment cache limit applies to cached payload bytes.

## Checks and benchmarks

```powershell
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo check --manifest-path backend/Cargo.toml
cargo test --manifest-path backend/Cargo.toml
Set-Location frontend
npm run typecheck
npm run build
```

The GitHub Actions workflow runs Rust checks and tests, a frontend typecheck and build, a local HTTP throughput benchmark, and Docker image builds. To run the throughput script against a running backend, set `STREAMFLOW_API_BASE` (default `http://127.0.0.1:8080`) and run `python scripts/throughput_benchmark.py`.

`benchmark_render.py` is a local stress probe, not a Render-hosted capacity test. It can sample the backend process on Windows; it does not claim a fixed memory limit, CPU allocation, or throughput result. Benchmark numbers depend on the host, upstream availability, and network conditions, so this README does not publish a fixed baseline.

## License

MIT. See [LICENSE](LICENSE).
