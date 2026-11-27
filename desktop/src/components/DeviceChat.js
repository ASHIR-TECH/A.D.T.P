const { invoke } = window.__TAURI__.core;
import { show } from './Toast.js';
import { chatStore, initChatEvents } from '../lib/chatStore.js';
import { renderConversation, bindToolToggles } from '../lib/chatView.js';
import { escapeHtml } from '../lib/chatFormat.js';

/**
 * Paired-phone chat drawer.
 *
 * This is a view of the same conversation the dashboard shows. It still owns
 * its invokes (picking files, durable queues, agent calls), but messages,
 * target and attachment come from `chatStore`, so a send here appears in the
 * dashboard immediately and vice versa.
 */

let active = null;
const expandedTools = new Set();

const DIRECT = 'direct';
const AI = 'ai';

export function openDeviceChat(device) {
  if (!device || !device.device_id) return;
  closeDeviceChat();
  initChatEvents();
  chatStore.emit('chat-target', device.device_id);

  // Make sure the target pickers agree even if the dashboard was never opened.
  void (async () => {
    try {
      const paired = await invoke('list_paired_devices');
      chatStore.emit('device-list', paired?.devices || []);
    } catch {}
  })();

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
    sending: false,
    unsubscribe: null,
  };
  active = state;

  bind(state);
  state.unsubscribe = chatStore.subscribe(current => {
    if (!document.body.contains(root)) {
      closeDeviceChat();
      return;
    }
    sync(state, current);
  });

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
  const { root, timer, unsubscribe } = active;
  clearInterval(timer);
  if (unsubscribe) unsubscribe();
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
    <aside class="device-chat-drawer" role="dialog" aria-label="Chat with ${escapeHtml(name)}">
      <header class="device-chat-head">
        <div class="device-chat-avatar">${escapeHtml(initial)}</div>
        <div class="device-chat-identity">
          <div class="device-chat-name">${escapeHtml(name)}</div>
          <div class="device-chat-meta">
            <span class="device-chat-presence ${online ? 'online' : 'offline'}">
              <span class="device-chat-presence-dot"></span>${online ? 'Online' : 'Offline'}
            </span>
            ${os ? `<span class="device-chat-sep">·</span><span>${escapeHtml(os)}</span>` : ''}
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

      <div class="device-chat-target-row">
        <label for="device-chat-target">Message target</label>
        <select id="device-chat-target" class="chat-device-select" aria-label="Message target">
          <option value="">All devices</option>
        </select>
      </div>

      <div class="device-chat-thread" id="device-chat-thread" role="log" aria-live="polite">
        <div class="device-chat-loading">Loading conversation…</div>
      </div>

      <div class="device-chat-composer">
        <div class="device-chat-attach-chip" id="device-chat-attach-chip" hidden>
          <span class="device-chat-attach-icon" aria-hidden="true">📎</span>
          <span class="device-chat-attach-name" id="device-chat-attach-name"></span>
          <button class="device-chat-attach-remove" id="device-chat-attach-remove" title="Remove attachment" aria-label="Remove attachment">&times;</button>
        </div>
        <div class="device-chat-input-row">
          <button class="device-chat-attach-btn" id="device-chat-attach" title="Attach a file" aria-label="Attach a file">📎</button>
          <textarea id="device-chat-input" rows="1" maxlength="4000" placeholder="Message ${escapeHtml(name)}…"></textarea>
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

