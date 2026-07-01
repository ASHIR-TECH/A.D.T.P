export class Tabs {
  constructor(container, tabs, activeTab, onChange) {
    this.container = container;
    this.tabs = tabs;
    this.activeTab = activeTab;
    this.onChange = onChange;
    this.render();
  }

  render() {
    this.container.innerHTML = `
      <div class="tabs-bar" id="tabs-bar">
        ${this.tabs.map((t, i) => `
          <button class="tab-item ${t.id === this.activeTab ? 'active' : ''}" data-tab="${t.id}" data-index="${i}">
            ${t.label}
          </button>
        `).join('')}
        <div class="tab-underline" id="tab-underline"></div>
      </div>
      <div id="tab-content">${this.getContent()}</div>
    `;

    this.underline = this.container.querySelector('#tab-underline');
    this.positionUnderline();

    this.container.querySelectorAll('.tab-item').forEach(btn => {
      btn.addEventListener('click', () => this.select(btn.dataset.tab));
    });

    this.container.addEventListener('keydown', (e) => {
      const items = this.container.querySelectorAll('.tab-item');
      const current = Array.from(items).findIndex(b => b.dataset.tab === this.activeTab);
      if (e.key === 'ArrowRight' && current < items.length - 1) {
        this.select(items[current + 1].dataset.tab);
      } else if (e.key === 'ArrowLeft' && current > 0) {
        this.select(items[current - 1].dataset.tab);
      }
    });
  }

  select(tabId) {
    if (tabId === this.activeTab) return;
    const content = this.container.querySelector('#tab-content');
    content.classList.add('motion-fade-out');
    setTimeout(() => {
      this.activeTab = tabId;
      this.container.querySelectorAll('.tab-item').forEach(b => b.classList.toggle('active', b.dataset.tab === tabId));
      content.innerHTML = this.getContent();
      content.classList.remove('motion-fade-out');
      content.classList.add('motion-fade-in');
      this.positionUnderline();
      if (this.onChange) this.onChange(tabId);
      setTimeout(() => content.classList.remove('motion-fade-in'), 80);
    }, 80);
  }

  positionUnderline() {
    const active = this.container.querySelector('.tab-item.active');
    if (active && this.underline) {
      this.underline.style.left = active.offsetLeft + 'px';
      this.underline.style.width = active.offsetWidth + 'px';
    }
  }

  getContent() {
    const tab = this.tabs.find(t => t.id === this.activeTab);
    return tab ? tab.content || '' : '';
  }
}

export function renderTabs(container, tabs, activeTab, onChange) {
  new Tabs(container, tabs, activeTab, onChange);
}
