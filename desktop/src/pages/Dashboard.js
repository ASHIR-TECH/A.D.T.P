import { renderDonutWidget } from '../components/DonutWidget.js';
import { renderBadge } from '../components/Badge.js';
import { render as renderChat } from '../components/ChatPanel.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let transfers = [];
let health = null;

export async function render(container) {
  try {
    health = await invoke('get_app_health');
  } catch {}
  try {
    transfers = await invoke('list_transfers', { limit: 50 });
  } catch {}

  container.innerHTML = `
    <div class="dashboard-layout">
      <div class="dashboard-main">
        <div class="toolbar-row">
          <span class="date-range-pill">Last 24 Hours</span>
          <div style="margin-left:auto"></div>
        </div>
        <div class="widget-row" id="widget-row"></div>
        <div id="activity-widget"></div>
        <div class="toolbar-row" style="margin-top:var(--space-4)">
          <input class="search-input" id="table-search" placeholder="Search events..." />
          <div class="toolbar-actions">
            <button class="btn btn-sm btn-secondary" id="refresh-dash">Refresh</button>
          </div>
        </div>
        <div id="event-table"></div>
      </div>
      <div id="chat-panel-container" class="dashboard-sidebar"></div>
    </div>
  `;

  renderWidgets(container);
  renderActivity(container);
  renderEvents(container, transfers);
  renderChat(container.querySelector('#chat-panel-container'));

  container.querySelector('#refresh-dash')?.addEventListener('click', () => render(container));
  container.querySelector('#table-search')?.addEventListener('input', (e) => {
    filterEvents(container, e.target.value);
  });

  listen('health-update', (event) => {
    health = event.payload;
    renderWidgets(container);
  });

  listen('transfer-update', (event) => {
    transfers = event.payload?.transfers || transfers;
    renderEvents(container, transfers);
  });
}

function renderWidgets(container) {
  const row = container.querySelector('#widget-row');
  if (!row) return;

  const h = health || {};
  const q = h.queue || {};
  const delivered = q.delivered || 0;
  const pending = q.pending || 0;
  const inProgress = q.in_progress || 0;
  const failed = q.failed || 0;
  const totalTransfers = delivered + pending + inProgress + failed;
  const sessions = h.sessions || [];

  const segments = [
    { label: 'Delivered', value: delivered, color: '#3FA66B' },
    { label: 'In Progress', value: inProgress, color: '#D9A441' },
    { label: 'Failed', value: failed, color: '#C8430A' },
    { label: 'Pending', value: pending, color: '#8686AC' },
  ];

  row.innerHTML = '';
  const w1 = document.createElement('div');
  renderDonutWidget(w1, 'Transfer Status', segments, totalTransfers);
  row.appendChild(w1);

  const w2 = document.createElement('div');
  renderDonutWidget(w2, 'Connected Peers',
    sessions.map(s => ({ label: s.path || 'Unknown', value: 1, color: s.path === 'direct' ? '#3FA66B' : '#C9A84C' })),
    sessions.length
  );
  row.appendChild(w2);

  const w3 = document.createElement('div');
  renderDonutWidget(w3, 'Task Outcomes',
    [{ label: 'Active', value: 1, color: '#D9A441' }, { label: 'Completed', value: 0, color: '#3FA66B' }],
    1
  );
  row.appendChild(w3);

  const w4 = document.createElement('div');
  renderDonutWidget(w4, 'Queue', segments, totalTransfers);
  row.appendChild(w4);
}

function renderActivity(container) {
  const el = container.querySelector('#activity-widget');
  if (!el) return;
  const items = transfers?.transfers?.slice(0, 3) || [];
  const maxCount = items.length > 0 ? Math.max(...items.map(t => 1)) : 1;
  el.innerHTML = `
    <div class="activity-widget">
      <div class="activity-widget-title">Top Activity</div>
      ${items.length === 0 ? '<div style="color:var(--interactive-muted);font-size:13px;padding:8px 0">No recent activity</div>' :
        items.map((t, i) => `
          <div class="activity-bar">
            <span class="activity-bar-label">${t.filename || 'Transfer'}</span>
            <div class="activity-bar-track">
              <div class="activity-bar-fill" style="width:${((1 / maxCount) * 100)}%"></div>
            </div>
            <span class="activity-bar-count">${t.status || 'pending'}</span>
          </div>
        `).join('')
      }
    </div>
  `;
}

function renderEvents(container, data) {
  const el = container.querySelector('#event-table');
  if (!el) return;
  const items = data?.transfers || [];
  el.innerHTML = `
    <div class="data-table">
      <div class="table-header" style="grid-template-columns:2fr 3fr 1fr 1fr 1fr 1fr">
        <span>Event Name</span>
        <span>Summary</span>
        <span>Severity</span>
        <span>Status</span>
        <span>Peer</span>
        <span>Timestamp</span>
      </div>
      ${items.length === 0 ? '<div class="table-empty">No events to display</div>' :
        items.map(t => `
          <div class="table-row" style="grid-template-columns:2fr 3fr 1fr 1fr 1fr 1fr">
            <span style="font-weight:700;color:var(--white)">File Transfer</span>
            <span style="color:var(--interactive-muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="${t.filename || ''} sent to ${t.peer_id || 'unknown'}">${t.filename || '—'} → ${(t.peer_id || 'unknown').slice(0,12)}</span>
            <span><span class="status-dot ${t.status === 'delivered' ? 'connected' : t.status === 'failed' ? 'disconnected' : 'syncing'}"></span> ${t.status || 'pending'}</span>
            <span>${renderBadge(t.status === 'delivered' ? 'verified' : t.status === 'failed' ? 'alerted' : 'pending', t.status || 'pending')}</span>
            <span style="color:var(--interactive-muted)">${(t.peer_id || '—').slice(0,8)}</span>
            <span style="color:var(--interactive-muted);text-align:right">${t.created_at ? new Date(t.created_at).toLocaleString() : '—'}</span>
          </div>
        `).join('')
      }
    </div>
  `;
}

function filterEvents(container, query) {
  const rows = container.querySelectorAll('.table-row');
  rows.forEach(row => {
    const text = row.textContent.toLowerCase();
    row.style.display = text.includes(query.toLowerCase()) ? '' : 'none';
  });
}
