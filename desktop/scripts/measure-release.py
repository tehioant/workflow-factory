#!/usr/bin/env python3
"""Opt-in Linux desktop idle observation; metadata-only unless explicitly enabled."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError("an absolute path is required")
    return path.resolve()


def fixture_path(value):
    if not os.path.isabs(value) or any(p in (".", "..", "") for p in value.split("/")[1:]):
        raise argparse.ArgumentTypeError("fixture requires an explicit absolute path without aliases")
    return Path(value)


def copy_prepared_fixture(source, destination):
    """Copy exactly the two admitted synthetic payloads, never an arbitrary tree.

    A declaration/hash is provenance, not proof of nonsensitivity. Independently
    review the offline generator and validate projections before launch admission.
    """
    import stat
    import uuid
    from contextlib import ExitStack

    source = fixture_path(str(source))
    if destination.resolve() == source or source in destination.resolve().parents:
        raise ValueError("fixture destination must be separate from source")
    with ExitStack() as stack:
        def opened(name, parent=None, directory=False, private=True):
            flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
            if directory:
                flags |= os.O_DIRECTORY
            fd = os.open(name, flags, dir_fd=parent)
            stack.callback(os.close, fd)
            info = os.fstat(fd)
            if private and (info.st_uid != os.geteuid() or info.st_mode & 0o077):
                raise ValueError("fixture inputs must be private and owned")
            if not directory and (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1):
                raise ValueError("fixture inputs must be single-link regular files")
            return fd

        root = opened("/", directory=True, private=False)
        for index, part in enumerate(source.parts[1:]):
            root = opened(part, root, directory=True, private=index == len(source.parts) - 2)

        def read(name, parent, limit):
            fd = opened(name, parent)
            before = os.fstat(fd)
            if before.st_size > limit:
                raise ValueError("fixture byte limit exceeded")
            data = bytearray()
            while len(data) <= limit:
                chunk = os.read(fd, min(65536, limit + 1 - len(data)))
                if not chunk:
                    break
                data.extend(chunk)
            after = os.fstat(fd)
            if len(data) > limit or (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                raise ValueError("fixture changed or exceeded byte limit")
            return bytes(data), {"sha256": hashlib.sha256(data).hexdigest(),
                                 "bytes": len(data), "mode": oct(stat.S_IMODE(before.st_mode))}

        manifest_bytes, manifest_identity = read("synthetic-fixture.json", root, 16384)
        manifest = json.loads(manifest_bytes)
        run_id = manifest["run"]
        if not isinstance(run_id, str) or str(uuid.UUID(run_id)) != run_id:
            raise ValueError("invalid fixture run identity")
        if manifest["scenario"] != "SYNTHETIC-neverexecuted-A-B" or not re.fullmatch(r"[0-9a-f]{64}", manifest["generator_sha256"]):
            raise ValueError("explicit synthetic scenario and generator identity required")
        journal_path = f"inspection/{run_id}/journal"
        expected = {"definitions.sqlite3", journal_path}
        if set(manifest["files"]) != expected:
            raise ValueError("fixture must name only the database and selected journal")
        inspection = opened("inspection", root, directory=True)
        journal_dir = opened(run_id, inspection, directory=True)
        payloads = {}
        identities = {}
        for name, parent, leaf, limit in [("definitions.sqlite3", root, "definitions.sqlite3", 2097152),
                                          (journal_path, journal_dir, "journal", 1048576)]:
            data, identity = read(leaf, parent, limit)
            if manifest["files"][name] != {k: identity[k] for k in ("sha256", "bytes")}:
                raise ValueError("fixture payload differs from manifest")
            payloads[name] = data
            identities[name] = identity
        destination.mkdir(mode=0o700, exist_ok=False)
        (destination / "inspection").mkdir(mode=0o700)
        (destination / "inspection" / run_id).mkdir(mode=0o700)
        for name, data in payloads.items():
            fd = os.open(destination / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, "wb") as target:
                target.write(data)
        return {"source": str(source), "destination": str(destination),
                "scenario": manifest["scenario"], "generator_sha256": manifest["generator_sha256"],
                "manifest": manifest_identity, "files": identities,
                "destination_directory_mode": "0o700", "destination_file_mode": "0o600",
                "limit": "exactly 2 payloads; database <=2 MiB, journal <=1 MiB; no arbitrary tree copy"}


def run(argv, cwd=None):
    return subprocess.check_output(argv, cwd=cwd, text=True, timeout=5).strip()


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def candidate(args):
    head = run(["git", "rev-parse", "HEAD"], args.checkout)
    if head != args.expected_head:
        raise ValueError("HEAD differs from --expected-head")
    if run(["git", "status", "--porcelain", "--untracked-files=all"], args.checkout):
        raise ValueError("checkout must have clean tracked, index and untracked state")
    if not args.binary.is_file() or not os.access(args.binary, os.X_OK):
        raise ValueError("binary must be an executable regular file")
    actual = digest(args.binary)
    if actual != args.binary_sha256:
        raise ValueError("binary SHA-256 differs from supplied candidate receipt")
    return {"head": head, "tree": run(["git", "rev-parse", "HEAD^{tree}"], args.checkout),
            "binary": str(args.binary), "binary_sha256": actual,
            "binary_bytes": args.binary.stat().st_size,
            "provenance_limit": "Supplied hash binds bytes, not proof of a release build; retain build receipt."}


def hardware():
    result = {"os": platform.platform(), "architecture": platform.machine(),
              "logical_cpus": os.cpu_count(), "cpu": platform.processor()}
    if sys.platform.startswith("linux"):
        result["os_release"] = Path("/etc/os-release").read_text()
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                result["cpu"] = line.split(":", 1)[1].strip()
                break
        result["physical_memory_bytes"] = os.sysconf("SC_PHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")
    elif sys.platform == "darwin":
        result["cpu"] = run(["/usr/sbin/sysctl", "-n", "machdep.cpu.brand_string"])
        result["physical_memory_bytes"] = int(run(["/usr/sbin/sysctl", "-n", "hw.memsize"]))
        result["macos_version"] = platform.mac_ver()[0]
    return result


def proc_table():
    records = {}
    for path in Path("/proc").iterdir():
        if not path.name.isdigit():
            continue
        try:
            text = (path / "stat").read_text()
            tail = text[text.rindex(")") + 2:].split()
            pid = int(path.name)
            records[pid] = {"pid": pid, "ppid": int(tail[1]), "session": int(tail[3]),
                            "start_ticks": int(tail[19]),
                            "cpu_ticks": int(tail[11]) + int(tail[12]),
                            "rss_bytes": int(tail[21]) * os.sysconf("SC_PAGE_SIZE"),
                            "name": text[text.index("(") + 1:text.rindex(")")]}
        except (OSError, ValueError, IndexError):
            continue
    return records


def window_ids(pid, env):
    # No UI interaction, screenshot, title or contents are collected.
    if not shutil.which("xdotool") or not shutil.which("xwininfo"):
        return None
    found = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(pid)],
                           env=env, capture_output=True, text=True, timeout=1)
    mapped = []
    for value in found.stdout.split()[:32]:
        if not value.isdigit():
            continue
        info = subprocess.run(["xwininfo", "-id", value], env=env, capture_output=True,
                              text=True, timeout=1)
        if "Map State: IsViewable" in info.stdout:
            mapped.append(value)
    return mapped


def observe(args, report):
    if not sys.platform.startswith("linux"):
        report["observation"] = {"status": "unavailable", "reason":
            "Linux /proc + pidfd + X11 sampler only. macOS WKWebView helpers may be outside the app ancestry; native Instruments/Activity Monitor and window evidence required. No Linux fallback numbers."}
        return
    if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
        raise ValueError("Linux pidfd support required for identity-safe cleanup")
    if not args.display:
        raise ValueError("--observe-idle requires --display for an explicitly prepared private X11 display")
    home = args.output / "private-home"
    home.mkdir(mode=0o700)
    env = {"PATH": "/usr/bin:/bin", "HOME": str(home), "DISPLAY": args.display,
           "GDK_BACKEND": "x11", "LANG": "C.UTF-8"}
    for name in ("DBUS_SESSION_BUS_ADDRESS", "XAUTHORITY"):
        if name in os.environ:
            env[name] = os.environ[name]
    for key, folder in (("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache"),
                        ("XDG_CONFIG_HOME", "config"), ("XDG_RUNTIME_DIR", "runtime")):
        path = args.output / folder
        path.mkdir(mode=0o700)
        env[key] = str(path)
    if args.prepared_fixture is not None:
        report["prepared_fixture"] = copy_prepared_fixture(
            args.prepared_fixture, Path(env["XDG_DATA_HOME"]) / "com.workflowfactory.desktop")
    owned = {}  # (PID, kernel start time) -> pidfd; never signal a reused PID.
    samples = []
    obs = {"status": "started", "window_map": None, "samples": samples}
    report["observation"] = obs
    started = time.monotonic()
    with (args.output / "app.log").open("x") as log:
        app = subprocess.Popen([str(args.binary)], cwd=args.output, env=env,
                               stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                               start_new_session=True)
        def sample():
            table = proc_table()
            selected = {pid for (pid, start) in owned
                        if pid in table and table[pid]["start_ticks"] == start}
            # A new session is private to this child; include reparented WebKit helpers.
            selected.update(pid for pid, row in table.items() if row["session"] == app.pid)
            while True:
                following = selected | {pid for pid, row in table.items() if row["ppid"] in selected}
                if following == selected:
                    break
                selected = following
            rows = []
            for pid in selected:
                row = table[pid]
                key = (pid, row["start_ticks"])
                if key not in owned:
                    try:
                        fd = os.pidfd_open(pid)
                        fresh = proc_table().get(pid)
                        if fresh is None or fresh["start_ticks"] != key[1]:
                            os.close(fd)
                            continue
                        owned[key] = fd
                    except ProcessLookupError:
                        continue
                rows.append(row)
            return {"seconds_after_spawn": time.monotonic() - started, "processes": rows,
                    "rss_sum_bytes": sum(row["rss_bytes"] for row in rows)}
        try:
            while time.monotonic() - started < args.startup_seconds:
                samples.append(sample())
                if app.poll() is not None:
                    raise RuntimeError(f"application exited during startup: {app.returncode}")
                if obs["window_map"] is None:
                    ids = window_ids(app.pid, env)
                    if ids:
                        obs["window_map"] = {"window_ids": ids,
                            "detected_seconds_after_spawn": time.monotonic() - started,
                            "meaning": "X11 viewable window, polling upper bound; NOT first paint"}
                time.sleep(0.25)
            initial = sample()
            samples.append(initial)
            baseline = {(r["pid"], r["start_ticks"]): r["cpu_ticks"] for r in initial["processes"]}
            latest = dict(baseline)
            idle_start = time.monotonic()
            while time.monotonic() - idle_start < args.idle_seconds:
                if app.poll() is not None:
                    raise RuntimeError(f"application exited during idle: {app.returncode}")
                time.sleep(0.25)
                current = sample()
                samples.append(current)
                for row in current["processes"]:
                    latest[(row["pid"], row["start_ticks"])] = row["cpu_ticks"]
            elapsed = time.monotonic() - idle_start
            ticks = sum(max(0, ticks - baseline.get(key, 0)) for key, ticks in latest.items())
            obs.update(status="completed", idle_seconds_actual=elapsed,
                       idle_cpu_one_core_percent=ticks / os.sysconf("SC_CLK_TCK") / elapsed * 100,
                       idle_peak_sampled_rss_bytes=max(s["rss_sum_bytes"] for s in samples
                           if s["seconds_after_spawn"] >= initial["seconds_after_spawn"]))
        finally:
            # Sampling failure must never bypass cleanup of already-owned handles.
            try:
                samples.append(sample())
            except Exception as error:
                obs["cleanup_sampling_error"] = str(error)
            # The unreaped direct child is still ours even if the first sample failed.
            if app.poll() is None:
                app.terminate()
            for sig in (signal.SIGTERM, signal.SIGKILL):
                for fd in owned.values():
                    try:
                        signal.pidfd_send_signal(fd, sig)
                    except ProcessLookupError:
                        pass
                time.sleep(0.25)
            if app.poll() is None:
                app.kill()
            app.wait(timeout=3)
            table = proc_table()
            obs["cleanup_remaining_identities"] = [list(key) for key in owned
                if key[0] in table and table[key[0]]["start_ticks"] == key[1]
                and Path(f"/proc/{key[0]}/stat").exists()]
            for fd in owned.values():
                os.close(fd)
            if obs["cleanup_remaining_identities"]:
                obs["cleanup_limit"] = "Identities remain (possibly zombies); inspect explicitly, never infer cleanup success."


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("checkout", "binary", "output"):
        parser.add_argument("--" + name, required=True, type=absolute)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--package", type=absolute, help="Optional local .deb/.dmg; never installed or mounted")
    parser.add_argument("--observe-idle", action="store_true", help="Explicitly permit one bounded desktop launch")
    parser.add_argument("--prepared-fixture", type=fixture_path,
                        help="Explicit private synthetic fixture root; copied only with --observe-idle")
    parser.add_argument("--display", help="Private pre-existing X11 display; tool does not start Xvfb")
    parser.add_argument("--startup-seconds", type=float, default=5)
    parser.add_argument("--idle-seconds", type=float, default=10)
    args = parser.parse_args()
    try:
        if not re.fullmatch(r"[0-9a-f]{40}", args.expected_head):
            raise ValueError("--expected-head must be exactly 40 lowercase hex characters")
        if not re.fullmatch(r"[0-9a-f]{64}", args.binary_sha256):
            raise ValueError("--binary-sha256 must be exactly 64 lowercase hex characters")
        if not (1 <= args.startup_seconds <= 30 and 1 <= args.idle_seconds <= 60):
            raise ValueError("startup must be 1..30 seconds and idle 1..60 seconds")
        if args.output == args.checkout or args.checkout in args.output.parents:
            raise ValueError("output must be outside checkout")
        report = {"utc": datetime.now(timezone.utc).isoformat(), "argv": sys.argv,
                  "candidate": candidate(args), "hardware": hardware(),
                  "limits": ["No build/install/auth/Docker/model operations; metadata only unless opted in.",
                    "RSS sums double-count shared pages; sampled CPU misses between-sample exits.",
                    "WebKit descendants/session members included; detached new-session helpers may be missed.",
                    "No first-paint, UI interaction, canvas/log or workload acceptance evidence.",
                    "No VM/daemon/egress workload included; observation infrastructure excluded."]}
        if args.package:
            report["package"] = {"path": str(args.package), "sha256": digest(args.package),
                                 "archive_bytes": args.package.stat().st_size,
                                 "installed_footprint": "unmeasured; no install/mount performed"}
            if args.package.suffix == ".deb" and shutil.which("dpkg-deb"):
                report["package"]["declared_installed_size_kib"] = run(
                    ["dpkg-deb", "--field", str(args.package), "Installed-Size"])
                report["package"]["dependencies"] = run(["dpkg-deb", "--field", str(args.package), "Depends"])
        args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        parser.error(str(error))
    exit_code = 0
    try:
        if args.observe_idle:
            observe(args, report)
        else:
            report["observation"] = {"status": "not_requested", "desktop_launched": False}
        report["candidate_after"] = candidate(args)
    except (Exception, KeyboardInterrupt) as error:
        report["error"] = f"{type(error).__name__}: {error}"
        exit_code = 1
    finally:
        with (args.output / "observation.json").open("x") as receipt:
            json.dump(report, receipt, indent=2)
            receipt.write("\n")
    print(args.output / "observation.json")
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
