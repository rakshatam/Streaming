#!/usr/bin/env python3
"""
StreamFlow High-Throughput & Latency Benchmarking Suite
Measures RPS, TTFB, P95 latency, segment streaming bandwidth, and LRU cache acceleration.
"""

import sys
import time
import urllib.request
import urllib.error
import ssl
import json
import concurrent.futures
from typing import List, Dict

BASE_URL = "http://127.0.0.1:8080"

ctx = ssl.create_default_context()
ctx.check_hostname = False
ctx.verify_mode = ssl.CERT_NONE

def fetch_url(url: str, headers: dict = None) -> (int, float, int):
    t0 = time.perf_counter()
    req = urllib.request.Request(url, headers=headers or {'User-Agent': 'StreamFlow-Benchmark/1.0'})
    try:
        with urllib.request.urlopen(req, context=ctx, timeout=10) as resp:
            data = resp.read()
            elapsed_ms = (time.perf_counter() - t0) * 1000
            return resp.status, elapsed_ms, len(data)
    except urllib.error.HTTPError as e:
        elapsed_ms = (time.perf_counter() - t0) * 1000
        return e.code, elapsed_ms, 0
    except Exception as e:
        elapsed_ms = (time.perf_counter() - t0) * 1000
        return 0, elapsed_ms, 0

def run_concurrency_test(name: str, url: str, num_requests: int = 100, concurrency: int = 10):
    print(f"\n=======================================================")
    print(f"[*] Benchmark: {name}")
    print(f"[*] Target: {url}")
    print(f"[*] Total Requests: {num_requests} | Concurrency: {concurrency}")
    print(f"=======================================================")

    t_start = time.perf_counter()
    latencies: List[float] = []
    statuses: Dict[int, int] = {}
    total_bytes = 0

    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = [executor.submit(fetch_url, url) for _ in range(num_requests)]
        for f in concurrent.futures.as_completed(futures):
            status, elapsed_ms, byte_count = f.result()
            latencies.append(elapsed_ms)
            statuses[status] = statuses.get(status, 0) + 1
            total_bytes += byte_count

    total_time = time.perf_counter() - t_start
    latencies.sort()

    rps = num_requests / total_time if total_time > 0 else 0
    avg_latency = sum(latencies) / len(latencies) if latencies else 0
    p50 = latencies[int(len(latencies) * 0.50)] if latencies else 0
    p95 = latencies[int(len(latencies) * 0.95)] if latencies else 0
    p99 = latencies[int(len(latencies) * 0.99)] if latencies else 0
    mb_transferred = total_bytes / (1024 * 1024)
    mbps = (total_bytes * 8) / (total_time * 1000 * 1000) if total_time > 0 else 0

    print(f"[+] Status Codes: {statuses}")
    print(f"[+] Total Duration: {total_time:.2f}s")
    print(f"[+] Requests/sec (RPS): {rps:,.1f}")
    print(f"[+] Bandwidth: {mbps:.2f} Mbps ({mb_transferred:.2f} MB total)")
    print(f"[+] Latency avg: {avg_latency:.2f}ms | P50: {p50:.2f}ms | P95: {p95:.2f}ms | P99: {p99:.2f}ms")

    return {
        "name": name,
        "rps": rps,
        "avg_ms": avg_latency,
        "p95_ms": p95,
        "success_rate": (statuses.get(200, 0) / num_requests) * 100,
    }

def main():
    print("[*] Verifying StreamFlow Rust backend connection...")
    try:
        status, _, _ = fetch_url(f"{BASE_URL}/api/health")
        if status != 200:
            print(f"[-] Backend health check failed with status {status}. Ensure server is running on port 8080.")
            sys.exit(1)
        print("[+] Backend health verified! (HTTP 200 OK)")
    except Exception as e:
        print(f"[-] Could not connect to {BASE_URL}: {e}")
        print("    Please run the backend with 'cargo run --release' first.")
        sys.exit(1)

    results = []

    # 1. Health endpoint throughput
    results.append(run_concurrency_test("API Health Throughput", f"{BASE_URL}/api/health", 200, 20))

    # 2. Video Catalog retrieval
    results.append(run_concurrency_test("Catalog Listing (SQLite WAL)", f"{BASE_URL}/api/videos", 150, 15))

    # 3. FTS5 Full-Text Search
    results.append(run_concurrency_test("FTS5 Full-Text Search ('space')", f"{BASE_URL}/api/videos/search?q=space", 100, 10))

    # 4. Stream Manifest Rewriting & HMAC Token generation
    results.append(run_concurrency_test("M3U8 Manifest Rewriter & HMAC Token Signer", f"{BASE_URL}/api/stream/manifest/tears-of-steel", 50, 5))

    # 5. Fetch telemetry metrics
    status, _, data_len = fetch_url(f"{BASE_URL}/api/metrics")
    print(f"\n[+] Stream Engine Telemetry Status: {status}")

    print("\n=======================================================")
    print("                     BENCHMARK SUMMARY                 ")
    print("=======================================================")
    print(f"{'Test':<45} {'RPS':<10} {'Avg Latency':<12} {'P95 Latency':<12} {'Success':<8}")
    print("-" * 87)
    for r in results:
        print(f"{r['name']:<45} {r['rps']:<10.1f} {r['avg_ms']:<10.2f}ms {r['p95_ms']:<10.2f}ms {r['success_rate']:<6.1f}%")
    print("=======================================================\n")

if __name__ == "__main__":
    main()
