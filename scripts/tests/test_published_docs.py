"""Regression tests for the isolation gate, without network or filesystem mutations."""
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tarfile
import time
import unittest
from contextlib import redirect_stderr
from contextlib import redirect_stdout
from io import StringIO
from types import SimpleNamespace
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("published_docs", Path(__file__).parents[1] / "check_published_docs.py")
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)




def initialize_git(root):
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.invalid"], check=True)
    subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
    (root / ".gitignore").write_text("ignored.txt\ntarget/\n.cargo/\n")
    subprocess.run(["git", "-C", str(root), "add", ".gitignore"], check=True)
    subprocess.run(["git", "-C", str(root), "commit", "-qm", "fixture"], check=True)


class PublishedDocsTests(unittest.TestCase):
    def test_registry_environment_has_no_sibling_or_override_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            env = checker.isolated_environment(Path(directory), "1.95.2")
            self.assertEqual(env["CARGO_HOME"], str(Path(directory) / "cargo-home"))

    def test_registry_graph_allows_only_archive_and_optional_example_root(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "archive"
            root.mkdir()
            (root / "Cargo.toml").write_text("[package]\nname='qubit-fs-registry'\nversion='0.8.0'\n")
            example = Path(directory) / "example"
            example.mkdir()
            (example / "Cargo.toml").write_text("[package]\nname='filesystem-documentation-check'\nversion='0.0.0'\n")
            metadata = {"packages": [
                {"name": "qubit-fs-registry", "source": None, "manifest_path": str(root / "Cargo.toml")},
                {"name": "filesystem-documentation-check", "source": None, "manifest_path": str(example / "Cargo.toml")},
                {"name": "qubit-fs", "source": "registry+https://github.com/rust-lang/crates.io-index", "manifest_path": "/registry/qubit-fs/Cargo.toml"},
            ]}
            checker.validate_graph(metadata, "qubit-fs-registry", root, example_root=example)
            metadata["packages"][2]["source"] = "registry+https://example.invalid/index"
            with self.assertRaisesRegex(ValueError, "non-crates.io"):
                checker.validate_graph(metadata, "qubit-fs-registry", root, example_root=example)
            metadata["packages"][2]["source"] = "git+https://example.invalid/qubit-fs"
            with self.assertRaisesRegex(ValueError, "non-crates.io"):
                checker.validate_graph(metadata, "qubit-fs-registry", root, example_root=example)
            metadata["packages"][2]["source"] = "registry+https://github.com/rust-lang/crates.io-index"
            metadata["packages"].append({"name": "qubit-fs", "source": "registry+https://github.com/rust-lang/crates.io-index", "manifest_path": "/registry/qubit-fs-duplicate/Cargo.toml"})
            with self.assertRaisesRegex(ValueError, "duplicate filesystem"):
                checker.validate_graph(metadata, "qubit-fs-registry", root, example_root=example)
            metadata["packages"].pop()
            metadata["packages"].append({"name": "foreign", "source": None, "manifest_path": "/tmp/foreign/Cargo.toml"})
            with self.assertRaisesRegex(ValueError, "unexpected local"):
                checker.validate_graph(metadata, "qubit-fs-registry", root, example_root=example)

    def test_registry_resolution_summary_records_sorted_packages_and_archive(self):
        metadata = {"packages": [
            {"name": "zeta", "version": "2.0.0", "source": "registry+https://github.com/rust-lang/crates.io-index", "manifest_path": "/cargo/zeta/Cargo.toml"},
            {"name": "qubit-fs-registry", "version": "0.8.0", "source": None, "manifest_path": "/archive/Cargo.toml"},
        ]}
        summary = checker.resolution_summary(metadata, Path("/archive"), "abc123")
        self.assertIn(f"archive={Path('/archive').resolve()} input_sha256=abc123", summary)
        self.assertLess(summary.index("qubit-fs-registry@0.8.0"), summary.index("zeta@2.0.0"))
        self.assertIn("source=path:/archive/Cargo.toml", summary)

    def test_documentation_evidence_requires_complete_digests_and_local_dependencies(self):
        document_inputs = [
            {"path": path, "sha256": "0" * 64}
            for path in checker._CHECKED_DOCUMENTS
        ]
        package = lambda name: {
            "id": f"{name} 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)",
            "name": name,
            "version": "1.0.0",
            "source": "registry+https://github.com/rust-lang/crates.io-index",
            "features": [],
        }
        evidence = {
            "schema_version": 1,
            "source_mode": "candidate",
            "verified": True,
            "archive_sha256": "archive-digest",
            "input_sha256": "input-digest",
            "input_kind": "candidate_source_checkout",
            "source_checkout_sha256": "source-digest",
            "documentation_inputs": document_inputs,
            "documentation_inputs_sha256": hashlib.sha256(
                json.dumps(document_inputs, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest(),
            "expected_groups": ["0-1-0"],
            "groups": [{
                "group": "0-1-0",
                "features": {"async": False, "local": True, "inventory": False},
                "packages": [package("qubit-fs-registry"), package("qubit-fs-local"), package("qubit-local-files")],
                "programs": [{"target": "example", "run": True}, {"target": "example-no-run", "run": False}],
                "examples": [{"target": "example", "status": "passed"},
                             {"target": "example-no-run", "status": "no_run"}],
            }],
        }
        checker.validate_documentation_evidence(evidence, "candidate", "archive-digest", "input-digest")
        proper_subset = json.loads(json.dumps(evidence))
        proper_subset["expected_groups"] = ["0-1-0", "1-0-0"]
        with self.assertRaisesRegex(ValueError, "missing feature group"):
            checker.validate_documentation_evidence(proper_subset, "candidate", "archive-digest", "input-digest")
        incomplete = json.loads(json.dumps(evidence))
        incomplete["verified"] = False
        with self.assertRaisesRegex(ValueError, "successful"):
            checker.validate_documentation_evidence(incomplete, "candidate", "archive-digest", "input-digest")
        wrong_digest = json.loads(json.dumps(evidence))
        wrong_digest["input_sha256"] = "different"
        with self.assertRaisesRegex(ValueError, "digest mismatch"):
            checker.validate_documentation_evidence(wrong_digest, "candidate", "archive-digest", "input-digest")
        missing_local = json.loads(json.dumps(evidence))
        missing_local["groups"][0]["packages"].pop()
        with self.assertRaisesRegex(ValueError, "omits"):
            checker.validate_documentation_evidence(missing_local, "candidate", "archive-digest", "input-digest")
        cases = []
        empty = json.loads(json.dumps(evidence)); empty["groups"] = []
        cases.append((empty, "groups"))
        empty_examples = json.loads(json.dumps(evidence)); empty_examples["groups"][0]["examples"] = []
        cases.append((empty_examples, "examples"))
        missing_registry = json.loads(json.dumps(evidence)); missing_registry["groups"][0]["packages"].pop(0)
        cases.append((missing_registry, "registry identity"))
        missing_provider = json.loads(json.dumps(evidence)); missing_provider["groups"][0]["packages"].pop(1)
        cases.append((missing_provider, "omits"))
        duplicate_group = json.loads(json.dumps(evidence)); duplicate_group["groups"].append(json.loads(json.dumps(duplicate_group["groups"][0])))
        cases.append((duplicate_group, "duplicate"))
        duplicate_package = json.loads(json.dumps(evidence)); duplicate_package["groups"][0]["packages"].append(
            json.loads(json.dumps(duplicate_package["groups"][0]["packages"][0])))
        cases.append((duplicate_package, "duplicate package identity"))
        wrong_target = json.loads(json.dumps(evidence)); wrong_target["groups"][0]["examples"][0]["target"] = "other"
        cases.append((wrong_target, "target"))
        wrong_features = json.loads(json.dumps(evidence)); wrong_features["groups"][0]["features"]["async"] = True
        cases.append((wrong_features, "feature"))
        empty_packages = json.loads(json.dumps(evidence)); empty_packages["groups"][0]["packages"] = []
        cases.append((empty_packages, "empty package identity"))
        for invalid, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                checker.validate_documentation_evidence(invalid, "candidate", "archive-digest", "input-digest")

    def test_registry_check_never_probes_siblings_or_falls_back(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "source"
            root.mkdir()
            workspace = Path(directory) / "work"
            with patch.object(checker, "check", side_effect=RuntimeError("registry unavailable")) as strict:
                with self.assertRaisesRegex(RuntimeError, "registry unavailable"):
                    checker.check(root, workspace)
            strict.assert_called_once_with(root, workspace)

    def test_registry_package_copy_drops_only_local_path_hints(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            original = '[package]\nname="fixture"\nversion="1.0.0"\n[dependencies]\nqubit-fs = { path="../rs-fs", version="^0.3", default-features=false }\n'
            manifest.write_text(original)
            checker._normalize_registry_package_manifest(manifest)
            parsed = __import__("tomllib").loads(manifest.read_text())
            self.assertEqual(parsed["dependencies"]["qubit-fs"]["version"], "^0.3")
            self.assertNotIn("path", parsed["dependencies"]["qubit-fs"])
            self.assertEqual(original.splitlines()[0:3], manifest.read_text().splitlines()[0:3])

    def test_registry_package_copy_structurally_drops_paths_in_all_dependency_scopes(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            source_manifest = Path(directory) / "source-Cargo.toml"
            original = '''[package]
name = "fixture"
version = "1.0.0"

[dependencies]
first = { path = "../first", version = "^1", default-features = false }
middle = { version = "^2", path = "../middle", features = ["a"] }
last = { version = "^3", features = ["b"], path = "../last" }

[dependencies.subtable]
version = "^3"
path = "../subtable"

[dev-dependencies]
dev = { version = "^4", path = "../dev" }

[build-dependencies]
build = { path = "../build", version = "^5" }

[target.'cfg(unix)'.dependencies]
target = { version = "^6", path = "../target" }

[package.metadata.documentation]
path = "inert metadata, not a Cargo source"
'''
            manifest.write_text(original)
            source_manifest.write_text(original)
            checker._normalize_registry_package_manifest(manifest)
            parsed = __import__("tomllib").loads(manifest.read_text())
            for dependencies in (
                parsed["dependencies"], parsed["dev-dependencies"], parsed["build-dependencies"],
                parsed["target"]["cfg(unix)"]["dependencies"],
            ):
                for dependency in dependencies.values():
                    self.assertNotIn("path", dependency)
                    self.assertIn("version", dependency)
            self.assertFalse(manifest.with_name("Cargo.toml.registry").exists())
            self.assertEqual(
                parsed["package"]["metadata"]["documentation"]["path"],
                "inert metadata, not a Cargo source",
            )
            self.assertEqual(source_manifest.read_text(), original)




    def test_windows_run_assigns_suspended_process_to_job_before_polling(self):
        events = []

        class FakeProcess:
            returncode = 0

            def __init__(self, *args, creationflags, **kwargs):
                events.append(("spawn", creationflags))
                self.pid = 123
                self._handle = 456

            def poll(self):
                events.append(("poll", None))
                return self.returncode

        fake_subprocess = SimpleNamespace(Popen=FakeProcess,
                                          TimeoutExpired=checker.subprocess.TimeoutExpired)
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(checker, "os", SimpleNamespace(name="nt")), \
                    patch.object(checker, "subprocess", fake_subprocess), \
                    patch.object(checker, "_windows_job_functions", return_value=object()), \
                    patch.object(checker, "_configure_windows_job",
                                 side_effect=lambda _kernel, _process: events.append(("job", None)) or object()), \
                    patch.object(checker, "close_windows_job"):
                checker.run("fake-cargo", [], Path(directory), {}, Path(directory) / "run.log",
                            timeout_seconds=1)
        self.assertEqual(events, [("spawn", 0x00000004), ("job", None), ("poll", None)])


    @unittest.skipUnless(sys.platform != "win32", "Unix process groups are unavailable")
    def test_unix_timeout_cleanup_bounds_reaping_and_preserves_diagnostics(self):
        class FakeClock:
            now = 0.0

            def monotonic(self):
                return self.now

            def sleep(self, seconds):
                self.now += seconds

        clock = FakeClock()

        class UnreapableProcess:
            last_wait_timeouts = []

            def __init__(self, *args, stdout, **kwargs):
                self.wait_timeouts = []
                type(self).last_wait_timeouts = self.wait_timeouts
                self.pid = 1234
                self.poll_count = 0
                stdout.write("unix cleanup output marker")
                stdout.flush()

            def poll(self):
                self.poll_count += 1
                return None

            def wait(self, timeout=None):
                self.wait_timeouts.append(timeout)
                clock.now += timeout
                raise checker.subprocess.TimeoutExpired("fake process", timeout)

            def kill(self):
                raise OSError("direct kill marker")

        fake_subprocess = SimpleNamespace(Popen=UnreapableProcess,
                                          TimeoutExpired=checker.subprocess.TimeoutExpired)
        signals = []

        def fail_signal(group, received_signal):
            signals.append(received_signal)
            raise PermissionError(f"signal marker {received_signal}")

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            log = root / "unix-cleanup.log"
            with patch.object(checker, "subprocess", fake_subprocess), \
                    patch.object(checker.os, "killpg", side_effect=fail_signal), \
                    patch.object(checker.time, "sleep", side_effect=clock.sleep), \
                    patch.object(checker.time, "monotonic", side_effect=clock.monotonic):
                with self.assertRaisesRegex(RuntimeError, "signal marker|cleanup") as raised:
                    checker.run("fake-cargo", [], root, {}, log, timeout_seconds=0,
                                context="fake Unix timeout")

            self.assertEqual(signals[:2], [checker.signal.SIGTERM, checker.signal.SIGKILL], str(raised.exception))
            self.assertTrue(all(timeout is not None for timeout in UnreapableProcess.last_wait_timeouts))
            self.assertLessEqual(sum(UnreapableProcess.last_wait_timeouts), 5.9 + 1e-9)
            self.assertIn("unix cleanup output marker", log.read_text())
            self.assertRegex(str(raised.exception), "reap|signal|cleanup")

    def test_suspended_process_rollback_uses_bounded_waits(self):
        class UnreapableProcess:
            def __init__(self):
                self.wait_timeouts = []

            def poll(self):
                return None

            def kill(self):
                raise OSError("kill marker")

            def wait(self, timeout):
                self.wait_timeouts.append(timeout)
                raise checker.subprocess.TimeoutExpired("suspended child", timeout)

        process = UnreapableProcess()
        result = checker._kill_and_reap_process(process, wait_seconds=5)
        self.assertIn("wait exceeded cleanup deadline", result)
        self.assertIn("final bounded wait failed", result)
        self.assertEqual(len(process.wait_timeouts), 2)
        self.assertLessEqual(sum(process.wait_timeouts), 6)

    def test_windows_cleanup_runtime_errors_do_not_skip_timeout_log(self):
        class FakeProcess:
            def __init__(self, *args, stdout, **kwargs):
                self.returncode = None
                stdout.write("timeout output marker")
                stdout.flush()

            def poll(self):
                return self.returncode

            def kill(self):
                self.returncode = -9

            def wait(self, timeout=None):
                if self.returncode is None:
                    self.returncode = -9
                return self.returncode

        fake_subprocess = SimpleNamespace(Popen=FakeProcess, CREATE_SUSPENDED=4,
                                          TimeoutExpired=checker.subprocess.TimeoutExpired)
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "timeout.log"
            with patch.object(checker, "os", SimpleNamespace(name="nt")), \
                    patch.object(checker, "subprocess", fake_subprocess), \
                    patch.object(checker, "_windows_job_functions", return_value=object()), \
                    patch.object(checker, "_configure_windows_job", return_value=object()), \
                    patch.object(checker, "terminate_process_tree", side_effect=RuntimeError("initial close marker")), \
                    patch.object(checker, "close_windows_job", side_effect=RuntimeError("retry close marker")):
                with self.assertRaisesRegex(RuntimeError, "timed out.*retry close marker"):
                    checker.run("fake-cargo", [], Path(directory), {}, log, timeout_seconds=0,
                                context="fake Windows timeout")
            self.assertIn("timeout output marker", log.read_text())

    def test_windows_cleanup_runtime_errors_do_not_skip_normal_exit_log(self):
        class FakeProcess:
            def __init__(self, *args, stdout, **kwargs):
                self.returncode = 0
                stdout.write("normal output marker")
                stdout.flush()

            def poll(self):
                return self.returncode

        fake_subprocess = SimpleNamespace(Popen=FakeProcess, CREATE_SUSPENDED=4,
                                          TimeoutExpired=checker.subprocess.TimeoutExpired)
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "normal.log"
            with patch.object(checker, "os", SimpleNamespace(name="nt")), \
                    patch.object(checker, "subprocess", fake_subprocess), \
                    patch.object(checker, "_windows_job_functions", return_value=object()), \
                    patch.object(checker, "_configure_windows_job", return_value=object()), \
                    patch.object(checker, "close_windows_job", side_effect=RuntimeError("close marker")):
                with self.assertRaisesRegex(RuntimeError, "cleanup failed.*close marker"):
                    checker.run("fake-cargo", [], Path(directory), {}, log, timeout_seconds=2,
                                context="fake Windows normal exit")
            self.assertIn("normal output marker", log.read_text())

    def test_windows_job_close_failure_is_reported(self):
        close_results = iter([False, False, True])
        kernel32 = SimpleNamespace(CloseHandle=lambda _: next(close_results),
                                   TerminateJobObject=lambda *_: False)
        job = checker.WindowsJob(kernel32, 123)
        with patch.object(checker.ctypes, "WinError", return_value=OSError("close marker"), create=True), \
                patch.object(checker.ctypes, "get_last_error", return_value=5, create=True):
            with self.assertRaisesRegex(RuntimeError, "TerminateJobObject failed"):
                job.cleanup()
        self.assertEqual(job._handle, 123, "failed cleanup retains ownership for a later retry")
        with patch.object(checker.ctypes, "WinError", return_value=OSError("close marker"), create=True), \
                patch.object(checker.ctypes, "get_last_error", return_value=5, create=True):
            job.cleanup()
        self.assertIsNone(job._handle)

    def test_windows_job_termination_fallback_cleans_tree_after_close_failure(self):
        close_results = iter([False, True])
        kernel32 = SimpleNamespace(CloseHandle=lambda _: next(close_results),
                                   TerminateJobObject=lambda *_: True)
        job = checker.WindowsJob(kernel32, 123)
        with patch.object(checker.ctypes, "WinError", return_value=OSError("close marker"), create=True), \
                patch.object(checker.ctypes, "get_last_error", return_value=5, create=True):
            job.cleanup()
        self.assertIsNone(job._handle)

    def test_windows_thread_and_snapshot_close_failures_are_reported(self):
        kernel32 = SimpleNamespace(CloseHandle=lambda _: False)
        with patch.object(checker.ctypes, "WinError", return_value=OSError("close marker"), create=True), \
                patch.object(checker.ctypes, "get_last_error", return_value=5, create=True):
            with self.assertRaisesRegex(OSError, "primary thread"):
                checker._close_windows_handle(kernel32, 123, "primary thread")
            with self.assertRaisesRegex(OSError, "thread snapshot"):
                checker._close_windows_handle(kernel32, 123, "thread snapshot")

    def test_run_returns_success_output_and_logs_nonzero_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            success = checker.run(sys.executable, ["-c", "print('success marker')"], root, {},
                                  root / "success.log", timeout_seconds=2)
            self.assertIn("success marker", success)
            with self.assertRaisesRegex(RuntimeError, "failed with exit code 7"):
                checker.run(sys.executable, ["-c", "import sys; print('failure marker'); sys.exit(7)"],
                            root, {}, root / "failure.log", timeout_seconds=2, context="nonzero context")
            self.assertIn("failure marker", (root / "failure.log").read_text())

    def test_run_times_out_and_preserves_partial_output(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "timeout.log"
            started = time.monotonic()
            with self.assertRaisesRegex(RuntimeError, "unique timeout context"):
                checker.run(
                    sys.executable,
                    ["-c", "import sys,time; print('partial marker', flush=True); time.sleep(30)"],
                    Path(directory), {}, log, timeout_seconds=0.3, context="unique timeout context",
                )
            self.assertLess(time.monotonic() - started, 5)
            self.assertIn("partial marker", log.read_text())

    def test_successful_poll_after_overall_deadline_is_still_a_timeout(self):
        class FakeClock:
            now = 0.0

            def monotonic(self):
                return self.now

            def sleep(self, seconds):
                self.now += seconds

        clock = FakeClock()
        signals = []

        class LateSuccessProcess:
            pid = 1234
            returncode = 0

            def __init__(self, *args, stdout, **kwargs):
                stdout.write("late successful output marker")
                stdout.flush()

            def poll(self):
                clock.now = 0.6
                return self.returncode

            def wait(self, timeout=None):
                return self.returncode

            def kill(self):
                self.returncode = -9

        def missing_group(_group, received_signal):
            signals.append(received_signal)
            raise ProcessLookupError()

        fake_subprocess = SimpleNamespace(Popen=LateSuccessProcess,
                                          TimeoutExpired=checker.subprocess.TimeoutExpired)
        fake_os = SimpleNamespace(name="posix", killpg=missing_group)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            log = root / "late-success.log"
            with patch.object(checker, "os", fake_os), \
                    patch.object(checker, "subprocess", fake_subprocess), \
                    patch.object(checker, "signal", SimpleNamespace(SIGTERM=15, SIGKILL=9)), \
                    patch.object(checker.time, "monotonic", side_effect=clock.monotonic), \
                    patch.object(checker.time, "sleep", side_effect=clock.sleep):
                with self.assertRaisesRegex(RuntimeError, "overall publication deadline.*late successful group=boundary"):
                    checker.run("fake-cargo", [], root, {}, log, timeout_seconds=30,
                                overall_deadline=0.5,
                                context="late successful group=boundary")
            self.assertEqual(signals, [15, 9])
            self.assertIn("late successful output marker", log.read_text())

    def test_outer_timeout_cancels_nested_supervisor_and_reports_command_group_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "must-not-complete"
            log = root / "nested.log"
            leaf = "import pathlib,sys,time; time.sleep(1); pathlib.Path(sys.argv[1]).write_text('done')"
            nested = (
                "import subprocess,sys,time; print('nested supervisor started',flush=True); "
                f"subprocess.Popen([sys.executable,'-c',{leaf!r},sys.argv[1]]); time.sleep(30)"
            )
            started = time.monotonic()
            with self.assertRaisesRegex(RuntimeError, "overall publication deadline.*nested supervisor"):
                checker.run(sys.executable, ["-c", nested, str(marker)], root, {}, log,
                            timeout_seconds=30, overall_deadline=started + 0.3,
                            context="nested supervisor group=archive-examples")
            self.assertLess(time.monotonic() - started, 5)
            self.assertIn("nested supervisor started", log.read_text())
            self.assertFalse(marker.exists())

    def test_python_supervises_each_cargo_step_and_example_with_one_deadline(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            test_root = root / "package"
            project = root / "examples" / "0-1-0"
            project.mkdir(parents=True)
            (test_root / "Cargo.toml").parent.mkdir(parents=True)
            group = {
                "group": "0-1-0",
                "project": str(project),
                "features": {"async": False, "local": True, "inventory": False},
                "programs": [{"target": "example_bin", "run": True, "context": "doc example group=0-1-0"}],
            }
            executable = root / "example_bin"
            executable.write_text("#!/bin/sh\nexit 0\n")
            deadline = time.monotonic() + 60
            with patch.object(checker, "_run_rust_docs_helper", side_effect=[{"groups": [group]}, [] ,
                                                                             {"example_bin": str(executable)}]) as helper, \
                    patch.object(checker, "run", return_value='{"packages":[],"resolve":{"nodes":[]}}') as run_command:
                evidence = checker._check_documentation_examples(
                    "cargo", test_root, {}, root, overall_deadline=deadline,
                )
            self.assertEqual(evidence["expected_groups"], ["0-1-0"])
            self.assertEqual(evidence["groups"][0]["group"], "0-1-0")
            self.assertEqual(evidence["groups"][0]["examples"], [{"target": "example_bin", "status": "passed"}])
            self.assertEqual([call.args[4] for call in helper.call_args_list], ["prepare", "validate", "artifacts"])
            self.assertGreaterEqual(run_command.call_count, 4)
            self.assertTrue(all(call.kwargs["overall_deadline"] == deadline for call in run_command.call_args_list))

    def test_no_run_example_requires_cargo_executable_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            test_root = root / "package"
            project = root / "examples" / "0-0-0"
            project.mkdir(parents=True)
            group = {"group": "0-0-0", "project": str(project),
                     "features": {"async": False, "local": False, "inventory": False},
                     "programs": [{"target": "no_run", "run": False, "context": "example"}]}
            with patch.object(checker, "_run_rust_docs_helper", side_effect=[{"groups": [group]}, [], {}]), \
                    patch.object(checker, "run", return_value="{}"):
                with self.assertRaisesRegex(ValueError, "executable artifact"):
                    checker._check_documentation_examples(
                        "cargo", test_root, {}, root, overall_deadline=time.monotonic() + 60,
                    )

    def test_run_timeout_stops_grandchild(self):
        with tempfile.TemporaryDirectory() as directory:
            heartbeat = Path(directory) / "heartbeat"
            log = Path(directory) / "grandchild.log"
            child_code = "import pathlib,sys,time; p=pathlib.Path(sys.argv[1]); n=0; exec('while True: n+=1; p.write_text(str(n)); time.sleep(0.02)')"
            code = (
                "import pathlib,subprocess,sys,time; p=pathlib.Path(sys.argv[1]); p.write_text('parent'); "
                f"subprocess.Popen([sys.executable,'-c',{child_code!r},sys.argv[1]]); "
                "deadline=time.monotonic()+2; "
                "exec('while p.read_text()==\"parent\" and time.monotonic()<deadline: time.sleep(0.01)'); "
                "assert p.read_text()!=\"parent\"; print('parent marker',flush=True); time.sleep(30)"
            )
            with self.assertRaises(RuntimeError):
                checker.run(sys.executable, ["-c", code, str(heartbeat)], Path(directory), {}, log,
                            timeout_seconds=3, context="grandchild timeout")
            self.assertTrue(heartbeat.exists())
            first = heartbeat.read_text()
            self.assertNotEqual(first, "parent", "grandchild must run before timeout cleanup")
            time.sleep(0.1)
            self.assertEqual(first, heartbeat.read_text())

    def test_run_cleans_grandchild_after_parent_exits(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            heartbeat = root / "heartbeat"
            log = root / "parent-exits.log"
            child_code = (
                "import pathlib,sys,time; p=pathlib.Path(sys.argv[1]); n=0; "
                "exec('while True: n+=1; p.write_text(str(n)); time.sleep(0.02)')"
            )
            parent_code = (
                "import pathlib,subprocess,sys,time; "
                f"subprocess.Popen([sys.executable,'-c',{child_code!r},sys.argv[1]]); "
                "p=pathlib.Path(sys.argv[1]); deadline=time.monotonic()+2; "
                "exec('while not p.exists() and time.monotonic()<deadline: time.sleep(0.01)'); "
                "assert p.exists(); print('parent exited',flush=True)"
            )
            output = checker.run(sys.executable, ["-c", parent_code, str(heartbeat)], root, {}, log,
                                 timeout_seconds=3, context="parent exits before grandchild")
            self.assertIn("parent exited", output)
            self.assertTrue(heartbeat.exists())
            first = heartbeat.read_text()
            time.sleep(0.1)
            self.assertEqual(first, heartbeat.read_text())

    def test_timeout_cleanup_failure_is_reported_and_output_logged(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            log = root / "cleanup.log"
            with patch.object(checker, "terminate_process_tree", side_effect=RuntimeError("cleanup marker")):
                with self.assertRaisesRegex(RuntimeError, "cleanup marker"):
                    checker.run(sys.executable,
                                ["-c", "import sys,time; print('before cleanup',flush=True); time.sleep(30)"],
                                root, {}, log, timeout_seconds=0.2, context="cleanup context")
            self.assertIn("before cleanup", log.read_text())

    def test_main_preserves_workspace_when_check_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory) / "diagnostics"
            workspace.mkdir()
            with patch.object(checker.tempfile, "mkdtemp", return_value=str(workspace)), \
                    patch.object(checker, "check", side_effect=RuntimeError("expected failure")):
                stderr = StringIO()
                with patch("sys.argv", ["check_published_docs.py", directory]), redirect_stderr(stderr):
                    self.assertEqual(checker.main(), 1)
            self.assertTrue(workspace.is_dir())
            message = stderr.getvalue().split("Diagnostics preserved at ", 1)[1].strip()
            self.assertTrue(Path(message).samefile(workspace))

    def test_main_published_mode_dispatches_to_published_consumer(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory) / "diagnostics"
            workspace.mkdir()
            with patch.object(checker.tempfile, "mkdtemp", return_value=str(workspace)), \
                    patch.object(checker, "check_published") as published, \
                    patch.object(checker, "check") as candidate, \
                    patch("sys.argv", ["check_published_docs.py", directory, "--mode", "published"]), \
                    redirect_stdout(StringIO()):
                self.assertEqual(checker.main(), 0)
            published.assert_called_once_with(Path(directory).resolve(), workspace.resolve())
            candidate.assert_not_called()

    def test_main_rejects_unknown_mode(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch("sys.argv", ["check_published_docs.py", directory, "--mode", "unknown"]), \
                    redirect_stderr(StringIO()):
                with self.assertRaises(SystemExit) as result:
                    checker.main()
            self.assertEqual(result.exception.code, 2)

    def test_workflows_keep_archive_checks_out_of_ordinary_source_ci(self):
        repository = Path(__file__).parents[2]
        source_workflow = (repository / ".github/workflows/ci.yml").read_text()
        published_workflow = (repository / ".github/workflows/published-consumption.yml").read_text()

        self.assertIn("uses: qubit-ltd/rs-infra-ci/.github/workflows/github-ci.yml@main", source_workflow)
        self.assertIn("  published-checker-unit-tests:", source_workflow)
        self.assertIn("  windows-process-cleanup:", source_workflow)
        self.assertIn("Run published checker tests on Windows", source_workflow)
        self.assertIn("Run Rust process descendant cleanup tests on Windows", source_workflow)
        self.assertIn("  prepare-feature-matrix:", source_workflow)
        self.assertNotIn("published-documentation:", source_workflow)
        self.assertNotIn("check_published_docs.py .", source_workflow)
        self.assertNotIn("check_published_docs.py . --mode published", source_workflow)

        self.assertIn("  workflow_dispatch:", published_workflow)
        self.assertNotIn("  push:", published_workflow)
        self.assertNotIn("  pull_request:", published_workflow)
        self.assertIn("runs-on: ${{ matrix.os }}", published_workflow)
        self.assertIn("os: [ubuntu-latest, windows-latest]", published_workflow)
        self.assertIn("mode: [candidate, published]", published_workflow)
        self.assertIn("--mode ${{ matrix.mode }}", published_workflow)
        self.assertIn("actions/upload-artifact@v4", published_workflow)
        self.assertIn("${{ matrix.mode }}-${{ matrix.os }}-registry-diagnostics", published_workflow)
        self.assertIn("${{ runner.temp }}/qubit-registry-published-*", published_workflow)

        checker_path = repository / "scripts/check_published_docs.py"
        self.assertTrue(checker_path.is_file())
        result = subprocess.run(
            [sys.executable, str(checker_path), "--help"],
            cwd=repository,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("root", result.stdout)

    def test_unused_dispatch_input_is_absent(self):
        repository = Path(__file__).parents[2]
        workflow = (repository / ".github/workflows/ci.yml").read_text()
        self.assertNotIn("verify_published_dependencies", workflow)



    @unittest.skipIf(os.name == "nt", "requires Unix executable bits")
    def test_unix_snapshot_uses_worktree_executable_bit_and_digest_tracks_both_directions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize_git(root)
            script = root / "tool.sh"
            script.write_text("#!/bin/sh\nexit 0\n")
            script.chmod(0o755)
            subprocess.run(["git", "-C", str(root), "add", "tool.sh"], check=True)
            subprocess.run(["git", "-C", str(root), "update-index", "--chmod=+x", "tool.sh"], check=True)
            script.chmod(0o644)

            inputs = checker._collect_git_inputs(root)
            self.assertFalse(inputs["tool.sh"][1])
            destination = root.parent / (root.name + "-snapshot")
            checker._copy_git_inputs(root, destination, checker._input_digest(inputs))
            self.assertFalse((destination / "tool.sh").stat().st_mode & 0o111)

            script.chmod(0o755)
            executable_digest = checker._input_digest(checker._collect_git_inputs(root))
            self.assertNotEqual(executable_digest, checker._input_digest(inputs))
            script.chmod(0o644)
            self.assertEqual(checker._input_digest(checker._collect_git_inputs(root)), checker._input_digest(inputs))

    @unittest.skipIf(os.name == "nt", "requires Unix executable bits")
    def test_unix_snapshot_preserves_unstaged_executable_addition(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize_git(root)
            script = root / "tool.sh"
            script.write_text("#!/bin/sh\nexit 0\n")
            subprocess.run(["git", "-C", str(root), "add", "tool.sh"], check=True)
            script.chmod(0o755)
            inputs = checker._collect_git_inputs(root)
            self.assertTrue(inputs["tool.sh"][1])
            destination = root.parent / (root.name + "-snapshot")
            checker._copy_git_inputs(root, destination, checker._input_digest(inputs))
            self.assertTrue((destination / "tool.sh").stat().st_mode & 0o111)

    @unittest.skipIf(os.name == "nt", "requires Unix executable bits")
    def test_unix_snapshot_rejects_executable_changes_during_copy_in_both_directions(self):
        for starting_mode, changed_mode in ((0o755, 0o644), (0o644, 0o755)):
            with self.subTest(starting_mode=oct(starting_mode), changed_mode=oct(changed_mode)):
                with tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    initialize_git(root)
                    script = root / "tool.sh"
                    script.write_text("#!/bin/sh\nexit 0\n")
                    script.chmod(0o755)
                    subprocess.run(["git", "-C", str(root), "add", "tool.sh"], check=True)
                    subprocess.run(["git", "-C", str(root), "update-index", "--chmod=+x", "tool.sh"], check=True)
                    script.chmod(starting_mode)
                    expected = checker._input_digest(checker._collect_git_inputs(root))
                    real_copy = checker.shutil.copy2

                    def copy_then_change(source, target):
                        result = real_copy(source, target)
                        if Path(source) == script:
                            script.chmod(changed_mode)
                        return result

                    with patch.object(checker.shutil, "copy2", side_effect=copy_then_change):
                        with self.assertRaisesRegex(ValueError, "changed while snapshotting"):
                            checker._copy_git_inputs(root, root.parent / (root.name + "-snapshot"), expected)

    def test_windows_snapshot_keeps_index_executable_compensation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            initialize_git(root)
            script = root / "tool.sh"
            script.write_text("#!/bin/sh\nexit 0\n")
            script.chmod(0o644)
            subprocess.run(["git", "-C", str(root), "add", "tool.sh"], check=True)
            subprocess.run(["git", "-C", str(root), "update-index", "--chmod=+x", "tool.sh"], check=True)
            real_stat = Path.stat

            def stat_without_executable_bits(path, *args, **kwargs):
                result = real_stat(path, *args, **kwargs)
                if path == script:
                    return SimpleNamespace(st_mode=result.st_mode & ~0o111)
                return result

            with patch.object(checker, "os", SimpleNamespace(name="nt")):
                with patch.object(Path, "stat", stat_without_executable_bits):
                    self.assertTrue(checker._collect_git_inputs(root)["tool.sh"][1])






    def test_environment_drops_inherited_build_overrides(self):
        with patch.dict("os.environ", {"CARGO_HOME": "/old", "CARGO_ENCODED_RUSTFLAGS": "unsafe", "RUSTFLAGS": "unsafe", "CARGO_REGISTRIES_CRATES_IO_INDEX": "local", "RUSTC_WRAPPER": "wrapper"}):
            env = checker.isolated_environment(Path("/isolated"), "1.94.0")
        self.assertEqual(Path(env["CARGO_HOME"]), Path("/isolated") / "cargo-home")
        for key in ["CARGO_ENCODED_RUSTFLAGS", "CARGO_REGISTRIES_CRATES_IO_INDEX", "RUSTFLAGS", "RUSTC_WRAPPER"]:
            self.assertNotIn(key, env)

    def test_manifest_rejects_all_active_source_overrides(self):
        for source in ["path", "git", "workspace"]:
            for section in ["dependencies", "dev-dependencies", "build-dependencies"]:
                with self.assertRaises(ValueError):
                    checker.validate_manifest({section: {"qubit-fs": {source: "bad", "version": "0.3"}}})
        with self.assertRaises(ValueError):
            checker.validate_manifest({"target": {"cfg(unix)": {"dependencies": {"x": {"path": "../x"}}}}})
        for key in ["patch", "replace", "workspace"]:
            with self.assertRaises(ValueError):
                checker.validate_manifest({key: {}})
        checker.validate_manifest({"package": {"metadata": {"documentation": {"path": "../inert"}}}, "dependencies": {"qubit-fs": {"version": "0.3"}}})



if __name__ == "__main__":
    unittest.main()
