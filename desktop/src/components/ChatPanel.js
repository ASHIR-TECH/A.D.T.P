const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let messages = [];
let agentState = 'inactive';
let agentLabel = 'ADTP AI';
let selectedDeviceId = null;   // which device/peer to send files to
let attachedFile = null;        // { name, path } currently attached

export async function render(container) {
  try {
    const agents = await invoke('agent_list');
    const active = agents.find(a => a.enabled);
    agentState = active ? 'active' : 'inactive';
    agentLabel = active && active.provider === 'base' ? 'ADTP AI' : 'Agent';
  } catch { agentState = 'inactive'; }

  try {
    const paired = await invoke('list_paired_devices');
    const devices = paired?.devices || [];
    selectedDeviceId = devices.length > 0 ? devices[0].device_id : null;
  } catch {}

  container.innerHTML = `
    <div class="chat-panel">
      <div class="chat-header">
        <span class="chat-title">${agentLabel}</span>
        <div class="chat-controls">
          <select class="chat-device-select" id="chat-device-select">
            <option value="">All devices</option>
          </select>
          <span class="agent-dot ${agentState === 'active' ? 'active' : agentState === 'processing' ? 'processing motion-pulse' : 'inactive'}"></span>
          <span id="chat-status-text">${agentState === 'active' ? 'Ready' : agentState === 'processing' ? 'Thinking...' : 'Offline'}</span>
        </div>
      </div>
      <div class="chat-messages" id="chat-messages">
        ${messages.length === 0 ? '<div class="chat-empty">Ask the agent something</div>' : ''}
        ${messages.map(m => renderMessage(m)).join('')}
      </div>
      <div class="chat-input-area">
        <!-- Attached file chip -->
        <div id="chat-file-chip" class="chat-file-chip" style="display:none">
          <span class="chat-file-chip-icon">📎</span>
          <span class="chat-file-chip-name" id="chat-file-chip-name"></span>
          <button class="chat-file-chip-remove" id="chat-file-chip-remove">&times;</button>
        </div>
        <textarea class="chat-input" id="chat-input" placeholder="Message the agent…" rows="1" maxlength="4000" ${agentState !== 'active' ? 'disabled' : ''}></textarea>
        <div class="chat-actions">
          <button class="btn btn-sm btn-secondary btn-chat-attach" id="chat-attach" title="Attach file" ${agentState !== 'active' ? 'disabled' : ''}>📎</button>
          <button class="btn btn-sm btn-primary btn-chat-send" id="chat-send" title="Send" ${agentState !== 'active' ? 'disabled' : ''}>Send</button>
        </div>
      </div>
    </div>
  `;

  setupChat(container);
  listenToEvents(container);
}

