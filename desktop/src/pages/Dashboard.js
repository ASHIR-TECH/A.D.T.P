import { render as renderDropZone } from '../components/DropZone.js';
import { render as renderStatusBar } from '../components/StatusBar.js';
import { render as renderTransferList } from '../components/TransferList.js';
import { render as renderChat } from '../components/ChatPanel.js';

export async function render(container) {
  container.innerHTML = `
    <div class="dashboard-layout">
      <div class="dashboard-main">
        <h2 class="page-title">Dashboard</h2>
        <div id="status-bar-container"></div>
        <div id="drop-zone-container"></div>
        <div id="transfer-list-container"></div>
      </div>
      <div id="chat-panel-container" class="dashboard-sidebar"></div>
    </div>
  `;

  renderStatusBar(container.querySelector('#status-bar-container'));
  renderDropZone(container.querySelector('#drop-zone-container'));
  await renderTransferList(container.querySelector('#transfer-list-container'));
  renderChat(container.querySelector('#chat-panel-container'));
}
