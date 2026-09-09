// Served only by the explicit mock Vite dev server, never a production entry.
const frame = document.querySelector<HTMLIFrameElement>('#app')!;
const result = document.querySelector<HTMLPreElement>('#result')!;
const select = (id: string) => document.querySelector<HTMLSelectElement>(`#${id}`)!;
const settle = () => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));

async function waitForFilters() {
  for (let frameCount = 0; frameCount < 150; frameCount++) {
    if (frame.contentDocument?.querySelector('.filter-bar')) return;
    await settle();
  }
  throw new Error('Dashboard filters did not become ready; no layout pass recorded');
}

async function inspect(width: number, zoom: number, language: string) {
  await waitForFilters();
  frame.style.width = `${width}px`;
  frame.style.height = '900px';
  const doc = frame.contentDocument!;
  if (!doc.querySelector('.demo-notice')) throw new Error('Explicit synthetic dashboard required');
  doc.body.style.zoom = String(zoom);
  const control = doc.querySelector<HTMLSelectElement>('.language-select select')!;
  control.value = language;
  control.dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
  const bar = doc.querySelector<HTMLElement>('.filter-bar')!;
  const bounds = bar.getBoundingClientRect();
  const targets = Array.from(bar.querySelectorAll<HTMLElement>('.filter-scope-label, .filter-field, .period-option, .refresh-button'));
  const violations: string[] = [];
  targets.forEach((target, index) => {
    const a = target.getBoundingClientRect();
    if (a.left < bounds.left - 1 || a.right > bounds.right + 1) violations.push(`outside:${index}`);
    for (let other = index + 1; other < targets.length; other++) {
      const b = targets[other].getBoundingClientRect();
      if (Math.min(a.right, b.right) - Math.max(a.left, b.left) > 1 &&
          Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1) violations.push(`overlap:${index}/${other}`);
    }
  });
  return { width, zoom, language, panelWidth: Math.round(bounds.width), panelHeight: Math.round(bounds.height), controls: targets.length, violations };
}

document.querySelector('#inspect')!.addEventListener('click', async () => {
  try { result.textContent = JSON.stringify(await inspect(Number(select('width').value), Number(select('zoom').value), select('language').value), null, 2); }
  catch (error) { result.textContent = String(error); }
});
document.querySelector('#matrix')!.addEventListener('click', async () => {
  const reports = [];
  try {
    for (const language of ['en', 'zh-CN']) for (const width of [560, 700, 900, 1280, 1440]) for (const zoom of [0.8, 1, 1.6]) {
      reports.push(await inspect(width, zoom, language));
    }
    result.textContent = JSON.stringify({ cases: reports.length, failed: reports.filter(report => report.violations.length), reports }, null, 2);
  } catch (error) { result.textContent = `${String(error)}\n${JSON.stringify(reports)}`; }
});
frame.addEventListener('load', () => { result.textContent = 'Dashboard loaded. Choose Inspect or Run 30-case matrix.'; });
