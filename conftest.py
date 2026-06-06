"""Configure optional pytest plugins for repository-level test runs.

The workflow harness uses `cmd-mox`, but script-only test targets do not
install it. This file registers the plugin only when the package is available
so both test environments can share the same repository root.
"""

from importlib.util import find_spec

try:
    has_cmd_mox = find_spec("cmd_mox.pytest_plugin") is not None
except ImportError:
    has_cmd_mox = False

pytest_plugins: tuple[str, ...] = ("cmd_mox.pytest_plugin",) if has_cmd_mox else ()
