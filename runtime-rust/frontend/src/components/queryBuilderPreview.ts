import {
  defaultValueProcessor,
  formatQuery,
  type RuleGroupType,
  type RuleType,
  type ValueSource,
} from 'react-querybuilder';
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
const MULTI_VALUE_OPERATORS = new Set(['between', 'notBetween', 'in', 'notIn', 'not_in']);
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
  return formatQuery(toSqlPreviewGroup(sanitized.rules), {
    format: 'sql',
    valueProcessor: previewValueProcessor,
  });
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

function toSqlPreviewGroup(group: RuleGroupType): RuleGroupType {
  const rules: Array<RuleType | RuleGroupType> = [];
  for (const node of group.rules ?? []) {
    if ('rules' in node) {
      rules.push(toSqlPreviewGroup(node as RuleGroupType));
      continue;
    }
    const rule = toSqlPreviewRule(node as DbApiRule);
    if (rule) rules.push(rule as RuleType);
  }
  return {
    ...group,
    rules,
  };
}

function toSqlPreviewRule(rule: DbApiRule): DbApiRule | undefined {
  const next: DbApiRule = {
    ...rule,
    operator: normalizeOperatorName(rule.operator),
  };
  if (String(next.valueSource) !== 'param') return next;

  const placeholder = paramPlaceholder(next.value);
  next.valueSource = 'value';
  if (!placeholder) return undefined;

  next.value = MULTI_VALUE_OPERATORS.has(String(next.operator))
    ? [placeholder, placeholder]
    : placeholder;
  return next;
}

function paramPlaceholder(value: unknown): string | undefined {
  if (typeof value === 'string') return validParamName(value) ? `$${value.trim()}` : undefined;
  if (value && typeof value === 'object' && 'param' in value) {
    const param = (value as ParamValue).param;
    return typeof param === 'string' && validParamName(param) ? `$${param.trim()}` : undefined;
  }
  return undefined;
}

function normalizeOperatorName(operator: string): string {
  return OPERATOR_ALIASES[operator] ?? operator;
}

function previewValueProcessor(field: string, operator: string, value: unknown, valueSource?: ValueSource): string {
  if (valueSource === 'field') return String(value ?? '');
  return defaultValueProcessor(field, operator, value);
}

function validParamName(value?: string): value is string {
  return typeof value === 'string' && PARAM_NAME_PATTERN.test(value.trim());
}
