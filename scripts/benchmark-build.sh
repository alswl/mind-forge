#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
exec python3 - "$@" <<'PY'
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time

parser = argparse.ArgumentParser(description="Measure builds in a detached checkout with isolated dependency and compiler caches.")
parser.add_argument("language", choices=["rust", "go"])
parser.add_argument("revision")
parser.add_argument("--samples", type=int, default=5)
parser.add_argument("--edit-file")
parser.add_argument("--edit-from")
parser.add_argument("--edit-to")
parser.add_argument("--release-only", action="store_true", help="collect cold release samples without repeating the development workloads")
parser.add_argument("--working-tree", action="store_true", help="overlay the current tracked/untracked, non-ignored files without committing; record the snapshot identity")
parser.add_argument("--component-tests", action="store_true", help="execute each implementation's colocated unit gate after builds and include its retained test cache in disk accounting")
parser.add_argument("--development-tests-only", action="store_true", help="measure development builds and component tests, sample their isolated temporary high-water, and skip release/install workloads")
parser.add_argument("--cold-install-only", action="store_true", help="measure fresh source installs and sampled installation disk high-water without repeating build workloads")
args = parser.parse_args()
if args.samples < 5:
    parser.error("at least five samples are required")
if any((args.edit_file, args.edit_from, args.edit_to)) and not all((args.edit_file, args.edit_from, args.edit_to)):
    parser.error("provide all three --edit-* arguments")
if args.development_tests_only and (args.release_only or args.cold_install_only):
    parser.error("--development-tests-only excludes release and source-install workloads")
if args.cold_install_only and (args.release_only or args.component_tests or args.development_tests_only):
    parser.error("--cold-install-only is separate from build/component-test workloads")
if args.development_tests_only:
    args.component_tests = True
if args.language == "go" and not args.edit_file and not args.cold_install_only:
    parser.error("Go measurements require an explicit equivalent business edit via --edit-*")

root = Path.cwd()
evidence_root = root / "tmp/085-go-rewrite"
evidence_root.mkdir(parents=True, exist_ok=True)
run = Path(tempfile.mkdtemp(prefix=args.language + "-cost-", dir=evidence_root))
source = run / "source"
env = dict(os.environ)
for name in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_TARGET_DIR", "GOFLAGS", "GOWORK", "GOROOT"):
    env.pop(name, None)
env.update(GOTOOLCHAIN="local", CGO_ENABLED="0", GOWORK="off")
result = {"schema_version": 1, "language": args.language, "requested_revision": args.revision,
          "samples_per_workload": args.samples, "run_directory": str(run), "status": "running",
          "machine": {"platform": platform.platform(), "architecture": platform.machine(),
                      "cpu_count": os.cpu_count(), "load_average_start": os.getloadavg()},
          "workloads": {}, "commands": [],
          "scope": "Build-only measurements. Equivalent full tests and final SC verdict require the external fixed-harness acceptance record.",
          "selected_workloads": "development-build-and-component-tests-only" if args.development_tests_only else ("cold-release-and-warm-install" if args.release_only else "development-release-and-warm-install"),
          "not_measured": ["cold source installation", "temporary disk peak", "external fixed-harness test runtime"],
          "cache_policy": "Dedicated dependency source cache, fresh compiler cache per cold sample; never cleans global caches.",
          "flags": {"cgo": False, "rust_dev": "Cargo profile.dev", "rust_release": "Cargo profile.release (not stripped)", "go": "default compiler/linker flags (not stripped)"}}
if not args.component_tests:
    result["not_measured"].append("colocated unit execution and retained test cache")
if platform.system() == "Darwin":
    result["machine"]["ram_bytes"] = int(subprocess.check_output(["sysctl", "-n", "hw.memsize"], text=True))
    result["machine"]["model"] = subprocess.check_output(["sysctl", "-n", "hw.model"], text=True).strip()
    result["machine"]["processor"] = subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip()
    volume = subprocess.run(["/usr/sbin/diskutil", "info", "-plist", str(root)], capture_output=True)
    if volume.returncode == 0:
        information = plistlib.loads(volume.stdout)
        result["machine"]["filesystem"] = {key: information.get(key) for key in ("FilesystemType", "FilesystemName", "AllocationBlockSize")}
    else:
        result["machine"]["filesystem"] = "unavailable from diskutil"


