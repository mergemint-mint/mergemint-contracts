const CACHE_NAME = 'mergemint-v2';
const ASSETS_TO_CACHE = [
  '/',
  '/index.html',
  '/styles.css',
  '/app.js',
];

function isSensitiveOrApiRequest(request, url) {
  if (request.method !== 'GET') return true;
  if (url.pathname.includes('/api/') || url.pathname.includes('/tx/')) return true;
  if (request.headers.get('Authorization') || request.headers.get('Idempotency-Key')) return true;
  if (request.headers.get('Accept') === 'text/event-stream') return true;
  return false;
}

// Install: cache static app shell only
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {
      return cache.addAll(ASSETS_TO_CACHE);
    })
  );
});

// Activate: clean up all old caches (including legacy BOUNTY_CACHE)
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((cacheNames) => {
      return Promise.all(
        cacheNames.map((cacheName) => {
          if (cacheName !== CACHE_NAME) {
            return caches.delete(cacheName);
          }
          return undefined;
        })
      );
    })
  );
});

// Allow the application to purge cached state on wallet disconnect or account change
self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'CLEAR_WALLET_CACHE') {
    event.waitUntil(
      caches.keys().then((cacheNames) =>
        Promise.all(
          cacheNames
            .filter((name) => name !== CACHE_NAME)
            .map((name) => caches.delete(name))
        )
      )
    );
  }
});

// Fetch: never cache dynamic API, SSE, or wallet transaction endpoints
self.addEventListener('fetch', (event) => {
  const { request } = event;
  const url = new URL(request.url);

  if (isSensitiveOrApiRequest(request, url)) {
    event.respondWith(fetch(request, { cache: 'no-store' }));
    return;
  }

  if (request.destination === 'document') {
    event.respondWith(
      fetch(request)
        .then((networkResponse) => {
          if (networkResponse.ok) {
            const responseClone = networkResponse.clone();
            caches.open(CACHE_NAME).then((cache) => {
              cache.put(request, responseClone);
            });
          }
          return networkResponse;
        })
        .catch(() => caches.match(request))
    );
    return;
  }

  event.respondWith(
    fetch(request)
      .then((response) => {
        if (response.ok && request.destination === 'style') {
          const responseClone = response.clone();
          caches.open(CACHE_NAME).then((cache) => {
            cache.put(request, responseClone);
          });
        }
        return response;
      })
      .catch(() => caches.match(request))
  );
});
