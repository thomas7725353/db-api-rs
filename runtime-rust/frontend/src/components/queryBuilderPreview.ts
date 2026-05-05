import type { RuleGroupType, RuleType, ValueSource } from 'react-querybuilder';
import type { ParamSpec, QueryBuilderDsl } from '../api/types';

export type PreviewFormat = 'sql' | 'json';

type DbApiRule = RuleType & {
  valueSource?: ValueSource | 'param';
  value?: unknown;
};

type ParamValue = {
  param?: unknown;
  default?: unknown;
};

const PARAM_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;
const IDENTIFIER_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;
const OPERATOR_ALIASES: Record<string, string> = {
  begins_with: 'beginsWith',
  ends_with: 'endsWith',
  not_in: 'notIn',
  not_null: 'notNull',
};

export function sanitizeQueryBuilderDsl(dsl: QueryBuilderDsl): QueryBuilderDsl {
  return {
    ...dsl,
    rules: sanitizeGroup(dsl.rules),
  };
}

export function queryBuilderDslToPreview(dsl: QueryBuilderDsl, format: PreviewFormat): string {
  const sanitized = sanitizeQueryBuilderDsl(dsl);
  if (format === 'json') return JSON.stringify(sanitized, null, 2);
  return queryBuilderDslToSql(sanitized);
}

export function inferQueryBuilderPageParams(dsl: QueryBuilderDsl): ParamSpec[] {
  const params: ParamSpec[] = [];
  if (validParamName(dsl.limit?.param)) params.push({ name: dsl.limit.param.trim(), type: 'bigint' });
  if (validParamName(dsl.offset?.param)) params.push({ name: dsl.offset.param.trim(), type: 'bigint' });
  return params;
}

function sanitizeGroup(group: RuleGroupType): RuleGroupType {
  return {
    ...group,
    rules: (group.rules ?? []).map((node) => {
      if ('rules' in node) return sanitizeGroup(node as RuleGroupType);
      const rule = { ...(node as DbApiRule) };
      rule.operator = normalizeOperatorName(rule.operator);
      if (String(rule.valueSource) === 'param') {
        rule.value = sanitizeParamRuleValue(rule.value);
      }
      return rule as RuleType;
    }),
  };
}

function sanitizeParamRuleValue(value: unknown): unknown {
  if (typeof value === 'string') return validParamName(value) ? value.trim() : '';
  if (value && typeof value === 'object' && 'param' in value) {
    const paramValue = value as ParamValue;
    const param = typeof paramValue.param === 'string' ? paramValue.param.trim() : '';
    if (!validParamName(param)) return '';
    return paramValue.default === undefined ? param : { param, default: paramValue.default };
  }
  return '';
}

function normalizeOperatorName(operator: string): string {
  return OPERATOR_ALIASES[operator] ?? operator;
}

function validParamName(value?: string): value is string {
  return typeof value === 'string' && PARAM_NAME_PATTERN.test(value.trim());
}

function queryBuilderDslToSql(dsl: QueryBuilderDsl): string {
  const table = sqlIdentifier(dsl.table) ?? '/* invalid table */';
  const where = formatWhereGroup(dsl.rules);
  const listSql = compact([
    `select ${formatSelect(dsl.select)}`,
    `from ${table}`,
    where ? `where (${where})` : undefined,
    formatOrderBy(dsl.orderBy),
    `limit ${pageValue(dsl.limit, 20)}`,
    `offset ${pageValue(dsl.offset, 0)}`,
  ]).join('\n');

  if (!dsl.count) return listSql;

  const countSql = compact([
    'select count(*) as total',
    `from ${table}`,
    where ? `where (${where})` : undefined,
  ]).join('\n');
  return `${listSql}\n\n-- count\n${countSql}`;
}

function formatWhereGroup(group: RuleGroupType): string | undefined {
  const parts = (group.rules ?? [])
    .map((node) => {
      if ('rules' in node) {
        const nested = formatWhereGroup(node as RuleGroupType);
        return nested ? `(${nested})` : undefined;
      }
      return formatRule(node as DbApiRule);
    })
    .filter((part): part is string => Boolean(part));

  if (parts.length === 0) return undefined;
  const combinator = String(group.combinator ?? 'and').toLowerCase() === 'or' ? 'or' : 'and';
  return parts.join(` ${combinator} `);
}

function formatRule(rule: DbApiRule): string | undefined {
  const field = sqlIdentifier(rule.field);
  if (!field) return undefined;
  const operator = operatorKey(rule.operator);

  if (operator === 'null' || operator === 'isnull') return `${field} is null`;
  if (operator === 'notnull' || operator === 'isnotnull') return `${field} is not null`;

  if (['=', '==', '!=', '<>', '>', '>=', '<', '<='].includes(operator)) {
    const value = singleValueSql(rule);
    if (!value) return undefined;
    const sqlOperator = operator === '==' ? '=' : operator;
    return `${field} ${sqlOperator} ${value}`;
  }

  if (operator === 'contains' || operator === 'like') return likeRule(field, rule, 'contains', false);
  if (operator === 'beginswith') return likeRule(field, rule, 'beginsWith', false);
  if (operator === 'endswith') return likeRule(field, rule, 'endsWith', false);
  if (operator === 'doesnotcontain') return likeRule(field, rule, 'contains', true);
  if (operator === 'doesnotbeginwith') return likeRule(field, rule, 'beginsWith', true);
  if (operator === 'doesnotendwith') return likeRule(field, rule, 'endsWith', true);

  if (operator === 'in' || operator === 'notin') {
    const values = listValueSql(rule);
    if (!values) return undefined;
    return `${field} ${operator === 'notin' ? 'not in' : 'in'} (${values})`;
  }

  if (operator === 'between' || operator === 'notbetween') {
    const values = betweenValueSql(rule);
    if (!values) return undefined;
    return `${field} ${operator === 'notbetween' ? 'not between' : 'between'} ${values[0]} and ${values[1]}`;
  }

  return undefined;
}

