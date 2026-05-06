import type { ApiSql } from '../api/types';

export type ApiResponseMode = 'list' | 'page' | 'object' | 'count';

export function resultTypeParams(mode: ApiResponseMode): string {
  return `resultType=${mode}`;
}

export function responseModeRequiresCountSql(mode: ApiResponseMode): boolean {
  return mode === 'page' || mode === 'count';
}

export function buildViewSqlList(
  viewSqlText: string,
  viewCountSqlText: string,
  responseMode: ApiResponseMode,
): ApiSql[] {
  return [
    {
      sqlText: viewSqlText,
      transformPlugin: 'viewSql',
      transformPluginParams: resultTypeParams(responseMode),
    },
    ...(responseModeRequiresCountSql(responseMode)
      ? [
          {
            sqlText: viewCountSqlText,
            transformPlugin: 'viewSqlCount',
            transformPluginParams: '',
          },
        ]
      : []),
  ];
}