def save():
    (run / "results.json").write_text(json.dumps(result, indent=2) + "\n")


def capture(command):
    return subprocess.check_output(command, cwd=root, env=env, text=True).strip()


def measure(label, command, cwd=source):
    log = run / (str(len(result["commands"])) + "-" + label + ".log")
    started = time.monotonic()
    with log.open("wb") as stream:
        process = subprocess.run(command, cwd=cwd, env=env, stdout=stream, stderr=subprocess.STDOUT)
    elapsed = time.monotonic() - started
    result["commands"].append({"label": label, "argv": command, "cwd": str(cwd), "seconds": elapsed,
                               "exit": process.returncode, "log": log.name})
    save()
    if process.returncode:
        raise RuntimeError(f"{label} exited {process.returncode}; retained {log}")
    print(f"{label}: {elapsed:.3f}s", flush=True)
    return elapsed


def disk(path):
    apparent = allocated = 0
    visited = set()
    for directory, _, files in os.walk(path):
        for name in files:
            file = Path(directory) / name
            info = file.lstat()
            if (info.st_dev, info.st_ino) in visited:
                continue
            visited.add((info.st_dev, info.st_ino))
            apparent += info.st_size
            allocated += info.st_blocks * 512
    return {"apparent_bytes": apparent, "allocated_bytes": allocated}


def disk_roots(paths):
    apparent = allocated = 0
    visited = set()
    for path in paths:
        path = Path(path)
        files = [path] if path.is_file() else [Path(directory) / name
                                               for directory, _, names in os.walk(path)
                                               for name in names]
        for file in files:
            info = file.lstat()
            if (info.st_dev, info.st_ino) in visited:
                continue
            visited.add((info.st_dev, info.st_ino))
            apparent += info.st_size
            allocated += info.st_blocks * 512
    return {"apparent_bytes": apparent, "allocated_bytes": allocated}


def series(label, values):
    result["workloads"][label] = {"seconds": values, "median": statistics.median(values),
                                   "min": min(values), "max": max(values)}
    save()


