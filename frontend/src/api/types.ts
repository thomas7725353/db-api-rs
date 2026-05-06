import type { RuleGroupType } from 'react-querybuilder';

export type DbType = 'sqlite' | 'mysql' | 'postgres' | string;
export type ApiEngine = 'sql' | 'queryBuilder' | 'viewSql';

export interface DataSource {
  id?: string;
  name?: string;
  note?: string;
  type?: DbType;
  url?: string;
  username?: string;
  password?: string;
  driver?: string;
  tableSql?: string;
  createTime?: string;
  updateTime?: string;
  edit_password?: boolean;
}

export interface ApiSql {
  id?: number;
  apiId?: string;
  sqlText?: string;
  transformPlugin?: string;
  transformPluginParams?: string;
}

export interface ApiConfig {
  id?: string;
  name?: string;
  note?: string;
  path?: string;
  datasourceId?: string;
  sqlList?: ApiSql[];
  params?: string;
  status?: number;
  previlege?: number;
  groupId?: string;
  cachePlugin?: string;
  cachePluginParams?: string;
  createTime?: string;
  updateTime?: string;
  contentType?: string;
  openTrans?: number;
  jsonParam?: string;
  alarmPlugin?: string;
  alarmPluginParam?: string;
}

export interface ApiGroup {
  id?: string;
  name?: string;
}

export interface ApiConfigExport {
  api: ApiConfig[];
  sql: ApiSql[];
}

export interface ApiTreeNode {
  name: string;
  id?: string;
  children?: ApiConfig[];
}

export interface AppInfo {
  id?: string;
  secret?: string;
  name?: string;
  note?: string;
  expireDesc?: string;
  expireDuration?: number;
  token?: string;
  expireAt?: number;
}

export interface AccessLog {
  id?: string;
  url?: string;
  status?: number;
  duration?: number;
  timestamp?: number;
  ip?: string;
  appId?: string;
  apiId?: string;
  error?: string;
}

export interface TableInfo {
  name: string;
}

export interface TableColumn {
  name: string;
  type?: string;
}

export interface QueryBuilderDsl {
  type: 'queryBuilder';
  table: string;
  select: string[];
  rules: RuleGroupType;
  orderBy?: Array<{ field: string; direction: 'asc' | 'desc' }>;
  limit?: { param?: string; default?: number; max?: number };
  offset?: { param?: string; default?: number };
  count?: boolean;
}

export interface ParamSpec {
  name: string;
  type: 'string' | 'bigint' | 'double' | 'date' | 'Array<string>' | 'Array<bigint>' | 'Array<double>' | 'Array<date>' | string;
  note?: string;
  value?: unknown;
  values?: Array<{ va: unknown }>;
}
