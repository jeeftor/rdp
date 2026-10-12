export const videoModes = [
  { value: 'auto', label: 'Automatic' },
  { value: 'x11-opengl', label: 'X11 · OpenGL' },
  { value: 'wayland-opengl', label: 'Wayland · OpenGL' },
  { value: 'x11-opengles2', label: 'X11 · OpenGL ES' },
  { value: 'wayland-opengles2', label: 'Wayland · OpenGL ES' },
  { value: 'x11-software', label: 'X11 · Software' },
];

export function graphicsMode(profile) {
  return profile.video_mode || (profile.software_rendering ? 'x11-software' : 'auto');
}

export function preferredMode(confirmed, availability) {
  return videoModes.slice(1).find(mode => confirmed.includes(mode.value)
    && availability.find(item => item.mode === mode.value)?.available !== false)?.value;
}

export function trialLabel(trial) {
  if (trial?.skipped) return 'Not tested';
  if (trial?.kind === 'disconnected') return 'Disconnected';
  if (isAccountLocked(trial)) return 'Account locked';
  if (trial?.kind === 'credentials') return 'Sign-in failed';
  if (trial?.kind === 'certificate' || trial?.kind === 'certificate_changed') return 'Certificate rejected';
  if (trial?.kind === 'renderer') return 'Graphics failed';
  if (trial?.kind === 'crash') return 'Client crashed';
  if (trial?.kind === 'failed') return 'Failed';
  if (trial?.kind === 'blank') return 'Image looks blank';
  if (trial?.desktop_initialized || trial?.kind === 'initialized') return 'Needs visual confirmation';
  if (trial?.kind === 'pending') return 'Awaiting results';
  if (trial?.kind === 'testing') return 'Testing…';
  if (trial?.kind === 'running') return 'Desktop open';
  if (trial?.kind === 'closed') return 'Closed — appearance unconfirmed';
  if (trial?.kind === 'unavailable') return 'Unavailable';
  return trial ? 'Failed' : 'Not tested';
}

export function authenticationLabel(outcome) {
  if (outcome.success) return 'Sign-in accepted';
  if (isAccountLocked(outcome)) return 'Account locked';
  if (outcome.kind === 'credentials') return 'Sign-in rejected';
  if (['certificate', 'certificate_changed'].includes(outcome.kind)) return 'Certificate rejected';
  if (outcome.kind === 'crash') return 'Client crashed';
  if (outcome.kind === 'renderer') return 'Graphics failed';
  return 'Sign-in not completed';
}

export function canConfirmTrial(trial, available) {
  if (!available || !trial || isAccountLocked(trial) || ['credentials', 'renderer', 'crash', 'certificate', 'certificate_changed', 'failed'].includes(trial.kind)) return false;
  return Boolean(trial.desktop_initialized || ['running', 'initialized', 'closed', 'disconnected'].includes(trial.kind));
}

export function wizardCanEnter(step, reachability, trustReviewed, authentication = []) {
  if (step === 1) return true;
  if (!reachability?.success) return false;
  if (step === 2) return true;
  if (!trustReviewed) return false;
  return step === 3 || authentication.some(outcome => outcome.success);
}

export function isAccountLocked(outcome) {
  return outcome?.kind === 'account_locked' || /ERRCONNECT_ACCOUNT_LOCKED_OUT|account (?:is )?locked|account[^\n]*locked out/i.test(`${outcome?.summary || ''} ${outcome?.details || ''}`);
}
