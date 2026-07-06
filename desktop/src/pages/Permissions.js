const { invoke } = window.__TAURI__.core;
import { show } from '../components/Toast.js';
import { Tabs } from '../components/Tabs.js';

export function showPermissions() {
  const container = document.getElementById('page-container');
  renderPermissionsPage(container);
}

async function renderPermissionsPage(container) {
  let permConfig = { allowed_paths: [], writable_paths: [], denied_paths: [], allowed_commands: [], max_file_read_bytes: 10485760, allow_hidden_files: false, allow_remote_tasks: true };
  try { permConfig = await invoke('agent_get_permissions'); } catch {}

  container.innerHTML = `
    <h2 class="page-title gold-text">Agent Permissions</h2>
    <div id="perms-tabs"></div>
  `;

  const tabsContainer = container.querySelector('#perms-tabs');
  const tabs = [
    { id: 'paths', label: 'Allowed Paths', content: renderPathsTab(permConfig) },
    { id: 'commands', label: 'Commands', content: renderCommandsTab(permConfig) },
    { id: 'access', label: 'Access', content: renderAccessTab(permConfig) },
  ];

  new Tabs(tabsContainer, tabs, 'paths', (tabId) => {
    const tab = tabs.find(t => t.id === tabId);
    if (tab) tab.content = getTabContent(tabId, permConfig);
  });
}

function renderPathsTab(config) {
  return `
    <div class="permissions-section">
      <h3>Allowed Directories</h3>
      <p class="permissions-desc">Directories the agent can read from.</p>
      <div id="perms-allowed-paths" class="permissions-path-list">
        ${config.allowed_paths.map(p => `
          <div class="permissions-path-item"><span>${escapeHtml(p)}</span><button class="btn-icon" data-type="allowed" data-path="${escapeHtml(p)}"></button></div>
        `).join('')}
      </div>
      <button class="btn btn-sm btn-secondary" id="perms-add-path">+ Add Directory</button>
    </div>
    <div class="permissions-section">
      <h3>Writable Directories</h3>
      <p class="permissions-desc">Directories the agent can write artifacts to.</p>
      <div id="perms-writable-paths" class="permissions-path-list">
        ${config.writable_paths.map(p => `
          <div class="permissions-path-item"><span>${escapeHtml(p)}</span><button class="btn-icon" data-type="writable" data-path="${escapeHtml(p)}"></button></div>
        `).join('')}
      </div>
      <button class="btn btn-sm btn-secondary" id="perms-add-writable">+ Add Writable Directory</button>
    </div>
    <button class="btn btn-primary" id="perms-save-paths">Save</button>
  `;
}

function renderCommandsTab(config) {
  return `
    <div class="permissions-section">
      <h3>Allowed Commands</h3>
      <p class="permissions-desc">Shell commands the agent may execute.</p>
      <div id="perms-commands" class="permissions-command-list">
        ${config.allowed_commands.map(c => `
          <span class="permissions-command-tag">${escapeHtml(c)}<button class="btn-icon" data-cmd="${escapeHtml(c)}"></button></span>
        `).join('')}
      </div>
      <div class="perms-add-command">
        <input type="text" id="perms-new-command" placeholder="command name" />
        <button class="btn btn-sm btn-secondary" id="perms-add-command">Add</button>
      </div>
    </div>
    <button class="btn btn-primary" id="perms-save-commands">Save</button>
  `;
}

function renderAccessTab(config) {
  return `
    <div class="permissions-section">
      <h3>File Access</h3>
      <div class="permissions-toggle-row">
        <span>Allow hidden files (dotfiles)</span>
        <label class="permission-toggle"><input type="checkbox" id="perms-hidden" ${config.allow_hidden_files ? 'checked' : ''} /><span class="toggle-slider"></span></label>
      </div>
      <div class="field-group">
        <label>Max file read size (bytes)</label>
        <input type="number" id="perms-max-bytes" value="${config.max_file_read_bytes}" min="1024" max="1073741824" />
      </div>
    </div>
    <div class="permissions-section">
      <h3>Task Reception</h3>
      <div class="permissions-toggle-row">
        <span>Accept remote tasks from other peers</span>
        <label class="permission-toggle"><input type="checkbox" id="perms-remote" ${config.allow_remote_tasks ? 'checked' : ''} /><span class="toggle-slider"></span></label>
      </div>
    </div>
    <button class="btn btn-primary" id="perms-save-access">Save</button>
  `;
}

function escapeHtml(t) { const d = document.createElement('div'); d.textContent = t; return d.innerHTML; }
