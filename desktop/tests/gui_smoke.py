"""Exercise the real Tauri window, Rust commands, and profile files on Linux."""

from __future__ import annotations

import base64
import json
import os
from pathlib import Path
from collections.abc import Callable
import subprocess
import sys
import tempfile
import time
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


def request(method: str, path: str, payload: dict[str, Any] | None = None) -> Any:
    """Send a request to the local WebDriver without external Python packages."""
    body = json.dumps(payload).encode() if payload is not None else None
    message = Request(f"http://127.0.0.1:4444{path}", data=body, method=method,
                      headers={"Content-Type": "application/json"})
    try:
        with urlopen(message, timeout=30) as response:
            result: dict[str, Any] = json.load(response)
    except HTTPError as error:
        details = error.read(4096).decode(errors="replace")
        raise RuntimeError(f"WebDriver HTTP {error.code}: {details}") from error
    value: Any = result.get("value")
    if isinstance(value, dict) and "error" in value:
        raise RuntimeError(str(value))
    return value


def wait_for(check: Callable[[], Any], description: str) -> Any:
    """Wait briefly for an observable GUI or filesystem condition."""
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        result: Any = check()
        if result:
            return result
        time.sleep(0.1)
    raise AssertionError(f"Timed out waiting for {description}")


def main() -> None:
    """Run a save, detect, launch, and duplicate-protection smoke test."""
    bundled_monitors = sys.argv[3:] == ["--bundled-monitors"]
    application = str(Path(sys.argv[1]).resolve())
    screenshot = Path(sys.argv[2])
    with tempfile.TemporaryDirectory(prefix="rdpctl-gui-") as temporary:
        root = Path(temporary)
        profiles = root / "rdpctl" / "connections"
        profiles.mkdir(parents=True)
        existing: dict[str, Any] = {
            "id": "existing", "name": "Existing <b>profile</b>",
            "host": "windows.example.invalid", "user": "DOMAIN\\user",
            "fullscreen": False, "multi_monitor": False,
        }
        (profiles / "existing.json").write_text(json.dumps(existing))
        client = root / "fake-freerdp"
        client.write_text(
            '#!/bin/sh\nif [ "$1" = /list:monitor ]; then\n'
            "  printf 'listing 2 monitors:\\n     * [1] [screen] 1280x1024\\t+0+0\\n       [3] [second] 1920x1080\\t+1280+0\\n'\n"
            "  exit 255\nfi\n"
            f"if [ \"$1\" = /args-from:stdin ]; then cat > '{root / 'arguments'}'; "
            f"else printf '%s\\n' \"$@\" > '{root / 'arguments'}'; fi\n"
            f"case \"$(cat '{root / 'arguments'}')\" in\n"
            "  *+auth-only*)\n"
            f"    case \"$(cat '{root / 'arguments'}')\" in\n"
            "      */cert:fingerprint:sha256:*) exit 0 ;;\n"
            "      *) printf 'certificate not trusted\\nThe fingerprint for the host key sent by the remote host is "
            + "ab:" * 31 + "ab\\n'; exit 1 ;;\n"
            "    esac ;;\n"
            "esac\nprintf 'fake session output\\n'\nsleep 1\n"
        )
        client.chmod(0o700)
        environment = dict(os.environ, XDG_CONFIG_HOME=str(root), RDPCTL_FREERDP=str(client))
        if bundled_monitors:
            environment.pop("RDPCTL_FREERDP", None)
        driver = subprocess.Popen(["tauri-driver"], env=environment)
        session: str | None = None
        try:
            for _ in range(100):
                try:
                    request("GET", "/status")
                    break
                except URLError:
                    if driver.poll() is not None:
                        raise RuntimeError("tauri-driver exited before startup")
                    time.sleep(0.1)
            value = request("POST", "/session", {"capabilities": {"alwaysMatch": {
                "browserName": "wry",
                "tauri:options": {"application": application}
            }}})
            session = value["sessionId"]

            def javascript(script: str) -> Any:
                """Execute JavaScript inside the actual application webview."""
                return request("POST", f"/session/{session}/execute/sync", {"script": script, "args": []})

            wait_for(lambda: javascript("return document.querySelectorAll('.connection').length === 1"), "existing Go profile")
            assert javascript("return document.querySelector('.connection h2').textContent") == existing["name"]
            assert javascript("return document.querySelectorAll('.connection b').length") == 0
            javascript("document.querySelector('#detect-monitors').click()")
            if bundled_monitors:
                wait_for(lambda: javascript("return document.querySelectorAll('[name=monitor]').length > 0"), "bundled FreeRDP monitor enumeration")
                screenshot.parent.mkdir(parents=True, exist_ok=True)
                screenshot.write_bytes(base64.b64decode(request("GET", f"/session/{session}/screenshot")))
                print("Portable GUI smoke passed: detected actual monitors through the bundled FreeRDP client.")
                return
            wait_for(lambda: javascript("return document.querySelectorAll('[name=monitor]').length === 2"), "SDL monitor IDs")
            javascript("""
              const form = document.querySelector('#profile-form');
              form.elements.name.value = 'New workstation';
              form.elements.host.value = 'new.example.invalid';
              form.elements.user.value = 'user';
              form.elements.password.value = 'saved secret';
              form.elements.fullscreen.checked = true;
              form.elements.multi_monitor.checked = true;
              document.querySelector('[name=monitor][value="3"]').checked = true;
              form.requestSubmit();
            """)
            saved_path = profiles / "new-workstation.json"
            wait_for(saved_path.exists, "saved profile")
            saved: dict[str, Any] = json.loads(saved_path.read_text())
            assert saved["monitors"] == "3"
            assert saved["multi_monitor"] is True
            assert "password" not in saved
            assert saved_path.stat().st_mode & 0o777 == 0o600
            wait_for(lambda: javascript("return document.querySelectorAll('.connection').length === 2"), "saved connection card")
            password_path = profiles / "passwords" / "new-workstation"
            wait_for(password_path.exists, "saved plaintext password")
            assert password_path.read_text() == "saved secret"
            assert password_path.stat().st_mode & 0o777 == 0o600
            assert javascript("return document.querySelectorAll('.connection')[1].querySelector('[name=password]').type") == "text"
            assert javascript("return document.querySelectorAll('.connection')[1].querySelector('[name=password]').value") == "saved secret"
            assert javascript("return document.querySelectorAll('.connection')[1].querySelector('.display-settings').textContent.includes('Automatic graphics (Wayland / X11)')")
            assert javascript("return document.querySelectorAll('.connection')[1].querySelector('[name=video_mode]').value") == "auto"
            javascript("document.querySelectorAll('.connection')[1].querySelector('button').click()")
            expected_arguments = [
                "/v:new.example.invalid", "/u:user", "/multimon", "/monitors:3", "/f", "/cert:deny", "/p:saved secret"
            ]
            wait_for(lambda: (root / "arguments").exists() and (root / "arguments").read_text().splitlines() == expected_arguments, "complete FreeRDP launch arguments")
            javascript("document.querySelectorAll('.connection')[1].querySelectorAll('.actions button')[1].click()")
            wait_for(lambda: javascript("return document.querySelectorAll('.connection')[1].textContent.includes('Certificate trust failed')"), "certificate test failure")
            arguments = (root / "arguments").read_text()
            assert "+auth-only\n/sec:nla\n" in arguments
            assert "/multimon" not in arguments
            certificates = root / "freerdp" / "server"
            certificates.mkdir(parents=True)
            remembered = certificates / "new.example.invalid_3389.pem"
            remembered.write_text("remembered certificate")
            javascript("Array.from(document.querySelectorAll('.connection')[1].querySelectorAll('button')).find(button => button.textContent.startsWith('Back up remembered')).click()")
            assert remembered.exists()
            javascript("Array.from(document.querySelectorAll('.connection')[1].querySelectorAll('button')).find(button => button.textContent === 'Confirm certificate backup').click()")
            wait_for(lambda: not remembered.exists(), "certificate backup")
            assert next(certificates.glob("*.pem.previous-*")).read_text() == "remembered certificate"
            javascript("Array.from(document.querySelectorAll('.connection')[1].querySelectorAll('button')).find(button => button.textContent === 'Use detected fingerprint').click()")
            javascript("document.querySelectorAll('.connection')[1].querySelectorAll('.actions button')[1].click()")
            wait_for(lambda: javascript("return document.querySelectorAll('.connection')[1].textContent.includes('NLA authentication succeeded')"), "successful authentication test")
            assert json.loads(saved_path.read_text())["fingerprint"] == "ab" * 32
            wait_for(lambda: javascript("return document.querySelectorAll('.connection')[1].querySelector('.command').textContent.includes('/p:saved secret')"), "full GUI test command")
            javascript("""
              const card = document.querySelectorAll('.connection')[1];
              const editor = card.querySelector('details form');
              editor.elements.name.value = 'Edited workstation';
              editor.elements.host.value = 'edited.example.invalid';
              editor.elements.user.value = 'DOMAIN\\\\other';
              editor.elements.fullscreen.checked = false;
              editor.elements.monitors.value = '1,3';
              editor.requestSubmit();
            """)
            wait_for(lambda: json.loads(saved_path.read_text())["host"] == "edited.example.invalid", "edited connection")
            edited = json.loads(saved_path.read_text())
            assert edited["id"] == "new-workstation"
            assert edited["user"] == "DOMAIN\\other" and edited["monitors"] == "1,3"
            assert edited["fullscreen"] is False
            assert password_path.read_text() == "saved secret"
            javascript("""
              const editor = document.querySelectorAll('.connection')[1].querySelector('details form');
              editor.elements.monitors.value = '99';
              Array.from(editor.querySelectorAll('button')).find(button => button.textContent === 'Detect displays for this connection').click();
            """)
            wait_for(lambda: javascript("return document.querySelectorAll('.connection')[1].querySelector('details form').textContent.includes('99 are unavailable')"), "stale monitor diagnosis")
            javascript("""
              const editor = document.querySelectorAll('.connection')[1].querySelector('details form');
              for (const id of ['1', '3']) {
                const checkbox = editor.querySelector(`input[type=checkbox][value="${id}"]`);
                checkbox.checked = true;
                checkbox.dispatchEvent(new Event('change'));
              }
            """)
            javascript("document.querySelectorAll('.connection')[1].querySelector('button').click()")
            wait_for(lambda: "/v:edited.example.invalid" in (root / "arguments").read_text(), "edited FreeRDP launch")
            wait_for(lambda: javascript("return document.querySelectorAll('.connection')[1].querySelector('.command').textContent.includes('/v:edited.example.invalid')"), "full GUI launch command")
            wait_for(lambda: javascript("return document.querySelectorAll('.connection')[1].querySelector('.session-output').textContent.includes('fake session output')"), "live FreeRDP log")
            # Confirm real GUI trial commands and preference persistence with the fake client.
            javascript("window.videoCard = () => Array.from(document.querySelectorAll('.connection')).find(card => card.querySelector('h2').textContent === 'Edited workstation')")
            lower_index, higher_index = (4, 0) if os.environ.get("DISPLAY") else (3, 1)
            lower_mode = "x11-software" if os.environ.get("DISPLAY") else "wayland-opengles2"
            higher_mode = "x11-opengl" if os.environ.get("DISPLAY") else "wayland-opengl"
            higher_label = "X11 · OpenGL" if os.environ.get("DISPLAY") else "Wayland · OpenGL"
            javascript(f"window.videoLower = {lower_index}; window.videoHigher = {higher_index}")
            javascript("Array.from(window.videoCard().querySelectorAll('button')).find(button => button.textContent === 'Test video options').click()")
            if not os.environ.get("DISPLAY"):
                javascript("window.videoCard().querySelectorAll('.video-trial')[4].querySelector('button').click()")
                wait_for(lambda: javascript("return document.querySelector('#status').textContent.includes('requires DISPLAY')"), "unavailable X11 trial error")
            javascript("window.videoCard().querySelectorAll('.video-trial')[window.videoLower].querySelector('button').click()")
            wait_for(lambda: "/size:1280x720" in (root / "arguments").read_text(), "simple video trial arguments")
            trial_arguments = (root / "arguments").read_text()
            assert "/sec:nla" in trial_arguments
            assert "/monitors:" not in trial_arguments and "/multimon" not in trial_arguments
            wait_for(lambda: javascript("return !window.videoCard().querySelectorAll('.video-trial')[window.videoLower].querySelector('button').disabled"), "video trial exit")
            javascript("window.videoCard().querySelectorAll('.video-trial')[window.videoLower].querySelectorAll('button')[1].click()")
            wait_for(lambda: json.loads(saved_path.read_text()).get("video_mode") == lower_mode, "confirmed lower-priority renderer")
            javascript("window.videoCard().querySelectorAll('.video-trial')[window.videoHigher].querySelector('button').click()")
            wait_for(lambda: javascript("return window.videoCard().querySelector('.command').textContent.includes(\"SDL_RENDER_DRIVER='opengl'\")"), "OpenGL video trial")
            wait_for(lambda: javascript("return !window.videoCard().querySelectorAll('.video-trial')[window.videoHigher].querySelector('button').disabled"), "OpenGL trial exit")
            javascript("window.videoCard().querySelectorAll('.video-trial')[window.videoHigher].querySelectorAll('button')[1].click()")
            wait_for(lambda: json.loads(saved_path.read_text()).get("video_mode") == higher_mode, "OpenGL preference over lower-priority renderer")
            assert json.loads(saved_path.read_text()).get("monitors", "") == ""
            javascript("window.videoCard().dataset.refreshMarker = 'before'; document.querySelector('#refresh').click()")
            wait_for(lambda: javascript(f"return window.videoCard()?.dataset.refreshMarker !== 'before' && window.videoCard()?.querySelector('[name=video_mode]')?.value === {json.dumps(higher_mode)}"), "remembered mode after refresh")
            assert set(json.loads(saved_path.read_text())["working_video_modes"]) == {higher_mode, lower_mode}
            javascript("Array.from(window.videoCard().querySelectorAll('button')).find(button => button.textContent === 'Test video options').click()")
            javascript("window.videoCard().querySelectorAll('.video-trial')[window.videoLower].querySelector('button').click()")
            wait_for(lambda: javascript("return !window.videoCard().querySelectorAll('.video-trial')[window.videoLower].querySelector('button').disabled"), "repeated lower-priority trial exit")
            javascript("window.videoCard().querySelectorAll('.video-trial')[window.videoLower].querySelectorAll('button')[1].click()")
            wait_for(lambda: javascript(f"return document.querySelector('#status').textContent.includes({json.dumps('Remembered ' + higher_label)})"), "lower-priority renderer cannot downgrade confirmed OpenGL")
            assert json.loads(saved_path.read_text())["video_mode"] == higher_mode
            original = saved_path.read_bytes()
            javascript("""
              const form = document.querySelector('#profile-form');
              form.elements.name.value = 'New workstation';
              form.elements.host.value = 'changed.example.invalid';
              form.elements.user.value = 'other';
              form.requestSubmit();
            """)
            wait_for(lambda: javascript("return document.querySelector('#status').classList.contains('error')"), "duplicate error")
            assert saved_path.read_bytes() == original
            assert json.loads((profiles / "existing.json").read_text()) == existing
            screenshot.parent.mkdir(parents=True, exist_ok=True)
            screenshot.write_bytes(base64.b64decode(request("GET", f"/session/{session}/screenshot")))
            print("GUI smoke passed: existing profiles, safe rendering, save, monitors, launch, duplicate protection.")
        finally:
            if session:
                request("DELETE", f"/session/{session}")
            driver.terminate()
            try:
                driver.wait(timeout=5)
            except subprocess.TimeoutExpired:
                driver.kill()
                driver.wait()


if __name__ == "__main__":
    main()
