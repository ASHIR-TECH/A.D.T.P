const { invoke } = window.__TAURI__.core;

let containerEl = null;
let currentStatus = '';
let currentSession = '';

function renderTable(entries) {
  if (!containerEl) return;
  if (!entries || entries.length === 0) {
    containerEl.innerHTML = `<div class="audit-empty">No audit entries found.</div>`;
    return;
  }

  let html = `
    <div class="audit-header">
      <span>Filename</span>
      <span>Status</span>
      <span>Type</span>
      <span>Date</span>
      <span></span>
    </div>
  `;

  for (const e of entries) {
    const statusClass = e.status.replace(/_/g, '_');
    html += `
      <div class="audit-row">
        <span>${e.filename}</span>
        <span><span class="status-badge ${statusClass}">${e.status.replace(/_/g, ' ')}</span></span>
        <span>${e.transfer_type}</span>
        <span>${formatTime(e.initiated_at)}</span>
        <span><button class="btn-sm btn-secondary audit-detail" data-id="${e.id}">Detail</button></span>
      </div>
    `;
  }

  containerEl.innerHTML = html;

  containerEl.querySelectorAll('.audit-detail').forEach(btn => {
    btn.addEventListener('click', async () => {
      try {
        const ctx = await invoke('get_transfer_context', { id: btn.dataset.id });
        const { show } = await import('./Toast.js');
        show(`Context: ${ctx.summary || 'No summary available'}`, 'info');
      } catch {
        const { show } = await import('./Toast.js');
        show('Could not load context', 'warning');
      }
    });
  });
}

function formatTime(ts) {
  if (!ts) return '—';
  const d = new Date(ts);
  return d.toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
}

export async function render(container) {
  container.innerHTML = `
    <div class="audit-controls">
      <select id="audit-status-filter">
        <option value="">All statuses</option>
        <option value="delivered">Delivered</option>
        <option value="failed">Failed</option>
        <option value="pending">Pending</option>
        <option value="in_progress">In Progress</option>
      </select>
      <input type="text" id="audit-session-filter" placeholder="Session ID..." />
      <button class="btn btn-primary" id="audit-refresh">Refresh</button>
      <button class="btn btn-secondary" id="audit-verify">Verify Chain</button>
      <button class="btn btn-secondary" id="audit-export">Export CSV</button>
    </div>
    <div id="verify-banner"></div>
    <div class="audit-table" id="audit-table"></div>
  `;

  containerEl = container.querySelector('#audit-table');

  async function load() {
    currentStatus = container.querySelector('#audit-status-filter').value;
    currentSession = container.querySelector('#audit-session-filter').value;
    try {
      const data = await invoke('get_audit_log', {
        status: currentStatus || null,
        sessionId: currentSession || null,
      });
      renderTable(data.entries || []);
    } catch {
      renderTable([]);
    }
  }

  container.querySelector('#audit-refresh').addEventListener('click', load);
  container.querySelector('#audit-status-filter').addEventListener('change', load);
  container.querySelector('#audit-session-filter').addEventListener('keydown', (e) => {
    if (e.key === 'Enter') load();
  });

  container.querySelector('#audit-verify').addEventListener('click', async () => {
    try {
      const result = await invoke('verify_chain');
      const banner = container.querySelector('#verify-banner');
      if (result.valid) {
        banner.className = 'verify-banner valid';
        banner.textContent = `Chain valid — ${result.entries_checked} entries checked at ${new Date(result.verified_at).toLocaleString()}`;
      } else {
        banner.className = 'verify-banner invalid';
        banner.textContent = `Chain TAMPERED — first failure at entry ${result.first_failure_id}`;
      }
    } catch (err) {
      const { show } = await import('./Toast.js');
      show(`Verification failed: ${err}`, 'error');
    }
  });

  container.querySelector('#audit-export').addEventListener('click', async () => {
    try {
      const csv = await invoke('export_audit', {
        status: currentStatus || null,
        sessionId: currentSession || null,
      });
      const folder = await invoke('pick_save_folder');
      if (folder) {
        const path = `${folder}/adtp-audit-${Date.now()}.csv`;
        await invoke('write_file', { path, content: csv });
        const { show } = await import('./Toast.js');
        show(`Exported to ${path}`, 'success');
      }
    } catch (err) {
      const { show } = await import('./Toast.js');
      show(`Export failed: ${err}`, 'error');
    }
  });

  await load();
}