def run_sampled_development_test_sequence():
    cache = run / "sequence-cache"
    temporary = run / "_sequence-tmp"
    cache.mkdir()
    temporary.mkdir()
    sequence_env = env.copy()
    sequence_env["TMPDIR"] = str(temporary)
    if args.language == "rust":
        sequence_env["CARGO_TARGET_DIR"] = str(cache)
        commands = [[cargo, "build", "--locked", "--offline"],
                    [cargo, "test", "--locked", "--offline", "--bin", "mf"]]
    else:
        sequence_env.update(GOCACHE=str(cache), GOTMPDIR=str(temporary))
        commands = [[go, "build", "-o", "bin/mf", "./cmd/mf"],
                    [go, "test", "-count=1", "./internal/..."]]
    observations = []
    stop = threading.Event()
    started = time.monotonic()

    def sample_temporary():
        apparent = allocated = races = 0
        visited = set()
        for parent, _, names in os.walk(temporary):
            for name in names:
                try:
                    info = (Path(parent) / name).stat()
                except FileNotFoundError:
                    races += 1
                    continue
                key = (info.st_dev, info.st_ino)
                if key in visited:
                    continue
                visited.add(key)
                apparent += info.st_size
                allocated += info.st_blocks * 512
        observations.append({"elapsed_seconds": time.monotonic() - started,
                             "apparent_bytes": apparent, "allocated_bytes": allocated,
                             "stat_races": races})

    def observe_temporary():
        while not stop.wait(0.1):
            sample_temporary()

    sample_temporary()
    observer = threading.Thread(target=observe_temporary, name="development-test-disk-sampler")
    observer.start()
    command_results = []
    try:
        for index, command in enumerate(commands):
            log = run / f"sequence-{index}.log"
            command_started = time.monotonic()
            with log.open("wb") as stream:
                process = subprocess.run(command, cwd=source, env=sequence_env,
                                         stdout=stream, stderr=subprocess.STDOUT)
            command_results.append({"argv": command, "seconds": time.monotonic() - command_started,
                                    "exit": process.returncode, "log": log.name})
            save()
            if process.returncode:
                raise RuntimeError(f"sampled development/test command exited {process.returncode}; retained {log}")
    finally:
        stop.set()
        observer.join()
        sample_temporary()
    if not observations:
        raise RuntimeError("temporary disk sampler recorded no observations")
    output_roots = [cache]
    development_binary = cache / "debug/mf" if args.language == "rust" else source / "bin/mf"
    if args.language == "go":
        output_roots.append(source / "bin")
    version = subprocess.run([str(development_binary), "--version"], capture_output=True,
                             text=True, env=sequence_env)
    result["development_test_sequence"] = {
        "commands": command_results,
        "temporary_root": str(temporary),
        "temporary_directory_was_empty_before": True,
        "sampling": "Isolated TMPDIR/GOTMPDIR, sampled every 100ms while a fresh-cache dev build and complete colocated unit gate run; non-atomic directory scans may miss brief peaks.",
        "disk_observations": observations,
        "sampled_temporary_high_water": {key: max(row[key] for row in observations)
                                          for key in ("apparent_bytes", "allocated_bytes")},
        "max_observation_gap_seconds": max((b["elapsed_seconds"] - a["elapsed_seconds"]
                                             for a, b in zip(observations, observations[1:])), default=0),
        "stat_races": sum(row["stat_races"] for row in observations),
        "compiler_and_worktree_outputs_disk": disk_roots(output_roots),
        "development_binary_disk": disk_roots([development_binary]),
        "development_binary": {"sha256": hashlib.sha256(development_binary.read_bytes()).hexdigest(),
                               "bytes": development_binary.stat().st_size,
                               "version_probe": {"exit": version.returncode, "stdout": version.stdout,
                                                 "stderr": version.stderr}},
    }
    result["not_measured"] = ["release builds", "source installation", "external fixed-harness test runtime"]


