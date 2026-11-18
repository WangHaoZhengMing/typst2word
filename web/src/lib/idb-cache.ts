const DB_NAME = 'typst2word-studio-cache-v1';
const DB_VERSION = 1;
const STORE_PACKAGES = 'packages';
const STORE_FONTS = 'fonts';

let dbPromise: Promise<IDBDatabase | null> | null = null;

function openCacheDb(): Promise<IDBDatabase | null> {
  if (typeof indexedDB === 'undefined') {
    return Promise.resolve(null);
  }
  if (!dbPromise) {
    dbPromise = new Promise((resolve) => {
      try {
        const req = indexedDB.open(DB_NAME, DB_VERSION);
        req.onupgradeneeded = () => {
          const db = req.result;
          if (!db.objectStoreNames.contains(STORE_PACKAGES)) {
            db.createObjectStore(STORE_PACKAGES);
          }
          if (!db.objectStoreNames.contains(STORE_FONTS)) {
            db.createObjectStore(STORE_FONTS);
          }
        };
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => resolve(null);
      } catch {
        resolve(null);
      }
    });
  }
  return dbPromise;
}

async function getBytes(storeName: string, key: string): Promise<Uint8Array | null> {
  const db = await openCacheDb();
  if (!db) return null;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(storeName, 'readonly');
      const req = tx.objectStore(storeName).get(key);
      req.onsuccess = () => {
        const res = req.result;
        if (res instanceof Uint8Array) {
          resolve(res);
        } else if (res instanceof ArrayBuffer) {
          resolve(new Uint8Array(res));
        } else {
          resolve(null);
        }
      };
      req.onerror = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
}

async function putBytes(storeName: string, key: string, value: Uint8Array): Promise<void> {
  const db = await openCacheDb();
  if (!db) return;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(storeName, 'readwrite');
      tx.objectStore(storeName).put(value, key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => resolve();
    } catch {
      resolve();
    }
  });
}

export const getCachedPackage = (spec: string) => getBytes(STORE_PACKAGES, spec);
export const setCachedPackage = (spec: string, bytes: Uint8Array) =>
  putBytes(STORE_PACKAGES, spec, bytes);

export const getCachedFont = (key: string) => getBytes(STORE_FONTS, key);
export const setCachedFont = (key: string, bytes: Uint8Array) =>
  putBytes(STORE_FONTS, key, bytes);
