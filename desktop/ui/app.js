const status = document.querySelector('#status');
const connections = document.querySelector('#connections');
const form = document.querySelector('#profile-form');
const tauri = window.__TAURI__;

function message(text, error = false) {
  status.textContent = text;
  status.classList.toggle('error', error);
}

function element(tag, text, className) {
  const node = document.createElement(tag);
  node.textContent = text;
  if (className) node.className = className;
  return node;
}

function button(text, action, className) {
  const node = element('button', text, className);
  node.type = 'button';
  node.addEventListener('click', action);
  return node;
}

function field(text, input) {
  const label = element('label', text);
  label.append(input);
  return label;
}

async function addConnectionActions(card, profile) {
  const options = element('details');
  options.append(element('summary', 'Password and certificate settings'));
  const editor = element('form');
  const password = document.createElement('input');
  password.type = 'password';
  password.name = 'password';
  password.autocomplete = 'new-password';
  password.maxLength = 4096;
  const passwordStatus = element('p', '', 'hint');
  async function refreshPasswordStatus() {
    const saved = await tauri.core.invoke('password_saved', { id: profile.id });
    passwordStatus.textContent = saved ? 'Plaintext password saved. Leave blank to use it.' : 'No saved password. Enter one to test; FreeRDP can prompt when connecting.';
  }
  await refreshPasswordStatus();
  editor.append(field('Password', password), passwordStatus);
  const remember = document.createElement('input');
  remember.type = 'checkbox';
  remember.checked = true;
  const rememberLabel = element('label', '', 'check');
  rememberLabel.append(remember, document.createTextNode('Save entered password in plaintext locally'));
  editor.append(rememberLabel);
  const policies = {
    verify: ['Verify certificate', 'Require a trusted certificate and matching host name.'],
    tofu: ['Trust on first use', 'Remember the first certificate, including during a test; reject later changes.'],
    fingerprint: ['Pin SHA-256 fingerprint', 'Accept only the certificate with this SHA-256 fingerprint.'],
    ignore: ['Ignore certificate checks', 'Accept any server certificate for this connection.'],
  };
  const policy = document.createElement('select');
  policy.name = 'certificate';
  for (const [value, [text]] of Object.entries(policies)) {
    const option = element('option', text);
    option.value = value;
    policy.append(option);
  }
  policy.value = profile.certificate || 'verify';
  const hint = element('p', '', 'hint');
  const fingerprint = document.createElement('input');
  fingerprint.name = 'fingerprint';
  fingerprint.maxLength = 95;
  fingerprint.value = profile.fingerprint || '';
  const fingerprintLabel = field('SHA-256 fingerprint', fingerprint);
  function showPolicy() {
    hint.textContent = policies[policy.value][1];
    fingerprintLabel.hidden = policy.value !== 'fingerprint';
    fingerprint.required = !fingerprintLabel.hidden;
  }
  policy.addEventListener('change', showPolicy);
  showPolicy();
  editor.append(field('Server certificate', policy), hint, fingerprintLabel);
  const save = element('button', 'Save settings');
  save.type = 'submit';
  const forget = button('Forget saved password', async () => {
    try {
      await tauri.core.invoke('forget_password', { id: profile.id });
      password.value = '';
      await refreshPasswordStatus();
      message(`Forgot the saved password for ${profile.name}.`);
    } catch (error) { message(String(error), true); }
  });
  const editActions = element('div', '', 'actions');
  editActions.append(save, forget);
  editor.append(editActions);
  options.append(editor);
  async function applySettings() {
    const entered = password.value || null;
    profile = await tauri.core.invoke('update_profile', { profile: { ...profile, certificate: policy.value, fingerprint: fingerprint.value } });
    if (entered && remember.checked) {
      await tauri.core.invoke('save_password', { id: profile.id, password: entered });
      await refreshPasswordStatus();
    }
    return entered;
  }
  editor.addEventListener('submit', async (event) => {
    event.preventDefault();
    save.disabled = true;
    try {
      await applySettings();
      password.value = '';
      message(`Saved settings for ${profile.name}.`);
    } catch (error) { message(String(error), true); }
    finally { save.disabled = false; }
  });
  const result = element('div', '', 'test-result');
  async function run(testing) {
    connect.disabled = test.disabled = true;
    result.replaceChildren();
    try {
      const entered = await applySettings();
      if (testing) {
        message(`Testing ${profile.name}…`);
        const outcome = await tauri.core.invoke('test_profile', { id: profile.id, password: entered });
        result.classList.toggle('error', !outcome.success);
        result.append(element('p', outcome.summary));
        if (outcome.runtime_warning) result.append(element('p', outcome.runtime_warning));
        if (outcome.fingerprint) {
          result.append(element('p', `Server SHA-256: ${outcome.fingerprint}`));
          result.append(button('Use detected fingerprint', () => {
            policy.value = 'fingerprint';
            fingerprint.value = outcome.fingerprint;
            showPolicy();
            options.open = true;
            message('Compare this fingerprint with your server, then save settings or test again.');
          }));
        }
        if (['certificate', 'certificate_changed'].includes(outcome.kind)) {
          options.open = true;
          const reset = button('Back up remembered certificate…', () => {
            reset.disabled = true;
            result.append(element('p', 'Move only this host’s certificate to a backup. Choose trust on first use or a verified fingerprint before testing again.', 'hint'));
            const confirm = button('Confirm certificate backup', async () => {
              confirm.disabled = true;
              try {
                const found = await tauri.core.invoke('forget_certificate', { id: profile.id });
                message(found ? 'Certificate backed up. Choose your policy and test again.' : 'No remembered certificate exists for this host and port. Choose your certificate policy.');
                confirm.remove();
              } catch (error) { message(String(error), true); confirm.disabled = false; }
            });
            result.append(confirm);
          });
          result.append(reset);
        }
        const log = element('details');
        log.append(element('summary', 'Test details'), element('pre', outcome.details));
        result.append(log);
        message(`${profile.name}: ${outcome.summary}`, !outcome.success);
      } else {
        await tauri.core.invoke('launch_profile', { id: profile.id, password: entered });
        message(`Launched ${profile.name}. Continue in the FreeRDP window.`);
      }
      password.value = '';
    } catch (error) { options.open = true; message(String(error), true); }
    finally { connect.disabled = test.disabled = false; }
  }
  const connect = button('Connect', () => run(false), 'primary');
  const test = button('Test connection', () => run(true));
  const actions = element('div', '', 'actions');
  actions.append(connect, test);
  card.append(actions, options, result);
}

