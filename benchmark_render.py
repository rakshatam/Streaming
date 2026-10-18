#!/usr/bin/env python3
"""
Local StreamFlow stress and resource probe.
The script measures API performance locally; process metrics are available on Windows only.
"""

import concurrent.futures
import json
import math
import os
import subprocess
import time
import urllib.request
import urllib.error

API_BASE = os.environ.get("STREAMFLOW_API_BASE", "http://127.0.0.1:8080").rstrip("/")

def get_backend_process_info():
    if os.name != "nt":
        return None

    command = (
        "Get-Process streamflow-backend -ErrorAction SilentlyContinue | "
        "Select-Object -First 1 Id, "
        "@{Name='WorkingSetMB'; Expression={[math]::round($_.WorkingSet64 / 1MB, 2)}}, "
        "@{Name='PrivateMemoryMB'; Expression={[math]::round($_.PrivateMemorySize64 / 1MB, 2)}}, "
        "@{Name='TotalCpuSecs'; Expression={[math]::round($_.TotalProcessorTime.TotalSeconds, 3)}} | "
        "ConvertTo-Json"
    )
    try:
        out = subprocess.check_output(
            ["powershell", "-NoProfile", "-Command", command],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        if out:
            data = json.loads(out)
            return {
                "id": data.get("Id"),
                "working_set_mb": data.get("WorkingSetMB", 0.0),
                "private_memory_mb": data.get("PrivateMemoryMB", 0.0),
                "cpu_secs": data.get("TotalCpuSecs", 0.0),
            }
    except Exception as e:
        print(f"Failed to get process info: {e}")
    return None

def fetch_url(url, timeout=10):
    start = time.perf_counter()
    req = urllib.request.Request(url, headers={"User-Agent": "StreamFlowBenchmark/1.0"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = resp.read()
            status = resp.status
            elapsed_ms = (time.perf_counter() - start) * 1000.0
            return True, status, len(data), elapsed_ms, data
    except urllib.error.HTTPError as e:
        elapsed_ms = (time.perf_counter() - start) * 1000.0
        return False, e.code, 0, elapsed_ms, b""
    except Exception as e:
        elapsed_ms = (time.perf_counter() - start) * 1000.0
        return False, 0, 0, elapsed_ms, b""

def main():
    print("=" * 65)
    print(" STREAMFLOW LOCAL STRESS & RESOURCE PROBE")
    print(f" API Target: {API_BASE}")
    print("=" * 65)

    # 1. Baseline Memory
    proc_init = get_backend_process_info()
    print("\n[1] BASELINE PROCESS SAMPLE:")
    if proc_init:
        print(f"    - Process PID:       {proc_init['id']}")
        print(f"    - Working Set:       {proc_init['working_set_mb']:.2f} MB")
        print(f"    - Private Memory:    {proc_init['private_memory_mb']:.2f} MB")
        print(f"    - CPU Time:          {proc_init['cpu_secs']:.3f} s")
    else:
        print("    - Process metrics unavailable (this probe uses Windows process sampling).")

    # 2. Discover Videos & Manifests
    print("\n[2] DISCOVERING TARGET CATALOG & STREAMS...")
    ok, status, size, elapsed, data = fetch_url(f"{API_BASE}/api/videos")
    if not ok:
        print(f"Failed to fetch video catalog: HTTP {status}")
        return

    videos = json.loads(data.decode("utf-8"))
    print(f"    - Catalog count:     {len(videos)} channels/movies")

    # Collect URLs to test: catalog endpoints, search queries, manifests, and segments
    urls_to_test = [
        f"{API_BASE}/api/videos",
        f"{API_BASE}/api/videos/categories",
        f"{API_BASE}/api/videos/search?q=space",
        f"{API_BASE}/api/videos/search?q=live",
        f"{API_BASE}/api/videos/search?q=tears",
        f"{API_BASE}/api/metrics",
    ]

    segment_urls = []
    for v in videos:
        vid_id = v["id"]
        manifest_url = f"{API_BASE}/api/stream/manifest/{vid_id}"
        urls_to_test.append(manifest_url)

        m_ok, _, _, _, m_data = fetch_url(manifest_url)
        if m_ok:
            text = m_data.decode("utf-8", errors="ignore")
            # Extract rewritten gateway sub-playlists or segments
            lines = text.splitlines()
            for line in lines:
                line = line.strip()
                if line.startswith("/api/stream/"):
                    segment_urls.append(f"{API_BASE}{line}")
                elif "api/stream/" in line:
                    idx = line.find("/api/stream/")
                    segment_urls.append(f"{API_BASE}{line[idx:]}")

    print(f"    - Discovered stream endpoints: {len(segment_urls)} segment/playlist URIs")
    if segment_urls:
        urls_to_test.extend(segment_urls[:30])

    # 3. High Concurrency Load Test
    NUM_WORKERS = 20
    NUM_REQUESTS = 300
    print(f"\n[3] LAUNCHING LOAD STRESS TEST:")
    print(f"    - Total Requests:    {NUM_REQUESTS}")
    print(f"    - Concurrent Workers:{NUM_WORKERS}")
    print(f"    - Simulating video buffering and API client navigation...")

    proc_load_start = get_backend_process_info()
    request_targets = []
    for i in range(NUM_REQUESTS):
        request_targets.append(urls_to_test[i % len(urls_to_test)])

    latencies = []
    successes = 0
    failures = 0
    bytes_transferred = 0

    t_start = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=NUM_WORKERS) as executor:
        futures = [executor.submit(fetch_url, u, 15) for u in request_targets]
        for f in concurrent.futures.as_completed(futures):
            ok, code, sz, elapsed_ms, _ = f.result()
            # Include failed responses in latency statistics so slow failures
            # are not hidden from the reported tail latency.
            latencies.append(elapsed_ms)
            if ok and code in (200, 206):
                successes += 1
                bytes_transferred += sz
            else:
                failures += 1

    t_total = time.perf_counter() - t_start
    throughput_rps = NUM_REQUESTS / t_total

    latencies.sort()
    avg_lat = sum(latencies) / len(latencies) if latencies else 0.0
    med_lat = latencies[len(latencies) // 2] if latencies else 0.0
    p95_lat = latencies[min(math.ceil(len(latencies) * 0.95) - 1, len(latencies) - 1)] if latencies else 0.0
    min_lat = latencies[0] if latencies else 0.0
    max_lat = latencies[-1] if latencies else 0.0

    # This is a post-load sample, not a peak measurement during the load.
    proc_after = get_backend_process_info()

    # Query metrics from backend
    _, _, _, _, m_data = fetch_url(f"{API_BASE}/api/metrics")
    metrics_json = json.loads(m_data.decode("utf-8")) if m_data else {}

    print(f"\n[4] LOAD TEST RESULTS:")
    print(f"    - Total Time Elapsed: {t_total:.2f} s")
    print(f"    - Successful Requests:{successes}/{NUM_REQUESTS} ({(successes/NUM_REQUESTS)*100:.1f}%)")
    print(f"    - Failed Requests:    {failures}")
    print(f"    - Throughput:         {throughput_rps:.1f} req/s")
    print(f"    - Data Streamed:      {bytes_transferred / (1024*1024):.2f} MB")
    print(f"    - Latency Min:        {min_lat:.2f} ms")
    print(f"    - Latency Median:     {med_lat:.2f} ms")
    print(f"    - Latency Mean:       {avg_lat:.2f} ms")
    print(f"    - Latency P95:        {p95_lat:.2f} ms")
    print(f"    - Latency Max:        {max_lat:.2f} ms")
    print(f"    - Cache Hit Ratio:    {metrics_json.get('cache_hit_ratio_percent', 0.0)}%")
    print(f"    - Cached Segments:    {metrics_json.get('segment_cache_entries', 0)}")
    print(f"    - Cached Manifests:   {metrics_json.get('manifest_cache_entries', 0)}")

    print("\n[5] POST-LOAD PROCESS SAMPLE:")
    if proc_load_start and proc_after and proc_load_start["id"] == proc_after["id"]:
        cpu_seconds = proc_after["cpu_secs"] - proc_load_start["cpu_secs"]
        if cpu_seconds >= 0:
            average_cpu_percent = cpu_seconds / max(t_total, 0.001) * 100.0
            print(f"    - Working Set:       {proc_after['working_set_mb']:.2f} MB")
            print(f"    - Private Memory:    {proc_after['private_memory_mb']:.2f} MB")
            print(f"    - CPU Time Delta:    {cpu_seconds:.3f} s over {t_total:.2f} s")
            print(f"    - Average CPU Use:   {average_cpu_percent:.1f}% of one logical CPU")
        else:
            print("    - CPU sample changed during the probe; usage delta is unavailable.")
    else:
        print("    - Process metrics unavailable.")

    admin_token = os.environ.get("CACHE_ADMIN_TOKEN")
    if admin_token:
        print("\n[6] CLEARING CACHE WITH CONFIGURED ADMIN TOKEN:")
        req_clear = urllib.request.Request(
            f"{API_BASE}/api/cache/clear",
            data=b"",
            headers={"Authorization": f"Bearer {admin_token}", "User-Agent": "StreamFlowBenchmark/1.0"},
        )
        try:
            with urllib.request.urlopen(req_clear, timeout=5) as resp:
                print(f"    - Cache clear response: HTTP {resp.status}")
        except urllib.error.HTTPError as error:
            print(f"    - Cache clear failed: HTTP {error.code}")
    else:
        print("\n[6] Cache clear skipped; set CACHE_ADMIN_TOKEN to authorize it.")

    print("\nResource figures are local process samples; this script does not measure remote Render limits.")

if __name__ == "__main__":
    main()
