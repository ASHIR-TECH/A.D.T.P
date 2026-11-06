const { invoke } = window.__TAURI__.core;
import { show } from './Toast.js';

/**
 * Per-device chat drawer.
 *
 * Replaces the old `window.prompt` flow: one composer carries both text and a
 * file attachment, and every message is written to the same durable queue the
 * AI uses (`send_message_to_device` / `send_file_to_device`). The phone sees
 * both in MARS chat.
 */

let active = null; // { deviceId, root, timer, mode, attachedFile }

const DIRECT = 'direct';
const AI = 'ai';

export function openDeviceChat(device) {
  if (!device || !device.device_id) return;
  closeDeviceChat();

  const root = document.createElement('div');
  root.className = 'device-chat-overlay';
  root.innerHTML = shell(device);
  document.body.appendChild(root);
  document.body.classList.add('device-chat-open');

  const state = {
    deviceId: device.device_id,
    device,
    root,
    timer: null,
    mode: DIRECT,
    attachedFile: null,
    messages: [],
    files: [],
    sending: false,
  };
  active = state;

  bind(state);
  void refreshThread(state);
  state.timer = setInterval(() => {
    if (!document.body.contains(root)) return;
    void refreshThread(state, { quiet: true });
  }, 3000);

  // Let the browser paint the closed state before transitioning in, otherwise
  // the drawer snaps instead of sliding.
  requestAnimationFrame(() => requestAnimationFrame(() => root.classList.add('visible')));
  root.querySelector('#device-chat-input')?.focus();
}

export function closeDeviceChat() {
  if (!active) return;
  const { root, timer } = active;
  clearInterval(timer);
  active = null;
  document.removeEventListener('keydown', escListener);
  document.body.classList.remove('device-chat-open');
  root.classList.remove('visible');
  setTimeout(() => root.remove(), 260);
}

function shell(device) {
  const name = device.name || 'Paired phone';
  const online = !!device.online;
  const os = [device.os_name || device.platform, device.os_version].filter(Boolean).join(' ');
  const initial = name.trim().charAt(0).toUpperCase() || 'P';
  return `
    <div class="device-chat-scrim" data-close="1"></div>
    <aside class="device-chat-drawer" role="dialog" aria-label="Chat with ${esc(name)}">
      <header class="device-chat-head">
        <div class="device-chat-avatar">${esc(initial)}</div>
        <div class="device-chat-identity">
          <div class="device-chat-name">${esc(name)}</div>
          <div class="device-chat-meta">
            <span class="device-chat-presence ${online ? 'online' : 'offline'}">
              <span class="device-chat-presence-dot"></span>${online ? 'Online' : 'Offline'}
            </span>
            ${os ? `<span class="device-chat-sep">·</span><span>${esc(os)}</span>` : ''}
            ${batteryLabel(device)}
          </div>
        </div>
        <button class="device-chat-close" id="device-chat-close" title="Close" aria-label="Close chat">&times;</button>
      </header>

      <div class="device-chat-mode" role="tablist">
        <button class="device-chat-mode-btn active" data-mode="${DIRECT}" role="tab">Direct</button>
        <button class="device-chat-mode-btn" data-mode="${AI}" role="tab">Ask ADTP AI</button>
        <span class="device-chat-mode-hint" id="device-chat-mode-hint">Sends straight to the phone</span>
      </div>

      <div class="device-chat-thread" id="device-chat-thread">
        <div class="device-chat-loading">Loading conversation…</div>
      </div>

      <div class="device-chat-composer">
        <div class="device-chat-attach-chip" id="device-chat-attach-chip" hidden>
          <span class="device-chat-attach-icon">📎</span>
          <span class="device-chat-attach-name" id="device-chat-attach-name"></span>
          <button class="device-chat-attach-remove" id="device-chat-attach-remove" title="Remove attachment">&times;</button>
        </div>
        <div class="device-chat-input-row">
          <button class="device-chat-attach-btn" id="device-chat-attach" title="Attach a file" aria-label="Attach a file">📎</button>
          <textarea id="device-chat-input" rows="1" maxlength="4000" placeholder="Message ${esc(name)}…"></textarea>
          <button class="device-chat-send" id="device-chat-send" title="Send" aria-label="Send">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 2 11 13"/><path d="M22 2 15 22l-4-9-9-4 20-7z"/></svg>
          </button>
        </div>
        <div class="device-chat-foot">
          <span>Enter to send · Shift+Enter for a new line</span>
          <span id="device-chat-status"></span>
        </div>
      </div>
    </aside>
  `;
}

