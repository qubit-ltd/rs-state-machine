import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from types import SimpleNamespace


SCRIPT = Path(__file__).with_name("check-doc-consumers.py")
SPEC = importlib.util.spec_from_file_location("doc_consumers", SCRIPT)
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


class ParserTests(unittest.TestCase):
    def test_multiple_blocks_hidden_lines_and_attributes_preserved(self):
        md = "## Example\n```toml\na=1\n```\n```rust\n# let hidden = 1;\nassert_eq!(hidden, 1);\n```\n"
        attrs, body = checker.code_block(md, "Example", "rust", 1)
        self.assertEqual(attrs, "")
        self.assertIn("# let hidden = 1;", body)
        self.assertIn("assert_eq!(hidden, 1);", body)

    def test_repeated_heading_fails(self):
        with self.assertRaises(checker.CheckError):
            checker.section_text("## A\nx\n## A\ny\n", "A")

    def test_missing_heading_and_block_fail(self):
        with self.assertRaises(checker.CheckError):
            checker.code_block("## A\ntext\n", "Missing", "rust", 1)
        with self.assertRaises(checker.CheckError):
            checker.code_block("## A\n```rust\nx\n```\n", "A", "rust", 2)

    def test_empty_manifest_and_bad_profile_fail(self):
        for raw in ('{"expected_scenarios":0,"scenarios": []}', '{"expected_scenarios":1,"scenarios":[{"name":"x","document":"a","toml_section":"a","rust_section":"b","toml_block":1,"rust_block":1,"profile":"both"}]}'):
            with self.assertRaises(checker.CheckError):
                checker.parse_config(raw)

    def test_reduced_manifest_count_fails(self):
        raw = '{"expected_scenarios":2,"scenarios":[{"name":"x","document":"a","toml_section":"a","rust_section":"b","toml_block":1,"rust_block":1,"profile":"standard"}]}'
        with self.assertRaises(checker.CheckError):
            checker.parse_config(raw)

    def test_duplicate_rust_block_selector_fails(self):
        row = {"name":"one","document":"README.md","toml_section":"Install","toml_block":1,"rust_section":"Example","rust_block":1,"profile":"fast"}
        duplicate = {**row, "name":"two"}
        raw = json.dumps({"expected_scenarios":2,"scenarios":[row,duplicate]})
        with self.assertRaises(checker.CheckError):
            checker.parse_config(raw)

    def test_bad_toml_fails(self):
        item = {"name": "x", "profile": "standard"}
        with self.assertRaises(checker.CheckError):
            checker.manifest_text("not = [toml", item, Path("/project"))

    def test_feature_mismatch_fails(self):
        item = {"name": "x", "profile": "standard"}
        source = '[dependencies]\nqubit-state-machine = { version="0.11", default-features=false, features=["fast"] }\n'
        with self.assertRaises(checker.CheckError):
            checker.manifest_text(source, item, Path("/project"))

    def test_nonzero_cargo_status_propagates_and_temp_is_cleaned(self):
        with tempfile.TemporaryDirectory() as outer:
            root = Path(outer)
            (root / "doc.md").write_text("## Setup\n```toml\n[dependencies]\nqubit-state-machine = { version='0.1', default-features=false, features=['standard'] }\n```\n## Code\n```rust\nassert!(true);\n```\n")
            config = root / "config.json"
            config.write_text('{"expected_scenarios":1,"scenarios":[{"name":"x","document":"doc.md","toml_section":"Setup","toml_block":1,"rust_section":"Code","rust_block":1,"profile":"standard"}]}')
            captured = {}
            def fail(command, cwd, check, capture_output, text):
                captured["manifest"] = Path(command[command.index("--manifest-path") + 1]).parent
                return SimpleNamespace(returncode=7, stdout="", stderr="")
            self.assertEqual(checker.run_check(root, config, fail), 7)
            self.assertFalse(captured["manifest"].exists())


class FaultInjectionTests(unittest.TestCase):
    def test_missing_feature_and_direct_dependency_are_rejected(self):
        package = checker.toml_quote(str(checker.ROOT))
        cases = {
            "missing-standard-feature": (
                f'[dependencies]\nqubit-state-machine = {{ path = {package}, default-features = false, features = [] }}\n',
                "use qubit_state_machine::StateMachine;\n",
            ),
            "missing-qubit-atomic-direct-dependency": (
                f'[dependencies]\nqubit-state-machine = {{ path = {package}, default-features = false, features = ["standard"] }}\n',
                "use qubit_atomic::AtomicRef;\n",
            ),
        }
        with tempfile.TemporaryDirectory(prefix="rsm-doc-fault-injection-") as tmp:
            temp_root = Path(tmp)
            for name, (dependencies, example) in cases.items():
                with self.subTest(name=name):
                    consumer = temp_root / name
                    (consumer / "src").mkdir(parents=True)
                    (consumer / "Cargo.toml").write_text(
                        '[package]\nname = "fault-injection"\nversion = "0.0.0"\nedition = "2024"\n\n' + dependencies,
                        encoding="utf-8",
                    )
                    (consumer / "src" / "lib.rs").write_text(
                        "//! Fault injection example\n//!\n//! ```\n" + example + "//! ```\n",
                        encoding="utf-8",
                    )
                    result = subprocess.run(
                        ["cargo", "test", "--doc", "--manifest-path", str(consumer / "Cargo.toml"), "--target-dir", str(temp_root / "target")],
                        cwd=checker.ROOT,
                        check=False,
                        capture_output=True,
                        text=True,
                    )
                    self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertIn("error", (result.stdout + result.stderr).lower())


if __name__ == "__main__":
    unittest.main()
