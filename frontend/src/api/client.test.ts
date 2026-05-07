import { beforeEach, describe, expect, it, vi } from 'vitest';
import { withAuthHeader } from './client';

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

describe('withAuthHeader', () => {
  beforeEach(() => {
    vi.stubGlobal('localStorage', createStorage());
  });

  it('adds the saved management token to requests', () => {
    localStorage.setItem('token', 'dbapi-token');

    const init = withAuthHeader({ headers: { 'Content-Type': 'application/json' } });
    const headers = new Headers(init.headers);

    expect(headers.get('Authorization')).toBe('dbapi-token');
    expect(headers.get('Content-Type')).toBe('application/json');
  });

  it('does not replace an explicit Authorization header', () => {
    localStorage.setItem('token', 'management-token');

    const init = withAuthHeader({ headers: { Authorization: 'api-token' } });
    const headers = new Headers(init.headers);

    expect(headers.get('Authorization')).toBe('api-token');
  });
});
