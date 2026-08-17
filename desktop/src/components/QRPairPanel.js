const { invoke } = window.__TAURI__.core;
import { show } from './Toast.js';
import { showModal } from './Modal.js';

let sessionInterval = null;

function buildPairingUri(data) {
  const params = new URLSearchParams({
    peer_id: data.peer_id,
    host: data.host,
    port: String(data.port),
    rendezvous: data.rendezvous,
    nickname: data.nickname,
    pubkey: data.pubkey,
  });
  if (data.session_token) {
    params.set('session_token', data.session_token);
  }
  return `adtp://pair?${params.toString()}`;
}

function generateSessionToken() {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
}

async function renderQR(canvasEl, uri) {
  await QRCode.toCanvas(canvasEl, uri, {
    width: 240,
    margin: 2,
    color: {
      dark: '#C9A84C',
      light: '#0E0E1C',
    },
    errorCorrectionLevel: 'M',
  });
}

export async function showQRPairModal(container) {
  if (sessionInterval) {
    clearInterval(sessionInterval);
    sessionInterval = null;
  }

  let sessionMode = false;
  let countdown = 0;
  let countdownInterval = null;

  const loadingHtml = `
    <div class="qr-loading" style="text-align:center;padding:var(--space-8)">
      <div style="color:var(--interactive-muted);font-size:13px">Generating QR code...</div>
    </div>
  `;

  showModal(container, 'Pair a New Device', loadingHtml);

  let data;
  try {
    data = await invoke('get_pairing_qr_data', { sessionToken: null });
  } catch (err) {
    show(`Failed to load pairing data: ${err}`, 'error');
    return;
  }

  const uri = buildPairingUri(data);

  const body = document.querySelector('#modal-body');
  if (!body) return;

  body.innerHTML = `
    <div class="qr-pair-panel">
      <p class="qr-info">Scan this QR code with the MARS mobile app to add this PC as a trusted peer.</p>

      <div class="qr-canvas-wrap">
        <canvas id="qr-canvas" width="240" height="240"></canvas>
      </div>

      <div class="qr-peer-id-row">
        <span class="qr-info" style="margin:0">Peer ID:</span>
        <code style="font-family:monospace;font-size:12px;color:var(--gold);flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${escHtml(data.peer_id)}</code>
        <button class="btn-toolbar-action btn-sm" id="qr-copy-peer-id">Copy</button>
      </div>

      <div class="qr-info" style="margin-top:var(--space-2)">Nickname: <strong style="color:var(--white)">${escHtml(data.nickname)}</strong></div>

      <div id="qr-countdown" class="qr-countdown" style="display:none"></div>

      <div class="qr-actions">
        <button class="btn btn-sm btn-primary" id="qr-regenerate">Regenerate</button>
        <button class="btn btn-sm btn-secondary" id="qr-session-toggle">Session Mode: OFF</button>
      </div>
    </div>
  `;

  const canvasEl = body.querySelector('#qr-canvas');
  if (canvasEl) {
    try {
      await renderQR(canvasEl, uri);
    } catch (err) {
      show(`QR render failed: ${err}`, 'error');
    }
  }

  body.querySelector('#qr-copy-peer-id')?.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(data.peer_id);
      show('Peer ID copied', 'success', 'top-right');
    } catch {
      show('Failed to copy', 'error', 'top-right');
    }
  });

  body.querySelector('#qr-regenerate')?.addEventListener('click', async () => {
    if (countdownInterval) {
      clearInterval(countdownInterval);
      countdownInterval = null;
    }
    countdown = 0;
    body.querySelector('#qr-countdown').style.display = 'none';

    const token = sessionMode ? generateSessionToken() : null;
    try {
      data = await invoke('get_pairing_qr_data', { sessionToken: token });
    } catch (err) {
      show(`Failed to regenerate: ${err}`, 'error');
      return;
    }

    const newUri = buildPairingUri(data);
    body.querySelector('code').textContent = data.peer_id;
    try {
      await renderQR(canvasEl, newUri);
    } catch (err) {
      show(`QR render failed: ${err}`, 'error');
    }

    if (sessionMode) {
      startCountdown(body, canvasEl, data);
    }

    show('QR code regenerated', 'success', 'top-right');
  });

  body.querySelector('#qr-session-toggle')?.addEventListener('click', async () => {
    sessionMode = !sessionMode;
    const toggleBtn = body.querySelector('#qr-session-toggle');
    toggleBtn.textContent = `Session Mode: ${sessionMode ? 'ON' : 'OFF'}`;

    if (sessionMode) {
      const token = generateSessionToken();
      try {
        data = await invoke('get_pairing_qr_data', { sessionToken: token });
      } catch (err) {
        show(`Failed to enable session mode: ${err}`, 'error');
        sessionMode = false;
        toggleBtn.textContent = 'Session Mode: OFF';
        return;
      }
      const newUri = buildPairingUri(data);
      body.querySelector('code').textContent = data.peer_id;
      try {
        await renderQR(canvasEl, newUri);
      } catch (err) {
        show(`QR render failed: ${err}`, 'error');
      }
      startCountdown(body, canvasEl, data);
      show('Session mode enabled — QR expires in 60 seconds', 'info', 'top-right');
    } else {
      if (countdownInterval) {
        clearInterval(countdownInterval);
        countdownInterval = null;
      }
      countdown = 0;
      body.querySelector('#qr-countdown').style.display = 'none';
      canvasEl.classList.remove('qr-expired');

      try {
        data = await invoke('get_pairing_qr_data', { sessionToken: null });
      } catch (err) {
        show(`Failed to disable session mode: ${err}`, 'error');
        return;
      }
      const newUri = buildPairingUri(data);
      body.querySelector('code').textContent = data.peer_id;
      try {
        await renderQR(canvasEl, newUri);
      } catch (err) {
        show(`QR render failed: ${err}`, 'error');
      }
      show('Session mode disabled', 'info', 'top-right');
    }
  });

  function startCountdown(bodyEl, canvas, currentData) {
    countdown = 60;
    const countdownEl = bodyEl.querySelector('#qr-countdown');
    countdownEl.style.display = 'block';
    countdownEl.textContent = `Session expires in: ${countdown}s`;
    canvas.classList.remove('qr-expired');

    countdownInterval = setInterval(async () => {
      countdown--;
      countdownEl.textContent = `Session expires in: ${countdown}s`;

      if (countdown <= 0) {
        clearInterval(countdownInterval);
        countdownInterval = null;
        canvas.classList.add('qr-expired');

        const token = generateSessionToken();
        try {
          data = await invoke('get_pairing_qr_data', { sessionToken: token });
        } catch (err) {
          show(`Failed to refresh session QR: ${err}`, 'error');
          return;
        }
        const newUri = buildPairingUri(data);
        bodyEl.querySelector('code').textContent = data.peer_id;
        try {
          await renderQR(canvas, newUri);
        } catch (err) {
          show(`QR render failed: ${err}`, 'error');
        }
        startCountdown(bodyEl, canvas, data);
      }
    }, 1000);
  }
}

function escHtml(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}
