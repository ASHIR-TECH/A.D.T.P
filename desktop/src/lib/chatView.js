/**
 * One renderer for the one conversation. Both the dashboard panel and the
 * paired-phone drawer build their thread from this function and the same
 * `chatStore` snapshot, so they can never drift apart.
 */

import { escapeHtml, formatTime, formatDay, formatBytes, mediaCardHtml } from './chatFormat.js';

const STATUS_WHITELIST = new Set(['sent', 'delivered', 'pending', 'sending', 'failed', 'queued']);

export function renderConversation(messages, { expandedTools = new Set(), devices = [] } = {}) {
  if (!messages || messages.length === 0) {
    return '<div class="chat-empty">No messages yet — ask the agent or message a paired phone</div>';
  }
  const names = new Map(devices.map(d => [d.device_id, d.name]));
  let lastDay = '';

  return messages.map(msg => {
    const date = new Date(msg.timestamp);
    const dayKey = Number.isNaN(date.getTime()) ? '' : date.toDateString();
    let marker = '';
    if (dayKey && dayKey !== lastDay) {
      marker = `<div class="chat-day"><span>${escapeHtml(formatDay(msg.timestamp))}</span></div>`;
      lastDay = dayKey;
    }
    return marker + renderMessage(msg, {
      expandedTools,
      deviceName: names.get(msg.deviceId) || '',
    });
  }).join('');
}

export function renderMessage(msg, { expandedTools = new Set(), deviceName = '' } = {}) {
  const isUser = msg.role === 'user';
  const side = isUser ? 'right' : 'left';
  const label = isUser
    ? (msg.deviceId ? `To ${deviceName || shortId(msg.deviceId)}` : 'You')
    : msg.role === 'incoming'
      ? (deviceName || 'Phone')
      : (msg.label || 'Agent');

  let fileHtml = '';
  if ((msg.kind === 'file' || msg.kind === 'media') && (msg.filename || msg.filePath)) {
    const name = msg.filename || msg.filePath.split(/[\\/]/).pop() || msg.filePath;
    fileHtml = `
      <div class="chat-file-attached">
        <span aria-hidden="true">📎</span>
        <span>${escapeHtml(name)}</span>
        <span class="chat-file-size">${msg.size ? escapeHtml(formatBytes(msg.size)) : ''}</span>
      </div>`;
  }

  const mediaHtml = mediaCardHtml(msg.media);

  let bubbleHtml = '';
  if (msg.text) {
    bubbleHtml = `<div class="chat-msg-bubble">${escapeHtml(msg.text)}</div>`;
  }

  let transfersHtml = '';
  if (msg.transfers && msg.transfers.length > 0) {
    transfersHtml = `
      <div class="chat-transfers">
        ${msg.transfers.map(t => {
          const status = STATUS_WHITELIST.has(str(t.status)) ? str(t.status) : 'pending';
          return `
          <div class="chat-transfer">
            <span aria-hidden="true">📤</span>
            <span class="chat-transfer-name">${escapeHtml(t.filename || 'file')}</span>
            <span class="chat-transfer-status ${status}">${escapeHtml(status)}</span>
            ${t.device_id ? `<span class="chat-transfer-device">→ ${escapeHtml(t.device_id)}</span>` : ''}
          </div>`;
        }).join('')}
      </div>`;
  }

  let toolsHtml = '';
  if (msg.tool_calls && msg.tool_calls.length > 0) {
    toolsHtml = `
      <div class="chat-tool-steps">
        ${msg.tool_calls.map((tc, i) => {
          const key = `${msg.id}:${i}`;
          const open = expandedTools.has(key);
          const name = typeof tc === 'string' ? tc : (tc?.name || 'tool');
          return `
          <div class="chat-tool-call">
            <div class="chat-tool-call-header" data-tool-key="${escapeHtml(key)}">
              <span class="chat-tool-call-icon" aria-hidden="true">🔧</span>
              <span class="chat-tool-call-name">${escapeHtml(name)}</span>
              <span class="chat-tool-call-toggle">${open ? '−' : '+'}</span>
            </div>
            <div class="chat-tool-call-detail"${open ? '' : ' hidden'}>
              <code>${escapeHtml(JSON.stringify(tc, null, 2))}</code>
            </div>
          </div>`;
        }).join('')}
      </div>`;
  }

  const errorHtml = msg.error
    ? `<div class="chat-msg-error">${escapeHtml(msg.error)}</div>`
    : '';

  const status = STATUS_WHITELIST.has(str(msg.status)) ? str(msg.status) : 'sent';
  const tick = isUser
    ? `<span class="chat-msg-tick ${msg.delivered ? 'delivered' : ''}" title="${msg.delivered ? 'Delivered' : 'Queued'}">${msg.delivered ? '✓✓' : '✓'}</span>`
    : '';
  const time = escapeHtml(formatTime(msg.timestamp));

  return `
    <div class="chat-msg chat-msg-${side}" data-msg-id="${escapeHtml(msg.id)}">
      <div class="chat-msg-label">${escapeHtml(label)}</div>
      ${fileHtml}
      ${mediaHtml}
      ${bubbleHtml}
      ${transfersHtml}
      ${toolsHtml}
      ${errorHtml}
      <div class="chat-msg-meta">
        <time datetime="${escapeHtml(new Date(msg.timestamp).toISOString())}">${time}</time>
        <span class="chat-msg-state ${status}">${escapeHtml(status === 'sent' ? '' : status)}</span>
        ${tick}
      </div>
    </div>`;
}

/** Event delegation for expandable tool calls; no store mutation needed. */
export function bindToolToggles(root, expandedTools) {
  root.querySelectorAll('.chat-tool-call-header').forEach(header => {
    if (header.dataset.bound === '1') return;
    header.dataset.bound = '1';
    header.addEventListener('click', () => {
      const key = header.dataset.toolKey;
      const detail = header.nextElementSibling;
      const toggle = header.querySelector('.chat-tool-call-toggle');
      const open = !expandedTools.has(key);
      if (open) expandedTools.add(key);
      else expandedTools.delete(key);
      if (detail) detail.hidden = !open;
      if (toggle) toggle.textContent = open ? '−' : '+';
    });
  });
}

function str(value) {
  return value == null ? '' : String(value);
}

function shortId(id) {
  return str(id).slice(0, 10);
}
