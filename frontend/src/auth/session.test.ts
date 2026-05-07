import { beforeEach, describe, expect, it, vi } from 'vitest';
import { clearAuthSession, isAuthenticated, readAuthToken, saveAuthToken } from './session';

function createStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    clear: () => values.clear(),
    getItem: (key: string) => values.get(key) ?? null,
    key: (index: number) => Array.from(values.keys())[index] ?? null,
    removeItem: (key: string) => values.delete(key),
    setItem: (key: string, value: string) => values.set(key, value),
  };
}

describe('auth session', () => {
  beforeEach(() => {
    vi.stubGlobal('localStorage', createStorage());
    vi.stubGlobal('sessionStorage', createStorage());
    localStorage.clear();
    sessionStorage.clear();
  });

  it('persists and reads the current auth token', () => {
    saveAuthToken('dbapi-token');

    expect(readAuthToken()).toBe('dbapi-token');
    expect(isAuthenticated()).toBe(true);
  });

  it('reads the legacy token key for existing sessions', () => {
    localStorage.setItem('token', 'legacy-token');

    expect(readAuthToken()).toBe('legacy-token');
    expect(isAuthenticated()).toBe(true);
  });

  it('clears current, legacy, and temporary browser session state on logout', () => {
    localStorage.setItem('dbapi_auth_token', 'current-token');
    localStorage.setItem('token', 'legacy-token');
    sessionStorage.setItem('draft', 'unsaved');

    clearAuthSession();

    expect(readAuthToken()).toBeNull();
    expect(localStorage.getItem('token')).toBeNull();
    expect(sessionStorage.getItem('draft')).toBeNull();
    expect(isAuthenticated()).toBe(false);
  });
});
