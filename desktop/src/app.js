const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

import { render as renderSidebar } from './components/Sidebar.js';
import { render as renderTopBar } from './components/TopBar.js';

let currentTab = 'dashboard';
let notificationCount = 0;
let agentState = 'inactive';

function getPageTitle(tab) {
  const titles = { dashboard: 'Dashboard', history: 'History', settings: 'Settings' };
  return titles[tab] || tab;
}

function activateTab(tab) {
  currentTab = tab;
  renderTopBar(document.getElementById('topbar') || document.createElement('div'), getPageTitle(tab), notificationCount, agentState);
  loadPage(tab);
}

async function loadPage(tab) {
  const container = document.getElementById('page-container');
  const pageContainer = container;

  pageContainer.classList.add('motion-fade-out');
  setTimeout(async () => {
    pageContainer.innerHTML = '';
    pageContainer.classList.remove('motion-fade-out');
    try {
      const mod = await import(`./pages/${tab.charAt(0).toUpperCase() + tab.slice(1)}.js`);
      await mod.render(pageContainer);
    } catch (err) {
      pageContainer.innerHTML = `<p style="color:var(--ember);padding:24px;">Failed to load ${tab}: ${err.message}</p>`;
    }
    pageContainer.classList.add('motion-fade-in');
    setTimeout(() => pageContainer.classList.remove('motion-fade-in'), 80);
  }, 80);
}

// ===== Orb Ring Animation =====
const GLOBE_R   = 26;
const TILT_X    = 74 * Math.PI/180;
const TILT_Z    = 18 * Math.PI/180;
const PERSP_D   = 210;
const cosX = Math.cos(TILT_X), sinX = Math.sin(TILT_X);
const cosZ = Math.cos(TILT_Z), sinZ = Math.sin(TILT_Z);

const RING_LIGHT = [253,232,200];
const RING_SOFT  = [240,192,132];
const RING       = [217,119,6];
const RING_DUSK  = [194,133,106];

function lerp(a,b,t){ return a+(b-a)*t; }
function lerpColor(c1,c2,t){
  return `rgb(${Math.round(lerp(c1[0],c2[0],t))},${Math.round(lerp(c1[1],c2[1],t))},${Math.round(lerp(c1[2],c2[2],t))})`;
}
function seededRandom(seed){ const x = Math.sin(seed*999.999)*10000; return x - Math.floor(x); }

function project(r, theta){
  const x  = r*Math.cos(theta);
  const y  = r*Math.sin(theta);
  const y1 = y*cosX;
  const z1 = y*sinX;
  const x2 = x*cosZ - y1*sinZ;
  const y2 = x*sinZ + y1*cosZ;
  const scale = 1/(1 - z1/PERSP_D);
  return { x:x2*scale, y:y2*scale, front:z1 > 0 };
}

const STEPS = 130;
const thetas = [];
for (let i=0;i<=STEPS;i++) thetas.push((i/STEPS)*Math.PI*2);

function buildLoop(baseR, seed){
  const freqA  = 2 + Math.floor(seededRandom(seed*3+1)*2);
  const freqB  = 5 + Math.floor(seededRandom(seed*7+2)*3);
  const phaseA = seededRandom(seed*11+3)*Math.PI*2;
  const phaseB = seededRandom(seed*13+4)*Math.PI*2;
  const ampA   = 0.55 + seededRandom(seed*5+5)*0.45;
  const ampB   = 0.2  + seededRandom(seed*9+6)*0.3;
  const speed  = (seed % 2 === 0 ? 1 : -1) * (0.12 + seededRandom(seed*17+8)*0.1);
  return { baseR, freqA, freqB, phaseA, phaseB, ampA, ampB, speed, phaseOffset:0 };
}

function pointsFor(loop){
  const pts = new Array(thetas.length);
  for (let i=0;i<thetas.length;i++){
    const th = thetas[i];
    const n = loop.ampA*Math.sin(loop.freqA*th + loop.phaseA + loop.phaseOffset)
            + loop.ampB*Math.sin(loop.freqB*th + loop.phaseB + loop.phaseOffset*1.7);
    pts[i] = project(loop.baseR + n, th);
  }
  return pts;
}

