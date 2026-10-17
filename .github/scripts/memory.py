"""Compare production images on a local Linux Docker Engine (cgroup v2).

Creates disposable containers/anonymous data volumes and never opens deployed data.
Example: python3 .github/scripts/memory.py --baseline link-withu-python:v1.0.1 \
    --candidate link-withu:v1.0.1-rust --output /tmp/link-withu-memory.json
"""

import argparse
import http.client
import json
import platform
import socket
import statistics
import subprocess
import threading
import time
import uuid
from concurrent.futures import ThreadPoolExecutor
from datetime import UTC, datetime
from pathlib import Path
from urllib.error import URLError
from urllib.request import Request, urlopen

PASSWORD = "disposable-memory-benchmark-only"


def docker(*args):
    return subprocess.check_output(["docker", *args], text=True).strip()


def stats(container):
    # Docker CLI's memory display on cgroup v2 subtracts inactive_file. Reading
    # Engine stats avoids starting a measurement process inside the container.
    connection = http.client.HTTPConnection("localhost", timeout=10)
    connection.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.sock.settimeout(10)
    connection.sock.connect("/var/run/docker.sock")
    try:
        connection.request(
            "GET", f"/containers/{container}/stats?stream=false&one-shot=true"
        )
        response = connection.getresponse()
        if response.status != 200:
            raise RuntimeError(response.read().decode())
        memory = json.load(response)["memory_stats"]
    finally:
        connection.close()
    counters = memory["stats"]
    inactive = counters["inactive_file"]
    return {
        "docker_bytes": memory["usage"] - inactive,
        "cgroup_bytes": memory["usage"],
        "anon_bytes": counters["anon"],
        "inactive_file_bytes": inactive,
    }


def request(base, path, *, data=None, token=None, method=None):
    headers = {}
    if token:
        headers["Authorization"] = "Bearer " + token
    if data is not None:
        headers["Content-Type"] = "application/json"
    req = Request(
        base + path,
        data=json.dumps(data).encode() if data is not None else None,
        headers=headers,
        method=method,
    )
    with urlopen(req, timeout=60) as response:
        return response.read()


def workload(base, token, redirects, workers):
    # http.client deliberately does not follow external redirects.
    host, port = base.removeprefix("http://").split(":")

    def visit(i):
        connection = http.client.HTTPConnection(host, int(port), timeout=60)
        try:
            connection.request(
                "GET", f"/r/memory-{i % 32}", headers={"User-Agent": "memory-benchmark"}
            )
            response = connection.getresponse()
            if response.status != 302:
                raise RuntimeError(
                    f"Redirect failed: {response.status} {response.read()!r}"
                )
            response.read()
        finally:
            connection.close()

    start = time.monotonic()
    with ThreadPoolExecutor(max_workers=workers) as pool:
        list(pool.map(visit, range(redirects)))
    redirect_seconds = time.monotonic() - start
    for i in range(20):
        request(base, f"/api/links?sort={'visits' if i % 2 else 'newest'}", token=token)
    for _ in range(2):
        result = json.loads(request(base, "/api/analytics", token=token))
    csv_bytes = len(request(base, "/api/analytics/export", token=token))
    backup = json.loads(request(base, "/api/admin/backup", token=token))
    if len(backup["visits"]) != result["summary"]["total_visits"]:
        raise RuntimeError("Visit count and backup disagree")
    return {
        "redirect_seconds": redirect_seconds,
        "visits": len(backup["visits"]),
        "csv_bytes": csv_bytes,
    }


