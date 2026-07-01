const { invoke } = window.__TAURI__.core;

export function render(container) {
  container.innerHTML = `
    <div class="drop-zone" id="drop-zone">
      <div class="drop-zone-icon"></div>
      <div class="drop-zone-text">Drop files here or click to browse</div>
      <div class="drop-zone-hint">Files are sent to your configured agent</div>
    </div>
  `;

  const zone = container.querySelector('#drop-zone');

  zone.addEventListener('click', async () => {
    const path = await invoke('pick_file');
    if (path) {
      try {
        const result = await invoke('send_file', { filePath: path });
        const { show } = await import('./Toast.js');
        show(`Queued: ${result.filename}`, 'success');
      } catch (err) {
        const { show } = await import('./Toast.js');
        show(`Send failed: ${err}`, 'error');
      }
    }
  });

  zone.addEventListener('dragover', (e) => {
    e.preventDefault();
    zone.classList.add('drag-over');
  });

  zone.addEventListener('dragleave', () => {
    zone.classList.remove('drag-over');
  });

  zone.addEventListener('drop', async (e) => {
    e.preventDefault();
    zone.classList.remove('drag-over');
    const files = Array.from(e.dataTransfer.files);
    for (const file of files) {
      try {
        const result = await invoke('send_file', { filePath: file.path });
        const { show } = await import('./Toast.js');
        show(`Queued: ${result.filename}`, 'success');
      } catch (err) {
        const { show } = await import('./Toast.js');
        show(`Send failed: ${file.name} — ${err}`, 'error');
      }
    }
  });
}
