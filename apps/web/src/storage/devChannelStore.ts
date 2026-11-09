/**
 * Persistence of the sideloaded developer channel and its registry, so a page
 * reload behaves like a device reboot: the dev channel stays installed and
 * keeps its registry (as on a real Roku in developer mode).
 *
 * The package lives in IndexedDB (binary, possibly large); the registry JSON
 * lives in localStorage. Every access is guarded: private windows or blocked
 * storage fall back to an in-memory session.
 */

/** A device has a single developer channel slot. */
export const DEV_CHANNEL_ID = "dev";

export interface StoredChannel {
  fileName: string;
  title: string;
  version: string;
  zip: ArrayBuffer;
  icon: ArrayBuffer | null;
}

const DB_NAME = "roku-emu";
const STORE = "channels";
const REGISTRY_PREFIX = "roku-emu:registry:";

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("indexedDB open failed"));
  });
}

async function withStore<T>(
  mode: IDBTransactionMode,
  action: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T> {
  const database = await openDatabase();
  try {
    return await new Promise<T>((resolve, reject) => {
      const request = action(database.transaction(STORE, mode).objectStore(STORE));
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error ?? new Error("indexedDB request failed"));
    });
  } finally {
    database.close();
  }
}

export async function loadDevChannel(): Promise<StoredChannel | null> {
  try {
    const value = await withStore<StoredChannel | undefined>("readonly", (store) =>
      store.get(DEV_CHANNEL_ID),
    );
    return value ?? null;
  } catch {
    return null;
  }
}

export async function saveDevChannel(channel: StoredChannel): Promise<void> {
  try {
    await withStore("readwrite", (store) => store.put(channel, DEV_CHANNEL_ID));
  } catch {
    // Storage unavailable: the channel stays installed for this session only.
  }
}

export async function deleteDevChannel(): Promise<void> {
  try {
    await withStore("readwrite", (store) => store.delete(DEV_CHANNEL_ID));
  } catch {
    // Nothing persisted to remove.
  }
  saveRegistry(DEV_CHANNEL_ID, null);
}

export function loadRegistry(channelId: string): string | null {
  try {
    return localStorage.getItem(REGISTRY_PREFIX + channelId);
  } catch {
    return null;
  }
}

export function saveRegistry(channelId: string, json: string | null): void {
  try {
    if (json === null) localStorage.removeItem(REGISTRY_PREFIX + channelId);
    else localStorage.setItem(REGISTRY_PREFIX + channelId, json);
  } catch {
    // Quota exceeded or storage blocked: keep running without persistence.
  }
}
