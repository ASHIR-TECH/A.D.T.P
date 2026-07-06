import { renderDonutWidget } from '../components/DonutWidget.js';
import { renderBadge } from '../components/Badge.js';
import { render as renderChat } from '../components/ChatPanel.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let transfers = [];
let health = null;
let p2pTransfers = [];
let peers = [];

export async function render(container) {
  try {
    health = await invoke('get_app_health');
  } catch {}
  try {
    transfers = await invoke('list_transfers', { limit: 50 });
  } catch {}
  try {
    peers = await invoke('list_peers');
  } catch {}

  container.innerHTML = `
    <div class="dashboard-layout">
      <div class="dashboard-main">
        <div class="toolbar-row">
          <span class="date-range-pill">Last 24 Hours</span>
          <div style="margin-left:auto"></div>
          <div class="toolbar-actions">
            <button class="btn btn-sm btn-primary" id="send-to-peer-btn">Send to Peer</button>
          </div>
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
        <div id="dashboard-banner"></div>
      </div>
      <div id="chat-panel-container" class="dashboard-sidebar"></div>
    </div>

    <!-- Send to Peer Modal -->
    <div id="send-peer-modal" class="modal-backdrop" style="display:none">
      <div class="modal-panel">
        <div class="modal-header">
          <h2>Send File to Peer</h2>
          <button class="modal-close" id="modal-close">&times;</button>
        </div>
        <div class="field-group">
          <label>Peer</label>
          <select id="peer-select" class="peer-select">
            ${peers.length === 0 ? '<option value="">No peers configured</option>' : ''}
            ${peers.map(p => `<option value="${p.peer_id}">${p.nickname || p.peer_id.slice(0, 16)} — ${p.host}:${p.port}</option>`).join('')}
          </select>
          ${peers.length === 0 ? '<div style="color:var(--interactive-muted);font-size:12px;margin-top:4px">Add peers in Settings first</div>' : ''}
        </div>
        <div class="field-group">
          <label>File</label>
          <div style="display:flex;gap:var(--space-2);align-items:center">
            <input type="text" id="selected-file-path" readonly placeholder="No file selected" style="flex:1" />
            <button class="btn btn-sm btn-secondary" id="pick-file-btn">Browse</button>
          </div>
        </div>
        <div style="display:flex;gap:var(--space-2);justify-content:flex-end;margin-top:var(--space-5)">
          <button class="btn btn-sm btn-secondary" id="modal-cancel">Cancel</button>
          <button class="btn btn-sm btn-primary" id="modal-send" disabled>Send</button>
        </div>
        <div id="send-result" style="margin-top:var(--space-3);font-size:13px"></div>
      </div>
    </div>
  `;

  renderWidgets(container);
  renderActivity(container);
  renderEvents(container, transfers);
  renderChat(container.querySelector('#chat-panel-container'));

  setupSendToPeer(container);
  setupDashboardEvents(container);
  renderAgentBanner(container);
}

function setupSendToPeer(container) {
  const modal = container.querySelector('#send-peer-modal');
  const peerSelect = container.querySelector('#peer-select');
  const filePathInput = container.querySelector('#selected-file-path');
  const sendBtn = container.querySelector('#modal-send');
  const resultDiv = container.querySelector('#send-result');
  let pickedPath = null;

  function canSend() {
    sendBtn.disabled = !(peerSelect.value && pickedPath);
  }

  container.querySelector('#send-to-peer-btn').addEventListener('click', () => {
    modal.style.display = 'flex';
    resultDiv.textContent = '';
    sendBtn.disabled = true;
    pickedPath = null;
    filePathInput.value = '';
  });

  container.querySelector('#modal-close').addEventListener('click', () => {
    modal.style.display = 'none';
  });

  container.querySelector('#modal-cancel').addEventListener('click', () => {
    modal.style.display = 'none';
  });

  modal.addEventListener('click', (e) => {
    if (e.target === modal) modal.style.display = 'none';
  });

  peerSelect.addEventListener('change', canSend);

  container.querySelector('#pick-file-btn').addEventListener('click', async () => {
    try {
      const path = await invoke('pick_file');
      if (path) {
        pickedPath = path;
        filePathInput.value = path.split('/').pop() || path.split('\\').pop() || path;
        canSend();
      }
    } catch (err) {
      const { show } = await import('../components/Toast.js');
      show(`File picker error: ${err}`, 'error');
    }
  });

  sendBtn.addEventListener('click', async () => {
    const peerId = peerSelect.value;
    if (!peerId || !pickedPath) return;
    sendBtn.disabled = true;
    sendBtn.textContent = 'Sending...';
    resultDiv.textContent = '';
    resultDiv.style.color = '';

    try {
      const result = await invoke('send_file_to_peer', { filePath: pickedPath, peerId });
      resultDiv.textContent = result;
      resultDiv.style.color = 'var(--success)';
      const { show } = await import('../components/Toast.js');
      show('File sent to peer', 'success');

      p2pTransfers.unshift({
        filename: pickedPath.split('/').pop() || pickedPath.split('\\').pop() || pickedPath,
        peer_id: peerId,
        status: 'delivered',
        created_at: new Date().toISOString(),
        file_size: 0,
        id: 'p2p-' + Date.now(),
        transfer_type: 'p2p',
      });
      const merged = { transfers: [...(transfers?.transfers || []), ...p2pTransfers] };
      renderEvents(container, merged);
      renderActivity(container);

      setTimeout(() => { modal.style.display = 'none'; }, 1500);
    } catch (err) {
      resultDiv.textContent = `Failed: ${err}`;
      resultDiv.style.color = 'var(--ember)';
      const { show } = await import('../components/Toast.js');
      show(`Send failed: ${err}`, 'error');

      p2pTransfers.unshift({
        filename: pickedPath.split('/').pop() || pickedPath.split('\\').pop() || pickedPath,
        peer_id: peerId,
        status: 'failed',
        created_at: new Date().toISOString(),
        file_size: 0,
        id: 'p2p-' + Date.now(),
        transfer_type: 'p2p',
      });
      const merged = { transfers: [...(transfers?.transfers || []), ...p2pTransfers] };
      renderEvents(container, merged);
      renderActivity(container);
    }
    sendBtn.disabled = false;
    sendBtn.textContent = 'Send';
  });
}

