# Storage & Cache Maintenance Guide

This document explains why storage grows during local development in Rust and Next.js, how to reclaim disk space, and how to rebuild artifacts when needed.

---

## 1. Storage Consumption Breakdown

When building locally, the repository size expands from **< 1 MB of source code** to **over 6 GB** due to compiler output and dependencies:

| Directory | Typical Size | Description |
| :--- | :--- | :--- |
| `backend/target/debug/` | ~4.8 GB | Unoptimized binaries, incremental compilation cache, and full Windows PDB debug symbol databases (`.pdb`, `.rlib`, `.obj`) for all dependency crates. |
| `backend/target/release/` | ~1.0 GB | Optimized production binary and intermediate LLVM bitcode artifacts. |
| `frontend/node_modules/` | ~310 MB | Installed Node.js packages for Next.js, React, Tailwind CSS, and TypeScript. |
| `frontend/.next/` | ~150 MB | Next.js SWC/Webpack build cache, compiled page bundles, and static traces. |
| **Source Code & Git** | **< 1 MB** | Actual repository tracking size in Git (`.gitignore` excludes all the above). |

Neither `target/` nor `node_modules/` are tracked by Git or included in production Docker images (~85 MB total).

---

## 2. How to Reclaim Space (Clean Up)

### Option A: Clean Backend Build Artifacts (Frees ~5.7 GB)
Run `cargo clean` inside the `backend` folder:
```powershell
cd backend
cargo clean
cd ..
```
*Effect: Deletes `backend/target/`, immediately releasing ~5.6 to 5.8 GB of disk space in seconds.*

### Option B: Clean Frontend Cache (Frees ~150 MB)
Delete `.next` and `out` directories inside the `frontend` folder:
```powershell
cd frontend
Remove-Item -Recurse -Force .next, out -ErrorAction SilentlyContinue
cd ..
```

### Option C: Complete One-Line Repository Cleanup
Run this single command from the project root to purge all build caches:
```powershell
cd backend; cargo clean; cd ../frontend; Remove-Item -Recurse -Force .next, out -ErrorAction SilentlyContinue; cd ..
```
*After running this, the entire repository drops back to its minimal development footprint (~313 MB, consisting only of source files and installed node_modules).*

### Option D: Deep Clean (Frees Everything, Back to Pure Source Code < 1 MB)
If you want to archive or move the folder and remove `node_modules` as well:
```powershell
cd backend; cargo clean; cd ../frontend; Remove-Item -Recurse -Force node_modules, .next, out -ErrorAction SilentlyContinue; cd ..
```

---

## 3. How to Rebuild Files (When You Want to Run Again)

You can regenerate any deleted artifacts whenever you need to develop or test:

### 1. Rebuilding Backend
```powershell
cd backend
# Rebuild and run in release mode:
cargo run --release

# Or compile for testing:
cargo test
```
*Cargo will re-download missing crate binaries and recompile `target/` automatically.*

### 2. Rebuilding Frontend
```powershell
cd frontend
# If node_modules was deleted, re-install:
npm install

# Start local development server:
npm run dev

# Or compile production bundle:
npm run build
```

---

## 4. Reducing Debug Symbol Bloat in `Cargo.toml` (Optional)

If you compile often in debug mode and want to keep `target/debug/` under 600 MB instead of 4.8 GB, you can configure light debug symbols in `backend/Cargo.toml`:

```toml
[profile.dev]
debug = 1 # '1' includes line tables only (sufficient for panic stack traces, avoids multi-gigabyte PDB symbol bloat)

[profile.test]
debug = 1
```

---

## 5. Automated Periodic Cleanup Tool (`cargo-sweep`)

If you work on multiple Rust projects, you can install `cargo-sweep` to automatically remove build artifacts older than 30 days or not used in the latest build:

```powershell
# Install once
cargo install cargo-sweep

# Clean build artifacts older than 14 days
cd backend
cargo sweep --time 14
```
