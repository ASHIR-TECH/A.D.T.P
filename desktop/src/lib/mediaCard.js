/**
 * Shared MediaRef rendering helpers for ADTP's desktop chat surfaces.
 *
 * MARS uses the same rules in mobile/src/media/urls.ts: prefer the v2
 * `storedSize` field (with the v1 `compressedSize` alias) and treat terminal
 * statuses / elapsed `expiresAt` as unavailable. Keeping this in one module
 * means both desktop cards stay aligned with the phone's MediaCard.
 */

export function mediaSize(media) {
  if (!media) return 0;
  const value = media.storedSize ?? media.compressedSize ?? media.size ?? 0;
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? number : 0;
}

export function mediaUnavailable(media) {
  if (!media) return null;
  if (media.status === 'expired' || media.status === 'deleted') return media.status;
  if (typeof media.expiresAt === 'string' && media.expiresAt) {
    const deadline = Date.parse(media.expiresAt);
    if (Number.isFinite(deadline) && deadline <= Date.now()) return 'expired';
  }
  return null;
}

export function formatMediaSize(bytes) {
  if (!bytes) return '0 B';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function mediaStatusLabel(media) {
  const unavailable = mediaUnavailable(media);
  if (unavailable === 'expired') return 'Media expired';
  if (unavailable === 'deleted') return 'Media unavailable';
  const label = formatMediaSize(mediaSize(media));
  return media?.compressed ? `${label} · compressed` : label;
}
