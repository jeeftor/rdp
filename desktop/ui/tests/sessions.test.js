import test from 'node:test';
import assert from 'node:assert/strict';
import { beginSessionOperation, createSessionState, finishSessionOperation, hasSessions, recordSessionEnded, recordSessionStarted, replaceSessions } from '../sessions.js';

test('multiple sessions remain active until their exact processes end', () => {
  const state = createSessionState();
  recordSessionStarted(state, { id: 'workstation', pid: 11 });
  recordSessionStarted(state, { id: 'workstation', pid: 12 });
  recordSessionStarted(state, { id: 'other', pid: 13 });
  recordSessionEnded(state, 11);
  assert.equal(hasSessions(state, 'workstation'), true);
  recordSessionEnded(state, 12);
  assert.equal(hasSessions(state, 'workstation'), false);
  assert.equal(hasSessions(state, 'other'), true);
});

test('exit before launch response cannot resurrect a disconnected process', () => {
  const state = createSessionState();
  beginSessionOperation(state);
  recordSessionEnded(state, 21);
  assert.equal(recordSessionStarted(state, { id: 'workstation', pid: 21 }), false);
  finishSessionOperation(state);
  assert.equal(hasSessions(state, 'workstation'), false);
  // A later process can legitimately reuse the same operating-system identifier.
  assert.equal(recordSessionStarted(state, { id: 'workstation', pid: 21 }), true);
});

test('stale snapshot excludes processes whose end event arrived while loading', () => {
  const state = createSessionState();
  beginSessionOperation(state);
  recordSessionEnded(state, 31);
  replaceSessions(state, [{ id: 'workstation', pid: 31 }, { id: 'other', pid: 32 }]);
  finishSessionOperation(state);
  assert.equal(hasSessions(state, 'workstation'), false);
  assert.equal(hasSessions(state, 'other'), true);
});
