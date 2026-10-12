#!/usr/bin/env python3
"""Check PID-scoped screenshot verdicts against real X11 pixels."""
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import sys


def capture(helper: str, pid: int, output: Path, env: dict[str, str] | None = None) -> tuple[int, dict[str, object]]:
    """Run bounded capture and return its structured result."""
    result = subprocess.run([helper, "--pid", str(pid), "--output", str(output)],
                            capture_output=True, text=True, timeout=5, env=env, check=False)
    return result.returncode, json.loads(result.stdout)


def main() -> None:
    """Exercise visible, composited, blank, unavailable and private-file cases."""
    helper, fixture, directory = sys.argv[1:]
    destination = Path(directory)
    for mode, expected in [("black", "blank"), ("uniform", "blank"),
                           ("color", "content"), ("composite", "content"),
                           ("unmapped", "unavailable")]:
        process = subprocess.Popen([fixture, mode], stdout=subprocess.PIPE, text=True)
        try:
            assert process.stdout is not None
            assert process.stdout.readline().strip() == "ready"
            output = destination / f"{mode}.png"
            code, result = capture(helper, process.pid, output)
            assert result["status"] == expected, result
            assert code == (2 if expected == "unavailable" else 0), result
            if expected == "unavailable":
                assert not output.exists()
                continue
            assert (result["width"], result["height"]) == (256, 192), result
            assert result["file"] == str(output)
            assert stat.S_IMODE(output.stat().st_mode) == 0o600
            image = output.read_bytes()
            assert image[:8] == b"\x89PNG\r\n\x1a\n"
            assert struct.unpack(">II", image[16:24]) == (256, 192)
            if mode == "black":
                assert result["nonblack_ratio"] == 0 and result["variance"] == 0, result
            if mode == "composite":
                assert result["capture_method"] == "composite", result
            original = output.read_bytes()
            code, result = capture(helper, process.pid, output)
            assert code == 2 and result["status"] == "unavailable", result
            assert output.read_bytes() == original
        finally:
            process.terminate()
            process.wait(timeout=5)
    output = destination / "wrong-pid.png"
    code, result = capture(helper, 2147483647, output)
    assert code == 2 and result["status"] == "unavailable" and not output.exists(), result
    environment = os.environ.copy()
    environment.pop("DISPLAY", None)
    code, result = capture(helper, os.getpid(), destination / "no-display.png", environment)
    assert code == 2 and result["status"] == "unavailable", result
    assert "Wayland" in str(result["reason"]), result
    print("Window capture: PID isolation, client pixels, black/uniform/content verdicts, Composite, private PNG and unavailable cases passed")


if __name__ == "__main__":
    main()
