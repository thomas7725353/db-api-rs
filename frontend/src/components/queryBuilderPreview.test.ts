import { describe, expect, it } from 'vitest';
import type { RuleGroupType } from 'react-querybuilder';
import type { QueryBuilderDsl } from '../api/types';
import {
  inferQueryBuilderPageParams,
  queryBuilderDslToPreview,
  sanitizeQueryBuilderDsl,
} from './queryBuilderPreview';

const rules = {
  combinator: 'and',
  rules: [
    {
      field: 'name',
      operator: 'doesNotContain',
      valueSource: 'param',
      value: { param: 'keyword', default: 'draft', defaultText: '"draft"' },
    },
    {
      field: 'created_at',
      operator: 'between',
      valueSource: 'param',
      value: { param: 'createdRange', default: ['2026-01-01', '2026-01-31'] },
    },
    {
      field: 'status',
      operator: 'notIn',
      valueSource: 'param',
      value: { param: 'statuses', default: ['archived', 'deleted'] },
    },
    {
      field: 'updated_at',
      operator: '>=',
      valueSource: 'field',
      value: 'created_at',
    },
    {
      field: 'ignored',
      operator: '=',
      valueSource: 'param',
      value: { param: '   ' },
    },
  ],
} as unknown as RuleGroupType;

const dsl: QueryBuilderDsl = {
  type: 'queryBuilder',
  table: 'demo_items',
  select: ['id', 'name', 'status', 'created_at'],
  rules,
  orderBy: [{ field: 'id', direction: 'desc' }],
  limit: { param: 'limit', default: 20, max: 100 },
  offset: { param: 'offset', default: 0 },
  count: true,
};

describe('queryBuilderPreview', () => {
  it('converts rules to SQL with readable params and native operators', () => {
    const sql = queryBuilderDslToPreview(dsl, 'sql');

    expect(sql).toContain('select id, name, status, created_at');
    expect(sql).toContain('from demo_items');
    expect(sql).toContain('order by id desc');
    expect(sql).toContain('limit $limit');
    expect(sql).toContain('offset $offset');
    expect(sql).toContain('select count(*) as total');
    expect(sql).toContain('$keyword');
    expect(sql).toContain('$createdRange');
    expect(sql).toContain('$statuses');
    expect(sql).toContain('between');
    expect(sql).toContain('not in');
    expect(sql).toContain('not like');
    expect(sql).toContain('updated_at >= created_at');
    expect(sql).not.toContain("'$keyword'");
    expect(sql).not.toContain("'$status'");
    expect(sql).not.toContain('[object Object]');
    expect(sql).not.toContain('$   ');
    expect(sql).not.toContain('ignored');
  });

  it('keeps simple param placeholders unquoted in full SQL preview', () => {
    const previewDsl: QueryBuilderDsl = {
      ...dsl,
      select: ['id', 'name', 'status', 'note', 'created_at', 'updated_at'],
      rules: {
        combinator: 'and',
        rules: [
          {
            field: 'name',
            operator: 'contains',
            valueSource: 'param',
            value: 'keyword',
            skipWhen: ['missing', 'empty_string'],
          },
          {
            field: 'status',
            operator: '=',
            valueSource: 'param',
            value: { param: 'status', default: 12 },
            skipWhen: ['missing', 'empty_string'],
          },
        ],
      } as unknown as RuleGroupType,
    };

    const sql = queryBuilderDslToPreview(previewDsl, 'sql');

    expect(sql).toContain("name like '%' || $keyword || '%'");
    expect(sql).toContain('status = $status');
    expect(sql).toContain('limit $limit');
    expect(sql).toContain('offset $offset');
    expect(sql).not.toContain("'$status'");
  });

  it('converts rules to sanitized full DSL JSON', () => {
    const preview = queryBuilderDslToPreview(dsl, 'json');
    const parsed = JSON.parse(preview) as QueryBuilderDsl;

    expect(parsed).toEqual({
      ...dsl,
      rules: {
        ...rules,
        rules: [
          {
            field: 'name',
            operator: 'doesNotContain',
            valueSource: 'param',
            value: { param: 'keyword', default: 'draft' },
          },
          {
            field: 'created_at',
            operator: 'between',
            valueSource: 'param',
            value: { param: 'createdRange', default: ['2026-01-01', '2026-01-31'] },
          },
          {
            field: 'status',
            operator: 'notIn',
            valueSource: 'param',
            value: { param: 'statuses', default: ['archived', 'deleted'] },
          },
          {
            field: 'updated_at',
            operator: '>=',
            valueSource: 'field',
            value: 'created_at',
          },
          {
            field: 'ignored',
            operator: '=',
            valueSource: 'param',
            value: '',
          },
        ],
      },
    });
    expect(sanitizeQueryBuilderDsl(dsl).limit).toEqual({ param: 'limit', default: 20, max: 100 });
  });

  it('infers only pagination params as bigint', () => {
    expect(inferQueryBuilderPageParams(dsl)).toEqual([
      { name: 'limit', type: 'bigint' },
      { name: 'offset', type: 'bigint' },
    ]);
  });
});
