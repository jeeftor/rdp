"""Check that GUI polling retries transport failures without hiding test errors."""

from __future__ import annotations

from http.client import RemoteDisconnected
from pathlib import Path
import runpy
import unittest
from unittest.mock import Mock, patch
from urllib.error import HTTPError


class PollingTests(unittest.TestCase):
    """Exercise the same polling function used by native GUI checks."""

    def setUp(self) -> None:
        """Load the smoke-test helpers without launching the application."""
        self.wait_for = runpy.run_path(str(Path(__file__).with_name("gui_smoke.py")))["wait_for"]

    def test_transient_disconnect_recovers(self) -> None:
        """Retry a read after a dropped connection and return its observed value."""
        check = Mock(side_effect=[RemoteDisconnected("closed"), "ready"])
        with patch("time.sleep"):
            self.assertEqual(self.wait_for(check, "ready"), "ready")
        self.assertEqual(check.call_count, 2)

    def test_script_error_is_not_retried(self) -> None:
        """Keep WebDriver script failures visible immediately."""
        failure = HTTPError("http://localhost", 500, "script error", {}, None)
        check = Mock(side_effect=failure)
        with self.assertRaises(HTTPError):
            self.wait_for(check, "ready")
        self.assertEqual(check.call_count, 1)
        failure.close()

    def test_transport_failure_remains_bounded(self) -> None:
        """Report the last transport error if polling reaches its deadline."""
        failure = RemoteDisconnected("closed")
        check = Mock(side_effect=failure)
        with patch("time.monotonic", side_effect=[0, 1, 21]), patch("time.sleep"):
            with self.assertRaises(AssertionError) as caught:
                self.wait_for(check, "ready")
        self.assertIs(caught.exception.__cause__, failure)
        self.assertEqual(check.call_count, 1)


if __name__ == "__main__":
    unittest.main()
