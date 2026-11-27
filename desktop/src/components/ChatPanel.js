const { invoke } = window.__TAURI__.core;
import { chatStore, initChatEvents } from '../lib/chatStore.js';
import { renderConversation, bindToolToggles } from '../lib/chatView.js';
import { escapeHtml } from '../lib/chatFormat.js';

let unsubscribe = null;
const expandedTools = new Set();

export async function render(container) {
  initChatEvents();

  // Surfaces keep their own invokes; only the conversation state is shared.
  try {
    const agents = await invoke('agent_list');
    const active = agents.find(a => a.enabled);
    chatStore.emit('agent-status', {
      state: active ? 'active' : 'inactive',
      label: active && active.provider === 'base' ? 'ADTP AI' : 'Agent',
    });
  } catch {
    chatStore.emit('agent-status', { state: 'inactive' });
  }

  try {
    const paired = await invoke('list_paired_devices');
    chatStore.emit('device-list', paired?.devices || []);
  } catch {}

  container.innerHTML = `
    <div class="chat-panel">
      <div class="chat-header">
        <span class="chat-title" id="chat-title">ADTP AI</span>
        <div class="chat-controls">
          <select class="chat-device-select" id="chat-device-select" aria-label="Message target">
            <option value="">All devices</option>
          </select>
          <span class="agent-dot inactive"></span>
          <span id="chat-status-text">Offline</span>
        </div>
      </div>
      <div class="chat-messages" id="chat-messages" role="log" aria-live="polite"></div>
      <div class="chat-input-area">
        <div id="chat-file-chip" class="chat-file-chip" hidden>
          <span class="chat-file-chip-icon" aria-hidden="true">📎</span>
          <span class="chat-file-chip-name" id="chat-file-chip-name"></span>
          <button class="chat-file-chip-remove" id="chat-file-chip-remove" title="Remove attachment" aria-label="Remove attachment">&times;</button>
        </div>
        <div class="chat-composer-row">
          <textarea class="chat-input" id="chat-input" placeholder="Message the agent…" rows="1" maxlength="4000"></textarea>
          <div class="chat-actions">
            <button class="btn btn-sm btn-secondary btn-chat-attach" id="chat-attach" title="Attach file" aria-label="Attach file">📎</button>
            <button class="btn btn-sm btn-primary btn-chat-send" id="chat-send" title="Send">Send</button>
          </div>
        </div>
      </div>
    </div>
  `;

  bind(container);

  if (unsubscribe) unsubscribe();
  unsubscribe = chatStore.subscribe(state => {
    if (!document.body.contains(container)) {
      if (unsubscribe) unsubscribe();
      unsubscribe = null;
      return;
    }
    sync(container, state);
  });
}

function sync(container, state) {
  const title = container.querySelector('#chat-title');
  const select = container.querySelector('#chat-device-select');
  const statusText = container.querySelector('#chat-status-text');
  const input = container.querySelector('#chat-input');
  const send = container.querySelector('#chat-send');
  const attach = container.querySelector('#chat-attach');
  const chip = container.querySelector('#chat-file-chip');
  const chipName = container.querySelector('#chat-file-chip-name');
  const thread = container.querySelector('#chat-messages');
  if (!thread) return;

  if (title) title.textContent = state.agentLabel || 'ADTP AI';

  // Rebuild options only when devices actually change, so an open select is
  // never closed underneath the person using it.
  const deviceKey = state.devices.map(d => `${d.device_id}|${d.name}|${d.online}`).join(';');
  if (select && select.dataset.deviceKey !== deviceKey) {
    select.dataset.deviceKey = deviceKey;
    select.innerHTML = '<option value="">All devices</option>' +
      state.devices.map(d => `<option value="${escapeHtml(d.device_id)}">${escapeHtml(d.name || d.device_id)} — ${d.online ? 'Online' : 'Offline'}</option>`).join('');
  }
  if (select) select.value = state.selectedDeviceId || '';

  const enabled = state.agentState === 'active';
  const statusLabel = enabled ? 'Ready' : state.agentState === 'processing' ? 'Thinking...' : 'Offline';
  if (statusText) statusText.textContent = statusLabel;
  container.querySelectorAll('.agent-dot').forEach(dot => {
    dot.className = `agent-dot ${enabled ? 'active' : state.agentState === 'processing' ? 'processing motion-pulse' : 'inactive'}`;
  });
  if (input) input.disabled = !enabled;
  if (attach) attach.disabled = !enabled;
  if (send) send.disabled = !enabled || !(input?.value.trim() || state.attachedFile);

  if (chip) chip.hidden = !state.attachedFile;
  if (chipName) chipName.textContent = state.attachedFile?.name || '';

  const atBottom = thread.scrollHeight - thread.scrollTop - thread.clientHeight < 48;
  thread.innerHTML = renderConversation(state.messages, {
    expandedTools,
    devices: state.devices,
  });
  bindToolToggles(thread, expandedTools);
  if (atBottom || state.messages.length === 0) thread.scrollTop = thread.scrollHeight;
}

