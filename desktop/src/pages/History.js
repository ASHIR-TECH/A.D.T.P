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
    <div class="data-table">
      <div class="table-header" style="grid-template-columns:1.5fr 1.5fr 1.5fr 1fr 1.5fr 1fr">
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
        const statusText = entry.status || 'pending';
        const badgeVariant = statusText === 'delivered' || entry.integrity_verified ? 'verified' :
                             statusText === 'failed' ? 'alerted' : 'pending';
        return `
          <div class="table-row" style="grid-template-columns:1.5fr 1.5fr 1.5fr 1fr 1.5fr 1fr">
            <span style="font-family:monospace;font-size:12px;color:var(--interactive-muted)">${(entry.id || entry.log_id || '—').toString().slice(0,16)}</span>
            <span style="color:var(--interactive-muted)">${entry.timestamp || entry.created_at ? new Date(entry.timestamp || entry.created_at).toISOString() : '—'}</span>
            <span style="font-family:monospace;font-size:12px;color:var(--interactive-muted)">${(entry.session_id || entry.peer_id || '—').slice(0,16)}</span>
            <span>${entry.action || entry.event || 'transfer'}</span>
            <span style="color:var(--interactive-muted)">${entry.actor || entry.peer_id || '—'}</span>
            <span>${renderBadge(badgeVariant, statusText)}</span>
          </div>
        `;
      }).join('');
    }

    container.querySelector('#page-info').textContent = `Showing ${page * PAGE_SIZE + 1} to ${Math.min((page + 1) * PAGE_SIZE, total)} of ${total} entries`;
    container.querySelector('#page-prev').disabled = page === 0;
    container.querySelector('#page-next').disabled = (page + 1) * PAGE_SIZE >= total;
  } catch (err) {
    container.querySelector('#audit-rows').innerHTML = `<div class="table-empty">Failed to load: ${err}</div>`;
  }
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
