const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let currentTab = 'dashboard';

function activateTab(tab) {
  document.querySelectorAll('.nav-list li').forEach(li => {
    li.classList.toggle('active', li.dataset.tab === tab);
  });
  currentTab = tab;
  loadPage(tab);
}

async function loadPage(tab) {
  const container = document.getElementById('page-container');
  container.innerHTML = '';

  try {
    const mod = await import(`./pages/${tab.charAt(0).toUpperCase() + tab.slice(1)}.js`);
    await mod.render(container);
  } catch (err) {
    container.innerHTML = `<p style="color: var(--red); padding: 24px;">Failed to load ${tab}: ${err.message}</p>`;
  }
}

document.addEventListener('DOMContentLoaded', () => {
  document.querySelectorAll('.nav-list li').forEach(li => {
    li.addEventListener('click', () => activateTab(li.dataset.tab));
  });

  listen('notify', (event) => {
    const { title, body } = event.payload;
    if (title && body) {
      import('./components/Toast.js').then(({ show }) => {
        show(`${title}: ${body}`, 'info');
      });
    }
  });

  listen('agent-response', (event) => {
    const container = document.getElementById('chat-panel-container');
    if (container) {
      import('./components/ChatPanel.js').then(({ render }) => render(container));
    }
  });

  listen('tray-send-path', (event) => {
    const path = event.payload;
    if (currentTab !== 'dashboard') {
      activateTab('dashboard');
    } else {
      loadPage('dashboard');
    }
  });

  activateTab('dashboard');
});
