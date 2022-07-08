/*
 * Unless otherwise noted, this file is released and thus subject to the
 * terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
 * "Incompatible With Secondary Licenses", as defined by the MPL2.
 * If a copy of the MPL2 was not distributed with this file, you can
 * obtain one at https://mozilla.org/MPL/2.0/.
 */

var cacheName = 'gamegirl-pwa';
var filesToCache = [
  './',
  './index.html',
  './gamegirl.js',
  './gamegirl_bg.wasm',
];

/* Start the service worker and cache all of the app's content */
self.addEventListener('install', function (e) {
  e.waitUntil(
    caches.open(cacheName).then(function (cache) {
      return cache.addAll(filesToCache);
    })
  );
});

/* Serve cached content when offline */
self.addEventListener('fetch', function (e) {
  e.respondWith(
    caches.match(e.request).then(function (response) {
      return response || fetch(e.request);
    })
  );
});
