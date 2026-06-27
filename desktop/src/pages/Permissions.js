const { invoke } = window.__TAURI__.core;
import { show } from '../components/Toast.js';
import { render as renderPermissionItem } from '../components/PermissionItem.js';

export function showPermissions() {
  const container = document.getElementById('page-container');
  renderPermissionsPage(container);
}

async function renderPermissionsPage(container) {
  let permConfig = { allowed_paths: [], writable_paths: [], denied_paths: [], allowed_commands: [], max_file_read_bytes: 10485760, allow_hidden_files: false, allow_remote_tasks: true };
  let agentConfig = { provider: '', model: '' };

  try {
    permConfig = await invoke('agent_get_permissions');
    const status = await invoke('agent_get_status');
    agentConfig = status;
  } catch {}

  container.innerHTML = `
    <h2 class="page-title">Agent Permissions</h2>
    <div class="permissions-page">
      <div class="permissions-section">
        <h3>Allowed Directories</h3>
        <p class="permissions-desc">Directories the agent can read files from.</p>
        <div id="perms-allowed-paths" class="permissions-path-list"></div>
        <button class="btn btn-secondary btn-sm" id="perms-add-path">+ Add Directory</button>
      </div>

      <div class="permissions-section">
        <h3>Writable Directories</h3>
        <p class="permissions-desc">Directories the agent can write files to (artifacts from peers).</p>
        <div id="perms-writable-paths" class="permissions-path-list"></div>
        <button class="btn btn-secondary btn-sm" id="perms-add-writable">+ Add Writable Directory</button>
      </div>

      <div class="permissions-section">
        <h3>Allowed Commands</h3>
        <p class="permissions-desc">Shell commands the agent may execute.</p>
        <div id="perms-commands" class="permissions-command-list"></div>
        <div class="perms-add-command">
          <input type="text" id="perms-new-command" placeholder="command name" />
          <button class="btn btn-secondary btn-sm" id="perms-add-command">Add</button>
        </div>
      </div>

      <div class="permissions-section">
        <h3>File Access</h3>
        <div class="permissions-toggle-row">
          <span>Allow hidden files (dotfiles)</span>
          <label class="permission-toggle">
            <input type="checkbox" id="perms-hidden" ${permConfig.allow_hidden_files ? 'checked' : ''} />
            <span class="toggle-slider"></span>
          </label>
        </div>
        <div class="settings-field">
          <label>Max file read size (bytes)</label>
          <input type="number" id="perms-max-bytes" value="${permConfig.max_file_read_bytes}" min="1024" max="1073741824" />
        </div>
      </div>

      <div class="permissions-section">
        <h3>Task Reception</h3>
        <div class="permissions-toggle-row">
          <span>Accept remote tasks from other peers</span>
          <label class="permission-toggle">
            <input type="checkbox" id="perms-remote" ${permConfig.allow_remote_tasks ? 'checked' : ''} />
            <span class="toggle-slider"></span>
          </label>
        </div>
      </div>

      <div class="permissions-actions">
        <button class="btn btn-primary" id="perms-save">Save Permissions</button>
        <button class="btn btn-secondary" id="perms-back">Back to Settings</button>
      </div>
    </div>
  `;

  renderPathList(container.querySelector('#perms-allowed-paths'), permConfig.allowed_paths, (paths) => {
    permConfig.allowed_paths = paths;
  });

  renderPathList(container.querySelector('#perms-writable-paths'), permConfig.writable_paths, (paths) => {
    permConfig.writable_paths = paths;
  });

  renderCommandList(container.querySelector('#perms-commands'), permConfig.allowed_commands, (cmds) => {
    permConfig.allowed_commands = cmds;
  });

  container.querySelector('#perms-add-path').addEventListener('click', async () => {
    try {
      const { invoke } = window.__TAURI__.core;
      const folder = await invoke('pick_save_folder');
      if (folder && !permConfig.allowed_paths.includes(folder)) {
        permConfig.allowed_paths.push(folder);
        renderPathList(container.querySelector('#perms-allowed-paths'), permConfig.allowed_paths);
      }
    } catch {}
  });

  container.querySelector('#perms-add-writable').addEventListener('click', async () => {
    try {
      const folder = await invoke('pick_save_folder');
      if (folder && !permConfig.writable_paths.includes(folder)) {
        permConfig.writable_paths.push(folder);
        renderPathList(container.querySelector('#perms-writable-paths'), permConfig.writable_paths);
      }
    } catch {}
  });

  container.querySelector('#perms-add-command').addEventListener('click', () => {
    const input = container.querySelector('#perms-new-command');
    const cmd = input.value.trim().toLowerCase();
    if (cmd && !permConfig.allowed_commands.includes(cmd)) {
      permConfig.allowed_commands.push(cmd);
      renderCommandList(container.querySelector('#perms-commands'), permConfig.allowed_commands);
      input.value = '';
    }
  });

  container.querySelector('#perms-save').addEventListener('click', async () => {
    permConfig.allow_hidden_files = container.querySelector('#perms-hidden').checked;
    permConfig.allow_remote_tasks = container.querySelector('#perms-remote').checked;
    permConfig.max_file_read_bytes = parseInt(container.querySelector('#perms-max-bytes').value) || 10485760;

    try {
      await invoke('agent_update_permissions', { config: permConfig });
      show('Permissions saved', 'success');
    } catch (err) {
      show(`Failed to save: ${err}`, 'error');
    }
  });

  container.querySelector('#perms-back').addEventListener('click', async () => {
    const { render } = await import('./Settings.js');
    render(document.getElementById('page-container'));
  });
}

function renderPathList(container, paths, onChange) {
  container.innerHTML = paths.map((p, i) => `
    <div class="permissions-path-item">
      <span>${escapeHtml(p)}</span>
      <button class="btn-icon" data-index="${i}">✕</button>
    </div>
  `).join('');

  container.querySelectorAll('.btn-icon').forEach(btn => {
    btn.addEventListener('click', () => {
      const idx = parseInt(btn.dataset.index);
      paths.splice(idx, 1);
      renderPathList(container, paths, onChange);
      if (onChange) onChange(paths);
    });
  });
}

function renderCommandList(container, commands, onChange) {
  container.innerHTML = commands.map((c, i) => `
    <span class="permissions-command-tag">
      ${escapeHtml(c)}
      <button class="btn-icon" data-index="${i}">✕</button>
    </span>
  `).join('');

  container.querySelectorAll('.btn-icon').forEach(btn => {
    btn.addEventListener('click', () => {
      const idx = parseInt(btn.dataset.index);
      commands.splice(idx, 1);
      renderCommandList(container, commands, onChange);
      if (onChange) onChange(commands);
    });
  });
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}