function setupDashboardEvents(container) {
  container.querySelector('#refresh-dash')?.addEventListener('click', () => {
    render(container.parentElement ? container : container);
  });
  container.querySelector('#table-search')?.addEventListener('input', (e) => {
    filterEvents(container, e.target.value);
  });

  listen('health-update', (event) => {
    health = event.payload;
    renderWidgets(container);
  });

  listen('transfer-update', () => {
    invoke('list_transfers', { limit: 50 }).then(data => {
      transfers = data;
      const merged = { transfers: [...(data.transfers || []), ...p2pTransfers] };
      renderEvents(container, merged);
      renderActivity(container);
    }).catch(() => {});
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

  const transferSegments = [
    { label: 'Delivered', value: delivered, color: '#3FA66B' },
    { label: 'In Progress', value: inProgress, color: '#D9A441' },
    { label: 'Failed', value: failed, color: '#C8430A' },
    { label: 'Pending', value: pending, color: '#8686AC' },
  ];

  const peerSegments = peers.length > 0
    ? peers.map(p => ({ label: p.nickname || p.peer_id.slice(0, 8), value: 1, color: '#C9A84C' }))
    : [{ label: 'No peers', value: 1, color: '#8686AC' }];

  row.innerHTML = '';
  const w1 = document.createElement('div');
  renderDonutWidget(w1, 'Transfer Status', transferSegments, totalTransfers);
  row.appendChild(w1);

  const w2 = document.createElement('div');
  renderDonutWidget(w2, 'Connected Peers',
    sessions.map(s => ({ label: s.path || 'Unknown', value: 1, color: s.path === 'direct' ? '#3FA66B' : '#C9A84C' })),
    sessions.length
  );
  row.appendChild(w2);

  const w3 = document.createElement('div');
  renderDonutWidget(w3, 'Known Peers', peerSegments, peers.length);
  row.appendChild(w3);

  const w4 = document.createElement('div');
  renderDonutWidget(w4, 'Queue', transferSegments, totalTransfers);
  row.appendChild(w4);
}

function renderActivity(container) {
  const el = container.querySelector('#activity-widget');
  if (!el) return;
  const combined = [...(transfers?.transfers || []), ...p2pTransfers]
    .sort((a, b) => new Date(b.created_at || 0) - new Date(a.created_at || 0))
    .slice(0, 3);
  const maxCount = combined.length > 0 ? Math.max(...combined.map(t => 1)) : 1;

  el.innerHTML = `
    <div class="activity-widget">
      <div class="activity-widget-title">Top Activity</div>
      ${combined.length === 0 ? '<div style="color:var(--interactive-muted);font-size:13px;padding:8px 0">No recent activity</div>' :
        combined.map((t, i) => `
          <div class="activity-bar">
            <span class="activity-bar-label">${t.filename || 'Transfer'}${t.transfer_type === 'p2p' ? ' [P2P]' : ''}</span>
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
  items.sort((a, b) => new Date(b.created_at || 0) - new Date(a.created_at || 0));
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
        items.map(t => {
          const isP2p = t.transfer_type === 'p2p';
          return `
          <div class="table-row" style="grid-template-columns:2fr 3fr 1fr 1fr 1fr 1fr${isP2p ? ';border-left:2px solid var(--gold)' : ''}">
            <span style="font-weight:700;color:var(--white)">${isP2p ? 'P2P File' : 'File Transfer'}</span>
            <span style="color:var(--interactive-muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="${t.filename || ''} sent to ${t.peer_id || 'unknown'}">${t.filename || '—'} → ${(t.peer_id || 'unknown').slice(0,12)}</span>
            <span><span class="status-dot ${t.status === 'delivered' ? 'connected' : t.status === 'failed' ? 'disconnected' : 'syncing'}"></span> ${t.status || 'pending'}</span>
            <span>${renderBadge(t.status === 'delivered' ? 'verified' : t.status === 'failed' ? 'alerted' : 'pending', t.status || 'pending')}</span>
            <span style="color:var(--interactive-muted)">${(t.peer_id || '—').slice(0,8)}</span>
            <span style="color:var(--interactive-muted);text-align:right">${t.created_at ? new Date(t.created_at).toLocaleString() : '—'}</span>
          </div>
        `}).join('')
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

async function renderAgentBanner(container) {
  try {
    const agents = await invoke('agent_list');
    if (agents.length > 0) return;
  } catch {}
  const el = container.querySelector('#dashboard-banner');
  if (!el) return;
  el.innerHTML = `
    <div class="banner gold" style="margin-top:var(--space-4)">
      <span>Set up your AI agent to unlock natural language file commands</span>
      <button class="btn btn-sm btn-ember" id="dash-banner-setup">Set Up Agent</button>
      <button class="banner-close" id="dash-banner-close"></button>
    </div>
  `;
  el.querySelector('#dash-banner-setup')?.addEventListener('click', () => import('./AgentSetup.js').then(mod => mod.showSetup()));
  el.querySelector('#dash-banner-close')?.addEventListener('click', () => {
    const banner = el.querySelector('.banner');
    banner.style.maxHeight = '0';
    banner.style.padding = '0';
    banner.style.opacity = '0';
    banner.style.overflow = 'hidden';
  });
}
