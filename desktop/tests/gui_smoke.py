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
        error.msg = f"{error.msg}: {details}"
        raise
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
    """Exercise the compact Vue dialogs, Rust IPC, and real profile storage."""
    bundled_monitors = sys.argv[3:] == ["--bundled-monitors"]
    application = str(Path(sys.argv[1]).resolve())
    screenshot = Path(sys.argv[2])
    with tempfile.TemporaryDirectory(prefix="rdpctl-gui-") as temporary:
        root = Path(temporary)
        profiles = root / "rdpctl/connections"
        profiles.mkdir(parents=True)
        existing = {"id": "existing", "name": "Existing <b>profile</b>", "host": "windows.example.invalid", "user": "DOMAIN\\user", "fullscreen": False, "multi_monitor": False}
        original = json.dumps(existing)
        (profiles / "existing.json").write_text(original)
        client = root / "fake-freerdp"
        args_path = root / "arguments"
        client.write_text(
            '#!/bin/sh\nif [ "$1" = /list:monitor ]; then\n'
            "printf 'listing 2 monitors:\\n * [1] [screen] 1280x1024 +0+0\\n [3] [second] 1920x1080 +1280+0\\n'; exit 255; fi\n"
            f'if [ "$1" = /args-from:stdin ]; then cat > "{args_path}"; else printf \'%s\\n\' "$@" > "{args_path}"; fi\n'
            f'case "$(cat "{args_path}")" in *+auth-only*)\n'
            f'case "$(cat "{args_path}")" in */cert:fingerprint:sha256:*) exit 0;;\n'
            "*) printf 'certificate not trusted\\nThe fingerprint for the host key is " + "ab:" * 31 + "ab\\n'; exit 1;; esac;; esac\n"
            "printf 'Local framebuffer format BGRA32\\nRemote framebuffer format BGRA32\\nfake session output\\n'\n"
            f'case "$(cat "{args_path}")" in */size:1280x720*) sleep 6;; *) sleep 1;; esac\n'
        )
        client.chmod(0o700)
        probe = root / "bin/rdp-render-probe"
        probe.parent.mkdir()
        probe.write_text('#!/bin/sh\nbackend=${SDL_VIDEO_DRIVER:-${SDL_VIDEODRIVER:-}}\nif [ -z "$backend" ]; then if [ -n "${WAYLAND_DISPLAY:-}" ]; then backend=wayland; else backend=x11; fi; fi\nrenderer=${SDL_RENDER_DRIVER:-opengl}\nprintf \'%s|%s\\n\' "$backend" "$renderer"\nprintf \'FreeRDP graphics: %s/%s passed frame presentation (llvmpipe LLVM)\\n\' "$backend" "$renderer" >&2\n')
        probe.chmod(0o700)
        # A valid tiny PNG tests IPC previews; native capture pixels are tested under Xvfb separately.
        import struct
        import zlib
        def chunk(kind: bytes, data: bytes) -> bytes:
            """Encode one PNG chunk with a correct CRC."""
            return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
        png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', 1, 1, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b'\x00\xff\x00\x00')) + chunk(b'IEND', b'')
        capture = root / "bin/rdp-capture-window"
        capture.write_text('#!/usr/bin/env python3\nimport json, os, sys\npath=sys.argv[sys.argv.index("--output")+1]\nfd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)\nos.write(fd,' + repr(png) + ')\nos.close(fd)\nprint(json.dumps({"status":"content","reason":"Fixture image has content; appearance unconfirmed","file":path,"width":1,"height":1,"nonblack_ratio":1.0,"variance":1200.0,"capture_method":"composite"}))\n')
        capture.chmod(0o700)
        environment = dict(os.environ, XDG_CONFIG_HOME=str(root), XDG_STATE_HOME=str(root / 'state'), RDPCTL_FREERDP=str(client))
        if bundled_monitors:
            environment.pop('RDPCTL_FREERDP', None)
        driver = subprocess.Popen(['tauri-driver'], env=environment)
        session: str | None = None
        try:
            for _ in range(100):
                try:
                    request('GET', '/status')
                    break
                except URLError:
                    if driver.poll() is not None:
                        raise RuntimeError('tauri-driver exited before startup')
                    time.sleep(0.1)
            value = request('POST', '/session', {'capabilities': {'alwaysMatch': {'browserName': 'wry', 'tauri:options': {'application': application}}}})
            session = value['sessionId']
            def javascript(script: str) -> Any:
                """Execute JavaScript inside the actual native webview."""
                return request('POST', f'/session/{session}/execute/sync', {'script': script, 'args': []})
            def click(test_id: str) -> None:
                """Activate an accessible control through its stable test identifier."""
                wait_for(lambda: javascript(f"return !!document.querySelector('[data-testid={test_id}]') && !document.querySelector('[data-testid={test_id}]').disabled"), f'{test_id} enabled')
                javascript(f"document.querySelector('[data-testid={test_id}]').click()")
            def fill(test_id: str, value: str) -> None:
                """Enter text and notify Vue's real input handler."""
                javascript(f"const input=document.querySelector('[data-testid={test_id}]'); input.value={json.dumps(value)}; input.dispatchEvent(new Event('input', {{bubbles:true}}))")
            def action(profile_id: str, test_id: str) -> None:
                """Activate a specific connection row's action."""
                wait_for(lambda: javascript(f"return !!document.querySelector('[data-profile-id={profile_id}] [data-testid={test_id}]') && !document.querySelector('[data-profile-id={profile_id}] [data-testid={test_id}]').disabled"), f'{profile_id} {test_id} enabled')
                javascript(f"document.querySelector('[data-profile-id={profile_id}] [data-testid={test_id}]').click()")
            def choose(test_id: str, label: str) -> None:
                """Choose a PrimeVue option using its visible label."""
                click(test_id)
                wait_for(lambda: javascript("return document.querySelectorAll('[role=option]').length > 0"), 'select options')
                javascript(f"Array.from(document.querySelectorAll('[role=option]')).find(option=>option.textContent.trim()==={json.dumps(label)}).click()")
            def close_dialog() -> None:
                """Close the test dialog using its explicit Close action."""
                javascript("Array.from(document.querySelector('[data-testid=test-dialog]').querySelectorAll('button')).find(button=>button.textContent.trim()==='Close').click()")
                wait_for(lambda: javascript("return !document.querySelector('[data-testid=test-dialog]')"), 'test dialog closed')
            wait_for(lambda: javascript("return document.querySelector('[data-profile-id=existing]')"), 'legacy JSON connection table')
            assert javascript("return document.querySelector('[data-testid=connections-table]').textContent.includes('Existing <b>profile</b>')")
            assert not javascript("return document.querySelector('[data-testid=connections-table] b')")
            assert javascript("return !!document.querySelector('[data-profile-id=existing] [data-testid=edit]')")
            click('new-connection')
            wait_for(lambda: javascript("return document.querySelector('[data-testid=editor-dialog]')"), 'new connection dialog')
            click('detect-displays')
            wait_for(lambda: javascript("return document.querySelector('[data-testid=editor-dialog] .monitor-list')"), 'SDL display detection')
            if bundled_monitors:
                screenshot.parent.mkdir(parents=True, exist_ok=True)
                screenshot.write_bytes(base64.b64decode(request('GET', f'/session/{session}/screenshot')))
                print('Portable GUI smoke passed: detected actual bundled FreeRDP monitors.')
                return
            choose('certificate-policy', 'Ignore certificate checks')
            choose('certificate-policy', 'Verify certificate')
            fill('profile-name', 'New workstation')
            fill('profile-host', 'new.example.invalid')
            fill('profile-user', 'user')
            fill('profile-password', 'saved secret')
            javascript("document.getElementById('fullscreen').click(); document.getElementById('multi-monitor').click(); document.getElementById('monitor-3').click()")
            click('save-connection')
            saved_path = profiles / 'new-workstation.json'
            wait_for(saved_path.exists, 'saved profile')
            saved = json.loads(saved_path.read_text())
            assert saved['monitors'] == '3' and saved['multi_monitor'] and saved['fullscreen']
            assert 'password' not in saved
            assert saved_path.stat().st_mode & 0o777 == 0o600
            password_path = profiles / 'passwords/new-workstation'
            wait_for(password_path.exists, 'saved plaintext password')
            assert password_path.read_text() == 'saved secret'
            assert password_path.stat().st_mode & 0o777 == 0o600
            wait_for(lambda: javascript("return document.querySelector('[data-profile-id=new-workstation]')"), 'new connection row')
            action('new-workstation', 'edit')
            wait_for(lambda: javascript("return document.querySelector('[data-testid=profile-password]')?.value==='saved secret'"), 'visible saved password')
            assert javascript("return document.querySelector('[data-testid=profile-password]').type==='text'")
            fill('profile-name', 'Edited workstation')
            fill('profile-host', 'edited.example.invalid')
            fill('profile-user', 'DOMAIN\\other')
            fill('monitor-ids', '99')
            click('detect-displays')
            wait_for(lambda: javascript("return document.querySelector('#status').textContent.includes('99 unavailable')"), 'stale monitor diagnosis')
            fill('monitor-ids', '1,3')
            click('save-connection')
            wait_for(lambda: json.loads(saved_path.read_text())['host'] == 'edited.example.invalid', 'edited profile')
            action('new-workstation', 'test')
            wait_for(lambda: javascript("return document.querySelector('[data-testid=test-dialog]')"), 'test dialog')
            click('test-authentication')
            wait_for(lambda: javascript("return document.querySelector('[data-testid=authentication-result]').textContent.includes('Certificate trust failed')"), 'certificate rejection')
            remembered = root / 'freerdp/server/edited.example.invalid_3389.pem'
            remembered.parent.mkdir(parents=True)
            remembered.write_text('remembered certificate')
            click('backup-certificate'); click('confirm-certificate-backup')
            wait_for(lambda: not remembered.exists(), 'host-only certificate backup')
            assert next(remembered.parent.glob('*.pem.previous-*')).read_text() == 'remembered certificate'
            click('pin-fingerprint')
            wait_for(lambda: javascript("return document.querySelector('[data-testid=fingerprint]')?.value==='" + 'ab' * 32 + "'"), 'detected fingerprint in editor')
            click('save-connection')
            wait_for(lambda: json.loads(saved_path.read_text()).get('certificate') == 'fingerprint', 'saved fingerprint policy')
            click('test-authentication')
            wait_for(lambda: javascript("return document.querySelector('[data-testid=authentication-result]').textContent.includes('Sign-in accepted')"), 'NLA authentication success')
            assert javascript("return document.querySelector('[data-testid=authentication-result]').textContent.includes('/p:saved secret')")
            click('test-all')
            # Real batch launches run sequentially; fixture client exits after six seconds each.
            deadline = time.monotonic() + 70
            while time.monotonic() < deadline:
                if javascript("return document.querySelector('[data-testid=test-status]').textContent.includes('Tests complete')"):
                    break
                time.sleep(0.2)
            else:
                raise AssertionError('Batch video tests did not finish')
            modes = ['x11-opengl', 'x11-opengles2', 'x11-software'] if os.environ.get('DISPLAY') else ['wayland-opengl', 'wayland-opengles2']
            unavailable = 'wayland-opengl' if os.environ.get('DISPLAY') else 'x11-opengl'
            assert javascript(f"return document.querySelector('[data-testid=open-{unavailable}]').disabled")
            for mode in modes:
                assert not javascript(f"return document.querySelector('[data-testid=confirm-{mode}]').disabled")
                assert javascript(f"return document.querySelector('[data-testid=result-{mode}]').textContent.includes('confirmation')")
            if os.environ.get('DISPLAY'):
                assert javascript("return document.querySelector('[data-testid=video-results]').textContent.includes('Image has content')")
            lower, higher = ('x11-software', 'x11-opengl') if os.environ.get('DISPLAY') else ('wayland-opengles2', 'wayland-opengl')
            assert not json.loads(saved_path.read_text()).get('working_video_modes')
            click('confirm-' + lower)
            wait_for(lambda: json.loads(saved_path.read_text()).get('video_mode') == lower, 'operator confirmation saved')
            click('confirm-' + higher)
            wait_for(lambda: json.loads(saved_path.read_text()).get('video_mode') == higher, 'confirmed OpenGL preference')
            click('confirm-' + lower)
            wait_for(lambda: 'Remembered' in javascript("return document.querySelector('#status').textContent"), 'lower-mode confirmation')
            assert json.loads(saved_path.read_text())['video_mode'] == higher
            assert json.loads(saved_path.read_text()).get('monitors', '') == ''
            click('compare-authentication')
            wait_for(lambda: javascript("return document.querySelectorAll('[data-testid=authentication-result]').length === 2"), 'stdin and CLI comparison')
            click('export-diagnostics')
            exported_dir = root / 'state/rdpctl/diagnostics'
            wait_for(lambda: list(exported_dir.glob('rdp-*.json')), 'diagnostic export')
            exported = json.loads(next(exported_dir.glob('rdp-*.json')).read_text())
            auth_attempts = [item for item in exported['attempts'] if item['authentication']]
            assert {item['authentication']['transport'] for item in auth_attempts} == {'stdin', 'argv'}
            assert any('/p:saved secret' in item['command'] for item in exported['attempts'])
            assert any(item.get('screenshot', {}).get('status') == 'content' for item in exported['attempts'] if item.get('screenshot')) == bool(os.environ.get('DISPLAY'))
            screenshot.parent.mkdir(parents=True, exist_ok=True)
            screenshot.write_bytes(base64.b64decode(request('GET', f'/session/{session}/screenshot')))
            close_dialog()
            saved_before_duplicate = saved_path.read_bytes()
            click('new-connection')
            wait_for(lambda: javascript("return document.querySelector('[data-testid=editor-dialog]')"), 'duplicate new dialog')
            fill('profile-name', 'New workstation'); fill('profile-host', 'changed.example.invalid'); fill('profile-user', 'other')
            click('save-connection')
            wait_for(lambda: 'already exists' in javascript("return document.querySelector('#status').textContent"), 'duplicate refusal')
            assert saved_path.read_bytes() == saved_before_duplicate
            assert (profiles / 'existing.json').read_text() == original
            print('GUI smoke passed: compact Vue table/dialogs, edit/password, certificate, batch video, capture evidence, preference, export and duplicate protection.')
        except Exception:
            if session:
                try:
                    screenshot.parent.mkdir(parents=True, exist_ok=True)
                    screenshot.write_bytes(base64.b64decode(request('GET', f'/session/{session}/screenshot')))
                    print('GUI failure state:', javascript("return {status:document.querySelector('#status')?.textContent, dialogs:Array.from(document.querySelectorAll('[role=dialog]')).map(node=>node.textContent), buttons:Array.from(document.querySelectorAll('[data-testid]')).filter(node=>node.tagName==='BUTTON').map(node=>({id:node.dataset.testid,disabled:node.disabled}))}"), file=sys.stderr)
                except Exception as snapshot_error:
                    print(f'Failure snapshot unavailable: {snapshot_error}', file=sys.stderr)
            raise
        finally:
            if session:
                try:
                    request('DELETE', f'/session/{session}')
                except Exception as failure:
                    print(f'WebDriver cleanup: {failure}', file=sys.stderr)
            driver.terminate()
            driver.wait(timeout=10)


if __name__ == '__main__':
    main()