function bind(container) {
  const input = container.querySelector('#chat-input');
  const send = container.querySelector('#chat-send');
  const attach = container.querySelector('#chat-attach');
  const select = container.querySelector('#chat-device-select');
  const thread = container.querySelector('#chat-messages');
  if (!input || !send) return;

  bindToolToggles(thread, expandedTools);

  input.addEventListener('input', () => {
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, 120) + 'px';
    const state = chatStore.getState();
    send.disabled = state.agentState !== 'active' || !(input.value.trim() || state.attachedFile);
  });

  attach?.addEventListener('click', async () => {
    try {
      const path = await invoke('pick_file');
      if (!path) return;
      const name = path.split('/').pop() || path.split('\\').pop() || path;
      chatStore.emit('chat-attachment', { name, path });
    } catch (err) {
      const { show } = await import('../components/Toast.js');
      show(`File picker error: ${err}`, 'error');
    }
  });

  container.querySelector('#chat-file-chip-remove')?.addEventListener('click', e => {
    e.stopPropagation();
    chatStore.emit('chat-attachment', null);
  });

  select?.addEventListener('change', e => {
    chatStore.emit('chat-target', e.target.value || null);
  });

  send.addEventListener('click', () => void sendMessage(container));

  input.addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void sendMessage(container);
    }
  });
}