function sync(state, current) {
  const { root } = state;
  const select = root.querySelector('#device-chat-target');
  const deviceKey = current.devices.map(d => `${d.device_id}|${d.name}|${d.online}`).join(';');
  if (select && select.dataset.deviceKey !== deviceKey) {
    select.dataset.deviceKey = deviceKey;
    select.innerHTML = '<option value="">All devices</option>' +
      current.devices.map(d => `<option value="${escapeHtml(d.device_id)}">${escapeHtml(d.name || d.device_id)} — ${d.online ? 'Online' : 'Offline'}</option>`).join('');
  }
  if (select) select.value = current.selectedDeviceId || '';

  const paired = current.devices.find(d => d.device_id === state.deviceId);
  if (paired) {
    const name = root.querySelector('.device-chat-name');
    if (name) name.textContent = paired.name || state.deviceId;
    const presence = root.querySelector('.device-chat-presence');
    if (presence) {
      presence.className = `device-chat-presence ${paired.online ? 'online' : 'offline'}`;
      presence.innerHTML = `<span class="device-chat-presence-dot"></span>${paired.online ? 'Online' : 'Offline'}`;
    }
  }

  const chip = root.querySelector('#device-chat-attach-chip');
  const chipName = root.querySelector('#device-chat-attach-name');
  if (chip) chip.hidden = !current.attachedFile;
  if (chipName) chipName.textContent = current.attachedFile?.name || '';

  const input = root.querySelector('#device-chat-input');
  const sendBtn = root.querySelector('#device-chat-send');
  if (sendBtn) sendBtn.disabled = state.sending || !(input?.value.trim() || current.attachedFile);

  const thread = root.querySelector('#device-chat-thread');
  if (!thread) return;
  const atBottom = thread.scrollHeight - thread.scrollTop - thread.clientHeight < 48;
  const firstRender = thread.dataset.rendered !== '1';
  thread.innerHTML = renderConversation(current.messages, {
    expandedTools,
    devices: current.devices,
  });
  bindToolToggles(thread, expandedTools);
  if (firstRender || atBottom) thread.scrollTop = thread.scrollHeight;
  thread.dataset.rendered = '1';
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

  root.querySelector('#device-chat-target')?.addEventListener('change', e => {
    chatStore.emit('chat-target', e.target.value || null);
  });

  const input = root.querySelector('#device-chat-input');
  const sendBtn = root.querySelector('#device-chat-send');

  const resize = () => {
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, 132) + 'px';
    sendBtn.disabled = state.sending || !input.value.trim() && !chatStore.getState().attachedFile;
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
      chatStore.emit('chat-attachment', { name, path });
      resize();
    } catch (err) {
      show(`File picker error: ${err}`, 'error');
    }
  });

  root.querySelector('#device-chat-attach-remove')?.addEventListener('click', () => {
    chatStore.emit('chat-attachment', null);
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
    chatStore.emit('device-thread', {
      deviceId: state.deviceId,
      messages: messageList?.messages || [],
      files: fileList?.files || [],
    });
  } catch (err) {
    if (!quiet) {
      const thread = state.root.querySelector('#device-chat-thread');
      if (thread) {
        thread.innerHTML = `<div class="device-chat-error">Could not load this conversation.<br><span>${escapeHtml(String(err))}</span></div>`;
      }
    }
  }
}

async function send(state) {
  if (state.sending) return;
  const input = state.root.querySelector('#device-chat-input');
  const statusEl = state.root.querySelector('#device-chat-status');
  const current = chatStore.getState();
  const text = input.value.trim();
  const file = current.attachedFile;
  if (!text && !file) return;

  const targetId = current.selectedDeviceId || state.deviceId;
  const name = state.device.name || targetId;
  state.sending = true;
  state.root.querySelector('#device-chat-send').disabled = true;
  if (statusEl) statusEl.textContent = state.mode === AI ? 'Asking ADTP AI…' : 'Sending…';

  // Single optimistic row, reconciled with the durable row on the next poll.
  const optimisticId = chatStore.addMessage({
    role: 'user',
    origin: state.mode === AI ? 'agent' : 'device',
    deviceId: targetId,
    kind: file ? 'file' : 'text',
    text: text || `📎 ${file.name}`,
    filename: file?.name || null,
    file_path: file?.path || null,
    pending: true,
    status: 'sending',
  });

  input.value = '';
  input.style.height = 'auto';
  chatStore.emit('chat-attachment', null);

  try {
    if (state.mode === AI) {
      await sendViaAgent(targetId, name, text, file);
    } else {
      await sendDirect(targetId, name, text, file);
    }
    chatStore.patchMessage(optimisticId, { delivered: true, status: 'sent', pending: false });
    if (statusEl) statusEl.textContent = 'Sent';
    setTimeout(() => { if (statusEl) statusEl.textContent = ''; }, 1600);
  } catch (err) {
    chatStore.patchMessage(optimisticId, { status: 'failed', error: String(err) });
    if (statusEl) statusEl.textContent = 'Failed';
    show(`Send failed: ${err}`, 'error', 'top-right');
  } finally {
    state.sending = false;
    const sendBtn = state.root.querySelector('#device-chat-send');
    if (sendBtn) sendBtn.disabled = !input.value.trim() && !chatStore.getState().attachedFile;
    void refreshThread(state, { quiet: true });
  }
}

