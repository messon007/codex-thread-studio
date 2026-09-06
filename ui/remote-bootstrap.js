(() => {
  // The desktop WebView already has an origin-scoped native credential.
  if (window.__CODEX_THREAD_STUDIO_GATEWAY__) return;
  const fragment = new URLSearchParams(window.location.hash.slice(1));
  const query = new URLSearchParams(window.location.search);
  const storageKey = 'studio.remote.gateway-token';
  let saved = '';
  try { saved = window.sessionStorage.getItem(storageKey) || ''; } catch {}
  const token = fragment.get('token') || query.get('token') || saved;
  if (!/^[a-f0-9]{32}$/u.test(token)) return;
  let persisted = false;
  try {
    window.sessionStorage.setItem(storageKey, token);
    persisted = true;
  } catch {}
  Object.defineProperty(window, '__CODEX_THREAD_STUDIO_GATEWAY__', {
    value: Object.freeze({ token, hostPlatform: __STUDIO_HOST_PLATFORM__, remote: true }),
    configurable: false,
    enumerable: false,
    writable: false,
  });
  query.delete('token');
  fragment.delete('token');
  // Private/restricted browsers may disable storage. Retain a fragment only in
  // that case so reload still works, without putting the secret in HTTP URLs.
  if (!persisted) fragment.set('token', token);
  const search = query.toString();
  const hash = fragment.toString();
  window.history.replaceState(null, '', `${window.location.pathname}${search ? `?${search}` : ''}${hash ? `#${hash}` : ''}`);
})();
