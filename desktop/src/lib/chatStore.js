/**
 * The single conversation for ADTP desktop.
 *
 * Dashboard chat and the Settings → Paired Phones chat drawer are two views of
 * this store, not two independent message arrays. Surfaces still own their
 * Tauri `invoke` calls; this module owns message normalization, deduplication,
 * the shared target/attachment, and the incoming event stream.
 *
 *   const off = chatStore.subscribe(state => render(state));
 *   chatStore.emit('agent-message', payload);
 */

const listeners = new Set();
const MESSAGE_LIMIT = 500;

let eventsBound = false;
let localSeq = 0;

const state = {
  messages: [],
  devices: [],
  selectedDeviceId: null,   // null = "All devices"
  attachedFile: null,       // { name, path }
  agentState: 'inactive',
  agentLabel: 'ADTP AI',
  sending: false,
};

/* ------------------------------------------------------------------ *
 * Helpers
 * ------------------------------------------------------------------ */

function str(value, fallback = '') {
  return value == null ? fallback : String(value);
}

function first(src, keys, fallback = undefined) {
  if (!src || typeof src !== 'object') return fallback;
  for (const key of keys) {
    if (src[key] != null && src[key] !== '') return src[key];
  }
  return fallback;
}

function toTimestamp(value) {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (value == null || value === '') return Date.now();
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? Date.now() : date.getTime();
}

function normalizeToolCalls(value) {
  if (!value) return [];
  return Array.isArray(value) ? value : [value];
}

/** Stable identity for a message that the gateway already gave us an id for. */
function hasServerId(src) {
  return Boolean(first(src, ['id', 'message_id', 'log_id'], null));
}

/**
 * Canonical shape shared by both chat surfaces.
 * role: 'user' (desktop → phone / agent), 'incoming' (phone → desktop),
 *       'assistant' (agent reply). origin: 'device' | 'agent'.
 */
export function normalizeMessage(raw, defaults = {}) {
  const src = raw && typeof raw === 'object' ? raw : { content: str(raw) };
  const roleRaw = str(first(src, ['role', 'direction', 'from', 'sender'], defaults.role || '')).toLowerCase();
  let role = 'assistant';
  if (['user', 'me', 'outgoing', 'desktop'].includes(roleRaw)) role = 'user';
  if (['incoming', 'inbound', 'phone', 'device'].includes(roleRaw)) role = 'incoming';

  const origin = defaults.origin || (role === 'incoming' ? 'device' : 'agent');
  const media = first(src, ['media', 'media_ref'], null);
  const filePath = first(src, ['file_path', 'filePath', 'path'], null);
  const filename = first(src, ['filename', 'file_name', 'name'], media?.name || null);
  const kindRaw = str(first(src, ['kind', 'type'], '')).toLowerCase();
  const kind = ['file', 'media'].includes(kindRaw)
    ? kindRaw
    : media ? 'media' : (filePath || filename) ? 'file' : 'text';
  const delivered = Boolean(first(src, ['delivered', 'delivered_at', 'deliveredAt'], false));

  return {
    id: first(src, ['id', 'message_id', 'log_id'], null),
    origin,
    role,
    kind,
    deviceId: str(first(src, ['device_id', 'deviceId', 'target_device_id', 'recipient'], defaults.deviceId ?? null)) || null,
    label: first(src, ['label', 'sender_name'], null),
    text: str(first(src, ['content', 'text', 'message', 'body'], '')),
    filename: filename ? str(filename) : null,
    size: Number(first(src, ['file_size', 'fileSize', 'size'], 0)) || 0,
    media,
    filePath: filePath ? str(filePath) : null,
    timestamp: toTimestamp(first(src, ['timestamp', 'created_at', 'createdAt', 'time'], Date.now())),
    delivered,
    pending: Boolean(src.pending),
    status: str(first(src, ['status'], src.pending ? 'sending' : 'sent')),
    tool_calls: normalizeToolCalls(src.tool_calls),
    transfers: Array.isArray(src.transfers) ? src.transfers : [],
    error: first(src, ['error'], null),
  };
}

