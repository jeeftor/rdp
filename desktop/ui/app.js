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
      const connect = element('button', 'Connect', 'primary');
      connect.type = 'button';
      connect.addEventListener('click', async () => {
        connect.disabled = true;
        try {
          await tauri.core.invoke('launch_profile', { id: profile.id });
          message(`Launched ${profile.name}. Continue in the FreeRDP window.`);
        } catch (error) {
          message(String(error), true);
        } finally {
          connect.disabled = false;
        }
      });
      card.append(connect);
      connections.append(card);
    }
    message(`${profiles.length} saved connection${profiles.length === 1 ? '' : 's'}.`);
  } catch (error) {
    message(String(error), true);
  }
}

form.addEventListener('submit', async (event) => {
  event.preventDefault();
  const values = new FormData(form);
  const button = form.querySelector('button');
  button.disabled = true;
  try {
    const profile = await tauri.core.invoke('create_profile', { profile: {
      id: '', name: values.get('name'), host: values.get('host'), user: values.get('user'),
      fullscreen: values.has('fullscreen'), multi_monitor: values.has('multi_monitor'), monitors: values.getAll('monitor').join(','),
    } });
    form.reset();
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
    message(payload.success ? `${payload.name} closed.` : `${payload.name} exited with ${payload.code ?? 'no exit code'}. Check the FreeRDP window or terminal for details.`, !payload.success);
  });
  await refresh();
} else {
  form.querySelector('button').disabled = true;
  message('Open this interface through the rdpctl desktop application.', true);
}
