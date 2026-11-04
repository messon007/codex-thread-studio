/* Application-owned isolated world. Project scripts and resource URLs are removed
 * by Rust; CSP and disabled markup JavaScript are independent boundaries. */
(() => {
  const epoch = document.body.dataset.studioEpoch;
  const send = value => window.webkit.messageHandlers.reader.postMessage(JSON.stringify({epoch, ...value}));
  const headings = [...document.querySelectorAll('[id^="studio-html-heading-"]')];
  let selectionFrame = 0, scrollFrame = 0;
  document.addEventListener('selectionchange', () => {
    if (!selectionFrame) selectionFrame = requestAnimationFrame(() => {
      selectionFrame = 0;
      const original = String(getSelection() || '');
      let text = original;
      // Translation retains its existing Unicode prefix. Native comments apply
      // their independent UTF-8 quote bound; native Copy keeps full selection.
      if (text.length > 16000) text = [...text.slice(0, 32002)].slice(0, 16000).join('');
      send({kind: 'selection', text, complete: text.length === original.length});
    });
  });
  const position = () => {
    scrollFrame = 0;
    let heading = null;
    for (const node of headings) {
      if (node.getBoundingClientRect().top > 48) break;
      heading = Number(node.id.slice('studio-html-heading-'.length));
    }
    send({kind: 'heading', id: heading});
  };
  addEventListener('scroll', () => { if (!scrollFrame) scrollFrame = requestAnimationFrame(position); }, {passive: true});
  window.StudioHtml = Object.freeze({
    jump(id) { document.getElementById(`studio-html-heading-${id}`)?.scrollIntoView({block: 'start'}); position(); },
    theme(dark) { document.documentElement.dataset.theme = dark ? 'dark' : 'light'; },
  });
  const schedulePosition = () => { if (!scrollFrame) scrollFrame = requestAnimationFrame(position); };
  new ResizeObserver(schedulePosition).observe(document.body);
  send({kind: 'ready'}); schedulePosition();
})();
