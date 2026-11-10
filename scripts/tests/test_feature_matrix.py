import json
import tomllib
import unittest
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[2]


class FeatureMatrixTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with (PROJECT_ROOT / "Cargo.toml").open("rb") as manifest_file:
            cls.manifest = tomllib.load(manifest_file)
        with (PROJECT_ROOT / ".infra/ci/cargo-matrix.json").open(encoding="utf-8") as matrix_file:
            cls.matrix = json.load(matrix_file)

    def test_default_features_are_empty(self):
        self.assertEqual(
            self.manifest["features"]["default"],
            [],
            "A non-empty default feature set requires an explicit update to the matrix coverage strategy.",
        )

    def test_matrix_has_unique_names(self):
        names = [check["name"] for check in self.matrix["checks"]]
        self.assertEqual(len(names), len(set(names)))

    def test_matrix_covers_each_feature_combination(self):
        expected = {
            "default": (True, frozenset()),
            "async-only": (False, frozenset({"async"})),
            "inventory-only": (False, frozenset({"inventory"})),
            "async-inventory": (False, frozenset({"async", "inventory"})),
        }
        actual = {
            check["name"]: (
                check["defaultFeatures"],
                frozenset(check["features"]),
            )
            for check in self.matrix["checks"]
        }
        self.assertEqual(actual, expected)

    def test_each_check_runs_check_test_and_doc(self):
        for check in self.matrix["checks"]:
            with self.subTest(check=check["name"]):
                self.assertEqual(set(check["commands"]), {"check", "test", "doc"})
                self.assertEqual(len(check["commands"]), 3)

    def test_matrix_features_are_declared_in_manifest(self):
        declared_features = set(self.manifest["features"])
        for check in self.matrix["checks"]:
            with self.subTest(check=check["name"]):
                self.assertLessEqual(set(check["features"]), declared_features)

    def test_matrix_version_remains_one(self):
        self.assertEqual(self.matrix["version"], 1)


if __name__ == "__main__":
    unittest.main()
