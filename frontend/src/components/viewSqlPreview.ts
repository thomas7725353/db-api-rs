export interface ViewSqlPreview {
  sql: string;
}

const VARIABLE_PATTERN =
  /\[\[\s*([A-Za-z_][A-Za-z0-9_]*)\s*\|\s*([A-Za-z_][A-Za-z0-9_]*)(?:\(([^)]*)\))?\s*\]\]/g;

export function renderViewSqlPreview(
  template: string,
  params: Record<string, unknown>,
): ViewSqlPreview {
  const sql = template.replace(
    VARIABLE_PATTERN,
    (_match, name: string, filter: string, args: string | undefined) => {
      const value = params[name];
      if (filter === 'ident') return renderIdent(value);
      if (filter === 'ident_list') return renderIdentList(value);
      if (filter === 'int') return renderInt(value, parseFilterArgs(args));
      throw new Error(`Unsupported filter: ${filter}`);
    },
  );
  return { sql };
}

function renderIdent(value: unknown): string {
  if (typeof value !== 'string') throw new Error('identifier must be a string');
  validateIdentifier(value);
  return value.trim();
}

function renderIdentList(value: unknown): string {
  const values = Array.isArray(value)
    ? value
    : typeof value === 'string'
      ? value
          .split(',')
          .map((item) => item.trim())
          .filter(Boolean)
      : [];
  if (values.length === 0) throw new Error('identifier list cannot be empty');
  return values.map(renderIdent).join(', ');
}

function renderInt(value: unknown, args: Record<string, number>): string {
  const fallback = args.default ?? 0;
  const parsed =
    typeof value === 'number' && Number.isFinite(value)
      ? Math.trunc(value)
      : typeof value === 'string' && /^-?\d+$/.test(value.trim())
        ? Number.parseInt(value.trim(), 10)
        : fallback;
  let next = parsed;
  if (args.min !== undefined) next = Math.max(args.min, next);
  if (args.max !== undefined) next = Math.min(args.max, next);
  return String(next);
}

function parseFilterArgs(raw: string | undefined): Record<string, number> {
  if (!raw?.trim()) return {};
  const args: Record<string, number> = {};
  for (const part of raw.split(',')) {
    const [key, value] = part.split('=').map((item) => item.trim());
    if (!key || !/^-?\d+$/.test(value ?? '')) continue;
    args[key] = Number.parseInt(value, 10);
  }
  return args;
}

function validateIdentifier(raw: string) {
  const trimmed = raw.trim();
  if (trimmed === '*') return;
  const segments = trimmed.split('.');
  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    if (segment === '*') {
      if (index === segments.length - 1) return;
      throw new Error(`Invalid identifier: ${raw}`);
    }
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(segment)) {
      throw new Error(`Invalid identifier: ${raw}`);
    }
  }
}