async function sendDirect(deviceId, name, text, file) {
  if (file) {
    // Shared blob pipeline first: a MediaRef travels inside the durable device
    // message, so MARS chat renders a thumbnail + download link.
    let media = null;
    try {
      media = await invoke('upload_media', { path: file.path, sender: 'adtp-desktop', recipient: deviceId });
    } catch (err) {
      console.warn('Media upload failed; using the staged-file chat fallback', err);
    }
    if (media) {
      try {
        await invoke('send_message_to_device', { deviceId, text, media });
        if (!text) show(`Sent ${file.name} to ${name}`, 'success', 'top-right');
        return;
      } catch (err) {
        console.warn('Media message failed; using the staged-file chat fallback', err);
      }
    }
  }

  if (!text && !file) return;

  let inboxMedia = null;
  if (file) {
    // Stage the bytes in the phone's inbox, then still emit a chat message
    // carrying a MediaRef that points at this gateway, so both surfaces render
    // a real media card.
    const queued = await invoke('send_file_to_device', { deviceId, filePath: file.path });
    try {
      const status = await invoke('get_api_status');
      const base = status?.phone_url || status?.url;
      const record = queued?.file;
      if (base && record?.id) {
        inboxMedia = {
          schemaVersion: 2,
          id: record.id,
          serviceUrl: String(base).replace(/\/$/, ''),
          name: record.filename || file.name,
          mimeType: record.mime_type || 'application/octet-stream',
          size: record.file_size || 0,
          storedSize: record.file_size || 0,
          compressedSize: record.file_size || 0,
          compressed: false,
          hasThumbnail: false,
          createdAt: record.created_at || new Date().toISOString(),
          status: 'ready',
        };
      }
    } catch (err) {
      console.warn('Could not build an inbox media card; sending text only', err);
    }
  }

  await invoke('send_message_to_device', {
    deviceId,
    text: text || (file ? `📎 ${file.name}` : ''),
    ...(inboxMedia ? { media: inboxMedia } : {}),
  });
}

async function sendViaAgent(deviceId, name, text, file) {
  let attachedMedia = null;
  let deliveredMedia = false;
  if (file) {
    try {
      attachedMedia = await invoke('upload_media', { path: file.path, sender: 'adtp-desktop', recipient: deviceId });
    } catch (err) {
      console.warn('Media upload failed; falling back to local path', err);
    }
  }
  if (attachedMedia) {
    try {
      await invoke('send_message_to_device', {
        deviceId,
        text: text || '',
        media: attachedMedia,
      });
      deliveredMedia = true;
    } catch (err) {
      console.warn('Direct media delivery failed; asking the agent to retry', err);
    }
  }
  const context = [
    `[Target device: ${name} ${deviceId}]`,
    attachedMedia
      ? `[Attached media: ${attachedMedia.name} — ${attachedMedia.serviceUrl}/api/media/file/${attachedMedia.id}?kind=original&download=1]\n[Attached media ref: ${JSON.stringify(attachedMedia)}]`
      : file ? `[Attached file: ${file.path}]` : '',
    text ? '' : file ? `Send this file to ${name}.` : '',
    file && !deliveredMedia
      ? `To deliver the attached media to ${name}, call forward_media_to_device with device_id "${deviceId}" (the name "${name}" also works).`
      : '',
    deliveredMedia ? 'The media was already delivered to the phone — do not send it again.' : '',
  ].filter(Boolean).join('\n');
  const prompt = text ? `${text}\n\n${context}` : context;
  const response = await invoke('agent_send_message', { message: prompt });
  if (response) show(response, 'info', 'top-right');
}
