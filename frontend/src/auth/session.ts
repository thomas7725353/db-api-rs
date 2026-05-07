const AUTH_TOKEN_KEY = 'dbapi_auth_token';
const LEGACY_TOKEN_KEY = 'token';

export function readAuthToken(): string | null {
  return localStorage.getItem(AUTH_TOKEN_KEY) || localStorage.getItem(LEGACY_TOKEN_KEY);
}

export function saveAuthToken(token: string): void {
  localStorage.setItem(AUTH_TOKEN_KEY, token);
  localStorage.setItem(LEGACY_TOKEN_KEY, token);
}

export function clearAuthSession(): void {
  localStorage.removeItem(AUTH_TOKEN_KEY);
  localStorage.removeItem(LEGACY_TOKEN_KEY);
  sessionStorage.clear();
}

export function isAuthenticated(): boolean {
  return Boolean(readAuthToken());
}
