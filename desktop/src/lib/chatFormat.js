/** Small, shared formatting helpers for both ADTP chat surfaces. */

import { mediaUnavailable, mediaStatusLabel } from './mediaCard.js';

export function escapeHtml(value) {
  const node = document.createElement('div');
  node.textContent = value == null ? '' : String(value);
  return node.innerHTML;
}

export function formatTime(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

export function formatDay(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === today.toDateString()) return 'Today';
  if (date.toDateString() === yesterday.toDateString()) return 'Yesterday';
  return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

export function formatBytes(bytes) {
  const size = Number(bytes);
  if (!Number.isFinite(size) || size <= 0) return 'File';
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * Media card for blob-backed attachments. Works for agent MediaRefs and for
 * inbound files staged by the gateway (`id` = transfer id, `serviceUrl` =
 * gateway). Download links always use the original + download=1 contract.
 */
export function mediaCardHtml(media) {
  if (!media || !media.id || !media.serviceUrl) return '';
  const base = String(media.serviceUrl).replace(/\/$/, '');
  const unavailable = mediaUnavailable(media);
  const thumbUrl = !unavailable && media.hasThumbnail
    ? `${base}/api/media/file/${encodeURIComponent(media.id)}?kind=thumbnail`
    : '';
  const downloadUrl = `${base}/api/media/file/${encodeURIComponent(media.id)}?kind=original&download=1`;
  return `
    <div class="chat-media-card">
      <div class="chat-media-thumb">
        ${thumbUrl
          ? `<img src="${escapeHtml(thumbUrl)}" alt="${escapeHtml(media.name || 'media')}" />`
          : '<span aria-hidden="true">🖼</span>'}
      </div>
      <div class="chat-media-detail">
        <div class="chat-media-name">${escapeHtml(media.name || 'media')}</div>
        <div class="chat-media-meta">${escapeHtml(mediaStatusLabel(media))}</div>
        ${unavailable ? '' : `<a class="chat-media-download" href="${escapeHtml(downloadUrl)}" target="_blank" rel="noopener noreferrer">⬇ Download</a>`}
      </div>
    </div>`;
}
