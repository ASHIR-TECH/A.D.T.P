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

function portOf(url) {
  try {
    return new URL(url).port || (url.startsWith('https') ? '443' : '80');
  } catch {
    return '40003';
  }
}

/**
 * `mobile` starts/reuses the gateway before returning a payload, so MARS can
 * never scan a URL that nothing is listening on. `peer` is the desktop-to-
 * desktop payload (no gateway requirement).
 */
async function loadPairing(mode) {
  if (mode === 'mobile') {
    const info = await invoke('get_mobile_pairing_qr_data');
    return { payload: info.payload, gateway: info };
  }
  return { payload: await invoke('get_pairing_qr_data') };
}

export async function showQRPairModal(container, options = {}) {
  const mode = options.mode === 'mobile' ? 'mobile' : 'peer';

  if (countdownInterval) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }

  const loadingHtml = `
    <div class="qr-loading" style="text-align:center;padding:var(--space-8)">
      <div style="color:var(--interactive-muted);font-size:13px">
        ${mode === 'mobile' ? 'Starting the phone listener...' : 'Generating QR code...'}
      </div>
    </div>
  `;

  showModal(
    container,
    mode === 'mobile' ? 'Pair your phone (MARS)' : 'Pair a New Device',
    loadingHtml
  );

  let data;
  try {
    data = await loadPairing(mode);
  } catch (err) {
    show(`Failed to load pairing data: ${err}`, 'error');
    return;
  }

  const body = document.querySelector('#modal-body');
  if (!body) return;

  async function refresh(bodyEl) {
    try {
      data = await loadPairing(mode);
      renderPanel(bodyEl, data, mode, refresh);
      show('QR code regenerated', 'success', 'top-right');
    } catch (err) {
      show(`Failed to regenerate: ${err}`, 'error');
    }
  }

  renderPanel(body, data, mode, refresh);
}

function renderPanel(bodyEl, data, mode, onRefresh) {
  const payload = data.payload;
  const gateway = data.gateway;
  const isMobile = mode === 'mobile';

  if (countdownInterval) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }

  const agentInfo = payload.agentApiUrl
    ? `Agent API: <strong style="color:var(--white)">${escHtml(payload.agentApiUrl)}</strong>`
    : `Agent API derived from relay (port 40003)`;

  const gatewayLine = isMobile && gateway
    ? `
      <div class="qr-info" style="margin-top:var(--space-2)">
        <span class="badge ${gateway.gatewayRunning ? 'badge-verified' : 'badge-pending'}">
          ${gateway.gatewayRunning ? 'Listener online' : 'Listener offline'}
        </span>
        <span style="margin-left:var(--space-2)">${escHtml(gateway.gatewayMessage || '')}</span>
      </div>
      <div class="qr-info">
        Phone URL: <strong style="color:var(--white)">${escHtml(payload.agentApiUrl)}</strong>
        <button class="btn-toolbar-action btn-sm" id="qr-copy-url">Copy</button>
      </div>
      <div class="qr-info" style="color:var(--interactive-muted);font-size:12px">
        Phone and PC must be on the same network. If MARS says it cannot reach the desktop,
        allow TCP ${escHtml(portOf(payload.agentApiUrl))} through the PC firewall
        (e.g. <code>sudo ufw allow ${escHtml(portOf(payload.agentApiUrl))}/tcp</code>).
      </div>
    `
    : '';

  bodyEl.innerHTML = `
    <div class="qr-pair-panel">
      <p class="qr-info">
        ${isMobile
          ? 'Open MARS, choose Pair, and scan this code. The listener starts automatically and stays online after this window closes.'
          : 'Scan this QR code with the MARS mobile app to configure this PC as a device.'}
      </p>

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
      ${gatewayLine}

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

  bodyEl.querySelector('#qr-copy-peer-id')?.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(payload.desktopId);
      show('Desktop ID copied', 'success', 'top-right');
    } catch {
      show('Failed to copy', 'error', 'top-right');
    }
  });

  bodyEl.querySelector('#qr-copy-url')?.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(payload.agentApiUrl);
      show('Phone URL copied', 'success', 'top-right');
    } catch {
      show('Failed to copy', 'error', 'top-right');
    }
  });

  bodyEl.querySelector('#qr-regenerate')?.addEventListener('click', () => {
    onRefresh(bodyEl);
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
      show('QR expired — regenerating with a new token', 'info', 'top-right');
      onRefresh(bodyEl);
    }
  };

  tick();
  countdownInterval = setInterval(tick, 1000);
}

function escHtml(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}
