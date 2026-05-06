import type { ParamSpec, TableColumn } from '../api/types';

export interface ColumnParamOption {
  value: string;
  label: string;
  type: ParamSpec['type'];
  rawType?: string;
  missing?: boolean;
}

const GENERATED_COLUMNS = new Set(['created_at', 'updated_at']);

export function mapColumnTypeToParamType(rawType?: string): ParamSpec['type'] {
  const type = String(rawType ?? '').toLowerCase();
  if (/\b(bigint|integer|int|smallint|tinyint|serial|bigserial)\b/.test(type)) return 'bigint';
  if (/\b(double|float|real|decimal|numeric|number)\b/.test(type)) return 'double';
  if (/\b(date|time|timestamp|datetime)\b/.test(type)) return 'date';
  return 'string';
}

export function columnParamOptions(columns: TableColumn[]): ColumnParamOption[] {
  return columns
    .filter((column) => Boolean(column.name))
    .map((column) => {
      const mappedType = mapColumnTypeToParamType(column.type);
      return {
        value: column.name,
        label: column.type ? `${column.name} (${column.type})` : column.name,
        type: mappedType,
        rawType: column.type,
      };
    });
}

export function optionForMissingParam(param: ParamSpec): ColumnParamOption {
  return {
    value: param.name,
    label: `${param.name}（字段不存在）`,
    type: normalizeMappedParamType(param.type),
    rawType: param.type,
    missing: true,
  };
}

export function mergeParamOptions(params: ParamSpec[], options: ColumnParamOption[]): ColumnParamOption[] {
  const known = new Set(options.map((option) => option.value));
  const missing = params
    .filter((param) => param.name && !known.has(param.name))
    .map(optionForMissingParam);
  return [...options, ...missing];
}

export function syncParamTypesFromColumns(params: ParamSpec[], columns: TableColumn[]): ParamSpec[] {
  const typeByName = new Map(columns.map((column) => [column.name, mapColumnTypeToParamType(column.type)]));
  return params.map((param) => {
    const mappedType = typeByName.get(param.name);
    return mappedType ? { ...param, type: mappedType } : param;
  });
}

export function findColumnParamOption(options: ColumnParamOption[], name: string): ColumnParamOption | undefined {
  return options.find((option) => option.value === name);
}

export function firstUnusedColumnOption(options: ColumnParamOption[], params: ParamSpec[]): ColumnParamOption | undefined {
  const used = new Set(params.map((param) => param.name).filter(Boolean));
  return options.find((option) => !option.missing && !GENERATED_COLUMNS.has(option.value) && !used.has(option.value));
}

export function inferSqlTableName(sql: string): string {
  const normalized = sql.replace(/\s+/g, ' ').trim();
  const match = normalized.match(/\b(?:from|into|update)\s+["`[]?([a-zA-Z_][\w]*)["`\]]?/i);
  return match?.[1] ?? '';
}

export function hasFixedIdParamContract(sql: string, params: ParamSpec[]): boolean {
  const normalized = sql.replace(/\s+/g, ' ').trim().toLowerCase();
  const isSingleIdParam = params.length === 1 && params[0]?.name === 'id';
  const isReadOrDelete = /^(select|delete)\b/.test(normalized);
  const filtersById = /\bwhere\s+id\s*=\s*\$id\b/.test(normalized);
  return isSingleIdParam && isReadOrDelete && filtersById;
}

function normalizeMappedParamType(type: string | undefined): ParamSpec['type'] {
  if (type === 'number') return 'bigint';
  if (type === 'double' || type === 'date' || type === 'bigint') return type;
  if (String(type ?? '').startsWith('Array<')) return type ?? 'string';
  return 'string';
}
