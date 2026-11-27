"""Navigation fallback must not invent support or change existing connector links."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

MODULE_PATH = Path(__file__).with_name("generate.py")
SPEC = importlib.util.spec_from_file_location("connector_docs_generator", MODULE_PATH)
generator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(generator)


class ConnectorNavigationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.docs_dir = self.root / "docs-generated" / "connectors"
        self.docs_dir.mkdir(parents=True)
        self.guides = self.root / "docs" / "integrations"
        self.guides.mkdir(parents=True)
        self.root_patch = patch.object(generator, "REPO_ROOT", self.root)
        self.root_patch.start()
        self.addCleanup(self.root_patch.stop)

    def test_incomplete_probe_uses_unique_existing_guide(self):
        (self.guides / "sample-bank.md").write_text("# Bank integration\n")
        self.assertEqual(
            generator.connector_navigation_paths("sample", {}, self.docs_dir),
            ("../docs/integrations/sample-bank.md", "docs/integrations/sample-bank.md"),
        )

    def test_generated_page_keeps_existing_navigation(self):
        (self.docs_dir / "sample.md").write_text("# Generated reference\n")
        (self.guides / "sample-bank.md").write_text("# Bank integration\n")
        self.assertEqual(
            generator.connector_navigation_paths("sample", {}, self.docs_dir),
            ("connectors/sample.md", None),
        )

    def test_supported_probe_retains_page_that_will_be_generated(self):
        probe = {"flows": {"authorize": {"Card": {"status": "supported"}}}}
        (self.guides / "sample-bank.md").write_text("# Bank integration\n")
        self.assertEqual(
            generator.connector_navigation_paths("sample", probe, self.docs_dir),
            ("connectors/sample.md", None),
        )

    def test_ambiguous_or_missing_guide_does_not_guess(self):
        self.assertEqual(
            generator.connector_navigation_paths("sample", {}, self.docs_dir),
            ("connectors/sample.md", None),
        )
        for name in ["sample-bank.md", "sample-wallet.md"]:
            (self.guides / name).write_text("# Integration\n")
        self.assertEqual(
            generator.connector_navigation_paths("sample", {}, self.docs_dir),
            ("connectors/sample.md", None),
        )

    def test_llm_navigation_uses_explicit_guide_and_preserves_default(self):
        default = generator.snippets.render_llms_txt_entry("sample", "Sample", {}, [])
        explicit = generator.snippets.render_llms_txt_entry(
            "sample", "Sample", {}, [], doc_path="docs/integrations/sample-bank.md"
        )
        self.assertIn("doc: docs/connectors/sample.md", default)
        self.assertIn("doc: docs/integrations/sample-bank.md", explicit)


if __name__ == "__main__":
    unittest.main()