function splitRuns(pts){
  const runs = [];
  let cur = [pts[0]], curFront = pts[0].front;
  for (let i=1;i<pts.length;i++){
    if (pts[i].front === curFront){
      cur.push(pts[i]);
    } else {
      runs.push({front:curFront, pts:cur});
      cur = [pts[i]];
      curFront = pts[i].front;
    }
  }
  runs.push({front:curFront, pts:cur});
  return runs;
}

function toPathD(pts){
  return pts.map((p,i)=> `${i===0?'M':'L'}${p.x.toFixed(2)},${p.y.toFixed(2)}`).join(' ');
}

const svgNS = 'http://www.w3.org/2000/svg';
const backSvg  = document.getElementById('orbRingsBack');
const frontSvg = document.getElementById('orbRingsFront');

const loopDefs = [];
const CORE_N = 16, CORE_START = GLOBE_R*1.06, CORE_STEP = 0.85;
for (let i=0;i<CORE_N;i++){
  loopDefs.push({ r: CORE_START + i*CORE_STEP, t: i/(CORE_N-1), kind:'core', seed:i+1 });
}
const OUTER_RADII = [45, 51.5, 59, 67];
OUTER_RADII.forEach((r,i)=>{
  loopDefs.push({ r, t: i/(OUTER_RADII.length-1), kind:'outer', seed: CORE_N + i + 1 });
});

const rendered = [];
loopDefs.forEach(def => {
  const loop = buildLoop(def.r, def.seed);
  let color, width, opacity;
  if (def.kind === 'core'){
    color   = lerpColor(RING_LIGHT, RING, def.t*0.75);
    width   = lerp(0.22, 0.13, def.t);
    opacity = lerp(0.85, 0.55, def.t);
  } else {
    color   = lerpColor(RING, RING_DUSK, def.t);
    width   = lerp(0.16, 0.1, def.t);
    opacity = lerp(0.5, 0.32, def.t);
  }

  const backPaths = [], frontPaths = [];
  function makePath(){
    const p = document.createElementNS(svgNS,'path');
    p.setAttribute('fill','none');
    p.setAttribute('stroke', color);
    p.setAttribute('stroke-width', width.toFixed(3));
    p.setAttribute('stroke-linecap','round');
    p.style.opacity = opacity.toFixed(2);
    return p;
  }

  for (let k=0;k<3;k++){
    const bp = makePath(); backSvg.appendChild(bp);  backPaths.push(bp);
    const fp = makePath(); frontSvg.appendChild(fp); frontPaths.push(fp);
  }

  rendered.push({ loop, backPaths, frontPaths });
});

function update(){
  rendered.forEach(({loop, backPaths, frontPaths}) => {
    const pts = pointsFor(loop);
    const runs = splitRuns(pts);
    let bi = 0, fi = 0;
    runs.forEach(run => {
      if (run.pts.length < 2) return;
      const d = toPathD(run.pts);
      if (run.front){
        if (fi < frontPaths.length){ frontPaths[fi].setAttribute('d', d); frontPaths[fi].style.display=''; fi++; }
      } else {
        if (bi < backPaths.length){ backPaths[bi].setAttribute('d', d); backPaths[bi].style.display=''; bi++; }
      }
    });
    for (;fi<frontPaths.length;fi++) frontPaths[fi].style.display='none';
    for (;bi<backPaths.length;bi++)  backPaths[bi].style.display='none';
  });
}

document.addEventListener('DOMContentLoaded', () => {
  renderSidebar(document.getElementById('sidebar'), currentTab, activateTab);

  const topbar = document.createElement('div');
  topbar.id = 'topbar';
  document.getElementById('content').insertBefore(topbar, document.getElementById('page-container'));
  renderTopBar(topbar, getPageTitle(currentTab), notificationCount, agentState);

  listen('notify', (event) => {
    import('./components/Toast.js').then(({ show }) => {
      const { title, body } = event.payload;
      if (title && body) show(`${title}: ${body}`, 'info');
    });
  });

  listen('agent-status', (event) => {
    agentState = event.payload?.state || 'inactive';
    const topbar = document.getElementById('topbar');
    if (topbar) renderTopBar(topbar, getPageTitle(currentTab), notificationCount, agentState);
  });

  activateTab('dashboard');

  let lastT = null;
  function tick(t){
    if (lastT === null) lastT = t;
    const dt = (t - lastT)/1000;
    lastT = t;
    rendered.forEach(({loop}) => { loop.phaseOffset += loop.speed*dt; });
    update();
    requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);
});