function renderMessage(m) {
  const isUser = m.role === 'user';
  const hasToolCalls = m.tool_calls && m.tool_calls.length > 0;
  const content = escapeHtml(m.content || '');
  const time = new Date(m.timestamp).toLocaleTimeString();

  // Attached file info (if the message carried a file path)
  let fileHtml = '';
  if (m.file_path) {
    const fileName = m.file_path.split('/').pop() || m.file_path.split('\\').pop() || m.file_path;
    fileHtml = `
      <div class="chat-file-attached">
        <span class="chat-file-icon">📎</span>
        <span class="chat-file-name">${escapeHtml(fileName)}</span>
      </div>
    `;
  }

  if (isUser) {
    return `
      <div class="chat-msg chat-msg-right">
        <div class="chat-msg-label">You</div>
        ${fileHtml}
        <div class="chat-msg-bubble">${content}</div>
        <div class="chat-msg-time">${time}</div>
      </div>
    `;
  }

  let toolSteps = '';
  if (hasToolCalls) {
    toolSteps = m.tool_calls.map((tc, i) => `
      <div class="chat-tool-call">
        <div class="chat-tool-call-header" data-index="${i}">
          <span class="chat-tool-call-icon">🔧</span>
          <span class="chat-tool-call-name">${escapeHtml(tc.name || tc)}</span>
          <span class="chat-tool-call-toggle">+</span>
        </div>
        <div class="chat-tool-call-detail" style="display:none">
          <code>${escapeHtml(JSON.stringify(tc, null, 2))}</code>
        </div>
      </div>
    `).join('');
  }

  // AI file transfer results (send_file_to_device, send_file, etc.)
  let transferHtml = '';
  if (m.transfers && m.transfers.length > 0) {
    transferHtml = `
      <div class="chat-transfers">
        ${m.transfers.map(t => `
          <div class="chat-transfer">
            <span class="chat-transfer-icon">📤</span>
            <span class="chat-transfer-name">${escapeHtml(t.filename || 'file')}</span>
            <span class="chat-transfer-status ${t.status || 'pending'}">${t.status || 'pending'}</span>
            ${t.device_id ? `<span class="chat-transfer-device">→ ${escapeHtml(t.device_id)}</span>` : ''}
          </div>
        `).join('')}
      </div>
    `;
  }

  return `
    <div class="chat-msg chat-msg-left">
      <div class="chat-msg-label">Agent</div>
      ${fileHtml}
      <div class="chat-msg-bubble">${content}</div>
      ${transferHtml}
      ${toolSteps ? `<div class="chat-tool-steps">${toolSteps}</div>` : ''}
      <div class="chat-msg-time">${time}</div>
    </div>
  `;
}

function listenToEvents(container) {
  listen('agent-message', (event) => {
    const payload = event.payload;
    // Normalize: tool_calls may be an array or a single object
    if (payload && payload.tool_calls && !Array.isArray(payload.tool_calls)) {
      payload.tool_calls = [payload.tool_calls];
    }
    messages.push(payload);
    renderMessages(container);
  });

  listen('agent-status', (event) => {
    agentState = event.payload?.state || 'inactive';
    updateChatUI(container);
  });
}

function updateChatUI(container) {
  const input = container.querySelector('#chat-input');
  const send = container.querySelector('#chat-send');
  const attach = container.querySelector('#chat-attach');
  const statusText = container.querySelector('#chat-status-text');
  const dots = container.querySelectorAll('.agent-dot');
  const deviceSelect = container.querySelector('#chat-device-select');
  if (!input) return;

  const enabled = agentState === 'active';
  input.disabled = !enabled;
  send.disabled = !enabled || !input.value.trim() && !attachedFile;
  attach.disabled = !enabled;
  statusText.textContent = enabled ? 'Ready' : agentState === 'processing' ? 'Thinking...' : 'Offline';
  dots.forEach(d => {
    d.className = `agent-dot ${enabled ? 'active' : agentState === 'processing' ? 'processing motion-pulse' : 'inactive'}`;
  });
}

function setupChat(container) {
  const input = container.querySelector('#chat-input');
  const send = container.querySelector('#chat-send');
  const attach = container.querySelector('#chat-attach');
  const fileChip = container.querySelector('#chat-file-chip');
  const fileChipName = container.querySelector('#chat-file-chip-name');
  const fileChipRemove = container.querySelector('#chat-file-chip-remove');
  const deviceSelect = container.querySelector('#chat-device-select');
  if (!input || !send) return;

  // Populate device select with paired phones
  (async () => {
    try {
      const paired = await invoke('list_paired_devices');
      const devices = paired?.devices || [];
      if (devices.length > 0) {
        deviceSelect.innerHTML = '<option value="">All devices</option>' +
          devices.map(d => `<option value="${escapeHtml(d.device_id)}">${escapeHtml(d.name)} — ${d.online ? 'Online' : 'Offline'}</option>`).join('');
        // Keep the select in sync with the default target chosen in render().
        selectedDeviceId = selectedDeviceId || devices[0].device_id;
        deviceSelect.value = selectedDeviceId;
      }
    } catch {}
  })();

  input.addEventListener('input', () => {
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, 120) + 'px';
    updateSendButton(send);
  });

  attach.addEventListener('click', async () => {
    try {
      const path = await invoke('pick_file');
      if (!path) return;
      const fileName = path.split('/').pop() || path.split('\\').pop() || path;
      attachedFile = { name: fileName, path };
      fileChipName.textContent = fileName;
      fileChip.style.display = 'flex';
      updateSendButton(send);
    } catch (err) {
      const { show } = await import('../components/Toast.js');
      show(`File picker error: ${err}`, 'error');
    }
  });

  fileChipRemove.addEventListener('click', (e) => {
    e.stopPropagation();
    attachedFile = null;
    fileChip.style.display = 'none';
    updateSendButton(send);
  });

  deviceSelect.addEventListener('change', (e) => {
    selectedDeviceId = e.target.value || null;
  });

  send.addEventListener('click', () => sendMessage(container));

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage(container);
    }
  });
}

