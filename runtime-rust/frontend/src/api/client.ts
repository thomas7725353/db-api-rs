export interface ApiEnvelope<T> {
  success?: boolean;
  msg?: string;
  data?: T;
}

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status?: number,
    public readonly body?: unknown,
  ) {
    super(message);
  }
}

export async function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return apiRequest<T>(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

export async function apiGet<T>(path: string): Promise<T> {
  return apiRequest<T>(path, { method: 'GET' });
}

export async function apiRequest<T>(path: string, init: RequestInit): Promise<T> {
  const response = await fetch(path, init);
  const text = await response.text();
  const payload = parsePayload(text);

  if (!response.ok) {
    const message = extractMessage(payload) ?? response.statusText;
    throw new ApiError(message, response.status, payload);
  }

  if (isEnvelope(payload)) {
    if (payload.success === false) {
      throw new ApiError(payload.msg || 'Request failed', response.status, payload);
    }
    return (payload.data ?? payload) as T;
  }

  return payload as T;
}

function parsePayload(text: string): unknown {
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function isEnvelope(value: unknown): value is ApiEnvelope<unknown> {
  return Boolean(
    value &&
      typeof value === 'object' &&
      ('success' in value || 'msg' in value || 'data' in value),
  );
}

function extractMessage(value: unknown): string | undefined {
  if (isEnvelope(value)) return value.msg;
  if (typeof value === 'string') return value;
  return undefined;
}