try:
    print(f"Evidence: {run}", flush=True)
    measure("clone", ["git", "clone", "--shared", "--no-checkout", str(root), str(source)], cwd=root)
    measure("checkout", ["git", "checkout", "--detach", args.revision])
    result["revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source, text=True).strip()
    result["source_dirty_before"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=source, text=True).strip()
    assert not result["source_dirty_before"]
    if args.working_tree:
        if result["revision"] != capture(["git", "rev-parse", "HEAD"]):
            raise RuntimeError("--working-tree requires the current HEAD revision")
        names = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=root).decode().split("\0")
        manifest = {}
        for name in sorted(set(names) - {""}):
            original_file, copied_file = root / name, source / name
            if not original_file.exists() and not original_file.is_symlink():
                if copied_file.is_file() or copied_file.is_symlink():
                    copied_file.unlink()
                continue
            copied_file.parent.mkdir(parents=True, exist_ok=True)
            if copied_file.is_symlink():
                copied_file.unlink()
            if original_file.is_symlink():
                if copied_file.exists():
                    copied_file.unlink()
                target = os.readlink(original_file)
                copied_file.symlink_to(target)
                manifest[name] = {"symlink": target}
            elif original_file.is_file():
                shutil.copy2(original_file, copied_file)
                manifest[name] = {"sha256": hashlib.sha256(original_file.read_bytes()).hexdigest(), "mode": original_file.stat().st_mode & 0o777}
        snapshot = json.dumps(manifest, sort_keys=True).encode()
        (run / "source-files.json").write_bytes(snapshot + b"\n")
        result["source_snapshot_sha256"] = hashlib.sha256(snapshot).hexdigest()
        result["source_dirty_before"] = subprocess.check_output(["git", "status", "--porcelain"], cwd=source, text=True).strip()
        result["scope"] = "Working-tree measurement with an explicit source snapshot; final SC-001/002/003 conclusions require the fixed-harness acceptance and remaining coverage gates."
        save()
    if args.language == "rust":
        env["CARGO_HOME"] = str(run / "rust-home")
        env["RUSTC_WRAPPER"] = ""
        env["RUSTC_WORKSPACE_WRAPPER"] = ""
        cargo = capture(["rustup", "which", "cargo"])
        env["RUSTC"] = capture(["rustup", "which", "rustc"])
        result["toolchain"] = capture([env["RUSTC"], "-vV"])
        toolchain_root = Path(capture([env["RUSTC"], "--print", "sysroot"]))
        measure("dependency-download", [cargo, "fetch", "--locked"])
        dependency_cache = run / "rust-home"
        build = [cargo, "build", "--locked", "--offline"]
        release = build + ["--release"]
        relative_edit = args.edit_file or "src/model/manifest.rs"
        before = args.edit_from or "defaults::PROJECTS_DIR.to_string()"
        after = args.edit_to or "String::from(defaults::PROJECTS_DIR)"
    else:
        go = shutil.which("go")
        result["toolchain"] = capture([go, "version"])
        if not result["toolchain"].startswith("go version go1.26.11 "):
            raise RuntimeError("expected pinned Go 1.26.11")
        toolchain_root = Path(capture([go, "env", "GOROOT"]))
        # Go's ./... walk also visits ignored repository directories. Keep
        # dependency source and intermediate files under names it skips.
        env.update(GOMODCACHE=str(run / "_go-mod"), GOPATH=str(run / "_go-path"),
                   GOCACHE=str(run / "download-go-cache"), GOTMPDIR=str(run / "_go-tmp"))
        Path(env["GOTMPDIR"]).mkdir()
        measure("dependency-download", [go, "mod", "download", "all"])
        dependency_cache = run / "_go-mod"
        build = [go, "build", "-o", "bin/mf", "./cmd/mf"]
        release = build
        relative_edit, before, after = args.edit_file, args.edit_from, args.edit_to
    result["toolchain_disk"] = disk(toolchain_root)
    result["dependency_disk"] = disk(dependency_cache)
    if args.language == "go":
        env.update(GOPROXY="off", GOSUMDB="off")
    if args.cold_install_only:
        result["selected_workloads"] = "cold-source-install-only"
        result["scope"] = "Cold source installation and sampled installation disk high-water only; not the complete development/test sequence or final SC acceptance."
        result["not_measured"] = ["whole-development/test-sequence temporary peak", "build/edit/unit workloads in this invocation", "external fixed-harness test runtime"]
        result["disk_sampling"] = {"requested_interval_seconds": 0.1, "interpretation": "Sampled estimates from non-atomic directory scans, not exact peaks or rigorous bounds. Short peaks may be missed and file races affect estimates. Scan duration adds to the interval; raw elapsed timestamps are retained. Installation timing includes sampler overhead."}
        timings = []
        result["cold_install_samples"] = []
        for index in range(args.samples):
            cache = run / f"install-cold-cache-{index}"
            installed = run / f"install-cold-{index}"
            temporary = run / f"_install-tmp-{index}"
            for directory in (cache, installed, temporary):
                directory.mkdir()
                if any(directory.iterdir()):
                    raise RuntimeError("cold installation requires empty dedicated directories")
            env["TMPDIR"] = str(temporary)
            if args.language == "rust":
                env["CARGO_TARGET_DIR"] = str(cache)
                command = [cargo, "install", "--path", str(source), "--root", str(installed), "--locked", "--offline", "--force"]
            else:
                env.update(GOCACHE=str(cache), GOTMPDIR=str(temporary), GOBIN=str(installed / "bin"))
                command = [go, "install", "./cmd/mf"]
            observations = []
            observer_errors = []
            stop = threading.Event()
            started = time.monotonic()

            def sample_disk():
                values = {"elapsed_seconds": time.monotonic() - started, "stat_races": 0}
                seen = set()
                for label, directory in (("compiler_cache", cache), ("temporary", temporary), ("installed", installed)):
                    apparent = allocated = 0
                    for parent, _, names in os.walk(directory):
                        for name in names:
                            try:
                                info = (Path(parent) / name).stat()
                            except FileNotFoundError:
                                values["stat_races"] += 1
                                continue
                            key = (info.st_dev, info.st_ino)
                            if key in seen:
                                continue
                            seen.add(key)
                            apparent += info.st_size
                            allocated += info.st_blocks * 512
                    values[label] = {"apparent_bytes": apparent, "allocated_bytes": allocated}
                observations.append(values)

            def observe_disk():
                try:
                    while not stop.wait(0.1):
                        sample_disk()
                except Exception as error:
                    observer_errors.append(str(error))

            sample_disk()
            observer = threading.Thread(target=observe_disk)
            observer.start()
            try:
                timings.append(measure(f"source-install-cold-{index}", command))
            finally:
                stop.set()
                observer.join()
                sample_disk()
                result["cold_install_samples"].append({"index": index, "cache_and_destination_empty_before": True,
                    "disk_observations": observations,
                    "sampled_temporary_high_water": {kind: max(row["temporary"][kind] for row in observations) for kind in ("apparent_bytes", "allocated_bytes")},
                    "sampled_combined_high_water": {kind: max(sum(row[label][kind] for label in ("compiler_cache", "temporary", "installed")) for row in observations) for kind in ("apparent_bytes", "allocated_bytes")},
                    "max_observation_gap_seconds": max((b["elapsed_seconds"] - a["elapsed_seconds"] for a, b in zip(observations, observations[1:])), default=0)})
                save()
            if observer_errors:
                raise RuntimeError(f"disk sampler failed: {observer_errors}")
            binary = installed / "bin/mf"
            probe = subprocess.run([str(binary), "--version"], env=env, capture_output=True, text=True)
            if probe.returncode:
                raise RuntimeError("installed binary metadata probe failed")
            result["cold_install_samples"][-1]["binary"] = {"sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "bytes": binary.stat().st_size,
                "version_probe": {"exit": probe.returncode, "stdout": probe.stdout, "stderr": probe.stderr}}
            if index + 1 < args.samples:
                for directory in (cache, installed, temporary):
                    shutil.rmtree(directory)
            save()
        series("source-install-cold", timings)
        result["retained_last_sample_disk"] = {"compiler_cache": disk(cache), "temporary": disk(temporary), "installed": disk(installed)}
        result["source_disk_including_git_metadata"] = disk(source)
        result["load_average_end"] = os.getloadavg()
        result["status"] = "measured-cold-source-install-and-sampled-temporary-disk-only"
        save()
        print(f"Results: {run / 'results.json'}", flush=True)
        sys.exit(0)
    edit = (source / relative_edit).resolve()
    if source.resolve() not in edit.parents:
        raise RuntimeError("edit must stay inside isolated source")
    original = edit.read_text()
    if original.count(before) != 1 or before == after:
        raise RuntimeError("business edit must replace exactly one non-identical expression")
    result["business_edit"] = {"file": relative_edit, "before": before, "after": after,
                               "sampling": "same equivalent expression edit with a unique comment per sample, so Go cannot reuse a previously compiled identical source"}
    samples = []
    for index in range(0 if args.release_only else args.samples):
        compile_cache = run / f"cold-{index}"
        if args.language == "rust":
            env["CARGO_TARGET_DIR"] = str(compile_cache)
        else:
            env["GOCACHE"] = str(compile_cache)
        samples.append(measure(f"cold-{index}", build))
        result.setdefault("cold_sample_disk", []).append(disk(compile_cache))
        if index + 1 < args.samples:
            shutil.rmtree(compile_cache)
    if samples:
        series("cold-dev", samples)
        series("no-op", [measure(f"no-op-{index}", build) for index in range(args.samples)])
    edits = []
    for index in range(0 if args.release_only else args.samples):
        try:
            edit.write_text(original.replace(before, after) + f"\n// isolated build sample {index}\n")
            edits.append(measure(f"business-edit-{index}", build))
        finally:
            edit.write_text(original)
        measure(f"restore-edit-{index}", build)
    if edits:
        series("business-edit", edits)
    if args.development_tests_only:
        tests = ([cargo, "test", "--locked", "--offline", "--bin", "mf"] if args.language == "rust"
                 else [go, "test", "-count=1", "./internal/..."])
        series("component-unit-first-then-warm", [measure(f"component-unit-{index}", tests) for index in range(args.samples)])
        result["component_tests"] = {
            "argv": tests, "actual_executions": args.samples,
            "sampling": "First unit target compilation then warm repeats; tests execute every time. No product CLI/integration harness is invoked.",
            "comparison_boundary": "Each implementation's complete colocated unit gate; declaration counts and structure differ. Equivalent behavior is evaluated separately by the fixed external harness."
        }
        run_sampled_development_test_sequence()
        result["source_disk_including_git_metadata"] = disk(source)
        result["load_average_end"] = os.getloadavg()
        result["status"] = "measured-development-unit-and-sampled-temporary-high-water"
        save()
        print(f"Results: {run / 'results.json'}", flush=True)
        sys.exit(0)
    if args.release_only:
        compile_cache = run / "release-cache"
        env["CARGO_TARGET_DIR" if args.language == "rust" else "GOCACHE"] = str(compile_cache)
    release_samples = []
    for index in range(args.samples):
        release_cache = compile_cache / "release" if args.language == "rust" else compile_cache
        if release_cache.exists():
            shutil.rmtree(release_cache)
        release_samples.append(measure(f"release-cold-{index}", release))
    series("release-cold", release_samples)
    series("release-warm", [measure(f"release-warm-{index}", release) for index in range(args.samples)])
    installed = run / "install"
    installed.mkdir()
    binary = compile_cache / "release/mf" if args.language == "rust" else source / "bin/mf"
    if args.language == "rust":
        install = [cargo, "install", "--path", str(source), "--root", str(installed), "--locked", "--offline", "--force"]
    else:
        env["GOBIN"] = str(installed)
        install = [go, "install", "./cmd/mf"]
    series("source-install-warm", [measure(f"source-install-{index}", install) for index in range(args.samples)])
    if args.component_tests:
        tests = ([cargo, "test", "--locked", "--offline", "--bin", "mf"] if args.language == "rust"
                 else [go, "test", "-count=1", "./internal/..."])
        series("component-unit-first-then-warm", [measure(f"component-unit-{index}", tests) for index in range(args.samples)])
        result["component_tests"] = {
            "argv": tests, "actual_executions": args.samples,
            "sampling": "First unit target compilation then warm repeats; tests execute every time. No product CLI/integration harness is invoked.",
            "comparison_boundary": "Each implementation's complete colocated unit gate; declaration counts and structure differ. Equivalent business behavior is evaluated separately by the fixed external harness, not inferred from unit timing."
        }
    result["binary"] = {"sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "bytes": binary.stat().st_size}
    version = subprocess.run([str(binary), "--version"], capture_output=True, text=True, env=env)
    result["binary"]["version_probe"] = {"exit": version.returncode, "stdout": version.stdout, "stderr": version.stderr}
    result["compiler_and_worktree_outputs_disk"] = disk(compile_cache)
    if args.language == "go":
        result["worktree_binary_disk"] = disk(source / "bin")
    result["installed_binary_disk"] = disk(installed)
    result["source_disk_including_git_metadata"] = disk(source)
    result["load_average_end"] = os.getloadavg()
    result["status"] = "measured-build-workloads-only"
except Exception as error:
    result["status"] = "failed"
    result["error"] = str(error)
    save()
    raise
save()
print(f"Results: {run / 'results.json'}", flush=True)
PY
