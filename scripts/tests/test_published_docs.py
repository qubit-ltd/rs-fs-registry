"""Regression tests for the isolation gate, without network or filesystem mutations."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("published_docs", Path(__file__).parents[1] / "check_published_docs.py")
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class PublishedDocsTests(unittest.TestCase):
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
