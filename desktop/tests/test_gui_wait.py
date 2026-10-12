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
        helpers = runpy.run_path(str(Path(__file__).with_name("gui_smoke.py")))
        self.wait_for = helpers["wait_for"]
        self.read_with_retry = helpers["read_with_retry"]

    def test_state_reads_preserve_false_and_empty_values(self) -> None:
        """Return legitimate negative observations immediately instead of waiting for truth."""
        for value in (False, None, "", 0):
            with self.subTest(value=value):
                read = Mock(return_value=value)
                self.assertEqual(self.read_with_retry(read), value)
                self.assertEqual(read.call_count, 1)

    def test_state_read_retries_transport_failure_then_returns_false(self) -> None:
        """Recover a disconnected read without turning its negative result into success."""
        read = Mock(side_effect=[RemoteDisconnected("closed"), False])
        with patch("time.sleep"):
            self.assertIs(self.read_with_retry(read), False)
        self.assertEqual(read.call_count, 2)

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
