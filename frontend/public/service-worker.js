const CACHE_NAME = 'mergemint-v1';
const BOUNTY_CACHE = 'mergemint-bounties-v1';
const ASSETS_TO_CACHE = [
  '/',
  '/index.html',
  '/styles.css',
  '/app.js',
];

// Install: cache app shell
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {
      return cache.addAll(ASSETS_TO_CACHE);
    })
  );
});

// Activate: clean up old caches
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((cacheNames) => {
      return Promise.all(
        cacheNames.map((cacheName) => {
          if (cacheName !== CACHE_NAME && cacheName !== BOUNTY_CACHE) {
            return caches.delete(cacheName);
          }
        })
      );
    })
  );
});

// Fetch: serve from cache, fall back to network
self.addEventListener('fetch', (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // Cache API responses (bounties, bounty details, etc.)
  if (request.method === 'GET' && (url.pathname.includes('/api/') || request.destination === 'document')) {
    event.respondWith(
      caches.match(request).then((response) => {
        return (
          response ||
          fetch(request)
            .then((networkResponse) => {
              // Cache successful responses
              if (networkResponse.ok && request.destination === 'document') {
                const responseClone = networkResponse.clone();
                caches.open(BOUNTY_CACHE).then((cache) => {
                  cache.put(request, responseClone);
                });
              }
              return networkResponse;
            })
            .catch(() => {
              // Fall back to cached version if offline
              return caches.match(request);
            })
        );
      })
    );
  } else {
    // For other requests, try network first, then cache
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
  }
});
