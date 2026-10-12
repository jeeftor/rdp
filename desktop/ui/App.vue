<script setup>
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue';
import Button from 'primevue/button';
import Checkbox from 'primevue/checkbox';
import Column from 'primevue/column';
import DataTable from 'primevue/datatable';
import Dialog from 'primevue/dialog';
import InputText from 'primevue/inputtext';
import Message from 'primevue/message';
import Select from 'primevue/select';
import Tag from 'primevue/tag';
import { authenticationLabel, canConfirmTrial, isAccountLocked, wizardCanEnter, graphicsMode, preferredMode, trialLabel, videoModes } from './video.js';

const tauri = window.__TAURI__;
const invoke = (command, args) => tauri.core.invoke(command, args);
const profiles = ref([]);
const availability = ref([]);
const status = ref('Loading connections…');
const error = ref(false);
const busy = ref(false);
const editorOpen = ref(false);
const testsOpen = ref(false);
const selected = ref(null);
const editor = reactive({});
const password = ref('');
const originalPassword = ref('');
const rememberPassword = ref(true);
const monitors = ref([]);
const testState = reactive({});
const certificateBackup = ref(false);
const unlisten = [];
const activeTrial = ref(null);
const guided = ref(false);
const guideStep = ref(1);
const trustReviewed = ref(false);
const guidePassword = ref('');
const guideCredential = ref(null);
const pendingFailure = ref(null);
const pendingRetry = ref(null);
const deleteOpen = ref(false);
const deleteTarget = ref(null);
const guideSteps = ['Server', 'Trust / credentials', 'Sign-in', 'Video'];
const videoAllowed = computed(() => !guided.value || wizardCanEnter(4, current.value?.reachability, trustReviewed.value, current.value?.auth));

const policies = [
  { value: 'verify', label: 'Verify certificate' },
  { value: 'tofu', label: 'Trust on first use' },
  { value: 'fingerprint', label: 'Pin SHA-256' },
  { value: 'ignore', label: 'Ignore certificate checks' },
];
const modeOptions = computed(() => videoModes.map(mode => ({ ...mode,
  disabled: availability.value.find(item => item.mode === mode.value)?.available === false,
})));
const current = computed(() => selected.value ? testState[selected.value.id] : null);
const trialRows = computed(() => videoModes.slice(1).map(mode => {
  const available = availability.value.find(item => item.mode === mode.value);
  const trial = current.value?.trials[mode.value];
  return { ...mode, available: available?.available !== false, reason: available?.reason,
    trial, confirmed: selected.value?.working_video_modes?.includes(mode.value),
    status: available?.available === false && !trial ? 'Unavailable' : trialLabel(trial) };
}));
const selectedMonitors = computed({
  get: () => (editor.monitors || '').split(',').filter(Boolean),
  set: value => { editor.monitors = value.join(','); },
});

