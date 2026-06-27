const { invoke, listen } = window.__TAURI__.core;
const { listen: eventListen } = window.__TAURI__.event;

let messages = [];
let agentState = 'inactive';

export async function render(container) {
  container.innerHTML = `
    <div id="chat-panel" class="chat-panel">
      <div class="chat-header">
        <span class="chat-title">Agent</span>
        <span class="chat-status" id="chat-status">
          <span class="chat-dot ${agentState}" id="chat-dot"></span>
          <span id="chat-status-text">${agentState === 'active' ? 'Ready' : agentState === 'processing' ? 'Thinking...' : 'Not configured'}</span>
        </span>
      </div>
      <div class="chat-messages" id="chat-messages">
        <div class="chat-empty">Configure an agent in Settings to start chatting.</div>
      </div>
      <div class="chat-input-area">
        <textarea class="chat-input" id="chat-input" placeholder="Ask the agent..." rows="1" maxlength="4000"></textarea>
        <button class="btn btn-primary btn-chat-send" id="chat-send" disabled>Send</button>
      </div>
    </div>
  `;

  const status = await invoke('agent_get_status').catch(() => null);
  if (status && status.active) {
    agentState = 'active';
    updateStatus(container);
  }

  setupInputAutoResize(container);
  setupSendHandler(container);
  setupKeyHandler(container);

  eventListen('agent-message', (event) => {
    messages.push(event.payload);
    renderMessages(container);
  });

  eventListen('agent-status', (event) => {
    agentState = event.payload.state || 'inactive';
    updateStatus(container);
  });
}

function updateStatus(container) {
  const dot = container.querySelector('#chat-dot');
  const text = container.querySelector('#chat-status-text');
  const input = container.querySelector('#chat-input');
  const sendBtn = container.querySelector('#chat-send');
  if (!dot) return;

  dot.className = `chat-dot ${agentState}`;
  switch (agentState) {
    case 'active':
      text.textContent = 'Ready';
      input.disabled = false;
      sendBtn.disabled = false;
      break;
    case 'processing':
      text.textContent = 'Thinking...';
      input.disabled = true;
      sendBtn.disabled = true;
      break;
    case 'error':
      text.textContent = 'Error';
      input.disabled = true;
      sendBtn.disabled = true;
      break;
    default:
      text.textContent = 'Not configured';
      input.disabled = true;
      sendBtn.disabled = true;
  }
}

function setupInputAutoResize(container) {
  const input = container.querySelector('#chat-input');
  input.addEventListener('input', () => {
    input.style.height = 'auto';
    input.style.height = Math.min(input.scrollHeight, 120) + 'px';
  });
}

function setupSendHandler(container) {
  const input = container.querySelector('#chat-input');
  const sendBtn = container.querySelector('#chat-send');

  sendBtn.addEventListener('click', () => sendMessage(container));
}

function setupKeyHandler(container) {
  const input = container.querySelector('#chat-input');
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
  updateStatus(container);

  try {
    const response = await invoke('agent_send_message', { message: text });
    messages.push({ role: 'assistant', content: response, timestamp: Date.now() });
    agentState = 'active';
  } catch (err) {
    messages.push({ role: 'assistant', content: `Error: ${err}`, timestamp: Date.now() });
    agentState = 'error';
  }
  renderMessages(container);
  updateStatus(container);
}

function renderMessages(container) {
  const el = container.querySelector('#chat-messages');
  if (!el) return;

  if (messages.length === 0) {
    el.innerHTML = '<div class="chat-empty">No messages yet. Ask the agent something!</div>';
    return;
  }

  el.innerHTML = messages.map(m => {
    const align = m.role === 'user' ? 'chat-msg-right' : 'chat-msg-left';
    const label = m.role === 'user' ? 'You' : 'Agent';
    return `
      <div class="chat-msg ${align}">
        <div class="chat-msg-label">${label}</div>
        <div class="chat-msg-bubble">${escapeHtml(m.content)}</div>
        <div class="chat-msg-time">${new Date(m.timestamp).toLocaleTimeString()}</div>
      </div>
    `;
  }).join('');

  el.scrollTop = el.scrollHeight;
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}