function updateSendButton(sendBtn) {
  const hasText = document.querySelector('#chat-input').value.trim().length > 0;
  sendBtn.disabled = !(hasText || attachedFile);
}

async function sendMessage(container) {
  const input = container.querySelector('#chat-input');
  const text = input.value.trim();
  if (!text && !attachedFile) return;

  const filePath = attachedFile ? attachedFile.path : null;
  const fileName = attachedFile ? attachedFile.name : null;

  // Optimistic append — the user's message renders instantly
  const userMsg = { role: 'user', content: text || `[Sent file: ${fileName}]`, timestamp: Date.now(), file_path: filePath };
  messages.push(userMsg);
  renderMessages(container);
  input.value = '';
  input.style.height = 'auto';
  attachedFile = null;
  container.querySelector('#chat-file-chip').style.display = 'none';
  updateSendButton(container.querySelector('#chat-send'));

  agentState = 'processing';
  updateChatUI(container);

  try {
    // Send to the desktop agent. If a specific device is selected, include it
    // in the message context so the AI can target it with send_file_to_device.
    const deviceName = selectedDeviceId
      ? (container.querySelector('#chat-device-select')?.selectedOptions?.[0]?.textContent || '')
          .replace(/\s+—\s+(Online|Offline)$/, '')
      : '';
    const deviceCtx = selectedDeviceId
      ? `\n[Target device: ${deviceName ? `${deviceName} ` : ''}${selectedDeviceId}]`
      : '';
    // Use the captured filePath — `attachedFile` is cleared above so the chip
    // disappears immediately, which used to drop the attachment entirely.
    const agentMsg = filePath
      ? `${text}${deviceCtx}\n\n[Attached file: ${filePath}]`
      : `${text}${deviceCtx}`;
    const response = await Promise.race([
      invoke('agent_send_message', { message: agentMsg }),
      new Promise((_, reject) => setTimeout(() => reject(new Error('Agent timed out after 30s')), 30000)),
    ]);
    messages.push({ role: 'assistant', content: response, timestamp: Date.now() });
    agentState = 'active';
  } catch (err) {
    messages.push({ role: 'assistant', content: `Error: ${err}`, timestamp: Date.now() });
    agentState = 'error';
    setTimeout(() => { agentState = 'active'; updateChatUI(container); }, 2000);
  }
  renderMessages(container);
  updateChatUI(container);
}

function renderMessages(container) {
  const el = container.querySelector('#chat-messages');
  if (!el) return;
  if (messages.length === 0) {
    el.innerHTML = '<div class="chat-empty">Ask the agent something</div>';
    return;
  }
  el.innerHTML = messages.map(m => renderMessage(m)).join('');
  el.querySelectorAll('.chat-tool-call-header').forEach(header => {
    header.addEventListener('click', () => {
      const detail = header.nextElementSibling;
      const toggle = header.querySelector('.chat-tool-call-toggle');
      if (detail) {
        const isHidden = detail.style.display === 'none' || !detail.style.display;
        detail.style.display = isHidden ? 'block' : 'none';
        toggle.textContent = isHidden ? '−' : '+';
      }
    });
  });
  el.scrollTop = el.scrollHeight;
}

function escapeHtml(text) {
  const d = document.createElement('div');
  d.textContent = text;
  return d.innerHTML;
}
