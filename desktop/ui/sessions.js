export function createSessionState() {
  return { active: new Map(), ended: new Set(), pending: 0 };
}

export function beginSessionOperation(state) {
  state.pending += 1;
}

export function finishSessionOperation(state) {
  state.pending -= 1;
  if (state.pending === 0) state.ended.clear();
}

export function recordSessionStarted(state, session) {
  if (state.ended.has(session.pid)) return false;
  state.active.set(session.pid, session);
  return true;
}

export function recordSessionEnded(state, pid) {
  state.active.delete(pid);
  if (state.pending > 0) state.ended.add(pid);
}

export function replaceSessions(state, sessions) {
  state.active.clear();
  for (const session of sessions) recordSessionStarted(state, session);
}

export function hasSessions(state, id) {
  return Array.from(state.active.values()).some(session => session.id === id);
}
