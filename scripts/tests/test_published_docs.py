"""Regression tests for the isolation gate, without network or filesystem mutations."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("published_docs", Path(__file__).parents[1] / "check_published_docs.py")
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class PublishedDocsTests(unittest.TestCase):
    def test_ci_runs_published_documentation_gate_for_all_required_events(self):
        repository = Path(__file__).parents[2]
        workflow_lines = (repository / ".github/workflows/ci.yml").read_text().splitlines()

        def indented_block(header: str, end_header: str | None = None) -> list[str]:
            start = workflow_lines.index(header)
            end = next(
                (index for index in range(start + 1, len(workflow_lines))
                 if (workflow_lines[index].startswith(end_header) if end_header else
                     workflow_lines[index].startswith("  ")
                     and len(workflow_lines[index]) > 2
                     and not workflow_lines[index][2].isspace())),
                len(workflow_lines),
            )
            return workflow_lines[start:end]

        triggers = indented_block("on:", "permissions:")
        self.assertIn("  pull_request:", triggers)
        self.assertIn("  push:", triggers)
        self.assertIn("      - main", triggers)
        self.assertIn("  workflow_dispatch:", triggers)
        self.assertFalse(any("verify_published_dependencies" in line for line in triggers))
        dispatch = triggers[triggers.index("  workflow_dispatch:") + 1:]
        self.assertFalse(any(line.startswith("    ") for line in dispatch))

        job = indented_block("  published-documentation:")
        self.assertFalse(any(line.startswith("    if:") for line in job))
        self.assertFalse(any(line.strip().startswith("continue-on-error:") for line in job))
        run_commands = [line.strip()[len("run:"):].strip() for line in job if line.strip().startswith("run:")]
        self.assertEqual(
            [command for command in run_commands if "check_published_docs.py" in command],
            ["python3 scripts/check_published_docs.py ."],
        )
        self.assertEqual(run_commands.count("python3 scripts/check_published_docs.py ."), 1)

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

    def test_packaging_uses_isolated_cwd_when_source_has_parent_cargo_config(self):
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory)
            source = temporary / "user" / "project"
            source.mkdir(parents=True)
            cargo_config = temporary / ".cargo"
            cargo_config.mkdir()
            (cargo_config / "config.toml").write_text("[build]\ntarget-dir = 'inherited'\n")
            workspace = temporary / "isolated"
            workspace.mkdir()
            environment = {"CARGO_HOME": str(workspace / "cargo-home")}

            with patch.object(checker, "run", return_value="") as run:
                checker.package_source("cargo", source, workspace, environment)

            command_cwd = workspace / "cargo-command"
            self.assertTrue(command_cwd.is_dir())
            run.assert_called_once_with(
                "cargo",
                [
                    "package",
                    "--manifest-path",
                    str(source / "Cargo.toml"),
                    "--allow-dirty",
                    "--no-verify",
                ],
                command_cwd,
                environment,
                workspace / "package.log",
            )

    def test_environment_drops_inherited_build_overrides(self):
        with patch.dict("os.environ", {"CARGO_HOME": "/old", "CARGO_ENCODED_RUSTFLAGS": "unsafe", "RUSTFLAGS": "unsafe", "CARGO_REGISTRIES_CRATES_IO_INDEX": "local", "RUSTC_WRAPPER": "wrapper"}):
            env = checker.isolated_environment(Path("/isolated"))
        self.assertEqual(env["CARGO_HOME"], "/isolated/cargo-home")
        self.assertEqual(env["QUBIT_FS_REGISTRY_DOC_DEPS"], "published")
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

    def test_graph_rejects_duplicate_versions_and_wrong_sources(self):
        root = Path("/isolated/package")
        own = {"name": "qubit-fs-registry", "source": None, "manifest_path": str(root / "Cargo.toml")}
        fs = {"name": "qubit-fs", "source": "registry+https://example.invalid/index", "manifest_path": "/registry/fs/Cargo.toml"}
        checker.validate_graph({"packages": [own, fs]}, "qubit-fs-registry", root)
        for bad in [dict(fs, source=None), dict(fs, source="git+https://example.invalid/repo")]:
            with self.assertRaises(ValueError):
                checker.validate_graph({"packages": [own, bad]}, "qubit-fs-registry", root)
        with self.assertRaises(ValueError):
            checker.validate_graph({"packages": [own, fs, dict(fs, version="0.2")]}, "qubit-fs-registry", root)


if __name__ == "__main__":
    unittest.main()
