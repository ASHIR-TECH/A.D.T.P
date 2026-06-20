import { render as renderAuditLog } from '../components/AuditLog.js';

export async function render(container) {
  container.innerHTML = `
    <h2 class="page-title">History</h2>
    <div id="audit-container"></div>
  `;

  await renderAuditLog(container.querySelector('#audit-container'));
}
