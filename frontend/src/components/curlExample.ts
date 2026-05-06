export interface CurlFormParam {
  name: string;
  value?: unknown;
  values?: Array<{ va: unknown }>;
}

export interface CurlCommandInput {
  url: string;
  contentType: string;
  token?: string;
  body?: string;
  params?: CurlFormParam[];
}

export function generateCurlCommand(input: CurlCommandInput): string {
  const lines = [`curl -X POST ${shellQuote(input.url)}`];
  lines.push(`  -H ${shellQuote(`Content-Type: ${input.contentType}`)}`);

  const token = input.token?.trim();
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