async function refresh() {
  try {
    const profiles = await tauri.core.invoke('list_profiles');
    connections.replaceChildren();
    if (!profiles.length) {
      connections.append(element('h2', 'Ready for your first connection'), element('p', 'Save a host and username to get started.', 'hint'));
    }
    for (const profile of profiles) {
      const card = element('article', '', 'connection');
      card.append(element('h2', profile.name), element('p', profile.host), element('p', profile.user, 'hint'));
      const settings = [profile.fullscreen ? 'Full screen' : 'Windowed', profile.multi_monitor ? 'Multiple monitors' : 'Single monitor'];
      if (profile.monitors) settings.push(`Monitors ${profile.monitors}`);
      card.append(element('p', settings.join(' · '), 'hint'));
      await addConnectionActions(card, profile);
      connections.append(card);
    }
    message(`${profiles.length} saved connection${profiles.length === 1 ? '' : 's'}.`);
  } catch (error) {
    message(String(error), true);
  }
}

form.elements.certificate.addEventListener('change', () => {
  const visible = form.elements.certificate.value === 'fingerprint';
  document.querySelector('#new-fingerprint').hidden = !visible;
  form.elements.fingerprint.required = visible;
});
form.addEventListener('submit', async (event) => {
  event.preventDefault();
  const values = new FormData(form);
  const button = form.querySelector('button[type=submit]');
  button.disabled = true;
  try {
    const profile = await tauri.core.invoke('create_profile', { profile: {
      id: '', name: values.get('name'), host: values.get('host'), user: values.get('user'),
      fullscreen: values.has('fullscreen'), multi_monitor: values.has('multi_monitor'), monitors: values.getAll('monitor').join(','),
      certificate: values.get('certificate'), fingerprint: values.get('fingerprint'),
    } });
    if (values.get('password') && values.has('remember_password')) {
      try {
        await tauri.core.invoke('save_password', { id: profile.id, password: values.get('password') });
      } catch (error) {
        await refresh();
        message(`Saved ${profile.name}, but could not save its password: ${error}`, true);
        return;
      }
    }
    form.reset();
    form.elements.certificate.dispatchEvent(new Event('change'));
    await refresh();
    message(`Saved ${profile.name}.`);
  } catch (error) {
    message(String(error), true);
  } finally {
    button.disabled = false;
  }
});

document.querySelector('#refresh').addEventListener('click', refresh);
document.querySelector('#detect-monitors').addEventListener('click', async (event) => {
  const button = event.currentTarget;
  button.disabled = true;
  try {
    const monitors = await tauri.core.invoke('list_monitors');
    const list = document.querySelector('#monitors');
    list.replaceChildren();
    for (const monitor of monitors) {
      const label = element('label', '', 'check');
      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.name = 'monitor';
      checkbox.value = String(monitor.id);
      label.append(checkbox, document.createTextNode(monitor.label));
      list.append(label);
    }
    message(`Found ${monitors.length} display${monitors.length === 1 ? '' : 's'}.`);
  } catch (error) {
    message(String(error), true);
  } finally {
    button.disabled = false;
  }
});
if (tauri) {
  await tauri.event.listen('session-ended', ({ payload }) => {
    message(payload.success ? `${payload.name} closed.` : `${payload.name} exited with ${payload.code ?? 'no exit code'}. Use Test connection for diagnostics.`, !payload.success);
  });
  await refresh();
} else {
  form.querySelector('button[type=submit]').disabled = true;
  message('Open this interface through the rdpctl desktop application.', true);
}