function likeRule(field: string, rule: DbApiRule, mode: 'contains' | 'beginsWith' | 'endsWith', negated: boolean): string | undefined {
  const value = likeValueSql(rule, mode);
  if (!value) return undefined;
  return `${field} ${negated ? 'not like' : 'like'} ${value}`;
}

function singleValueSql(rule: DbApiRule): string | undefined {
  const source = String(rule.valueSource ?? 'value');
  if (source === 'param') return paramSql(rule.value);
  if (source === 'field') return fieldSql(rule.value);
  return literalSql(rule.value);
}

function likeValueSql(rule: DbApiRule, mode: 'contains' | 'beginsWith' | 'endsWith'): string | undefined {
  const source = String(rule.valueSource ?? 'value');
  if (source === 'param') {
    const value = paramSql(rule.value);
    return value ? likePattern(value, mode, false) : undefined;
  }
  if (source === 'field') {
    const value = fieldSql(rule.value);
    return value ? likePattern(value, mode, false) : undefined;
  }
  return likePattern(literalRaw(rule.value), mode, true);
}

function listValueSql(rule: DbApiRule): string | undefined {
  const source = String(rule.valueSource ?? 'value');
  if (source === 'param') return paramSql(rule.value);
  const values = listValues(rule.value);
  if (!values.length) return undefined;
  const sqlValues = source === 'field' ? values.map(fieldSql) : values.map(literalSql);
  if (sqlValues.some((value) => !value)) return undefined;
  return sqlValues.join(', ');
}

function betweenValueSql(rule: DbApiRule): [string, string] | undefined {
  const source = String(rule.valueSource ?? 'value');
  if (source === 'param') {
    const value = paramSql(rule.value);
    return value ? [`${value}[0]`, `${value}[1]`] : undefined;
  }
  const values = listValues(rule.value);
  if (values.length !== 2) return undefined;
  const sqlValues = source === 'field' ? values.map(fieldSql) : values.map(literalSql);
  if (!sqlValues[0] || !sqlValues[1]) return undefined;
  return [sqlValues[0], sqlValues[1]];
}

function paramSql(value: unknown): string | undefined {
  if (typeof value === 'string') return validParamName(value) ? `$${value.trim()}` : undefined;
  if (value && typeof value === 'object' && 'param' in value) {
    const param = (value as ParamValue).param;
    return typeof param === 'string' && validParamName(param) ? `$${param.trim()}` : undefined;
  }
  return undefined;
}

function fieldSql(value: unknown): string | undefined {
  return typeof value === 'string' ? sqlIdentifier(value) : undefined;
}

function listValues(value: unknown): unknown[] {
  if (Array.isArray(value)) return value;
  if (typeof value === 'string') {
    return value
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean);
  }
  return [];
}

function likePattern(value: string, mode: 'contains' | 'beginsWith' | 'endsWith', literal: boolean): string {
  if (literal) {
    if (mode === 'contains') return literalSql(`%${value}%`);
    if (mode === 'beginsWith') return literalSql(`${value}%`);
    return literalSql(`%${value}`);
  }
  if (mode === 'contains') return "'%' || " + value + " || '%'";
  if (mode === 'beginsWith') return `${value} || '%'`;
  return "'%' || " + value;
}

function literalRaw(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value === null || value === undefined) return '';
  return String(value);
}

function literalSql(value: unknown): string {
  if (typeof value === 'number' || typeof value === 'bigint') return String(value);
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (value === null || value === undefined) return 'null';
  return `'${String(value).replace(/'/g, "''")}'`;
}

function formatSelect(select: string[]): string {
  const fields = (select ?? [])
    .map((field) => sqlIdentifier(field))
    .filter((field): field is string => Boolean(field));
  return fields.length > 0 ? fields.join(', ') : '*';
}

function formatOrderBy(orderBy?: QueryBuilderDsl['orderBy']): string | undefined {
  const items = (orderBy ?? [])
    .map((item) => {
      const field = sqlIdentifier(item.field);
      if (!field) return undefined;
      const direction = item.direction === 'asc' ? 'asc' : 'desc';
      return `${field} ${direction}`;
    })
    .filter((item): item is string => Boolean(item));
  return items.length > 0 ? `order by ${items.join(', ')}` : undefined;
}

function pageValue(spec: QueryBuilderDsl['limit'] | QueryBuilderDsl['offset'], fallback: number): string {
  if (validParamName(spec?.param)) return `$${spec.param.trim()}`;
  return String(spec?.default ?? fallback);
}

function sqlIdentifier(value: string): string | undefined {
  const trimmed = value.trim();
  if (trimmed === '*') return trimmed;
  return IDENTIFIER_PATTERN.test(trimmed) ? trimmed : undefined;
}

function operatorKey(operator: string): string {
  return normalizeOperatorName(operator).trim().replace(/[\s_-]/g, '').toLowerCase();
}

function compact(values: Array<string | undefined>): string[] {
  return values.filter((value): value is string => Boolean(value));
}
