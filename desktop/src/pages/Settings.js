const { invoke } = window.__TAURI__.core;

import { show } from '../components/Toast.js';
import { renderCard } from '../components/AgentCard.js';

let agents = [];
let advancedOpen = false;

async function save(settings) {
  try {
    await invoke('save_settings', { new_settings: settings });
    show('Settings saved', 'success');
  } catch (err) {
    show(`Failed to save: ${err}`, 'error');
  }
}

function renderAgents(container) {
  const list = container.querySelector('#agent-list');
  list.innerHTML = '';
  agents.forEach((agent, i) => {
    renderCard(list, agent, i, (idx, updated) => {
      agents[idx] = updated;
      save(getSettings(container));
    }, (idx) => {
      agents.splice(idx, 1);
      renderAgents(container);
      save(getSettings(container));
    });
  });
}

function getSettings(container) {
  return {
    api_url: container.querySelector('#api-url').value,
    poll_transfers_ms: parseInt(container.querySelector('#poll-transfers').value) || 2000,
    poll_health_ms: parseInt(container.querySelector('#poll-health').value) || 5000,
    minimize_to_tray: container.querySelector('#minimize-tray').checked,
    theme: 'dark',
    agents,
  };
}

export async function render(container) {
  let settings;
  try {
    settings = await invoke('load_settings');
  } catch {
    settings = { api_url: 'http://127.0.0.1:40003', poll_transfers_ms: 2000, poll_health_ms: 5000, minimize_to_tray: true, theme: 'dark', agents: [] };
  }

  agents = (settings.agents || []).map(a => ({ ...a }));

  container.innerHTML = `
    <h2 class="page-title">Settings</h2>
    <div class="settings-section">
      <h3>Connection</h3>
      <div class="settings-field">
        <label>API URL</label>
        <input type="text" id="api-url" value="${settings.api_url}" />
      </div>
      <div class="settings-field">
        <label>API Token (write-only, saved to keychain)</label>
        <input type="password" id="api-token" placeholder="Enter new token..." />
      </div>
    </div>

    <div class="settings-section">
      <h3>Agents</h3>
      <div id="agent-list"></div>
      <button class="btn btn-secondary" id="add-agent">+ Add Agent</button>
    </div>

    <div class="settings-section">
      <button class="advanced-toggle" id="advanced-toggle">⚙ Advanced Settings</button>
      <div class="advanced-content ${advancedOpen ? 'open' : ''}" id="advanced-content">
        <div class="settings-field">
          <label>Transfer Poll Interval (ms)</label>
          <input type="number" id="poll-transfers" value="${settings.poll_transfers_ms || 2000}" />
        </div>
        <div class="settings-field">
          <label>Health Poll Interval (ms)</label>
          <input type="number" id="poll-health" value="${settings.poll_health_ms || 5000}" />
        </div>
        <div class="settings-field">
          <label>Minimize to Tray</label>
          <input type="checkbox" id="minimize-tray" ${settings.minimize_to_tray !== false ? 'checked' : ''} />
        </div>
      </div>
    </div>
  `;

  renderAgents(container);

  const tokenInput = container.querySelector('#api-token');
  tokenInput.addEventListener('change', async () => {
    const token = tokenInput.value.trim();
    if (token.length >= 32) {
      try {
        await invoke('save_api_token', { token });
        show('Token saved to keychain', 'success');
        tokenInput.value = '';
      } catch (err) {
        show(`Failed to save token: ${err}`, 'error');
      }
    } else if (token.length > 0) {
      show('Token must be at least 32 characters', 'warning');
    }
  });

  const urlInput = container.querySelector('#api-url');
  urlInput.addEventListener('change', () => save(getSettings(container)));

  container.querySelector('#add-agent').addEventListener('click', () => {
    agents.push({ name: '', host: '', port: 0, default: false });
    renderAgents(container);
  });

  container.querySelector('#advanced-toggle').addEventListener('click', () => {
    advancedOpen = !advancedOpen;
    const content = container.querySelector('#advanced-content');
    content.classList.toggle('open');
  });

  ['poll-transfers', 'poll-health', 'minimize-tray'].forEach(id => {
    const el = container.querySelector(`#${id}`);
    if (el) el.addEventListener('change', () => save(getSettings(container)));
  });
}
