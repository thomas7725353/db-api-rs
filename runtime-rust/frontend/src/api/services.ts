import { apiGet, apiPost, apiRequest } from './client';
import type { AccessLog, ApiConfig, ApiGroup, AppInfo, DataSource } from './types';

export const systemService = {
  version: () => apiPost<string>('/system/version'),
  mode: () => apiPost<string>('/system/mode'),
  ipPort: () => apiPost<string>('/system/getIPPort'),
  ip: () => apiPost<string>('/system/getIP'),
};

export const datasourceService = {
  list: () => apiPost<DataSource[]>('/datasource/getAll'),
  detail: (id: string) => apiPost<DataSource | null>(`/datasource/detail/${id}`),
  create: (input: DataSource) => apiPost<unknown>('/datasource/add', input),
  update: (input: DataSource) => apiPost<unknown>('/datasource/update', input),
  remove: (id: string) => apiPost<unknown>(`/datasource/delete/${id}`),
  connect: (input: DataSource) => apiPost<unknown>('/datasource/connect', input),
};

export const groupService = {
  list: () => apiPost<ApiGroup[]>('/group/getAll'),
  create: (name: string) => apiPost<unknown>('/group/create', { name }),
  remove: (id: string) => apiPost<unknown>(`/group/delete/${id}`),
};

export const apiConfigService = {
  list: () => apiPost<ApiConfig[]>('/apiConfig/getAll'),
  search: (input: { keyword?: string; field?: string; groupId?: string }) =>
    apiPost<ApiConfig[]>('/apiConfig/search', input),
  detail: (id: string) => apiPost<ApiConfig | null>(`/apiConfig/detail/${id}`),
  create: (input: ApiConfig) => apiPost<unknown>('/apiConfig/add', input),
  update: (input: ApiConfig) => apiPost<unknown>('/apiConfig/update', input),
  remove: (id: string) => apiPost<unknown>(`/apiConfig/delete/${id}`),
  online: (id: string) => apiPost<unknown>(`/apiConfig/online/${id}`),
  offline: (id: string) => apiPost<unknown>(`/apiConfig/offline/${id}`),
};

export const appService = {
  list: () => apiPost<AppInfo[]>('/app/getAll'),
  create: (input: Pick<AppInfo, 'name' | 'note' | 'expireDesc'>) =>
    apiPost<AppInfo>('/app/create', input),
  remove: (id: string) => apiPost<unknown>(`/app/delete/${id}`),
  authorize: (appId: string, groupIds: string[]) =>
    apiPost<unknown>('/app/auth', { appId, groupIds: groupIds.join(',') }),
  authGroups: (appId: string) => apiPost<string[]>(`/app/getAuthGroups/${appId}`),
  token: (appid: string, secret: string) =>
    apiGet<{ token: string; appId: string; expireAt: number }>(
      `/token/generate?appid=${encodeURIComponent(appid)}&secret=${encodeURIComponent(secret)}`,
    ),
};

export const monitorService = {
  search: (input: Record<string, unknown>) => apiPost<AccessLog[]>('/access/search', input),
  countByDay: (input: Record<string, unknown>) =>
    apiPost<Array<Record<string, unknown>>>('/access/countByDay', input),
  successRatio: (input: Record<string, unknown>) =>
    apiPost<Record<string, unknown>>('/access/successRatio', input),
};

export async function callUserApi(
  path: string,
  body: unknown,
  contentType: string,
  token?: string,
): Promise<unknown> {
  const headers: Record<string, string> = { 'Content-Type': contentType };
  if (token) headers.Authorization = token;
  const requestBody =
    contentType === 'application/x-www-form-urlencoded'
      ? new URLSearchParams(body as Record<string, string>).toString()
      : typeof body === 'string'
        ? body
        : JSON.stringify(body);
  return apiRequest(`/api/${path.replace(/^\/+/, '')}`, {
    method: 'POST',
    headers,
    body: requestBody,
  });
}
