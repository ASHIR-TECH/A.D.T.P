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

  let ownPeerId = '';
  try {
    ownPeerId = await invoke('get_own_peer_id');
  } catch {}

  let natType = 'Unknown';
  try {
    natType = await invoke('get_own_nat_type');
  } catch {}

  let connStatus = null;
  try {
    connStatus = await invoke('get_connection_status');
  } catch {}

  let agentStatus = null;
  try {
    agentStatus = await invoke('get_agent_status');
  } catch {}

  let peers = [];
  try {
    peers = await invoke('list_peers');
  } catch {}

  container.innerHTML = `
    <div class="settings-page">
      <!-- My Peer ID Card -->
      <div class="settings-card">
        <div class="settings-card-header">
          <h3>My Peer ID</h3>
        </div>
        <div class="field-group">
          <label>This computer's unique ID — share this so other PCs can send files to you</label>
          <div class="peer-id-display">
            <code id="own-peer-id">${ownPeerId || 'Initializing...'}</code>
            <button class="btn-toolbar-action" id="copy-peer-id" ${!ownPeerId ? 'disabled' : ''}>Copy</button>
          </div>
        </div>
        <div class="field-group" style="margin-top:var(--space-3)">
          <label>Connection</label>
          <div style="display:flex;gap:var(--space-4);font-size:13px;color:var(--interactive-muted)">
            <span>NAT: <strong style="color:var(--white)">${natType}</strong></span>
            <span>Peers: <strong style="color:var(--white)">${connStatus ? connStatus.connected_peers : '?'}</strong></span>
            <span>Port: <strong style="color:var(--white)">${connStatus ? connStatus.endpoint_port : '?'}</strong></span>
          </div>
        </div>
        ${agentStatus ? `
        <div class="field-group">
          <label>Agent Status</label>
          <div style="display:flex;gap:var(--space-4);font-size:13px;color:var(--interactive-muted)">
            <span>Agents: <strong style="color:var(--white)">${agentStatus.active_count}/${agentStatus.agent_count} active</strong></span>
          </div>
        </div>
        ` : ''}
      </div>

      <!-- Peers Card -->
      <div class="settings-card">
        <div class="settings-card-header">
          <h3>Peers</h3>
          <button class="btn btn-sm btn-primary" id="show-add-peer">+ Add Peer</button>
        </div>
        <div id="add-peer-form" style="display:none;margin-bottom:var(--space-4);padding:var(--space-4);background:var(--bg-deepest);border-radius:var(--radius-md)">
          <div class="field-group">
            <label>Peer ID (hex)</label>
            <input type="text" id="new-peer-id" placeholder="a1b2c3d4..." />
          </div>
          <div class="field-group">
            <label>Host</label>
            <input type="text" id="new-peer-host" placeholder="192.168.1.100 or example.com" />
          </div>
          <div class="field-group">
            <label>Port</label>
            <input type="number" id="new-peer-port" value="40001" />
          </div>
          <div class="field-group">
            <label>Nickname (optional)</label>
            <input type="text" id="new-peer-nickname" placeholder="Home PC" />
          </div>
          <div class="field-group">
            <label>Public Key (hex, optional)</label>
            <input type="text" id="new-peer-pubkey" placeholder="Leave empty if unknown" />
          </div>
          <div style="display:flex;gap:var(--space-2)">
            <button class="btn btn-sm btn-primary" id="save-peer">Save Peer</button>
            <button class="btn btn-sm btn-secondary" id="cancel-add-peer">Cancel</button>
          </div>
        </div>
        <div id="peer-list">
          ${peers.length === 0 ? `
          <div class="settings-empty">
            <div class="settings-empty-title">No peers configured</div>
            <div class="settings-empty-text">Add a peer to start sending files between PCs</div>
          </div>
          ` : peers.map(p => `
          <div class="peer-item" data-peer-id="${escHtml(p.peer_id)}">
            <div class="peer-item-info">
              <div class="peer-item-name">${escHtml(p.nickname || p.peer_id.slice(0, 16))}</div>
              <div class="peer-item-detail">${escHtml(p.peer_id)} · ${escHtml(p.host)}:${p.port} · ${p.trust_level}</div>
            </div>
            <button class="peer-remove-btn" title="Remove peer">&times;</button>
          </div>
          `).join('')}
        </div>
      </div>

      <!-- CLI Card -->
      <div class="settings-card">
        <div class="settings-card-header">
          <h3>CLI / Terminal</h3>
        </div>
        <div class="field-group">
          <label>ADTP Desktop bundles a full CLI — use it for scripting, automation, or power-user workflows</label>
          <div style="display:flex;gap:var(--space-2);margin-top:var(--space-3)">
            <button class="btn btn-sm btn-primary" id="launch-cli">Open Terminal (CLI)</button>
          </div>
          <div style="margin-top:var(--space-4);font-size:12px;color:var(--interactive-muted)">
            <strong>Quick reference:</strong>
            <code style="display:block;margin-top:var(--space-1);padding:var(--space-2);background:var(--bg-deepest);border-radius:var(--radius-sm)">
adtp send &lt;file&gt;        &mdash; send a file
adtp status              &mdash; check transfer status
adtp peer list           &mdash; list peers
adtp agent ask "..."     &mdash; ask the AI agent
adtp desktop             &mdash; launch this desktop app from CLI
adtp                     &mdash; enter REPL mode
            </code>
          </div>
        </div>
      </div>

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
          <button class="btn-toolbar-action" id="test-connection">Test Connection</button>
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

  container.querySelector('#show-add-peer')?.addEventListener('click', () => {
    const form = container.querySelector('#add-peer-form');
    form.style.display = form.style.display === 'none' ? 'block' : 'none';
  });

  container.querySelector('#cancel-add-peer')?.addEventListener('click', () => {
    container.querySelector('#add-peer-form').style.display = 'none';
  });

  container.querySelector('#save-peer')?.addEventListener('click', async () => {
    const peerId = container.querySelector('#new-peer-id').value.trim();
    const host = container.querySelector('#new-peer-host').value.trim();
    const port = parseInt(container.querySelector('#new-peer-port').value) || 40001;
    const nickname = container.querySelector('#new-peer-nickname').value.trim() || null;
    const pubkey = container.querySelector('#new-peer-pubkey').value.trim() || '0000000000000000000000000000000000000000000000000000000000000000';
    if (!peerId || !host) {
      show('Peer ID and Host are required', 'warning');
      return;
    }
    try {
      await invoke('add_peer', { peerId, publicKey: pubkey, host, port, nickname, trustLevel: 'unverified' });
      show('Peer added', 'success', 'top-right');
      container.querySelector('#add-peer-form').style.display = 'none';
      container.querySelector('#new-peer-id').value = '';
      container.querySelector('#new-peer-host').value = '';
      container.querySelector('#new-peer-port').value = '40001';
      container.querySelector('#new-peer-nickname').value = '';
      container.querySelector('#new-peer-pubkey').value = '';
      render(container);
    } catch (err) {
      show(`Failed to add peer: ${err}`, 'error');
    }
  });

  container.querySelectorAll('.peer-remove-btn').forEach(btn => {
    btn.addEventListener('click', async () => {
      const item = btn.closest('.peer-item');
      const peerId = item.dataset.peerId;
      const name = item.querySelector('.peer-item-name')?.textContent || peerId;
      if (!confirm(`Remove peer "${name}"?`)) return;
      try {
        await invoke('remove_peer', { peerId });
        show(`Peer "${name}" removed`, 'success', 'top-right');
        item.remove();
      } catch (err) {
        show(`Failed to remove peer: ${err}`, 'error');
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

  container.querySelector('#launch-cli')?.addEventListener('click', async () => {
    try {
      const homeDir = await invoke('get_home_dir');
      const cliPath = await invoke('launch_cli');
      show(`CLI launched — check your terminal`, 'success', 'top-right');
    } catch (err) {
      show(`Couldn't open terminal: ${err}`, 'error', 'top-right');
    }
  });

  container.querySelector('#copy-peer-id')?.addEventListener('click', async () => {
    const peerId = container.querySelector('#own-peer-id')?.textContent;
    if (peerId) {
      try {
        await navigator.clipboard.writeText(peerId);
        show('Peer ID copied to clipboard', 'success', 'top-right');
      } catch {
        show('Failed to copy', 'error', 'top-right');
      }
    }
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
