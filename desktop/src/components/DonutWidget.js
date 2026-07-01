export function renderDonutWidget(container, title, segments, centreValue) {
  const total = segments.reduce((s, seg) => s + seg.value, 0) || 1;
  let cumulative = 0;
  const paths = segments.map(seg => {
    const startAngle = (cumulative / total) * 360;
    cumulative += seg.value;
    const endAngle = (cumulative / total) * 360;
    return { ...seg, startAngle, endAngle };
  });

  const svgSize = 100;
  const cx = svgSize / 2;
  const cy = svgSize / 2;
  const r = 40;
  const circumference = 2 * Math.PI * r;

  let svgPaths = '';
  paths.forEach((seg, i) => {
    const offset = (seg.startAngle / 360) * circumference;
    const length = ((seg.endAngle - seg.startAngle) / 360) * circumference;
    svgPaths += `<circle cx="${cx}" cy="${cy}" r="${r}" fill="none" stroke="${seg.color}" stroke-width="8"
      stroke-dasharray="${length} ${circumference - length}"
      stroke-dashoffset="${-offset}"
      transform="rotate(-90 ${cx} ${cy})"
      style="transition: stroke-dasharray 0.3s ease" />`;
  });

  const legendHtml = segments.filter(s => s.value > 0).map(s => `
    <span class="donut-legend-item">
      <span class="donut-legend-dot" style="background:${s.color}"></span>
      ${s.label} (${s.value})
    </span>
  `).join('');

  container.innerHTML = `
    <div class="donut-widget">
      <div class="donut-widget-title">${title}</div>
      <svg class="donut-svg" viewBox="0 0 ${svgSize} ${svgSize}">
        ${svgPaths}
      </svg>
      <div class="donut-center-label gold-text">${centreValue !== undefined ? centreValue : total}</div>
      <div class="donut-legend">${legendHtml}</div>
    </div>
  `;
}
