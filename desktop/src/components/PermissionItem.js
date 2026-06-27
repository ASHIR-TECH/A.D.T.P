export function render(container, item) {
  const el = document.createElement('div');
  el.className = 'permission-item';
  el.innerHTML = `
    <div class="permission-item-info">
      <div class="permission-item-name">${escapeHtml(item.name)}</div>
      ${item.description ? `<div class="permission-item-desc">${escapeHtml(item.description)}</div>` : ''}
      ${item.last_used ? `<div class="permission-item-used">Last used: ${new Date(item.last_used).toLocaleString()}</div>` : ''}
    </div>
    <label class="permission-toggle">
      <input type="checkbox" ${item.granted ? 'checked' : ''} data-key="${escapeHtml(item.key)}" />
      <span class="toggle-slider"></span>
    </label>
  `;

  const checkbox = el.querySelector('input[type="checkbox"]');
  checkbox.addEventListener('change', () => {
    item.granted = checkbox.checked;
    if (item.onChange) item.onChange(item.key, checkbox.checked);
  });

  container.appendChild(el);
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}
