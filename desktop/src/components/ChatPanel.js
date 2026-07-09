const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let messages = [];
let agentState = 'inactive';

export async function render(container) {
  try {
    const agents = await invoke('agent_list');
    agentState = agents.some(a => a.enabled) ? 'active' : 'inactive';
  } catch { agentState = 'inactive'; }

  container.innerHTML = `
    <div class="chat-panel">
      <div class="chat-header">
        <span class="chat-title">Agent</span>
        <span class="chat-status">
          <span class="agent-dot ${agentState === 'active' ? 'active' : agentState === 'processing' ? 'processing motion-pulse' : 'inactive'}"></span>
          <span id="chat-status-text">${agentState === 'active' ? 'Ready' : agentState === 'processing' ? 'Thinking...' : 'Offline'}</span>
        </span>
      </div>
      <div class="chat-messages" id="chat-messages">
        ${messages.length === 0 ? '<div class="chat-empty">Ask the agent something</div>' : ''}
        ${messages.map(m => renderMessage(m)).join('')}
      </div>
      <div class="chat-input-area">
        <textarea class="chat-input" id="chat-input" placeholder="Ask the agent..." rows="1" maxlength="4000" ${agentState !== 'active' ? 'disabled' : ''}></textarea>
        <button class="btn btn-sm btn-primary btn-chat-send" id="chat-send" ${agentState !== 'active' ? 'disabled' : ''}>Send</button>
      </div>
    </div>
  `;

  setupChat(container);
  listenToEvents(container);
}

function renderMessage(m) {
  const isUser = m.role === 'user';
  const hasToolCalls = m.tool_calls && m.tool_calls.length > 0;
  const content = escapeHtml(m.content);
  const time = new Date(m.timestamp).toLocaleTimeString();

  if (isUser) {
    return `
      <div class="chat-msg chat-msg-right">
        <div class="chat-msg-label">You</div>
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
          <span class="chat-tool-call-name">${escapeHtml(tc)}</span>
          <span class="chat-tool-call-toggle">+</span>
        </div>
        <div class="chat-tool-call-detail" style="display:none">
          <code>${escapeHtml(JSON.stringify(tc, null, 2))}</code>
        </div>
      </div>
    `).join('');
  }

  return `
    <div class="chat-msg chat-msg-left">
      <div class="chat-msg-label">Agent</div>
      <div class="chat-msg-bubble">
        ${content}
        ${toolSteps ? `<div class="chat-tool-steps">${toolSteps}</div>` : ''}
      </div>
      <div class="chat-msg-time">${time}</div>
    </div>
  `;
}

function listenToEvents(container) {
  listen('agent-message', (event) => {
    messages.push(event.payload);
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
  const statusText = container.querySelector('#chat-status-text');
  const dots = container.querySelectorAll('.agent-dot');
  if (!input) return;

  const enabled = agentState === 'active';
  input.disabled = !enabled;
  send.disabled = !enabled;
  statusText.textContent = enabled ? 'Ready' : agentState === 'processing' ? 'Thinking...' : 'Offline';
  dots.forEach(d => {
    d.className = `agent-dot ${enabled ? 'active' : agentState === 'processing' ? 'processing motion-pulse' : 'inactive'}`;
  });
}

function setupChat(container) {
  const input = container.querySelector('#chat-input');
  const send = container.querySelector('#chat-send');
  if (!input || !send) return;

  input.addEventListener('input', () => {
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, 120) + 'px';
  });

  send.addEventListener('click', () => sendMessage(container));

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage(container);
    }
  });
}

async function sendMessage(container) {
  const input = container.querySelector('#chat-input');
  const text = input.value.trim();
  if (!text) return;

  messages.push({ role: 'user', content: text, timestamp: Date.now() });
  renderMessages(container);
  input.value = '';
  input.style.height = 'auto';

  agentState = 'processing';
  updateChatUI(container);

  try {
    const response = await invoke('agent_send_message', { message: text });
    messages.push({ role: 'assistant', content: response, timestamp: Date.now() });
    agentState = 'active';
  } catch (err) {
    messages.push({ role: 'assistant', content: `Error: ${err}`, timestamp: Date.now() });
    agentState = 'error';
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