function message(text, failed = false) { status.value = text; error.value = failed; }
function state(id) {
  if (!testState[id]) testState[id] = { auth: [], trials: {}, reachability: null, command: '', details: '', graphics: '', live: '' };
  return testState[id];
}
function modeLabel(profile) { return videoModes.find(mode => mode.value === graphicsMode(profile))?.label || 'Automatic'; }
async function run(action) {
  if (busy.value) return;
  busy.value = true;
  try { await action(); } catch (failure) { message(String(failure), true); }
  finally { busy.value = false; }
}
async function refresh() {
  availability.value = await invoke('video_availability');
  profiles.value = await invoke('list_profiles');
  if (selected.value) selected.value = profiles.value.find(profile => profile.id === selected.value.id) || null;
  message(`${profiles.value.length} connection${profiles.value.length === 1 ? '' : 's'}`);
}
async function edit(profile = null, retry = null) {
  await run(async () => {
    pendingRetry.value = retry;
    Object.assign(editor, profile || { id: '', name: '', host: '', user: '', certificate: 'verify', fingerprint: '',
      fullscreen: false, multi_monitor: false, monitors: '', working_video_modes: [] });
    editor.video_mode = profile ? graphicsMode(profile) : 'auto';
    password.value = profile ? (await invoke('saved_password', { id: profile.id })) || '' : '';
    originalPassword.value = password.value;
    rememberPassword.value = true;
    monitors.value = [];
    editorOpen.value = true;
  });
}
async function save() {
  const retry = pendingRetry.value;
  const entered = retry ? password.value : password.value || null;
  const before = profiles.value.find(profile => profile.id === editor.id);
  const hostChanged = Boolean(before && before.host !== editor.host);
  const identityChanged = Boolean(before && (before.user !== editor.user || (before.certificate || 'verify') !== editor.certificate || (before.fingerprint || '') !== (editor.fingerprint || '') || originalPassword.value !== password.value));
  let saved = null;
  await run(async () => {
    const value = { ...editor, software_rendering: editor.video_mode === 'x11-software' };
    const profile = await invoke(editor.id ? 'update_profile' : 'create_profile', { profile: value });
    Object.assign(editor, profile);
    if (password.value && rememberPassword.value) await invoke('save_password', { id: profile.id, password: password.value });
    editorOpen.value = false;
    await refresh();
    message(`Saved ${profile.name}`);
    saved = profile;
    if (hostChanged || identityChanged) state(profile.id).auth = [];
    if (guided.value && selected.value?.id === profile.id) {
      guidePassword.value = (await invoke('saved_password', { id: profile.id })) || '';
      if (hostChanged || identityChanged) {
        trustReviewed.value = false;
        guideCredential.value = null;
        guideStep.value = hostChanged ? 1 : 2;
      }
      if (hostChanged) { current.value.reachability = null; message('Server changed. Check reachability again.'); }
    }
  });
  if (saved && retry && !retry.locked && !hostChanged) {
    pendingRetry.value = null;
    selected.value = profiles.value.find(profile => profile.id === saved.id) || saved;
    state(saved.id);
    if (guided.value) { guideCredential.value = entered; guidePassword.value = entered || ''; trustReviewed.value = true; guideStep.value = 3; }
    if (retry.action === 'connect') await connect(selected.value, entered);
    else if (retry.action === 'desktop' && guided.value) {
      await authenticate(false, entered);
      if (current.value.auth.some(outcome => outcome.success)) await openDesktop({ value: retry.mode }, entered);
    }
    else if (retry.action === 'desktop') await openDesktop({ value: retry.mode }, entered);
    else if (retry.action === 'all' && guided.value) {
      await authenticate(false, entered);
      if (current.value.auth.some(outcome => outcome.success)) await testAll(entered);
    }
    else if (retry.action === 'all') await testAll(entered);
    else await authenticate(retry.action === 'compare', entered);
  } else if (saved && retry && hostChanged && !guided.value) {
    await openWizard(saved);
    message('Server changed. Check reachability before retrying.');
  }
}
async function detect() {
  await run(async () => {
    monitors.value = await invoke('list_monitors', { videoMode: editor.video_mode });
    const missing = selectedMonitors.value.filter(id => !monitors.value.some(monitor => String(monitor.id) === id));
    message(missing.length ? `Monitor IDs ${missing.join(',')} unavailable; select displays or clear IDs.` : `${monitors.value.length} displays found`, missing.length > 0);
  });
}
async function connect(profile, credential = null) {
  await run(async () => {
    const launched = await invoke('launch_profile', { id: profile.id, password: credential });
    Object.assign(state(profile.id), { command: launched.command, graphics: launched.graphics, live: '' });
    message(`Opened ${profile.name}`);
  });
}
function openTests(profile, wizard = false) {
  guided.value = wizard;
  selected.value = profile;
  state(profile.id);
  certificateBackup.value = false;
  testsOpen.value = true;
}
async function openWizard(profile) {
  await run(async () => {
    openTests(profile, true);
    guideStep.value = 1;
    trustReviewed.value = false;
    guideCredential.value = null;
    current.value.reachability = null;
    current.value.auth = [];
    guidePassword.value = (await invoke('saved_password', { id: profile.id })) || '';
    message('Check server reachability first.');
  });
}
async function reachability() {
  await run(async () => {
    current.value.reachability = await invoke('test_reachability', { id: selected.value.id });
    message(current.value.reachability.summary, !current.value.reachability.success);
    if (current.value.reachability.success) guideStep.value = 2;
  });
}
function reviewTrust() {
  trustReviewed.value = true;
  guideStep.value = 3;
  message('Test sign-in with the displayed credentials and certificate policy.');
}
function queueRetry(profile, outcome, action, mode = null) {
  if (!profile || outcome.success) return;
  const diagnostic = { ...outcome, details: outcome.details || state(profile.id).live };
  if (isAccountLocked(diagnostic) || outcome.kind === 'credentials' || (guided.value && ['certificate', 'certificate_changed'].includes(outcome.kind))) {
    pendingFailure.value = { profile, summary: outcome.summary, details: diagnostic.details, locked: isAccountLocked(diagnostic), action, mode };
  }
}
watch([busy, pendingFailure, editorOpen], async ([running, failure, editing]) => {
  if (running || !failure || editing) return;
  pendingFailure.value = null;
  await edit(failure.profile, failure);
});
async function removeProfile() {
  await run(async () => {
    const target = deleteTarget.value;
    await invoke('delete_profile', { id: target.id });
    deleteOpen.value = false;
    editorOpen.value = false;
    if (selected.value?.id === target.id) testsOpen.value = false;
    delete testState[target.id];
    await refresh();
    message(`Deleted ${target.name}`);
  });
}
async function authenticate(compare = false, credential = null) {
  await run(async () => {
    const result = await invoke(compare ? 'compare_authentication' : 'test_profile', { id: selected.value.id, password: credential ?? (guided.value ? guideCredential.value : null) });
    current.value.auth = compare ? result : [result];
    message(compare ? 'Authentication comparison complete' : result.summary, !compare && !result.success);
    const rejected = current.value.auth.find(outcome => !outcome.success);
    if (rejected) queueRetry(selected.value, rejected, compare ? 'compare' : 'auth');
    else if (guided.value) guideStep.value = 4;
  });
}
async function testAll(credential = null) {
  if (!videoAllowed.value) { message('Complete reachability, trust review and sign-in before video tests.', true); return; }
  await run(async () => {
    message('Testing sign-in, then each available graphics mode (up to 15 seconds each)…');
    current.value.trials = {};
    current.value.auth = [];
    let result;
    try {
      result = await invoke('run_video_tests', { id: selected.value.id, password: credential ?? (guided.value ? guideCredential.value : null) });
    } catch (failure) {
      for (const trial of Object.values(current.value.trials)) {
        if (trial.kind === 'testing') { trial.kind = 'failed'; trial.summary = String(failure); }
      }
      throw failure;
    }
    current.value.auth = [result.authentication];
    for (const trial of result.trials) current.value.trials[trial.mode] = trial;
    message(result.authentication.success ? 'Tests complete. Confirm only desktops you saw.' : 'Sign-in not completed; desktop modes were not tested.', !result.authentication.success);
    if (!result.authentication.success) queueRetry(selected.value, result.authentication, 'all');
  });
}
async function openDesktop(row, credential = null) {
  if (!videoAllowed.value) return;
  await run(async () => {
    const id = selected.value.id;
    activeTrial.value = { id, mode: row.value };
    state(id).live = '';
    current.value.trials[row.value] = { kind: 'running', summary: 'Opening desktop…' };
    try {
      const launched = await invoke('launch_profile', { id, password: credential ?? (guided.value ? guideCredential.value : null), videoMode: row.value });
      const saved = state(id);
      const trial = saved.trials[row.value];
      const stillRunning = activeTrial.value?.id === id && activeTrial.value?.mode === row.value;
      Object.assign(trial, { command: launched.command, pid: launched.pid });
      if (stillRunning) {
        Object.assign(trial, { graphics: launched.graphics, kind: 'running', summary: 'Close the desktop before opening another mode.' });
        activeTrial.value.pid = launched.pid;
        message('Check the desktop, then close its window.');
      }
      Object.assign(saved, { command: launched.command, graphics: trial.graphics || launched.graphics });
    } catch (failure) {
      current.value.trials[row.value] = { kind: 'failed', summary: String(failure) };
      activeTrial.value = null;
      throw failure;
    }
  });
}
async function confirmMode(row) {
  await run(async () => {
    const confirmed = [...new Set([...(selected.value.working_video_modes || []), row.value])];
    const best = preferredMode(confirmed, availability.value);
    if (!best) throw new Error('No confirmed mode is available in this session.');
    selected.value = await invoke('update_profile', { profile: { ...selected.value,
      working_video_modes: confirmed, video_mode: best, software_rendering: best === 'x11-software', monitors: '' } });
    await refresh();
    message(`Remembered ${modeLabel(selected.value)}. Detect displays again if you use specific monitors.`);
  });
}
function canConfirm(row) {
  return canConfirmTrial(row.trial, row.available);
}
async function exportDiagnostics() {
  await run(async () => {
    const path = await invoke('export_diagnostics', { id: selected.value.id });
    message(`Diagnostics saved: ${path}`);
  });
}
async function copyCommand(command) {
  try { await navigator.clipboard.writeText(command); message('Command copied'); }
  catch (_) { message('Select the command text and press Ctrl+C.', true); }
}
async function pinFingerprint(outcome) {
  await edit(selected.value);
  editor.certificate = 'fingerprint';
  editor.fingerprint = outcome.fingerprint;
  message('Check this fingerprint against your server before saving.');
}
async function backupCertificate() {
  await run(async () => {
    const found = await invoke('forget_certificate', { id: selected.value.id });
    certificateBackup.value = false;
    message(found ? 'Certificate backed up. Edit the certificate policy before retesting.' : 'No saved certificate found.');
  });
}
onMounted(async () => {
  if (!tauri) { message('Open this interface with rdpctl-gui.', true); return; }
  unlisten.push(await tauri.event.listen('video-test-progress', ({ payload }) => {
    if (payload.phase !== 'starting') return;
    const saved = state(payload.id);
    for (const trial of Object.values(saved.trials)) {
      if (trial.kind === 'testing') { trial.kind = 'pending'; trial.summary = 'Waiting for final results.'; }
    }
    saved.trials[payload.mode] = { kind: 'testing', summary: 'Testing this mode; waiting for results.' };
    const label = videoModes.find(mode => mode.value === payload.mode)?.label || payload.mode;
    message(`Testing ${label}…`);
  }));
  unlisten.push(await tauri.event.listen('session-log', ({ payload }) => {
    const saved = state(payload.id);
    saved.live = (saved.live + payload.line + '\n').slice(-65536);
  }));
  unlisten.push(await tauri.event.listen('session-ended', ({ payload }) => {
    const saved = state(payload.id);
    saved.graphics = payload.outcome.graphics;
    const endedMode = activeTrial.value?.id === payload.id ? activeTrial.value.mode : null;
    if (payload.video_test && activeTrial.value?.id === payload.id && (!activeTrial.value.pid || activeTrial.value.pid === payload.pid)) {
      const previous = saved.trials[activeTrial.value.mode] || {};
      saved.trials[activeTrial.value.mode] = { ...previous, ...payload.outcome, exit_code: payload.code, details: saved.live };
      activeTrial.value = null;
    }
    message(`${payload.name}: ${payload.outcome.summary}`, !['initialized', 'closed'].includes(payload.outcome.kind));
    if (!payload.video_test || endedMode) queueRetry(profiles.value.find(profile => profile.id === payload.id), payload.outcome, payload.video_test ? 'desktop' : 'connect', endedMode);
  }));
  await run(refresh);
});
onUnmounted(() => unlisten.forEach(stop => stop()));
</script>