function bind(state) {
  const { root } = state;

  root.querySelector('[data-close]')?.addEventListener('click', closeDeviceChat);
  root.querySelector('#device-chat-close')?.addEventListener('click', closeDeviceChat);

  root.querySelectorAll('.device-chat-mode-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      state.mode = btn.dataset.mode;
      root.querySelectorAll('.device-chat-mode-btn').forEach(b => b.classList.toggle('active', b === btn));
      const hint = root.querySelector('#device-chat-mode-hint');
      if (hint) {
        hint.textContent = state.mode === AI
          ? 'ADTP AI can message and send files for you'
          : 'Sends straight to the phone';
      }
      root.querySelector('#device-chat-input')?.focus();
    });
  });

  const input = root.querySelector('#device-chat-input');
  const sendBtn = root.querySelector('#device-chat-send');

  const resize = () => {
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, 132) + 'px';
    sendBtn.disabled = !input.value.trim() && !state.attachedFile;
  };

  input.addEventListener('input', resize);
  input.addEventListener('keydown', e => {
    if (e.key === 'Escape') {
      closeDeviceChat();
      return;
    }
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void send(state);
    }
  });
  sendBtn.addEventListener('click', () => void send(state));

  root.querySelector('#device-chat-attach')?.addEventListener('click', async () => {
    try {
      const path = await invoke('pick_file');
      if (!path) return;
      const name = path.split('/').pop() || path.split('\\').pop() || path;
      state.attachedFile = { name, path };
      root.querySelector('#device-chat-attach-name').textContent = name;
      root.querySelector('#device-chat-attach-chip').hidden = false;
      resize();
    } catch (err) {
      show(`File picker error: ${err}`, 'error');
    }
  });

  root.querySelector('#device-chat-attach-remove')?.addEventListener('click', () => {
    state.attachedFile = null;
    root.querySelector('#device-chat-attach-chip').hidden = true;
    resize();
  });

  document.addEventListener('keydown', escListener);
}

function escListener(e) {
  if (e.key === 'Escape' && active) closeDeviceChat();
}

function batteryLabel(device) {
  if (device.battery_percent == null) return '';
  const charging = device.battery_charging ? ' ⚡' : '';
  return `<span class="device-chat-sep">·</span><span>${Math.round(device.battery_percent)}%${charging}</span>`;
}

async function refreshThread(state, { quiet } = {}) {
  if (!active || active !== state) return;
  try {
    const [messageList, fileList] = await Promise.all([
      invoke('list_device_messages', { deviceId: state.deviceId, includeDelivered: true }),
      invoke('list_device_files', { deviceId: state.deviceId, includeDelivered: true }),
    ]);
    // A newer drawer may have opened while these were in flight.
    if (!active || active !== state) return;
    state.messages = messageList?.messages || [];
    state.files = fileList?.files || [];
    renderThread(state);
  } catch (err) {
    if (!quiet) {
      const thread = state.root.querySelector('#device-chat-thread');
      if (thread) {
        thread.innerHTML = `<div class="device-chat-error">Could not load this conversation.<br><span>${esc(String(err))}</span></div>`;
      }
    }
  }
}

function renderThread(state, { forceScroll = false } = {}) {
  const thread = state.root.querySelector('#device-chat-thread');
  if (!thread) return;
  const atBottom =
    thread.scrollHeight - thread.scrollTop - thread.clientHeight < 48;

  const items = [
    ...state.messages.map(m => ({
      kind: 'text',
      id: m.id,
      createdAt: m.created_at,
      text: m.text,
      delivered: !!m.delivered_at,
    })),
    ...state.files.map(f => ({
      kind: 'file',
      id: f.id,
      createdAt: f.created_at,
      filename: f.filename,
      size: f.file_size,
      delivered: !!f.delivered_at,
    })),
  ].sort((a, b) => new Date(a.createdAt) - new Date(b.createdAt));

  if (items.length === 0) {
    thread.innerHTML = `
      <div class="device-chat-empty">
        <div class="device-chat-empty-icon">💬</div>
        <div class="device-chat-empty-title">Start the conversation</div>
        <div class="device-chat-empty-text">Messages and files you send here appear in MARS on ${esc(state.device.name || 'your phone')}.</div>
      </div>`;
    return;
  }

  let lastDay = '';
  const firstRender = thread.dataset.rendered !== '1';
  thread.innerHTML = items.map(item => {
    const date = new Date(item.createdAt);
    const day = Number.isNaN(date.getTime()) ? '' : date.toDateString();
    const dayLabel = day && day !== lastDay
      ? `<div class="device-chat-day">${esc(formatDay(date))}</div>`
      : '';
    lastDay = day || lastDay;
    return dayLabel + renderItem(item, state.device.name);
  }).join('');
  // Don't yank the thread to the bottom while the user is reading history.
  if (forceScroll || firstRender || atBottom) {
    thread.scrollTop = thread.scrollHeight;
  }
  thread.dataset.rendered = '1';
}

