const { invoke } = window.__TAURI__.core;

import { show } from '../components/Toast.js';
import { renderCard as renderAgentCard } from '../components/AgentCard.js';
import { renderCard as renderPeerCard } from '../components/PeerCard.js';
import { render as renderAgentStatus } from '../components/AgentStatusCard.js';

let agents = [];
let peers = [];
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
    renderAgentCard(list, agent, i, (idx, updated) => {
      agents[idx] = updated;
      save(getSettings(container));
    }, (idx) => {
      agents.splice(idx, 1);
      renderAgents(container);
      save(getSettings(container));
    });
  });
}

function renderPeers(container) {
  const list = container.querySelector('#peer-list');
  list.innerHTML = '';
  peers.forEach((peer, i) => {
    renderPeerCard(list, peer, i, (idx, updated) => {
      peers[idx] = updated;
      save(getSettings(container));
    }, (idx) => {
      peers.splice(idx, 1);
      renderPeers(container);
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
    peers,
  };
}

export async function render(container) {
  let settings;
  try {
    settings = await invoke('load_settings');
  } catch {
    settings = { api_url: 'http://127.0.0.1:40003', poll_transfers_ms: 2000, poll_health_ms: 5000, minimize_to_tray: true, theme: 'dark', agents: [], peers: [] };
  }

  agents = (settings.agents || []).map(a => ({ ...a }));
  peers = (settings.peers || []).map(p => ({ ...p }));

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
      <h3>Network Agents</h3>
      <div id="agent-list"></div>
      <button class="btn btn-secondary" id="add-agent">+ Add Agent</button>
    </div>

    <div class="settings-section" id="embedded-agent-section">
      <h3>Embedded Agent</h3>
      <div id="embedded-agent-card"></div>
    </div>

    <div class="settings-section">
      <h3>Peers</h3>
      <div id="peer-list"></div>
      <button class="btn btn-secondary" id="add-peer">+ Add Peer</button>
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
  renderPeers(container);
  renderAgentStatus(container.querySelector('#embedded-agent-card'));

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

  container.querySelector('#add-peer').addEventListener('click', () => {
    peers.push({ peer_id: '', nickname: '', host: '', port: 0, trust_level: 'Unverified' });
    renderPeers(container);
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