<template>
  <div class="app-shell dark">
    <header class="toolbar">
      <h1>Connections <span>{{ profiles.length }}</span></h1>
      <div class="actions">
        <Button label="Refresh" size="small" severity="secondary" outlined data-testid="refresh" :disabled="busy" @click="run(refresh)" />
        <Button label="New connection" size="small" data-testid="new-connection" :disabled="busy" @click="edit()" />
      </div>
    </header>
    <main>
      <DataTable :value="profiles" data-key="id" size="small" striped-rows scrollable scroll-height="flex" data-testid="connections-table">
        <Column field="name" header="Connection" sortable />
        <Column field="host" header="Host" />
        <Column field="user" header="User" />
        <Column header="Graphics"><template #body="{ data }"><span>{{ modeLabel(data) }}</span></template></Column>
        <Column header="Actions" :style="{ width: '20rem' }"><template #body="{ data }">
          <div class="actions" :data-profile-id="data.id">
            <Button label="Connect" size="small" data-testid="connect" :disabled="busy || !!activeTrial" @click="connect(data)" />
            <Button label="Edit" size="small" severity="secondary" outlined data-testid="edit" :disabled="busy" @click="edit(data)" />
            <Button label="Test" size="small" severity="secondary" outlined data-testid="test" :disabled="busy" @click="openTests(data)" />
            <Button label="Troubleshoot" size="small" severity="secondary" outlined data-testid="troubleshoot" :disabled="busy || !!activeTrial" @click="openWizard(data)" />
          </div>
        </template></Column>
        <template #empty>No connections yet. Choose New connection.</template>
      </DataTable>
    </main>
    <footer id="status" role="status" aria-live="polite" :class="{ error }">{{ status }}</footer>

    <Dialog v-model:visible="editorOpen" modal :header="pendingRetry ? 'Review sign-in / trust' : editor.id ? 'Edit connection' : 'New connection'" @hide="pendingRetry = null" class="editor-dialog" :style="{ width: '40rem' }" :breakpoints="{ '700px': '95vw' }" data-testid="editor-dialog">
      <Message v-if="pendingRetry" severity="warn" :closable="false" data-testid="retry-reason">{{ pendingRetry.summary }}<p v-if="pendingRetry.locked">Unlock the account on the server or wait for its lockout policy before testing again. Saving changes will not retry.</p><details v-if="pendingRetry.details"><summary>Diagnostic details</summary><pre>{{ pendingRetry.details }}</pre></details></Message>
      <p v-else-if="error" class="error dialog-status" role="alert">{{ status }}</p>
      <form id="profile-form" class="form-grid" @submit.prevent="save">
        <label>Name<InputText v-model="editor.name" name="name" required maxlength="160" size="small" data-testid="profile-name" /></label>
        <label>Host<InputText v-model="editor.host" name="host" required maxlength="255" size="small" data-testid="profile-host" /></label>
        <label>User<InputText v-model="editor.user" name="user" required maxlength="255" size="small" data-testid="profile-user" placeholder="DOMAIN\username" /></label>
        <label>Password<InputText v-model="password" name="password" type="text" maxlength="4096" autocomplete="new-password" size="small" data-testid="profile-password" /></label>
        <label class="check full"><Checkbox v-model="rememberPassword" binary input-id="remember-password" /><span>Save password in plaintext locally</span></label>
        <label>Certificate<Select v-model="editor.certificate" :options="policies" option-label="label" option-value="value" size="small" name="certificate" data-testid="certificate-policy" /></label>
        <label>Graphics<Select v-model="editor.video_mode" :options="modeOptions" option-label="label" option-value="value" option-disabled="disabled" size="small" name="video_mode" data-testid="graphics-mode" @change="monitors = []" /></label>
        <label v-if="editor.certificate === 'fingerprint'" class="full">SHA-256 fingerprint<InputText v-model="editor.fingerprint" name="fingerprint" required maxlength="95" size="small" data-testid="fingerprint" /></label>
        <div class="actions full">
          <label class="check"><Checkbox v-model="editor.fullscreen" binary input-id="fullscreen" /><span>Full screen</span></label>
          <label class="check"><Checkbox v-model="editor.multi_monitor" binary input-id="multi-monitor" /><span>Multiple monitors</span></label>
        </div>
        <div class="monitor-field full">
          <label>Monitor IDs <span class="muted">(blank uses defaults)</span><InputText v-model="editor.monitors" name="monitors" size="small" data-testid="monitor-ids" /></label>
          <Button label="Detect displays" size="small" severity="secondary" outlined data-testid="detect-displays" :disabled="busy" @click="detect" />
        </div>
        <div v-if="monitors.length" class="monitor-list full">
          <label v-for="monitor in monitors" :key="monitor.id" class="check"><Checkbox v-model="selectedMonitors" :value="String(monitor.id)" :input-id="`monitor-${monitor.id}`" /><span>{{ monitor.label }}</span></label>
        </div>
      </form>
      <template #footer>
        <Button v-if="editor.id" label="Forget password" size="small" severity="secondary" text data-testid="forget-password" :disabled="busy" @click="run(async () => { await invoke('forget_password', { id: editor.id }); password = ''; message('Saved password removed'); })" />
        <Button v-if="editor.id" label="Delete" size="small" severity="danger" outlined data-testid="delete-connection" :disabled="busy" @click="deleteTarget = profiles.find(profile => profile.id === editor.id) || { id: editor.id, name: editor.name }; error = false; deleteOpen = true" />
        <Button label="Cancel" size="small" severity="secondary" outlined data-testid="cancel-editor" @click="pendingRetry = null; editorOpen = false" />
        <Button :label="pendingRetry && !pendingRetry.locked ? 'Save and retry' : 'Save connection'" size="small" type="submit" form="profile-form" data-testid="save-connection" :disabled="busy" />
      </template>
    </Dialog>

    <Dialog v-model:visible="testsOpen" modal :header="selected ? `${guided ? 'Troubleshoot' : 'Test'} · ${selected.name}` : 'Connection tests'" class="test-dialog" :style="{ width: '57rem' }" :breakpoints="{ '1000px': '95vw' }" data-testid="test-dialog">
      <div v-if="current" class="tests-content">
        <p class="dialog-status" :class="{ error }" role="status" data-testid="test-status">{{ status }}</p>
        <nav v-if="guided" class="guide-nav" aria-label="Troubleshooting steps" data-testid="wizard-steps">
          <Button v-for="(step, index) in guideSteps" :key="step" :label="`${index + 1}. ${step}`" size="small" :severity="guideStep === index + 1 ? 'primary' : 'secondary'" :outlined="guideStep !== index + 1" :disabled="busy || !wizardCanEnter(index + 1, current.reachability, trustReviewed, current.auth)" :data-testid="`wizard-step-${index + 1}`" @click="guideStep = index + 1" />
        </nav>
        <section v-if="guided && guideStep === 1" class="guide-stage" data-testid="wizard-server">
          <p>Check whether {{ selected.host }} accepts a TCP connection.</p>
          <div class="actions"><Button label="Check server" size="small" data-testid="check-reachability" :disabled="busy" @click="reachability" /><Button label="Edit server" size="small" severity="secondary" outlined data-testid="wizard-edit-server" :disabled="busy" @click="edit(selected)" /></div>
          <p v-if="current.reachability" data-testid="wizard-reachability-result">{{ current.reachability.summary }}</p>
          <details v-if="current.reachability"><summary>Details</summary><pre>{{ current.reachability.details }}</pre></details>
        </section>
        <section v-if="guided && guideStep === 2" class="guide-stage" data-testid="wizard-trust">
          <p>Certificate: <strong>{{ policies.find(policy => policy.value === selected.certificate)?.label || 'Verify certificate' }}</strong><span v-if="selected.fingerprint"> · {{ selected.fingerprint }}</span></p>
          <div class="form-grid"><label>User<InputText :model-value="selected.user" readonly size="small" /></label><label>Password<InputText :model-value="guidePassword" readonly type="text" size="small" data-testid="wizard-password" /></label></div>
          <p class="hint">Review the username/domain and your certificate policy. Nothing is changed automatically.</p>
          <div class="actions"><Button label="Edit trust / credentials" size="small" severity="secondary" outlined data-testid="wizard-edit" :disabled="busy" @click="edit(selected)" /><Button label="Continue to sign-in" size="small" data-testid="wizard-review-trust" :disabled="busy" @click="reviewTrust" /></div>
        </section>
        <div v-show="!guided || guideStep >= 3" class="actions test-toolbar">
          <Button label="Test all video modes" size="small" data-testid="test-all" :disabled="busy || !!activeTrial || !videoAllowed" :loading="busy" @click="testAll()" />
          <Button label="Test sign-in" size="small" severity="secondary" outlined data-testid="test-authentication" :disabled="busy || !!activeTrial" @click="authenticate()" />
          <Button label="Compare CLI sign-in" size="small" severity="secondary" outlined data-testid="compare-authentication" :disabled="busy || !!activeTrial" @click="authenticate(true)" />
          <Button label="Edit" size="small" severity="secondary" outlined data-testid="test-edit" :disabled="busy" @click="edit(selected)" />
          <Button v-if="!guided" label="Troubleshoot" size="small" severity="secondary" outlined data-testid="test-troubleshoot" :disabled="busy || !!activeTrial" @click="openWizard(selected)" />
        </div>
        <p v-if="!guided || guideStep === 4" class="hint">Tests open each desktop for up to 15 seconds. Only you can confirm its picture. Saved mode: <strong>{{ modeLabel(selected) }}</strong>.</p>
        <div v-for="auth in current.auth" :key="auth.transport" class="auth-result" data-testid="authentication-result">
          <Tag :value="authenticationLabel(auth)" :severity="auth.success ? 'success' : 'danger'" />
          <span>{{ auth.transport }} · {{ auth.summary }} <span v-if="auth.exit_code != null">(exit {{ auth.exit_code }})</span></span>
          <details><summary>Details / command</summary><pre>{{ auth.command }}</pre><Button label="Copy command" size="small" severity="secondary" outlined @click="copyCommand(auth.command)" /><pre>{{ auth.details }}</pre><p v-if="auth.runtime_warning">{{ auth.runtime_warning }}</p></details>
          <div class="actions" v-if="auth.fingerprint || ['certificate', 'certificate_changed'].includes(auth.kind)">
            <Button v-if="auth.fingerprint" label="Pin detected fingerprint" size="small" severity="secondary" outlined data-testid="pin-fingerprint" @click="pinFingerprint(auth)" />
            <Button v-if="['certificate', 'certificate_changed'].includes(auth.kind)" label="Back up remembered certificate" size="small" severity="secondary" outlined data-testid="backup-certificate" @click="certificateBackup = true" />
          </div>
        </div>
        <Message v-if="certificateBackup" severity="warn" :closable="false">Back up only this host's certificate? Choose a trust policy before retesting. <Button label="Confirm backup" size="small" data-testid="confirm-certificate-backup" @click="backupCertificate" /></Message>
        <DataTable v-if="!guided || guideStep === 4" :value="trialRows" :row-class="row => row.trial?.kind === 'testing' ? 'active-video-test' : null" data-key="value" size="small" class="video-table" data-testid="video-results">
          <Column field="label" header="Mode" :style="{ width: '10.5rem' }" />
          <Column header="Result"><template #body="{ data }">
            <span :class="{ testing: data.trial?.kind === 'testing' }" :data-testid="`result-${data.value}`">{{ data.status }}</span>
            <small v-if="data.confirmed" class="confirmed">Previously confirmed</small>
            <small v-if="data.trial?.screenshot" class="capture-status">{{ { content: 'Image has content', blank: 'Blank image', unavailable: 'Not captured' }[data.trial.screenshot.status] || 'Not captured' }}</small>
            <details v-if="data.trial || !data.available" :data-testid="`details-${data.value}`"><summary>Details</summary>
              <p>{{ data.trial?.summary || data.reason }}<span v-if="data.trial?.exit_code != null"> · Exit {{ data.trial.exit_code }}</span></p>
              <p v-if="data.trial?.graphics">{{ data.trial.graphics }}</p><p v-if="data.trial?.screenshot">Screenshot: {{ data.trial.screenshot.summary || data.trial.screenshot.reason }}<span v-if="data.trial.screenshot.path"> · {{ data.trial.screenshot.path }}</span></p>
              <img v-if="data.trial?.screenshot?.data_url" class="capture-preview" :src="data.trial.screenshot.data_url" :alt="`${data.label} captured desktop`" />
              <pre v-if="data.trial?.command">{{ data.trial.command }}</pre>
              <Button v-if="data.trial?.command" label="Copy command" size="small" severity="secondary" outlined @click="copyCommand(data.trial.command)" />
              <pre v-if="data.trial?.details">{{ data.trial.details }}</pre>
            </details>
          </template></Column>
          <Column header="Actions" :style="{ width: '19rem' }"><template #body="{ data }"><div class="actions">
            <Button label="Open desktop" size="small" severity="secondary" outlined :data-testid="`open-${data.value}`" :disabled="busy || !data.available || !!activeTrial || !videoAllowed" :title="data.reason" @click="openDesktop(data)" />
            <Button label="I saw the desktop" size="small" severity="secondary" outlined :data-testid="`confirm-${data.value}`" :disabled="busy || !videoAllowed || !canConfirm(data)" @click="confirmMode(data)" />
          </div></template></Column>
        </DataTable>
        <p v-if="!guided || guideStep === 4" class="hint">Confirmed preference: OpenGL → OpenGL ES → software. Graphics probes and sign-in do not prove a usable picture.</p>
        <details data-testid="session-log"><summary>Live command / log</summary>
          <p class="hint">Commands and exports include your password.</p>
          <pre data-testid="session-command">{{ current.command || 'Connect or open a desktop to capture its command.' }}</pre>
          <Button v-if="current.command" label="Copy command" size="small" severity="secondary" outlined data-testid="copy-command" @click="copyCommand(current.command)" />
          <p>{{ current.graphics }}</p><pre>{{ current.live }}</pre>
        </details>
      </div>
      <template #footer><Button label="Export diagnostics" size="small" severity="secondary" outlined data-testid="export-diagnostics" :disabled="busy" @click="exportDiagnostics" /><Button label="Close" size="small" severity="secondary" outlined @click="testsOpen = false" /></template>
    </Dialog>
    <Dialog v-model:visible="deleteOpen" modal header="Delete connection?" :style="{ width: '28rem' }" data-testid="delete-dialog">
      <p v-if="error" class="error" role="alert">{{ status }}</p>
      <p>Delete <strong>{{ deleteTarget?.name }}</strong> and its saved password?</p>
      <p class="hint">Other connections and remembered server certificates stay unchanged.</p>
      <template #footer><Button label="Cancel" size="small" severity="secondary" outlined data-testid="cancel-delete" @click="deleteOpen = false" /><Button label="Delete connection" size="small" severity="danger" data-testid="confirm-delete" :disabled="busy" @click="removeProfile" /></template>
    </Dialog>
  </div>
</template>
