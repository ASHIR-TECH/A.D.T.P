const { invoke } = window.__TAURI__.core;
import { show } from '../components/Toast.js';

let currentStep = 1;
let setupState = {
  provider: '',
  model: '',
  apiKey: '',
  ollamaHost: 'http://localhost:11434',
  maxTokens: 4096,
  temperature: 0.7,
};

export function showSetup() {
  currentStep = 1;
  const container = document.getElementById('page-container');
  renderSetup(container);
}

function renderSetup(container) {
  container.innerHTML = `
    <h2 class="page-title">Set Up Agent</h2>
    <div class="setup-wizard">
      <div class="setup-steps">
        <div class="setup-step ${currentStep >= 1 ? 'active' : ''} ${currentStep > 1 ? 'done' : ''}">
          <span class="step-number">1</span>
          <span class="step-label">Provider</span>
        </div>
        <div class="setup-step ${currentStep >= 2 ? 'active' : ''} ${currentStep > 2 ? 'done' : ''}">
          <span class="step-number">2</span>
          <span class="step-label">API Key</span>
        </div>
        <div class="setup-step ${currentStep >= 3 ? 'active' : ''} ${currentStep > 3 ? 'done' : ''}">
          <span class="step-number">3</span>
          <span class="step-label">Permissions</span>
        </div>
      </div>
      <div class="setup-content" id="setup-content"></div>
    </div>
  `;

  renderStep(container);
}

function renderStep(container) {
  const content = container.querySelector('#setup-content');
  if (!content) return;

  switch (currentStep) {
    case 1:
      renderProviderStep(content);
      break;
    case 2:
      renderApiKeyStep(content);
      break;
    case 3:
      renderPermissionsStep(content);
      break;
  }
}

function renderProviderStep(container) {
  container.innerHTML = `
    <h3>Select Provider</h3>
    <p class="setup-desc">Choose an AI provider for your local agent.</p>
    <div class="provider-tiles">
      <div class="provider-tile ${setupState.provider === 'openai' ? 'selected' : ''}" data-provider="openai">
        <div class="provider-tile-icon">GPT</div>
        <div class="provider-tile-name">OpenAI (GPT)</div>
        <div class="provider-tile-desc">Uses gpt-4o. Internet connection required.</div>
      </div>
      <div class="provider-tile ${setupState.provider === 'anthropic' ? 'selected' : ''}" data-provider="anthropic">
        <div class="provider-tile-icon">CL</div>
        <div class="provider-tile-name">Anthropic (Claude)</div>
        <div class="provider-tile-desc">Uses claude-sonnet-4-6. Internet connection required.</div>
      </div>
      <div class="provider-tile ${setupState.provider === 'ollama' ? 'selected' : ''}" data-provider="ollama">
        <div class="provider-tile-icon">OL</div>
        <div class="provider-tile-name">Run Locally (Ollama)</div>
        <div class="provider-tile-desc">No internet required. Model runs locally.</div>
      </div>
    </div>
    <div class="setup-nav">
      <button class="btn btn-primary" id="setup-next" ${!setupState.provider ? 'disabled' : ''}>Next</button>
    </div>
  `;

  container.querySelectorAll('.provider-tile').forEach(tile => {
    tile.addEventListener('click', () => {
      container.querySelectorAll('.provider-tile').forEach(t => t.classList.remove('selected'));
      tile.classList.add('selected');
      setupState.provider = tile.dataset.provider;
      const next = container.querySelector('#setup-next');
      next.disabled = false;
    });
  });

  container.querySelector('#setup-next').addEventListener('click', () => {
    currentStep = 2;
    renderStep(document.getElementById('setup-content').parentElement);
  });
}