async function sendMessage(container) {
  const input = container.querySelector('#chat-input');
  if (!input) return;
  const state = chatStore.getState();
  const text = input.value.trim();
  const file = state.attachedFile;
  if (!text && !file) return;

  const filePath = file?.path || null;
  const fileName = file?.name || null;
  const targetId = state.selectedDeviceId;
  const target = state.devices.find(d => d.device_id === targetId);

  // One optimistic row represents the user's action in both surfaces. When a
  // phone is targeted, the durable device thread replaces this row by key.
  const optimisticId = chatStore.addMessage({
    role: 'user',
    origin: targetId ? 'device' : 'agent',
    deviceId: targetId || null,
    kind: filePath ? 'file' : 'text',
    text: text || `📎 ${fileName}`,
    filename: fileName,
    file_path: filePath,
    pending: true,
    status: 'sending',
  });

  input.value = '';
  input.style.height = 'auto';
  chatStore.emit('chat-attachment', null);
  chatStore.emit('chat-sending', true);
  chatStore.emit('agent-status', { state: 'processing' });

  let attachedMedia = null;

  try {
    // Compress + upload the attachment to the shared blob bucket first. The
    // agent then receives the same MediaRef shape MARS uses, so either side can
    // render the thumbnail + download link from one wire format.
    if (filePath) {
      try {
        attachedMedia = await invoke('upload_media', {
          path: filePath,
          sender: 'adtp-desktop',
          conversationId: 'default-session',
          recipient: targetId || undefined,
        });
      } catch (err) {
        console.warn('Media upload failed; falling back to local path', err);
      }
    }

    let deliveredMedia = false;
    if (attachedMedia && targetId) {
      // Deterministic delivery: don't rely on the model choosing a tool. Queue
      // the already-compressed MediaRef straight into the target phone's inbox
      // so MARS renders a real media card in chat.
      try {
        await invoke('send_message_to_device', {
          deviceId: targetId,
          text: text || '',
          media: attachedMedia,
        });
        deliveredMedia = true;
      } catch (err) {
        console.warn('Direct media delivery failed; asking the agent to retry', err);
      }
    } else if (filePath && targetId && !attachedMedia) {
      try {
        const queued = await invoke('send_file_to_device', { deviceId: targetId, filePath });
        // Without the blob relay, stage the bytes in the phone's inbox and
        // point a MediaRef at this gateway, which serves staged files on
        // /api/media/file/:id.
        let inboxMedia = null;
        try {
          const status = await invoke('get_api_status');
          const base = status?.phone_url || status?.url;
          const record = queued?.file;
          if (base && record?.id) {
            inboxMedia = {
              schemaVersion: 2,
              id: record.id,
              serviceUrl: String(base).replace(/\/$/, ''),
              name: record.filename || fileName,
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
          console.warn('Could not build an inbox media card', err);
        }
        await invoke('send_message_to_device', {
          deviceId: targetId,
          text: text || `📎 ${fileName}`,
          ...(inboxMedia ? { media: inboxMedia } : {}),
        });
        deliveredMedia = true;
      } catch (err) {
        console.warn('Plain file delivery failed', err);
      }
    }

    // If a specific device is selected, include it in the message context so
    // the AI can target it with send_file_to_device.
    const deviceName = target?.name || '';
    const deviceCtx = targetId
      ? `\n[Target device: ${deviceName ? `${deviceName} ` : ''}${targetId}]`
      : '';
    let agentMsg = `${text}${deviceCtx}`;
    if (attachedMedia) {
      agentMsg += `\n\n[Attached media: ${attachedMedia.name} — ${attachedMedia.serviceUrl}/api/media/file/${attachedMedia.id}?kind=original&download=1]`;
      if (deliveredMedia) {
        agentMsg += '\n[The media has already been delivered to the selected phone. Do not send it again.]';
      }
    } else if (filePath) {
      agentMsg += `\n\n[Attached file: ${filePath}]`;
    }

    const raw = await Promise.race([
      invoke('agent_send_message', { message: agentMsg }),
      new Promise((_, reject) => setTimeout(() => reject(new Error('Agent timed out after 30s')), 30000)),
    ]);

    // The agent may reply with a JSON envelope carrying a MediaRef; render it
    // with the same card as user media and keep the text readable.
    let replyContent = raw;
    let replyMedia = null;
    if (typeof raw === 'string' && raw.trim().startsWith('{')) {
      try {
        const parsed = JSON.parse(raw);
        if (parsed && (parsed.media_ref || parsed.media)) {
          replyMedia = parsed.media || parsed.media_ref;
          replyContent = parsed.text || parsed.message || '';
        }
      } catch { /* plain text reply */ }
    }

    if (attachedMedia) {
      chatStore.patchMessage(optimisticId, {
        media: attachedMedia,
        kind: 'media',
        delivered: true,
        status: 'sent',
      });
    }
    chatStore.addMessage({
      role: 'assistant',
      origin: 'agent',
      text: replyContent,
      media: replyMedia,
      deviceId: targetId || null,
    });
    chatStore.emit('agent-status', { state: 'active' });
  } catch (err) {
    chatStore.addMessage({
      role: 'assistant',
      origin: 'agent',
      text: `Error: ${err}`,
      error: String(err),
    });
    chatStore.emit('agent-status', { state: 'error' });
    setTimeout(() => chatStore.emit('agent-status', { state: 'active' }), 2000);
  } finally {
    chatStore.emit('chat-sending', false);
  }
}
