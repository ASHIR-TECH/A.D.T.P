export function renderCard(container, agent, index, onUpdate, onRemove) {
  const card = document.createElement('div');
  card.className = 'agent-card';

  const name = agent.name || `Agent ${index + 1}`;

  card.innerHTML = `
    <div class="agent-card-header">
      <h4>${name}</h4>
      <button class="btn-icon remove-agent" title="Remove agent">✕</button>
    </div>
    <div class="agent-fields">
      <div class="settings-field">
        <label>Name</label>
        <input type="text" class="agent-name" value="${agent.name || ''}" />
      </div>
      <div class="settings-field">
        <label>Host</label>
        <input type="text" class="agent-host" value="${agent.host || ''}" />
      </div>
      <div class="settings-field">
        <label>Port</label>
        <input type="number" class="agent-port" value="${agent.port || 0}" />
      </div>
      <div class="settings-field">
        <label>Default</label>
        <select class="agent-default">
          <option value="false" ${agent.default ? '' : 'selected'}>No</option>
          <option value="true" ${agent.default ? 'selected' : ''}>Yes</option>
        </select>
      </div>
    </div>
  `;

  card.querySelector('.remove-agent').addEventListener('click', () => onRemove(index));

  const nameInput = card.querySelector('.agent-name');
  const hostInput = card.querySelector('.agent-host');
  const portInput = card.querySelector('.agent-port');
  const defaultInput = card.querySelector('.agent-default');

  [nameInput, hostInput, portInput, defaultInput].forEach(input => {
    input.addEventListener('change', () => {
      onUpdate(index, {
        name: nameInput.value,
        host: hostInput.value,
        port: parseInt(portInput.value) || 0,
        default: defaultInput.value === 'true',
      });
    });
  });

  container.appendChild(card);
}

export function render() {}
