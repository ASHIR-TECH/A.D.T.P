const { invoke } = window.__TAURI__.core;
import { show } from './Toast.js';
import { showModal } from './Modal.js';

let countdownInterval = null;

async function renderQR(canvasEl, payload) {
  await QRCode.toCanvas(canvasEl, JSON.stringify(payload), {
    width: 240,
    margin: 2,
    color: {
      dark: '#C9A84C',
      light: '#0E0E1C',
    },
    errorCorrectionLevel: 'M',
  });
}

function secondsUntil(iso) {
  return Math.max(0, Math.round((new Date(iso).getTime() - Date.now()) / 1000));
}

export async function showQRPairModal(container) {
  if (countdownInterval) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }

  const loadingHtml = `
    <div class="qr-loading" style="text-align:center;padding:var(--space-8)">
      <div style="color:var(--interactive-muted);font-size:13px">Generating QR code...</div>
    </div>
  `;

  showModal(container, 'Pair a New Device', loadingHtml);

  let payload;
  try {
    payload = await invoke('get_pairing_qr_data');
  } catch (err) {
    show(`Failed to load pairing data: ${err}`, 'error');
    return;
  }

  const body = document.querySelector('#modal-body');
  if (!body) return;

  renderPanel(body, payload);

  body.querySelector('#qr-copy-peer-id')?.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(payload.desktopId);
      show('Desktop ID copied', 'success', 'top-right');
    } catch {
      show('Failed to copy', 'error', 'top-right');
    }
  });

  body.querySelector('#qr-regenerate')?.addEventListener('click', () => {
    refresh(body);
  });

  function refresh(bodyEl) {
    invoke('get_pairing_qr_data').then(async (fresh) => {
      payload = fresh;
      renderPanel(bodyEl, payload);
      show('QR code regenerated', 'success', 'top-right');
    }).catch((err) => {
      show(`Failed to regenerate: ${err}`, 'error');
    });
  }
}

function renderPanel(bodyEl, payload) {
  if (countdownInterval) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }

  const agentInfo = payload.agentApiUrl
    ? `Agent API: <strong style="color:var(--white)">${escHtml(payload.agentApiUrl)}</strong>`
    : `Agent API derived from relay (port 40003)`;

  bodyEl.innerHTML = `
    <div class="qr-pair-panel">
      <p class="qr-info">Scan this QR code with the MARS mobile app to configure this PC as a device.</p>

      <div class="qr-canvas-wrap">
        <canvas id="qr-canvas" width="240" height="240"></canvas>
      </div>

      <div class="qr-peer-id-row">
        <span class="qr-info" style="margin:0">Desktop ID:</span>
        <code style="font-family:monospace;font-size:12px;color:var(--gold);flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${escHtml(payload.desktopId)}</code>
        <button class="btn-toolbar-action btn-sm" id="qr-copy-peer-id">Copy</button>
      </div>

      <div class="qr-info" style="margin-top:var(--space-2)">Device: <strong style="color:var(--white)">${escHtml(payload.desktopName)}</strong></div>
      <div class="qr-info">Relay: <strong style="color:var(--white)">${escHtml(payload.relayUrl)}</strong></div>
      <div class="qr-info">${agentInfo}</div>

      <div id="qr-countdown" class="qr-countdown"></div>

      <div class="qr-actions">
        <button class="btn btn-sm btn-primary" id="qr-regenerate">Regenerate</button>
      </div>
    </div>
  `;

  const canvasEl = bodyEl.querySelector('#qr-canvas');
  renderQR(canvasEl, payload).catch((err) => {
    show(`QR render failed: ${err}`, 'error');
  });

  const countdownEl = bodyEl.querySelector('#qr-countdown');
  const tick = () => {
    const remaining = secondsUntil(payload.expiresAt);
    countdownEl.style.display = remaining > 0 ? 'block' : 'none';
    countdownEl.textContent = `QR expires in: ${remaining}s`;
    canvasEl.classList.toggle('qr-expired', remaining <= 0);

    if (remaining <= 0) {
      clearInterval(countdownInterval);
      countdownInterval = null;
      evaluateExpiry();
    }
  };

  let evaluating = false;
  async function evaluateExpiry() {
    if (evaluating) return;
    evaluating = true;
    try {
      const fresh = await invoke('get_pairing_qr_data');
      renderPanel(bodyEl, fresh);
      show('QR expired — regenerated with a new token', 'info', 'top-right');
    } catch (err) {
      show(`Failed to refresh QR: ${err}`, 'error');
    } finally {
      evaluating = false;
    }
  }

  tick();
  countdownInterval = setInterval(tick, 1000);
}

function escHtml(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}