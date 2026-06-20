import { render as renderDropZone } from '../components/DropZone.js';
import { render as renderStatusBar } from '../components/StatusBar.js';
import { render as renderTransferList } from '../components/TransferList.js';

export async function render(container) {
  container.innerHTML = `
    <h2 class="page-title">Dashboard</h2>
    <div id="status-bar-container"></div>
    <div id="drop-zone-container"></div>
    <div id="transfer-list-container"></div>
  `;

  renderStatusBar(container.querySelector('#status-bar-container'));
  renderDropZone(container.querySelector('#drop-zone-container'));
  await renderTransferList(container.querySelector('#transfer-list-container'));
}
