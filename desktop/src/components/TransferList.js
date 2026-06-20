const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let containerEl = null;

function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTime(ts) {
  if (!ts) return '—';
  const d = new Date(ts);
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function renderTable(transfers) {
  if (!containerEl) return;
  if (!transfers || transfers.length === 0) {
    containerEl.innerHTML = `<div class="transfer-empty">No transfers yet. Drop a file to begin.</div>`;
    return;
  }

  let html = `
    <div class="transfer-header">
      <span>Filename</span>
      <span>Status</span>
      <span>Size</span>
      <span>Type</span>
      <span>Time</span>
    </div>
  `;

  for (const t of transfers) {
    const statusClass = t.status.replace(/_/g, '_');
    html += `
      <div class="transfer-row">
        <span title="${t.id}">${t.filename}</span>
        <span><span class="status-badge ${statusClass}">${t.status.replace(/_/g, ' ')}</span></span>
        <span>${formatSize(t.file_size)}</span>
        <span>${t.transfer_type}</span>
        <span>${formatTime(t.initiated_at)}</span>
      </div>
    `;
  }

  containerEl.innerHTML = html;
}

export async function render(container) {
  container.innerHTML = `<div class="transfer-list" id="transfer-list"></div>`;
  containerEl = container.querySelector('#transfer-list');

  try {
    const data = await invoke('list_transfers', { limit: 100 });
    renderTable(data.transfers || []);
  } catch {
    renderTable([]);
  }

  listen('transfer-update', async () => {
    try {
      const data = await invoke('list_transfers', { limit: 100 });
      renderTable(data.transfers || []);
    } catch {}
  });
}
