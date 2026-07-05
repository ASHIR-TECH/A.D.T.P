const { invoke } = window.__TAURI__.core;
import { show } from '../components/Toast.js';
import { showBanner } from '../components/Banner.js';
import { render as renderAgentStatus } from '../components/AgentStatusCard.js';

let advancedOpen = false;

export async function render(container) {
  let settings;
  try {
    settings = await invoke('load_settings');
  } catch {
    settings = { api_url: 'http://127.0.0.1:40003', poll_transfers_ms: 2000, poll_health_ms: 5000, minimize_to_tray: true, theme: 'dark', agents: [], peers: [] };
  }

  let agents = [];
  try {
    agents = await invoke('agent_list');
  } catch {}

  container.innerHTML = `
    <div class="settings-page">
      <!-- Connection Card -->
      <div class="settings-card">
        <div class="settings-card-header">
          <h3>Connection</h3>
        </div>
        <div class="field-group">
          <label>API URL</label>
          <input type="text" id="api-url" value="${settings.api_url}" />
        </div>
        <div style="display:flex;gap:var(--space-2)">
          <button class="btn btn-sm btn-secondary" id="test-connection">Test Connection</button>
        </div>
      </div>

      <!-- Authentication Card -->
      <div class="settings-card">
        <div class="settings-card-header">
          <h3>Authentication</h3>
        </div>
        <div class="field-group">
          <label>API Token (write-only)</label>
          <input type="password" id="api-token" placeholder="Enter new token..." />
        </div>
        <button class="btn btn-sm btn-primary" id="save-token">Save Token</button>
      </div>

      <!-- Agents Card -->
      <div class="settings-card">
        <div class="settings-card-header">
          <h3>Agents</h3>
          <button class="btn btn-sm btn-primary" id="add-agent">+ Add Agent</button>
        </div>
        <div id="agent-list">
          ${agents.length === 0 ? `
          <div class="settings-empty">
            <div class="settings-empty-icon"></div>
            <div class="settings-empty-title">No agents configured</div>
            <div class="settings-empty-text">Click 'Add Agent' to add your first one</div>
          </div>
          ` : agents.map(a => `
          <div class="agent-item" data-id="${a.id}">
            <img class="agent-item-icon" src="${providerIcon(a.provider)}" alt="${escHtml(a.provider)}" />
            <div class="agent-item-info">
              <div class="agent-item-name">${escHtml(a.name)}</div>
              <div class="agent-item-detail">${escHtml(a.provider)} · ${escHtml(a.model)}${a.ollama_host ? ' · ' + escHtml(a.ollama_host) : ''}</div>
            </div>
            <label class="agent-toggle">
              <input type="checkbox" class="agent-toggle-input" ${a.enabled ? 'checked' : ''} />
              <span class="toggle-slider"></span>
            </label>
            <button class="agent-remove-btn" title="Remove agent">&times;</button>
          </div>
          `).join('')}
        </div>
      </div>

      <!-- Embedded Agent Card -->
      <div class="settings-card" id="embedded-agent-section">
        <div id="embedded-agent-card"></div>
      </div>

      <!-- Advanced Settings -->
      <div class="advanced-link" id="advanced-toggle">
        <span>Advanced Settings</span>
        <span></span>
      </div>
      <div id="advanced-content" style="display:${advancedOpen ? 'block' : 'none'};padding:var(--space-4);background:var(--surface-primary);border-radius:var(--radius-lg);margin-top:var(--space-2)">
        <div class="field-group">
          <label>Transfer Poll Interval (ms)</label>
          <input type="number" id="poll-transfers" value="${settings.poll_transfers_ms || 2000}" />
        </div>
        <div class="field-group">
          <label>Health Poll Interval (ms)</label>
          <input type="number" id="poll-health" value="${settings.poll_health_ms || 5000}" />
        </div>
        <div class="field-group">
          <label>
            <input type="checkbox" id="minimize-tray" ${settings.minimize_to_tray !== false ? 'checked' : ''} />
            Minimize to Tray
          </label>
        </div>
      </div>
    </div>
  `;

  renderAgentStatus(container.querySelector('#embedded-agent-card'));

  setupEventHandlers(container, settings);
}

