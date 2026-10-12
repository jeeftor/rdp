import test from 'node:test';
import assert from 'node:assert/strict';
import { authenticationLabel, canConfirmTrial, isAccountLocked, wizardCanEnter, graphicsMode, preferredMode, trialLabel } from '../video.js';

test('remembered graphics preference preserves higher modes and skips unavailable backends', () => {
  const working = ['x11-software', 'x11-opengles2', 'x11-opengl'];
  assert.equal(preferredMode(working, []), 'x11-opengl');
  assert.equal(preferredMode(working, [{ mode: 'x11-opengl', available: false }]), 'x11-opengles2');
  assert.equal(preferredMode(['wayland-opengl'], [{ mode: 'wayland-opengl', available: false }]), undefined);
});

test('legacy software profiles remain software until explicitly changed', () => {
  assert.equal(graphicsMode({ software_rendering: true }), 'x11-software');
  assert.equal(graphicsMode({ software_rendering: true, video_mode: 'x11-opengl' }), 'x11-opengl');
});

test('initialization and client closure never claim visual desktop success', () => {
  assert.equal(trialLabel({ kind: 'initialized' }), 'Needs visual confirmation');
  assert.equal(trialLabel({ kind: 'closed' }), 'Closed — appearance unconfirmed');
  assert.equal(trialLabel({ desktop_initialized: true, timed_out: true }), 'Needs visual confirmation');
  assert.equal(trialLabel({ skipped: true }), 'Not tested');
  assert.equal(trialLabel({ kind: 'testing' }), 'Testing…');
  assert.equal(trialLabel({ kind: 'pending' }), 'Awaiting results');
  assert.equal(trialLabel({ desktop_initialized: true, kind: 'blank' }), 'Image looks blank');
  assert.equal(trialLabel({ kind: 'crash', exit_code: 134 }), 'Client crashed');
});

test('client crashes and certificate rejection are distinct from rejected credentials', () => {
  assert.equal(authenticationLabel({ success: true }), 'Sign-in accepted');
  assert.equal(authenticationLabel({ success: false, kind: 'crash' }), 'Client crashed');
  assert.equal(authenticationLabel({ success: false, kind: 'certificate' }), 'Certificate rejected');
  assert.equal(authenticationLabel({ success: false, kind: 'credentials' }), 'Sign-in rejected');
});

test('failed desktop outcomes cannot be confirmed despite earlier initialization', () => {
  for (const kind of ['credentials', 'renderer', 'crash', 'certificate', 'failed']) {
    assert.equal(canConfirmTrial({ kind, desktop_initialized: true }, true), false);
    assert.notEqual(trialLabel({ kind, desktop_initialized: true }), 'Needs visual confirmation');
  }
  assert.equal(canConfirmTrial({ kind: 'blank', desktop_initialized: true }, true), true);
  assert.equal(canConfirmTrial({ kind: 'initialized' }, false), false);
});

test('guided video requires reachable server, reviewed trust and accepted sign-in', () => {
  assert.equal(wizardCanEnter(2, null, false, []), false);
  assert.equal(wizardCanEnter(3, { success: true }, false, []), false);
  assert.equal(wizardCanEnter(4, { success: true }, true, [{ success: false }]), false);
  assert.equal(wizardCanEnter(4, { success: true }, true, [{ success: true }]), true);
});

test('locked accounts are distinct from generic sign-in rejection and cannot be confirmed', () => {
  const locked = { kind: 'credentials', summary: 'The server rejected sign-in.', details: 'ERRCONNECT_ACCOUNT_LOCKED_OUT', desktop_initialized: true };
  assert.equal(isAccountLocked(locked), true);
  assert.equal(authenticationLabel(locked), 'Account locked');
  assert.equal(canConfirmTrial(locked, true), false);
  assert.equal(isAccountLocked({ kind: 'credentials', summary: 'Reason not specified.' }), false);
});