def measure(image, args, trial):
    name = "link-withu-benchmark-" + uuid.uuid4().hex[:12]
    container = docker(
        "run",
        "-d",
        "--name",
        name,
        "-p",
        "127.0.0.1::5201",
        "--mount",
        "type=volume,destination=/proj/data",
        "-e",
        "ADMIN_PASSWORD=" + PASSWORD,
        image,
    )
    samples, phase = [], "startup"
    stop = threading.Event()
    errors = []

    def monitor():
        while not stop.is_set():
            try:
                samples.append(
                    {
                        "phase": phase,
                        "elapsed_seconds": round(time.monotonic() - started, 3),
                        **stats(container),
                    }
                )
            except (
                OSError,
                ValueError,
                KeyError,
                RuntimeError,
                http.client.HTTPException,
            ) as error:
                errors.append(str(error))
                return
            stop.wait(0.2)

    started = time.monotonic()
    thread = threading.Thread(target=monitor)
    try:
        port = json.loads(docker("inspect", container))[0]["NetworkSettings"]["Ports"][
            "5201/tcp"
        ][0]["HostPort"]
        base = "http://127.0.0.1:" + port
        deadline = time.monotonic() + 60
        while True:
            try:
                health = json.loads(request(base, "/api/health"))
                break
            except (URLError, ConnectionError):
                if time.monotonic() > deadline:
                    raise RuntimeError(docker("logs", container)) from None
                time.sleep(0.2)
        thread.start()
        phase = "idle"
        time.sleep(args.settle)
        phase = "setup"
        token = json.loads(
            request(base, "/api/admin/login", data={"password": PASSWORD})
        )["token"]
        request(base, "/")
        request(base, "/openapi.json")
        for i in range(32):
            request(
                base,
                "/api/links",
                token=token,
                data={
                    "original_url": "https://example.org/path",
                    "short_code": f"memory-{i}",
                    "title": f"Memory {i}",
                    "tags": ["memory", f"group-{i % 4}"],
                },
            )
        rounds = []
        for i in range(args.rounds):
            phase = f"load-{i + 1}"
            rounds.append(workload(base, token, args.redirects, args.workers))
            phase = f"settled-{i + 1}"
            time.sleep(args.settle)
        if rounds[-1]["visits"] != args.redirects * args.rounds:
            raise RuntimeError("Lost visits")
    finally:
        stop.set()
        if thread.is_alive():
            thread.join(timeout=15)
        docker("rm", "-f", "-v", container)
    if errors:
        raise RuntimeError(errors)
    summary = {}
    for label in dict.fromkeys(sample["phase"] for sample in samples):
        values = [
            sample["docker_bytes"] for sample in samples if sample["phase"] == label
        ]
        summary[label] = {
            "median_mib": round(statistics.median(values) / 2**20, 2),
            "max_mib": round(max(values) / 2**20, 2),
            "samples": len(values),
        }
    result = {
        "image": image,
        "image_id": json.loads(docker("image", "inspect", image))[0]["Id"],
        "version": health["version"],
        "trial": trial,
        "summary": summary,
        "rounds": rounds,
        "samples": samples,
    }
    print(
        json.dumps({k: result[k] for k in ("image", "trial", "summary", "rounds")}),
        flush=True,
    )
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--rounds", type=int, default=2)
    parser.add_argument("--redirects", type=int, default=1000)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--settle", type=float, default=10)
    args = parser.parse_args()
    for name in ("trials", "rounds", "redirects", "workers", "settle"):
        if getattr(args, name) <= 0:
            parser.error(f"--{name} must be positive")
    info = json.loads(docker("info", "--format", "{{json .}}"))
    if info.get("CgroupVersion") != "2":
        parser.error("This benchmark requires cgroup v2")
    output = {
        "created_at": datetime.now(UTC).isoformat(),
        "platform": platform.platform(),
        "docker": {
            k: info[k] for k in ("ServerVersion", "Architecture", "NCPU", "MemTotal")
        },
        "parameters": {
            k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()
        },
        "results": [],
    }
    for trial in range(1, args.trials + 1):
        # Alternate order to reduce systematic cache/warm-up bias.
        images = (
            (args.baseline, args.candidate)
            if trial % 2
            else (args.candidate, args.baseline)
        )
        for image in images:
            output["results"].append(measure(image, args, trial))
            args.output.write_text(
                json.dumps(output, ensure_ascii=False, indent=2) + "\n"
            )


if __name__ == "__main__":
    main()