function renderApiKeyStep(container) {
  const isOllama = setupState.provider === 'ollama';

  container.innerHTML = `
    <h3>${isOllama ? 'Ollama Configuration' : 'API Key'}</h3>
    <p class="setup-desc">${isOllama ? 'Enter the host where Ollama is running.' : 'Enter your API key. It will be saved to your OS keychain.'}</p>
    ${!isOllama ? `
      <div class="settings-field">
        <label>API Key</label>
        <input type="password" id="setup-api-key" value="${setupState.apiKey}" placeholder="sk-..." />
      </div>
    ` : `
      <div class="settings-field">
        <label>Ollama Host</label>
        <input type="text" id="setup-ollama-host" value="${setupState.ollamaHost}" />
      </div>
    `}
    <div class="settings-field">
      <label>Model</label>
      <select id="setup-model">
        ${isOllama ? '<option value="llama3">Llama 3</option><option value="mistral">Mistral</option><option value="codellama">CodeLlama</option>' :
          setupState.provider === 'anthropic' ?
            '<option value="claude-sonnet-4-20250514">Claude Sonnet 4</option><option value="claude-opus-4-20250514">Claude Opus 4 (complex tasks)</option>' :
            '<option value="gpt-4o">GPT-4o</option><option value="gpt-4o-mini">GPT-4o Mini</option>'}
      </select>
    </div>
    <div class="setup-nav">
      <button class="btn btn-secondary" id="setup-back">Back</button>
      <button class="btn btn-primary" id="setup-next">Next</button>
    </div>
  `;

  const apiKeyInput = container.querySelector('#setup-api-key');
  const ollamaHostInput = container.querySelector('#setup-ollama-host');
  const modelSelect = container.querySelector('#setup-model');

  if (apiKeyInput) {
    apiKeyInput.addEventListener('change', () => { setupState.apiKey = apiKeyInput.value; });
  }
  if (ollamaHostInput) {
    ollamaHostInput.addEventListener('change', () => { setupState.ollamaHost = ollamaHostInput.value; });
  }
  if (modelSelect) {
    modelSelect.value = setupState.model || modelSelect.value;
    modelSelect.addEventListener('change', () => { setupState.model = modelSelect.value; });
  }

  container.querySelector('#setup-back').addEventListener('click', () => {
    currentStep = 1;
    renderStep(document.getElementById('setup-content').parentElement);
  });

  container.querySelector('#setup-next').addEventListener('click', () => {
    if (!isOllama && !setupState.apiKey) {
      show('API key is required', 'warning');
      return;
    }
    currentStep = 3;
    renderStep(document.getElementById('setup-content').parentElement);
  });
}

function renderPermissionsStep(container) {
  container.innerHTML = `
    <h3>Grant Permissions</h3>
    <p class="setup-desc">Set permissions for the agent. You can change these anytime from Settings.</p>
    <div id="setup-permissions-list"></div>
    <div class="setup-nav">
      <button class="btn btn-secondary" id="setup-back">Back</button>
      <button class="btn btn-primary" id="setup-activate">Activate Agent</button>
    </div>
  `;

  const permissions = [
    { key: 'documents', name: 'Documents', desc: '~/Documents — read access', granted: false },
    { key: 'downloads', name: 'Downloads', desc: '~/Downloads — read access', granted: false },
    { key: 'desktop', name: 'Desktop', desc: '~/Desktop — read access', granted: false },
    { key: 'hidden', name: 'Hidden Files', desc: 'Allow access to dotfiles (.env, .ssh)', granted: false },
    { key: 'shell', name: 'Shell Commands', desc: 'Allow: ls, cat, head, tail, wc, echo, date, pwd, whoami', granted: false },
    { key: 'remote', name: 'Remote Tasks', desc: 'Accept tasks from other peers', granted: true },
  ];

  const list = container.querySelector('#setup-permissions-list');
  permissions.forEach(p => {
    const item = document.createElement('div');
    item.className = 'permission-item';
    item.innerHTML = `
      <div class="permission-item-info">
        <div class="permission-item-name">${p.name}</div>
        <div class="permission-item-desc">${p.desc}</div>
      </div>
      <label class="permission-toggle">
        <input type="checkbox" ${p.granted ? 'checked' : ''} data-key="${p.key}" />
        <span class="toggle-slider"></span>
      </label>
    `;
    list.appendChild(item);
  });

  container.querySelector('#setup-back').addEventListener('click', () => {
    currentStep = 2;
    renderStep(document.getElementById('setup-content').parentElement);
  });

    container.querySelector('#setup-activate').addEventListener('click', async () => {
    const checked = {};
    container.querySelectorAll('#setup-permissions-list input[type="checkbox"]').forEach(cb => {
      checked[cb.dataset.key] = cb.checked;
    });

    const homedir = await invoke('get_home_dir').catch(() => '');
    const allowed = [];
    if (checked.documents && homedir) allowed.push(homedir + '/Documents');
    if (checked.downloads && homedir) allowed.push(homedir + '/Downloads');
    if (checked.desktop && homedir) allowed.push(homedir + '/Desktop');

    try {
      await invoke('agent_activate', {
        provider: setupState.provider,
        model: setupState.model || (setupState.provider === 'openai' ? 'gpt-4o' : setupState.provider === 'anthropic' ? 'claude-sonnet-4-20250514' : 'llama3'),
        apiKey: setupState.apiKey || '',
        ollamaHost: setupState.ollamaHost,
        maxTokens: setupState.maxTokens,
        permissions: {
          allowed_paths: allowed,
          writable_paths: [],
          denied_paths: [],
          allowed_commands: checked.shell ? ['ls', 'cat', 'head', 'tail', 'wc', 'echo', 'date', 'pwd', 'whoami', 'find', 'grep', 'sort', 'uniq', 'diff', 'file', 'du', 'df', 'ps'] : [],
          max_file_read_bytes: 10485760,
          allow_hidden_files: checked.hidden || false,
          allow_remote_tasks: checked.remote !== false,
        },
      });
      show('Agent activated successfully!', 'success');
      const { render } = await import('./Settings.js');
      const container = document.getElementById('page-container');
      await render(container);
    } catch (err) {
      show(`Failed to activate agent: ${err}`, 'error');
    }
  });
}
