import { renderBadge } from '../components/Badge.js';
import { show as showToast } from '../components/Toast.js';
import { showBanner } from '../components/Banner.js';

const { invoke } = window.__TAURI__.core;

let page = 0;
const PAGE_SIZE = 20;

export async function render(container) {
  container.innerHTML = `
    <div class="toolbar-row">
      <span class="date-range-pill" id="history-date-range">Last 24 Hours</span>
      <input class="search-input" id="history-search" placeholder="Search Session ID, Actor, or Hash..." />
      <div class="toolbar-actions">
        <button class="btn btn-sm btn-brown" id="export-csv">Export CSV</button>
        <button class="btn btn-sm btn-brown" id="verify-chain">Verify Chain</button>
        <button class="btn btn-sm btn-brown" id="refresh-history">Refresh</button>
      </div>
    </div>
    <div id="verify-result"></div>
    <div class="data-table history-table">
      <div class="table-header history-header">
        <span>Log ID</span>
        <span>Timestamp (UTC)</span>
        <span>Session ID</span>
        <span>Action</span>
        <span>Actor / IP</span>
        <span>Status</span>
      </div>
      <div id="audit-rows"></div>
      <div class="pagination">
        <span id="page-info">Showing 0 of 0 entries</span>
        <div class="pagination-chevrons">
          <button id="page-prev" disabled></button>
          <button id="page-next" disabled></button>
        </div>
      </div>
    </div>
  `;

  await loadPage(container);
  setupActions(container);
}

async function loadPage(container) {
  try {
    const result = await invoke('get_audit_log', { limit: PAGE_SIZE, offset: page * PAGE_SIZE });
    const rows = container.querySelector('#audit-rows');
    const items = result?.entries || result?.transfers || [];
    const total = result?.total || items.length;

    if (items.length === 0) {
      rows.innerHTML = '<div class="table-empty">No audit entries found</div>';
    } else {
      rows.innerHTML = items.map(entry => {
        const statusText = String(pick(entry, ['status'], 'pending'));
        const badgeVariant = statusText === 'delivered' || entry.integrity_verified ? 'verified' :
                             statusText === 'failed' ? 'alerted' : 'pending';
        const timestamp = formatUtc(pick(entry, ['timestamp', 'created_at', 'initiated_at', 'completed_at']));
        const logId = String(pick(entry, ['id', 'log_id'], '—'));
        const session = String(pick(entry, ['session_id', 'sender_peer_id', 'peer_id'], '—'));
        const action = String(pick(entry, ['action', 'event', 'transfer_type'], 'transfer'));
        const actor = String(pick(entry, ['actor', 'actor_name', 'initiated_by', 'device_name', 'sender_peer_id', 'peer_id'], '—'));
        const ip = String(pick(entry, ['ip', 'ip_address', 'actor_ip', 'source_ip', 'remote_addr', 'peer_addr', 'host'], '—'));
        return `
          <div class="table-row history-row">
            <span class="history-cell history-id" title="${esc(logId)}">${esc(logId.slice(0, 16))}</span>
            <span class="history-cell history-time" title="${esc(timestamp.title)}">${esc(timestamp.label)}</span>
            <span class="history-cell history-id" title="${esc(session)}">${esc(session.slice(0, 16))}</span>
            <span class="history-cell">${esc(action)}</span>
            <span class="history-actor-cell">
              <span class="history-actor" title="${esc(actor)}">${esc(actor)}</span>
              <span class="history-ip" title="${esc(ip)}">IP ${esc(ip)}</span>
            </span>
            <span class="history-cell">${renderBadge(badgeVariant, esc(statusText))}</span>
          </div>
        `;
      }).join('');
    }

    container.querySelector('#page-info').textContent = `Showing ${page * PAGE_SIZE + 1} to ${Math.min((page + 1) * PAGE_SIZE, total)} of ${total} entries`;
    container.querySelector('#page-prev').disabled = page === 0;
    container.querySelector('#page-next').disabled = (page + 1) * PAGE_SIZE >= total;
  } catch (err) {
    container.querySelector('#audit-rows').innerHTML = `<div class="table-empty">History unavailable — daemon may be offline</div>`;
  }
}

function pick(entry, keys, fallback = '') {
  if (!entry) return fallback;
  for (const key of keys) {
    if (entry[key] != null && entry[key] !== '') return entry[key];
  }
  return fallback;
}

function esc(value) {
  const node = document.createElement('div');
  node.textContent = value == null ? '' : String(value);
  return node.innerHTML;
}

/** One-line UTC stamp: 2026-11-27 09:41:23Z, with the full value as a tooltip. */
function formatUtc(value) {
  if (!value) return { label: '—', title: '' };
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    const raw = String(value);
    return { label: raw.slice(0, 24), title: raw };
  }
  const iso = date.toISOString();
  return {
    label: iso.replace('T', ' ').replace(/\.\d{3}Z$/, 'Z'),
    title: iso,
  };
}

function setupActions(container) {
  container.querySelector('#page-prev')?.addEventListener('click', () => {
    if (page > 0) { page--; loadPage(container); }
  });
  container.querySelector('#page-next')?.addEventListener('click', () => {
    page++; loadPage(container);
  });
  container.querySelector('#refresh-history')?.addEventListener('click', () => loadPage(container));
  container.querySelector('#history-search')?.addEventListener('input', (e) => {
    const q = e.target.value.toLowerCase();
    container.querySelectorAll('.table-row').forEach(row => {
      row.style.display = row.textContent.toLowerCase().includes(q) ? '' : 'none';
    });
  });
  container.querySelector('#export-csv')?.addEventListener('click', async () => {
    try {
      const csv = await invoke('export_audit');
      const folder = await invoke('pick_save_folder');
      if (folder) {
        await invoke('write_file', { path: `${folder}/adtp-audit-${Date.now()}.csv`, content: csv });
        showToast('Export complete', 'success');
      }
    } catch (err) {
      showToast(`Export failed: ${err}`, 'error');
    }
  });
  container.querySelector('#verify-chain')?.addEventListener('click', async () => {
    const btn = container.querySelector('#verify-chain');
    btn.disabled = true;
    btn.innerHTML = 'Verifying...';
    try {
      const result = await invoke('verify_chain');
      if (result.valid) {
        showToast(`Chain valid: ${result.entries_checked || 0} entries`, 'success');
      } else {
        showBanner('Chain integrity failure detected — audit log may be tampered', 'ember');
      }
    } catch (err) {
      showBanner(`Verification error: ${err}`, 'ember');
    }
    btn.disabled = false;
    btn.innerHTML = 'Verify Chain';
  });
}
