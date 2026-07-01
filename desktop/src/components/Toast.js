let visibleToasts = [];

export function show(message, variant) {
  const container = document.getElementById('toast-container');
  if (!container) return;

  if (visibleToasts.length >= 3) {
    const oldest = visibleToasts.shift();
    if (oldest && oldest.parentNode) oldest.parentNode.removeChild(oldest);
  }

  const toast = document.createElement('div');
  toast.className = `toast ${variant || 'info'}`;
  toast.innerHTML = `
    <span></span>
    <span>${message}</span>
    <button class="toast-close"></button>
  `;

  toast.querySelector('.toast-close').addEventListener('click', () => {
    removeToast(toast);
  });

  container.appendChild(toast);
  visibleToasts.push(toast);

  setTimeout(() => {
    removeToast(toast);
  }, 4000);
}

function removeToast(toast) {
  if (!toast || !toast.parentNode) return;
  toast.style.transition = 'opacity 200ms ease-in, transform 200ms ease-in';
  toast.style.opacity = '0';
  toast.style.transform = 'translateY(12px)';
  setTimeout(() => {
    if (toast.parentNode) toast.parentNode.removeChild(toast);
    visibleToasts = visibleToasts.filter(t => t !== toast);
  }, 200);
}
