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
        ${messages.map(m => `
          <div class="chat-msg ${m.role === 'user' ? 'chat-msg-right' : 'chat-msg-left'}">
            <div class="chat-msg-label">${m.role === 'user' ? 'You' : 'Agent'}</div>
            <div class="chat-msg-bubble">${escapeHtml(m.content)}</div>
            <div class="chat-msg-time">${new Date(m.timestamp).toLocaleTimeString()}</div>
          </div>
        `).join('')}
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
  el.innerHTML = messages.map(m => `
    <div class="chat-msg ${m.role === 'user' ? 'chat-msg-right' : 'chat-msg-left'}">
      <div class="chat-msg-label">${m.role === 'user' ? 'You' : 'Agent'}</div>
      <div class="chat-msg-bubble">${escapeHtml(m.content)}</div>
      <div class="chat-msg-time">${new Date(m.timestamp).toLocaleTimeString()}</div>
    </div>
  `).join('');
  el.scrollTop = el.scrollHeight;
}

function escapeHtml(text) {
  const d = document.createElement('div');
  d.textContent = text;
  return d.innerHTML;
}
