"""
agent-precommit: Smart pre-commit hooks for humans and AI coding agents.

This package provides a Python wrapper for the agent-precommit (apc) CLI tool.
The actual binary is bundled with this package and called via subprocess.

Usage:
    $ apc init          # Initialize configuration
    $ apc install       # Install git hook
    $ apc run           # Run checks
    $ apc detect        # Show detected mode
"""

import os
import subprocess
import sys
from pathlib import Path

__version__ = "0.1.0"
__all__ = ["main", "run_apc"]


def _get_binary_path() -> Path:
    """Get the path to the bundled apc binary."""
    # The binary is bundled in the same directory as this module
    module_dir = Path(__file__).parent

    if sys.platform == "win32":
        binary_name = "apc.exe"
    else:
        binary_name = "apc"

    binary_path = module_dir / binary_name

    if not binary_path.exists():
        # Fall back to looking in PATH
        import shutil
        found = shutil.which("apc")
        if found:
            return Path(found)
        raise FileNotFoundError(
            f"Could not find apc binary. Expected at {binary_path} or in PATH."
        )

    return binary_path


def run_apc(*args: str) -> subprocess.CompletedProcess:
    """
    Run the apc binary with the given arguments.

    Args:
        *args: Command-line arguments to pass to apc.

    Returns:
        CompletedProcess with the result.

    Example:
        >>> result = run_apc("detect")
        >>> print(result.stdout)
    """
    binary = _get_binary_path()
    return subprocess.run(
        [str(binary), *args],
        capture_output=True,
        text=True,
    )


def main() -> int:
    """
    Main entry point for the apc command.

    This function is called when running `apc` or `agent-precommit` from the command line.
    """
    binary = _get_binary_path()

    # Pass through all arguments to the binary
    result = subprocess.run([str(binary), *sys.argv[1:]])
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
