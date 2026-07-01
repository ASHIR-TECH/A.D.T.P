export function renderCard(container, peer, index, onUpdate, onRemove) {
  const card = document.createElement('div');
  card.className = 'peer-card';

  const name = peer.nickname || peer.peer_id?.slice(0, 16) || `Peer ${index + 1}`;

  card.innerHTML = `
    <div class="peer-card-header">
      <h4>${name}</h4>
      <button class="btn-icon remove-peer" title="Remove peer"></button>
    </div>
    <div class="peer-fields">
      <div class="settings-field">
        <label>Peer ID</label>
        <input type="text" class="peer-id" value="${peer.peer_id || ''}" readonly />
      </div>
      <div class="settings-field">
        <label>Nickname</label>
        <input type="text" class="peer-nickname" value="${peer.nickname || ''}" />
      </div>
      <div class="settings-field">
        <label>Host</label>
        <input type="text" class="peer-host" value="${peer.host || ''}" />
      </div>
      <div class="settings-field">
        <label>Port</label>
        <input type="number" class="peer-port" value="${peer.port || 0}" />
      </div>
      <div class="settings-field">
        <label>Trust Level</label>
        <select class="peer-trust">
          <option value="Trusted" ${peer.trust_level === 'Trusted' ? 'selected' : ''}>Trusted</option>
          <option value="Unverified" ${peer.trust_level === 'Unverified' || !peer.trust_level ? 'selected' : ''}>Unverified</option>
          <option value="Blocked" ${peer.trust_level === 'Blocked' ? 'selected' : ''}>Blocked</option>
        </select>
      </div>
    </div>
  `;

  card.querySelector('.remove-peer').addEventListener('click', () => onRemove(index));

  const idInput = card.querySelector('.peer-id');
  const nicknameInput = card.querySelector('.peer-nickname');
  const hostInput = card.querySelector('.peer-host');
  const portInput = card.querySelector('.peer-port');
  const trustInput = card.querySelector('.peer-trust');

  [nicknameInput, hostInput, portInput, trustInput].forEach(input => {
    input.addEventListener('change', () => {
      onUpdate(index, {
        peer_id: idInput.value,
        nickname: nicknameInput.value,
        host: hostInput.value,
        port: parseInt(portInput.value) || 0,
        trust_level: trustInput.value,
      });
    });
  });

  container.appendChild(card);
}
