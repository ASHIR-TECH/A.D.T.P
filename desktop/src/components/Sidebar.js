const NAV_ITEMS = [
  { tab: 'dashboard', icon: '', label: 'Dashboard' },
  { tab: 'history', icon: '', label: 'History' },
  { tab: 'settings', icon: '', label: 'Settings' },
];

export function render(container, activeTab, onNavigate) {
  container.innerHTML = `
    <div class="sidebar-header">
      <div class="sidebar-wordmark">ADTP</div>
    </div>
    <ul class="nav-list">
      ${NAV_ITEMS.map(item => `
        <li class="${item.tab === activeTab ? 'active' : ''}" data-tab="${item.tab}">
          <span class="nav-icon">${item.icon}</span>
          ${item.label}
        </li>
      `).join('')}
    </ul>
  `;

  container.querySelectorAll('.nav-list li').forEach(el => {
    el.addEventListener('click', () => {
      const tab = el.dataset.tab;
      if (tab === 'logout') return;
      container.querySelectorAll('.nav-list li').forEach(li => li.classList.remove('active'));
      el.classList.add('active');
      if (onNavigate) onNavigate(tab);
    });
  });
}
