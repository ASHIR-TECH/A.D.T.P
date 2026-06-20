const { listen } = window.__TAURI__.event;

let el = null;

export function render(container) {
  container.innerHTML = `
    <div class="status-bar" id="status-bar">
      <div class="status-indicator">
        <span class="status-dot disconnected" id="status-dot"></span>
        <span id="status-text">Connecting...</span>
      </div>
      <div class="queue-badge">
        <span>Pending: <span class="badge-count" id="queue-pending">0</span></span>
        <span>In Progress: <span class="badge-count" id="queue-in-progress">0</span></span>
      </div>
    </div>
  `;

  el = container.querySelector('#status-bar');

  listen('health-update', (event) => {
    const h = event.payload;
    const dot = el.querySelector('#status-dot');
    const text = el.querySelector('#status-text');

    if (h.daemon_connected) {
      if (h.queue_pending > 0 || h.queue_in_progress > 0) {
        dot.className = 'status-dot syncing';
        text.textContent = 'Syncing';
      } else {
        dot.className = 'status-dot connected';
        text.textContent = 'Connected';
      }
    } else {
      dot.className = 'status-dot disconnected';
      text.textContent = 'Disconnected';
    }
  });

  listen('queue-update', (event) => {
    const q = event.payload;
    if (!q) return;
    const pending = el.querySelector('#queue-pending');
    const inProg = el.querySelector('#queue-in-progress');
    if (pending) pending.textContent = q.pending ?? 0;
    if (inProg) inProg.textContent = q.in_progress ?? 0;
  });
}
