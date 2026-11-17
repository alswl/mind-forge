const figures = [...document.querySelectorAll('.mf-diagram')];
const status = (figure, state, message) => {
  figure.dataset.state = state;
  figure.querySelector('.mf-diagram-status').textContent = message;
  figure.querySelector('details').open = state !== 'rendered';
};
if (figures.length) {
  let mermaid;
  let timer;
  try {
    const module = await Promise.race([
      import('https://cdn.jsdelivr.net/npm/mermaid@11.12.0/dist/mermaid.esm.min.mjs'),
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('timeout')), 15000); })
    ]);
    mermaid = module.default;
    mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', suppressErrorRendering: true,
      maxTextSize: 50000, flowchart: { htmlLabels: false }, secure: ['securityLevel', 'startOnLoad', 'maxTextSize', 'suppressErrorRendering'] });
  } catch {
    figures.forEach(figure => status(figure, 'load-error', 'Mermaid could not load. Check CDN access and reload; diagram source is preserved below.'));
  } finally {
    clearTimeout(timer);
  }
  if (mermaid) {
    for (const [index, figure] of figures.entries()) {
      const source = figure.querySelector('pre code').textContent;
      const output = figure.querySelector('.mf-diagram-output');
      try {
        // Per-diagram configuration cannot weaken the renderer's security policy.
        if (/%%\s*\{/.test(source) || /^\s*---/.test(source)) throw new Error('diagram configuration disabled');
        const { svg } = await mermaid.render(`mf-diagram-${index}`, source, output);
        output.innerHTML = svg;
        status(figure, 'rendered', 'Diagram rendered. Source is available below.');
      } catch {
        output.replaceChildren();
        status(figure, 'parse-error', 'Mermaid diagram could not be parsed or uses unsupported configuration. Check its syntax; source is preserved below.');
      }
    }
  }
}