/** Phone-bound durable rows returned by `list_device_messages`. */
export function normalizeDeviceMessage(row, deviceId) {
  return normalizeMessage({
    id: row?.id,
    role: row?.direction === 'inbound' ? 'incoming' : 'user',
    text: row?.text,
    media: row?.media,
    created_at: row?.created_at,
    delivered_at: row?.delivered_at,
    device_id: row?.device_id || deviceId,
  }, { origin: 'device', deviceId });
}

/** Staged files returned by `list_device_files`. */
export function normalizeDeviceFile(row, deviceId) {
  return normalizeMessage({
    id: row?.id,
    kind: 'file',
    filename: row?.filename,
    file_size: row?.file_size,
    created_at: row?.created_at,
    delivered_at: row?.delivered_at,
    device_id: row?.device_id || deviceId,
  }, { origin: 'device', role: 'user', deviceId });
}

/** Attachments and media are the same user action; compare them by filename. */
function contentKey(msg) {
  return msg.filename
    ? `file:${str(msg.filename)}`
    : `text:${str(msg.text)}`;
}

/** The gateway describes inbound files as "📎 name"; match either shape. */
function attachmentName(msg) {
  if (msg.filename) return str(msg.filename);
  const match = str(msg.text).match(/📎\s*(.+)$/);
  return match ? match[1].trim() : '';
}

function sameAction(a, b) {
  if (a.deviceId !== b.deviceId) return false;
  if (contentKey(a) === contentKey(b)) return true;
  const nameA = attachmentName(a);
  const nameB = attachmentName(b);
  return Boolean(nameA && nameA === nameB);
}

/** Match an optimistic row to the durable row the gateway later returns. */
function pendingTwinIndex(msg) {
  if (!msg.deviceId || msg.pending) return -1;
  return state.messages.findIndex(m =>
    m.pending &&
    sameAction(m, msg) &&
    Math.abs(m.timestamp - msg.timestamp) < 5 * 60 * 1000
  );
}

/** The agent can emit a stream event and return the same final text via invoke. */
function isRecentAssistantEcho(msg) {
  if (msg.role !== 'assistant' || msg.text.length < 12) return false;
  return state.messages.some(m =>
    m.role === 'assistant' && m.text === msg.text && Math.abs(m.timestamp - msg.timestamp) < 8000
  );
}

function sortAndTrim() {
  state.messages.sort((a, b) => a.timestamp - b.timestamp);
  if (state.messages.length > MESSAGE_LIMIT) {
    state.messages.splice(0, state.messages.length - MESSAGE_LIMIT);
  }
}

function snapshot() {
  return Object.freeze({
    ...state,
    messages: Object.freeze(state.messages.slice()),
    devices: Object.freeze(state.devices.slice()),
    attachedFile: state.attachedFile ? Object.freeze({ ...state.attachedFile }) : null,
  });
}

function notify() {
  const current = snapshot();
  listeners.forEach(listener => {
    try { listener(current); } catch (err) { console.warn('[chatStore] subscriber failed', err); }
  });
}

/* ------------------------------------------------------------------ *
 * Store
 * ------------------------------------------------------------------ */