function renderItem(item, deviceName) {
  const time = formatTime(item.createdAt);
  const tick = item.delivered ? '<span class="device-chat-tick delivered" title="Delivered">✓✓</span>' : '<span class="device-chat-tick" title="Queued">✓</span>';
  if (item.kind === 'file') {
    return `
      <div class="device-chat-bubble-row out">
        <div class="device-chat-bubble file">
          <div class="device-chat-file-tile">
            <div class="device-chat-file-icon">📄</div>
            <div class="device-chat-file-info">
              <div class="device-chat-file-name">${esc(item.filename || 'file')}</div>
              <div class="device-chat-file-size">${formatBytes(item.size)}</div>
            </div>
          </div>
          <div class="device-chat-bubble-meta">${time}${tick}</div>
        </div>
      </div>`;
  }
  return `
    <div class="device-chat-bubble-row out">
      <div class="device-chat-bubble">
        <div class="device-chat-bubble-text">${esc(item.text)}</div>
        <div class="device-chat-bubble-meta">${time}${tick}</div>
      </div>
    </div>`;
}

function appendOptimistic(state, item) {
  const thread = state.root.querySelector('#device-chat-thread');
  if (!thread) return;
  const empty = thread.querySelector('.device-chat-empty');
  if (empty) thread.innerHTML = '';
  thread.insertAdjacentHTML('beforeend', renderItem({ delivered: false, ...item }, state.device.name));
  thread.scrollTop = thread.scrollHeight;
}

async function send(state) {
  if (state.sending) return;
  const input = state.root.querySelector('#device-chat-input');
  const statusEl = state.root.querySelector('#device-chat-status');
  const text = input.value.trim();
  const file = state.attachedFile;
  if (!text && !file) return;

  state.sending = true;
  state.root.querySelector('#device-chat-send').disabled = true;
  if (statusEl) statusEl.textContent = state.mode === AI ? 'Asking ADTP AI…' : 'Sending…';

  if (text) appendOptimistic(state, { kind: 'text', text, createdAt: new Date().toISOString() });
  if (file) {
    appendOptimistic(state, {
      kind: 'file',
      filename: file.name,
      size: 0,
      createdAt: new Date().toISOString(),
    });
  }

  input.value = '';
  input.style.height = 'auto';
  state.attachedFile = null;
  const chip = state.root.querySelector('#device-chat-attach-chip');
  if (chip) chip.hidden = true;

  try {
    if (state.mode === AI) {
      await sendViaAgent(state, text, file);
    } else {
      await sendDirect(state, text, file);
    }
    if (statusEl) statusEl.textContent = 'Sent';
    setTimeout(() => { if (statusEl) statusEl.textContent = ''; }, 1600);
  } catch (err) {
    if (statusEl) statusEl.textContent = 'Failed';
    show(`Send failed: ${err}`, 'error', 'top-right');
  } finally {
    state.sending = false;
    const sendBtn = state.root.querySelector('#device-chat-send');
    if (sendBtn) sendBtn.disabled = !input.value.trim() && !state.attachedFile;
    void refreshThread(state, { quiet: true });
  }
}

async function sendDirect(state, text, file) {
  const name = state.device.name || state.deviceId;
  if (text) {
    await invoke('send_message_to_device', { deviceId: state.deviceId, text });
  }
  if (file) {
    await invoke('send_file_to_device', { deviceId: state.deviceId, filePath: file.path });
  }
  if (!text && file) {
    // No message body, but the drawer should still confirm what happened.
    show(`Sent ${file.name} to ${name}`, 'success', 'top-right');
  }
}

async function sendViaAgent(state, text, file) {
  const name = state.device.name || state.deviceId;
  const context = [
    `[Target device: ${name} ${state.deviceId}]`,
    file ? `[Attached file: ${file.path}]` : '',
    text ? '' : file ? `Send this file to ${name}.` : '',
  ].filter(Boolean).join('\n');
  const prompt = text ? `${text}\n\n${context}` : context;
  const response = await invoke('agent_send_message', { message: prompt });
  if (response) show(response, 'info', 'top-right');
}

function formatTime(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function formatDay(date) {
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === today.toDateString()) return 'Today';
  if (date.toDateString() === yesterday.toDateString()) return 'Yesterday';
  return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

function formatBytes(bytes) {
  if (!bytes || Number.isNaN(bytes)) return 'File';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function esc(s) {
  const d = document.createElement('div');
  d.textContent = s == null ? '' : String(s);
  return d.innerHTML;
}
