export function renderBadge(variant, text) {
  const map = {
    verified: 'badge-verified',
    alerted: 'badge-alerted',
    pending: 'badge-pending',
    critical: 'badge-critical',
    relay: 'badge-relay',
  };
  const cls = map[variant] || 'badge-pending';
  return `<span class="badge ${cls}">${text || variant}</span>`;
}
