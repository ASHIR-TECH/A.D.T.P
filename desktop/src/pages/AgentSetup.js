const { invoke } = window.__TAURI__.core;
import { show } from '../components/Toast.js';
import { showModalContent } from '../components/Modal.js';
import { render as renderSettings } from './Settings.js';

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
  setupState.provider = '';
  setupState.apiKey = '';
  setupState.model = '';
  const modal = showModalContent(document.body, 'Set Up Agent', renderStepContent());
  renderStepEvents(modal);
}

function renderStepContent() {
  switch (currentStep) {
    case 1: return renderProviderStep();
    case 2: return renderApiKeyStep();
    case 3: return renderPermissionsStep();
    default: return '';
  }
}

function renderStepEvents(modal) {
  setTimeout(() => {
    const content = document.getElementById('modal-body');
    if (!content) return;

    if (currentStep === 1) {
      content.querySelectorAll('.provider-tile').forEach(tile => {
        tile.addEventListener('click', () => {
          content.querySelectorAll('.provider-tile').forEach(t => t.classList.remove('selected'));
          tile.classList.add('selected');
          setupState.provider = tile.dataset.provider;
          const next = content.querySelector('#setup-next');
          if (next) next.disabled = false;
        });
      });
      content.querySelector('#setup-next')?.addEventListener('click', () => {
        currentStep = 2;
        modal.setContent(renderStepContent());
        renderStepEvents(modal);
      });
    }

    if (currentStep === 2) {
      const keyInput = content.querySelector('#setup-api-key');
      const hostInput = content.querySelector('#setup-ollama-host');
      const modelSelect = content.querySelector('#setup-model');
      if (keyInput) keyInput.addEventListener('change', () => { setupState.apiKey = keyInput.value; });
      if (hostInput) hostInput.addEventListener('change', () => { setupState.ollamaHost = hostInput.value; });
      if (modelSelect) {
        setupState.model = modelSelect.value;
        modelSelect.addEventListener('change', () => { setupState.model = modelSelect.value; });
      }
      content.querySelector('#setup-back')?.addEventListener('click', () => {
        currentStep = 1;
        modal.setContent(renderStepContent());
        renderStepEvents(modal);
      });
      content.querySelector('#setup-next')?.addEventListener('click', () => {
        if (setupState.provider !== 'ollama' && !setupState.apiKey) {
          show('API key is required', 'warning');
          return;
        }
        currentStep = 3;
        modal.setContent(renderStepContent());
        renderStepEvents(modal);
      });
    }

    if (currentStep === 3) {
      content.querySelector('#setup-back')?.addEventListener('click', () => {
        currentStep = 2;
        modal.setContent(renderStepContent());
        renderStepEvents(modal);
      });
      content.querySelector('#setup-activate')?.addEventListener('click', async () => {
        const checked = {};
        content.querySelectorAll('#setup-permissions-list input[type="checkbox"]').forEach(cb => {
          checked[cb.dataset.key] = cb.checked;
        });
        const homedir = await invoke('get_home_dir').catch(() => '');
        const allowed = [];
        if (checked.documents && homedir) allowed.push(homedir + '/Documents');
        if (checked.downloads && homedir) allowed.push(homedir + '/Downloads');
        if (checked.desktop && homedir) allowed.push(homedir + '/Desktop');

        try {
          const agent = await invoke('agent_add', { request: {
            provider: setupState.provider,
            model: setupState.model || (setupState.provider === 'openai' ? 'gpt-4o' : setupState.provider === 'anthropic' ? 'claude-sonnet-4-20250514' : 'llama3'),
            apiKey: setupState.apiKey || '',
            ollamaHost: setupState.ollamaHost,
            maxTokens: setupState.maxTokens,
            permissions: {
              allowed_paths: allowed,
              writable_paths: [],
              denied_paths: [],
              allowed_commands: checked.shell ? ['ls','cat','head','tail','wc','echo','date','pwd','whoami','find','grep','sort','uniq','diff','file','du','df','ps'] : [],
              max_file_read_bytes: 10485760,
              allow_hidden_files: checked.hidden || false,
              allow_remote_tasks: checked.remote !== false,
            },
            enabled: true,
          }});
          show(`Agent "${agent.name}" added and enabled!`, 'success', 'top-right');
          modal.close();
          const pc = document.getElementById('page-container');
          if (pc) renderSettings(pc);
        } catch (err) {
          show(`Failed: ${err}`, 'error');
        }
      });
    }
  }, 50);
}

