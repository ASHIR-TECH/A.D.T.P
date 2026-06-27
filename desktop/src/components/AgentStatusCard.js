const { invoke } = window.__TAURI__.core;
import { show } from './Toast.js';

export async function render(container) {
  let status = { active: false, provider: '', model: '', last_active: null };
  try {
    status = await invoke('agent_get_status');
  } catch {}

  container.innerHTML = `
    <div class="agent-status-card">
      <div class="agent-status-header">
        <h3>Embedded Agent</h3>
        <span class="agent-status-badge ${status.active ? 'active' : 'inactive'}">
          ${status.active ? 'Active' : 'Inactive'}
        </span>
      </div>
      ${status.active ? `
        <div class="agent-status-details">
          <div class="agent-status-field">
            <span class="agent-status-label">Provider</span>
            <span>${status.provider || '-'}</span>
          </div>
          <div class="agent-status-field">
            <span class="agent-status-label">Model</span>
            <span>${status.model || '-'}</span>
          </div>
          <div class="agent-status-field">
            <span class="agent-status-label">Last Active</span>
            <span>${status.last_active ? new Date(status.last_active).toLocaleString() : '-'}</span>
          </div>
        </div>
        <div class="agent-status-actions">
          <button class="btn btn-danger btn-sm" id="agent-deactivate">Deactivate</button>
          <button class="btn btn-secondary btn-sm" id="agent-permissions">Permissions</button>
        </div>
      ` : `
        <p class="agent-status-empty">No agent configured. Add an API key to enable the local AI agent.</p>
        <button class="btn btn-primary" id="agent-setup">Add Agent</button>
      `}
    </div>
  `;

  const setupBtn = container.querySelector('#agent-setup');
  if (setupBtn) {
    setupBtn.addEventListener('click', () => {
      import('../pages/AgentSetup.js').then(mod => mod.showSetup());
    });
  }

  const deactivateBtn = container.querySelector('#agent-deactivate');
  if (deactivateBtn) {
    deactivateBtn.addEventListener('click', async () => {
      try {
        await invoke('agent_deactivate');
        show('Agent deactivated', 'success');
        render(container);
      } catch (err) {
        show(`Failed: ${err}`, 'error');
      }
    });
  }

  const permsBtn = container.querySelector('#agent-permissions');
  if (permsBtn) {
    permsBtn.addEventListener('click', () => {
      import('../pages/Permissions.js').then(mod => mod.showPermissions());
    });
  }
}
