import { describe, expect, it } from 'vitest';
import { buildViewSqlList, responseModeRequiresCountSql } from './apiEditorPayload';

describe('apiEditorPayload', () => {
  it('stores list and count templates for page view sql APIs', () => {
    expect(buildViewSqlList('select a.* from demo_items a limit [[ limit | int(default=10) ]]', 'select count(*) as total from demo_items', 'page')).toEqual([
      {
        sqlText: 'select a.* from demo_items a limit [[ limit | int(default=10) ]]',
        transformPlugin: 'viewSql',
        transformPluginParams: 'resultType=page',
      },
      {
        sqlText: 'select count(*) as total from demo_items',
        transformPlugin: 'viewSqlCount',
        transformPluginParams: '',
      },
    ]);
  });

  it('stores only list template for list view sql APIs', () => {
    expect(buildViewSqlList('select a.* from demo_items a', 'select count(*) as total from demo_items', 'list')).toHaveLength(1);
  });

  it('requires count sql only for page and count response modes', () => {
    expect(responseModeRequiresCountSql('page')).toBe(true);
    expect(responseModeRequiresCountSql('count')).toBe(true);
    expect(responseModeRequiresCountSql('list')).toBe(false);
    expect(responseModeRequiresCountSql('object')).toBe(false);
  });
});
