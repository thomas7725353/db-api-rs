import type { ApiMethod } from '../api/types';

export interface CurlFormParam {
  name: string;
  value?: unknown;
  values?: Array<{ va: unknown }>;
}

export interface CurlCommandInput {
  method?: ApiMethod;
  url: string;
  contentType: string;
  token?: string;
  body?: string;
  params?: CurlFormParam[];
}

export function generateCurlCommand(input: CurlCommandInput): string {
  const method = input.method || 'POST';
  const queryParams = method === 'GET' || method === 'DELETE' ? formParts(input.params ?? []) : [];
  const url = queryParams.length ? addQueryString(input.url, queryParams) : input.url;
  const lines = method === 'GET' ? [`curl ${shellQuote(url)}`] : [`curl -X ${method} ${shellQuote(url)}`];

  const token = input.token?.trim();
  if (method === 'GET' || method === 'DELETE') {
    if (token) {
      lines.push(`  -H ${shellQuote(`Authorization: ${token}`)}`);
    }
    return lines.map((line, index) => (index < lines.length - 1 ? `${line} \\` : line)).join('\n');
  }

  lines.push(`  -H ${shellQuote(`Content-Type: ${input.contentType}`)}`);
  if (token) {
    lines.push(`  -H ${shellQuote(`Authorization: ${token}`)}`);
  }

  if (input.contentType.startsWith('application/x-www-form-urlencoded')) {
    for (const part of formParts(input.params ?? [])) {
      lines.push(`  --data-urlencode ${shellQuote(part)}`);
    }
  } else {
    lines.push(`  --data-raw ${shellQuote(input.body ?? '{}')}`);
  }

  return lines.map((line, index) => (index < lines.length - 1 ? `${line} \\` : line)).join('\n');
}

function addQueryString(url: string, parts: string[]): string {
  const separator = url.includes('?') ? '&' : '?';
  return `${url}${separator}${parts.map(encodeQueryPart).join('&')}`;
}

function encodeQueryPart(part: string): string {
  const [key, ...rest] = part.split('=');
  return `${encodeURIComponent(key)}=${encodeURIComponent(rest.join('='))}`;
}

function formParts(params: CurlFormParam[]): string[] {
  const parts: string[] = [];
  for (const param of params) {
    if (!param.name) continue;

    if (param.values?.length) {
      for (const item of param.values) {
        parts.push(`${param.name}=${stringValue(item.va)}`);
      }
      continue;
    }

    parts.push(`${param.name}=${stringValue(param.value)}`);
  }
  return parts;
}

function stringValue(value: unknown): string {
  if (value === null || value === undefined) return '';
  return String(value);
}

function shellQuote(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`;
}