export const chatStore = {
  getState: snapshot,

  subscribe(listener) {
    listeners.add(listener);
    listener(snapshot());
    return () => listeners.delete(listener);
  },

  /** Add/merge one message. Returns its id, or null when it was a duplicate. */
  addMessage(raw, { silent = false } = {}) {
    const msg = normalizeMessage(raw);
    if (!msg.id) msg.id = `local-${++localSeq}`;

    const existing = state.messages.findIndex(m => m.id === msg.id);
    if (existing >= 0) {
      const merged = { ...state.messages[existing], ...msg };
      state.messages[existing] = merged;
      if (!silent) notify();
      return merged.id;
    }

    if (isRecentAssistantEcho(msg)) return null;

    const twin = pendingTwinIndex(msg);
    if (twin >= 0) {
      if (!hasServerId(raw)) msg.id = state.messages[twin].id;
      state.messages[twin] = msg;
    } else {
      state.messages.push(msg);
    }
    sortAndTrim();
    if (!silent) notify();
    return msg.id;
  },

  patchMessage(id, patch) {
    const index = state.messages.findIndex(m => m.id === id);
    if (index < 0) return;
    state.messages[index] = { ...state.messages[index], ...patch };
    notify();
  },

  /**
   * Merge the durable phone thread. Optimistic rows that the gateway has now
   * persisted are replaced (never appended twice); everything else is upserted
   * by server id.
   */
  syncDeviceThread(deviceId, messages = [], files = []) {
    const incoming = [
      ...(Array.isArray(messages) ? messages : []).map(row => normalizeDeviceMessage(row, deviceId)),
      ...(Array.isArray(files) ? files : []).map(row => normalizeDeviceFile(row, deviceId)),
    ];
    const beforeLength = state.messages.length;
    state.messages = state.messages.filter(m =>
      !(m.pending && m.deviceId === deviceId && incoming.some(row => sameAction(m, row)))
    );
    const reconciled = beforeLength !== state.messages.length;
    const knownIds = new Set(state.messages.map(m => m.id));
    let added = false;

    incoming.forEach(msg => {
      if (knownIds.has(msg.id)) return;
      const twin = pendingTwinIndex(msg);
      if (twin >= 0) {
        state.messages[twin] = msg;
      } else {
        state.messages.push(msg);
      }
      knownIds.add(msg.id);
      added = true;
    });

    // The drawer polls every 3s; stay quiet when the durable thread is
    // unchanged so neither surface re-renders for no reason.
    if (!reconciled && !added) return;
    sortAndTrim();
    notify();
  },

  setDevices(devices) {
    const list = Array.isArray(devices) ? devices : [];
    state.devices = list.map(device => ({ ...device }));
    if (state.selectedDeviceId && !list.some(d => d.device_id === state.selectedDeviceId)) {
      state.selectedDeviceId = list.length ? list[0].device_id : null;
    }
    notify();
  },

  setSelectedDevice(deviceId) {
    state.selectedDeviceId = deviceId || null;
    notify();
  },

  setAttachment(file) {
    state.attachedFile = file
      ? { name: str(file.name), path: str(file.path) }
      : null;
    notify();
  },

  setAgentState(agentState, label) {
    state.agentState = str(agentState, 'inactive');
    if (label) state.agentLabel = str(label);
    notify();
  },

  setSending(sending) {
    state.sending = Boolean(sending);
    notify();
  },

  /** All chat-domain events flow through here. */
  emit(type, payload) {
    switch (type) {
      case 'agent-message':
        this.addMessage({ ...(payload || {}), role: payload?.role || 'assistant' });
        break;
      case 'agent-status':
        this.setAgentState(payload?.state || 'inactive', payload?.label);
        break;
      case 'device-message':
      case 'device-message-inbound':
        this.addMessage({ ...(payload || {}), role: payload?.direction === 'inbound' ? 'incoming' : 'user' });
        break;
      case 'device-thread':
        this.syncDeviceThread(payload?.deviceId, payload?.messages, payload?.files);
        break;
      case 'device-list':
        this.setDevices(Array.isArray(payload) ? payload : payload?.devices);
        break;
      case 'chat-target':
        this.setSelectedDevice(payload);
        break;
      case 'chat-attachment':
        this.setAttachment(payload);
        break;
      case 'chat-sending':
        this.setSending(payload);
        break;
      default:
        break;
    }
  },
};

/**
 * Bind the incoming Tauri event stream once for the whole app. Invoke stays in
 * the surfaces; listening and normalization live here so neither surface can
 * miss a message or subscribe twice.
 */
export function initChatEvents() {
  if (eventsBound) return;
  const event = window.__TAURI__?.event;
  if (!event?.listen) return;
  eventsBound = true;

  event.listen('agent-message', e => chatStore.emit('agent-message', e.payload));
  event.listen('agent-status', e => chatStore.emit('agent-status', e.payload));
  event.listen('device-message', e => chatStore.emit('device-message', e.payload));
  event.listen('device-message-inbound', e => chatStore.emit('device-message-inbound', e.payload));
}