function renderProviderStep() {
  return `
    <h3>Select Provider</h3>
    <p class="setup-desc">Choose an AI provider for your local agent.</p>
    <div class="provider-tiles">
      <div class="provider-tile ${setupState.provider === 'openai' ? 'selected' : ''}" data-provider="openai">
        <img class="provider-tile-icon" src="icons/chatgpt.jpg" alt="OpenAI" />
        <div class="provider-tile-info">
          <div class="provider-tile-name">GPT (OpenAI)</div>
          <div class="provider-tile-desc"> Internet required.</div>
        </div>
      </div>
      <div class="provider-tile ${setupState.provider === 'anthropic' ? 'selected' : ''}" data-provider="anthropic">
        <img class="provider-tile-icon" src="icons/claude-color.svg" alt="Claude" />
        <div class="provider-tile-info">
          <div class="provider-tile-name">Claude(Anthropic)</div>
          <div class="provider-tile-desc">Internet required.</div>
        </div>
      </div>
      <div class="provider-tile ${setupState.provider === 'ollama' ? 'selected' : ''}" data-provider="ollama">
        <img class="provider-tile-icon" src="icons/ollama.svg" alt="Ollama" />
        <div class="provider-tile-info">
          <div class="provider-tile-name">Ollama (Locally)</div>
          <div class="provider-tile-desc">No internet required.</div>
        </div>
      </div>
    </div>
    <div class="setup-nav">
      <button class="btn btn-primary" id="setup-next" ${!setupState.provider ? 'disabled' : ''}>Next</button>
    </div>
  `;
}

function renderApiKeyStep() {
  const isOllama = setupState.provider === 'ollama';
  return `
    <h3>${isOllama ? 'Ollama Configuration' : 'API Key'}</h3>
    <p class="setup-desc">${isOllama ? 'Enter the host where Ollama is running.' : 'Your key is saved to the OS keychain, never to disk.'}</p>
    ${!isOllama ? `
      <div class="field-group">
        <label>API Key</label>
        <input type="password" id="setup-api-key" value="${setupState.apiKey}" placeholder="sk-..." />
      </div>
    ` : `
      <div class="field-group">
        <label>Ollama Host</label>
        <input type="text" id="setup-ollama-host" value="${setupState.ollamaHost}" />
      </div>
    `}
    <div class="field-group">
      <label>Model</label>
      <select id="setup-model">
        ${isOllama ? '<option value="llama3">Llama 3</option><option value="mistral">Mistral</option>' :
          setupState.provider === 'anthropic' ?
            '<option value="claude-sonnet-4-20250514">Claude Sonnet 4</option><option value="claude-opus-4-20250514">Claude Opus 4</option>' :
            '<option value="gpt-4o">GPT-4o</option><option value="gpt-4o-mini">GPT-4o Mini</option>'}
      </select>
    </div>
    <div class="setup-nav">
      <button class="btn btn-secondary" id="setup-back">Back</button>
      <button class="btn btn-primary" id="setup-next">Next</button>
    </div>
  `;
}

function renderPermissionsStep() {
  return `
    <h3>Grant Permissions</h3>
    <p class="setup-desc">Set permissions for the agent. Change anytime from Settings.</p>
    <div id="setup-permissions-list">
      ${[
        { key: 'documents', name: 'Documents', desc: '~/Documents — read access' },
        { key: 'downloads', name: 'Downloads', desc: '~/Downloads — read access' },
        { key: 'desktop', name: 'Desktop', desc: '~/Desktop — read access' },
        { key: 'hidden', name: 'Hidden Files', desc: 'Allow dotfiles (.env, .ssh)' },
        { key: 'shell', name: 'Shell Commands', desc: 'ls, cat, head, tail, wc, echo, date, pwd, whoami' },
        { key: 'remote', name: 'Remote Tasks', desc: 'Accept tasks from other peers', granted: true },
      ].map(p => `
        <div class="permission-item">
          <div class="permission-item-info">
            <div class="permission-item-name">${p.name}</div>
            <div class="permission-item-desc">${p.desc}</div>
          </div>
          <label class="permission-toggle">
            <input type="checkbox" data-key="${p.key}" ${p.granted ? 'checked' : ''} />
            <span class="toggle-slider"></span>
          </label>
        </div>
      `).join('')}
    </div>
    <div class="setup-nav">
      <button class="btn btn-secondary" id="setup-back">Back</button>
      <button class="btn btn-primary" id="setup-activate">Activate Agent</button>
    </div>
  `;
}
