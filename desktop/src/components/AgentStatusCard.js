const { invoke } = window.__TAURI__.core;
import { show } from './Toast.js';

export async function render(container) {
  let agents = [];
  try {
    agents = await invoke('agent_list');
  } catch {}

  const enabled = agents.filter(a => a.enabled);
  const hasAgent = enabled.length > 0;

  container.innerHTML = `
    <div class="agent-status-card">
      <div class="agent-status-header">
        <h3>Embedded Agent</h3>
        <span class="agent-status-badge ${hasAgent ? 'active' : 'inactive'}">
          ${hasAgent ? `${enabled.length} Active` : 'Inactive'}
        </span>
      </div>
      ${hasAgent ? `
        <div class="agent-status-list">
          ${enabled.map(a => `
            <div class="agent-status-field">
              <span class="agent-status-label">${a.name}</span>
              <span>${a.provider} · ${a.model}</span>
            </div>
          `).join('')}
        </div>
      ` : `
        <p class="agent-status-empty">No agents configured. Add one to enable the local AI agent.</p>
        <div class="agent-status-action-row"><button class="btn btn-primary" id="agent-setup">Add Agent</button></div>
      `}
    </div>
  `;

  const setupBtn = container.querySelector('#agent-setup');
  if (setupBtn) {
    setupBtn.addEventListener('click', () => {
      import('../pages/AgentSetup.js').then(mod => mod.showSetup());
    });
  }
}
