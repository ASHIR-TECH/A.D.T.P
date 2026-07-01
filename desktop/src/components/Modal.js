let activeModal = null;
let triggerElement = null;

export function showModal(container, title, contentHtml, onClose) {
  triggerElement = document.activeElement;

  const backdrop = document.createElement('div');
  backdrop.className = 'modal-backdrop';

  backdrop.innerHTML = `
    <div class="modal-panel" role="dialog" aria-modal="true">
      <div class="modal-header">
        <h2>${title}</h2>
        <button class="modal-close" id="modal-close"></button>
      </div>
      <div id="modal-body">${contentHtml}</div>
    </div>
  `;

  document.body.appendChild(backdrop);
  activeModal = backdrop;

  const close = () => closeModal(onClose);
  backdrop.querySelector('#modal-close').addEventListener('click', close);
  backdrop.addEventListener('click', (e) => { if (e.target === backdrop) close(); });
  document.addEventListener('keydown', keyHandler);

  focusTrap(backdrop);
}

function keyHandler(e) {
  if (e.key === 'Escape') {
    closeModal();
  }
  if (e.key === 'Tab') {
    const focusable = activeModal.querySelectorAll('button, input, select, textarea, [tabindex]:not([tabindex="-1"])');
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
  }
}

function closeModal(onClose) {
  if (!activeModal) return;
  document.removeEventListener('keydown', keyHandler);
  const panel = activeModal.querySelector('.modal-panel');
  if (panel) {
    panel.style.animation = 'none';
    panel.style.transform = 'scale(0.96)';
    panel.style.opacity = '0';
  }
  activeModal.style.animation = 'fadeOut 150ms ease-in both';
  setTimeout(() => {
    if (activeModal && activeModal.parentNode) activeModal.parentNode.removeChild(activeModal);
    activeModal = null;
    if (triggerElement) triggerElement.focus();
    if (onClose) onClose();
  }, 150);
}

function focusTrap(container) {
  const focusable = container.querySelectorAll('button, input, select, textarea, [tabindex]:not([tabindex="-1"])');
  if (focusable.length > 0) focusable[0].focus();
}

export function showModalContent(container, title, contentHtml, onClose) {
  showModal(container, title, contentHtml, onClose);
  return {
    close: () => closeModal(onClose),
    setContent: (html) => {
      const body = activeModal?.querySelector('#modal-body');
      if (body) body.innerHTML = html;
    }
  };
}
