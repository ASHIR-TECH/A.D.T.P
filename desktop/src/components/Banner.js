export function showBanner(message, variant, action) {
  let container = document.getElementById('banner-container');
  if (!container) {
    container = document.createElement('div');
    container.id = 'banner-container';
    const content = document.getElementById('content');
    content.insertBefore(container, content.firstChild);
  }

  container.innerHTML = `
    <div class="banner ${variant === 'ember' ? 'ember' : ''}">
      <span>${message}</span>
      ${action ? `<button class="btn btn-sm btn-primary" id="banner-action">${action.label}</button>` : ''}
      <button class="banner-close" id="banner-close"></button>
    </div>
  `;

  const closeBtn = container.querySelector('#banner-close');
  if (closeBtn) {
    closeBtn.addEventListener('click', () => {
      const banner = container.querySelector('.banner');
      banner.style.animation = 'none';
      banner.style.maxHeight = '0';
      banner.style.padding = '0';
      banner.style.opacity = '0';
      banner.style.overflow = 'hidden';
      setTimeout(() => { container.innerHTML = ''; }, 200);
    });
  }

  if (action) {
    container.querySelector('#banner-action')?.addEventListener('click', action.onClick);
  }
}

export function dismissBanner() {
  const container = document.getElementById('banner-container');
  if (container) container.innerHTML = '';
}