function escHtml(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

function providerIcon(provider) {
  const p = provider.toLowerCase();
  if (p.includes('openai') || p.includes('gpt')) return 'icons/chatgpt.jpg';
  if (p.includes('anthropic') || p.includes('claude')) return 'icons/claude-color.svg';
  if (p.includes('ollama')) return 'icons/ollama.svg';
  return 'icons/openai-color.svg';
}

function setupEventHandlers(container, settings) {
  const urlInput = container.querySelector('#api-url');
  urlInput?.addEventListener('change', () => saveSettings(container));

  const tokenInput = container.querySelector('#api-token');
  tokenInput?.addEventListener('change', async () => {
    const token = tokenInput.value.trim();
    if (token.length >= 32) {
      try {
        await invoke('save_api_token', { token });
        show('Token saved to keychain', 'success');
        tokenInput.value = '';
      } catch (err) { show(`Failed: ${err}`, 'error'); }
    } else if (token.length > 0) {
      show('Token must be at least 32 characters', 'warning');
    }
  });

  container.querySelector('#save-token')?.addEventListener('click', () => {
    tokenInput?.dispatchEvent(new Event('change'));
  });

  container.querySelector('#test-connection')?.addEventListener('click', async () => {
    try {
      const health = await invoke('get_app_health');
      show(`Connected — ${health?.daemon_connected ? 'online' : 'offline'}`, health?.daemon_connected ? 'success' : 'warning');
    } catch (err) {
      show(`Connection failed: ${err}`, 'error');
    }
  });

  container.querySelector('#add-agent')?.addEventListener('click', () => {
    import('./AgentSetup.js').then(mod => mod.showSetup());
  });

  container.querySelectorAll('.agent-toggle-input').forEach(toggle => {
    toggle.addEventListener('change', async () => {
      const item = toggle.closest('.agent-item');
      const id = item.dataset.id;
      try {
        await invoke('agent_toggle', { id, enabled: toggle.checked });
        show(toggle.checked ? 'Agent enabled' : 'Agent disabled', 'success', 'top-right');
        window.__TAURI__.event.emit('agent-status', { state: toggle.checked ? 'active' : 'inactive' }).catch(() => {});
      } catch (err) {
        show(`Failed: ${err}`, 'error', 'top-right');
        toggle.checked = !toggle.checked;
      }
    });
  });

  container.querySelectorAll('.agent-remove-btn').forEach(btn => {
    btn.addEventListener('click', async () => {
      const item = btn.closest('.agent-item');
      const id = item.dataset.id;
      const name = item.querySelector('.agent-item-name')?.textContent || id;
      if (!confirm(`Remove agent "${name}"?`)) return;
      try {
        await invoke('agent_remove', { id });
        show(`Agent "${name}" removed`, 'success', 'top-right');
        item.remove();
      } catch (err) {
        show(`Failed: ${err}`, 'error', 'top-right');
      }
    });
  });

  container.querySelector('#advanced-toggle')?.addEventListener('click', () => {
    advancedOpen = !advancedOpen;
    const content = container.querySelector('#advanced-content');
    const toggle = container.querySelector('#advanced-toggle');
    content.style.display = advancedOpen ? 'block' : 'none';
    toggle.style.animation = advancedOpen ? 'none' : '';
    if (advancedOpen) saveSettings(container);
  });

  ['poll-transfers', 'poll-health', 'minimize-tray'].forEach(id => {
    container.querySelector(`#${id}`)?.addEventListener('change', () => saveSettings(container));
  });

}

function saveSettings(container) {
  const s = {
    api_url: container.querySelector('#api-url')?.value || 'http://127.0.0.1:40003',
    poll_transfers_ms: parseInt(container.querySelector('#poll-transfers')?.value) || 2000,
    poll_health_ms: parseInt(container.querySelector('#poll-health')?.value) || 5000,
    minimize_to_tray: container.querySelector('#minimize-tray')?.checked !== false,
    theme: 'dark',
    agents: [],
    peers: [],
  };
  invoke('save_settings', { new_settings: s }).then(() => {}).catch(() => {});
}
