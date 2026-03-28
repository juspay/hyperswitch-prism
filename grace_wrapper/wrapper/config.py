"""Wrapper configuration."""

from pathlib import Path

# Base paths
WRAPPER_ROOT = Path(__file__).parent.parent
GRACE_ROOT = WRAPPER_ROOT.parent / "grace"
CONNECTOR_SERVICE_ROOT = WRAPPER_ROOT.parent

# Configuration
WRAPPER_CONFIG = {
    "max_workers": 4,
    "tasks_file": WRAPPER_ROOT / "tasks.txt",
    "inputs_dir": WRAPPER_ROOT / "inputs",
    "specs_path": GRACE_ROOT / "rulesbook" / "codegen" / "references" / "specs",
    "grace_src": GRACE_ROOT / "src",
}


def get_config():
    """Get wrapper configuration."""
    return WRAPPER_CONFIG
