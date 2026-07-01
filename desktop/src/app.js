const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

import { render as renderSidebar } from './components/Sidebar.js';
import { render as renderTopBar } from './components/TopBar.js';

let currentTab = 'dashboard';
let notificationCount = 0;
let agentState = 'inactive';

function getPageTitle(tab) {
  const titles = { dashboard: 'Dashboard', history: 'History', settings: 'Settings' };
  return titles[tab] || tab;
}

function activateTab(tab) {
  currentTab = tab;
  renderTopBar(document.getElementById('topbar') || document.createElement('div'), getPageTitle(tab), notificationCount, agentState);
  loadPage(tab);
}

async function loadPage(tab) {
  const container = document.getElementById('page-container');
  const pageContainer = container;

  pageContainer.classList.add('motion-fade-out');
  setTimeout(async () => {
    pageContainer.innerHTML = '';
    pageContainer.classList.remove('motion-fade-out');
    try {
      const mod = await import(`./pages/${tab.charAt(0).toUpperCase() + tab.slice(1)}.js`);
      await mod.render(pageContainer);
    } catch (err) {
      pageContainer.innerHTML = `<p style="color:var(--ember);padding:24px;">Failed to load ${tab}: ${err.message}</p>`;
    }
    pageContainer.classList.add('motion-fade-in');
    setTimeout(() => pageContainer.classList.remove('motion-fade-in'), 80);
  }, 80);
}

document.addEventListener('DOMContentLoaded', () => {
  renderSidebar(document.getElementById('sidebar'), currentTab, activateTab);

  const topbar = document.createElement('div');
  topbar.id = 'topbar';
  document.getElementById('content').insertBefore(topbar, document.getElementById('page-container'));
  renderTopBar(topbar, getPageTitle(currentTab), notificationCount, agentState);

  listen('notify', (event) => {
    import('./components/Toast.js').then(({ show }) => {
      const { title, body } = event.payload;
      if (title && body) show(`${title}: ${body}`, 'info');
    });
  });

  listen('agent-status', (event) => {
    agentState = event.payload?.state || 'inactive';
    const topbar = document.getElementById('topbar');
    if (topbar) renderTopBar(topbar, getPageTitle(currentTab), notificationCount, agentState);
  });

  activateTab('dashboard');
});
