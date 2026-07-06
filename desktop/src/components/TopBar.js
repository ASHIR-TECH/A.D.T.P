export function render(container, title, notificationCount, agentState) {
  const dotClass = agentState === 'active' ? 'active' : agentState === 'processing' ? 'processing motion-pulse' : 'inactive';
  container.innerHTML = `
    <div id="topbar">
      <div class="page-title gold-text">${title}</div>
      <div class="topbar-actions">
        <button class="topbar-icon-btn" id="notif-bell">
          ${notificationCount > 0 ? `<span class="badge badge-notification">${notificationCount}</span>` : ''}
        </button>
        <span class="agent-dot ${dotClass}"></span>
        <button class="topbar-icon-btn"></button>
      </div>
    </div>
  `;
}
