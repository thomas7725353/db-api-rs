import { apiDownload, apiGet, apiPost, apiRequest, apiUpload } from './client';
import type { AccessLog, ApiConfig, ApiGroup, ApiMethod, ApiTreeNode, AppInfo, DataSource, TableColumn } from './types';

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


export const tableService = {
  tables: (datasourceId?: string) => apiPost<string[]>('/table/getAllTables', { datasourceId }),
  columns: (datasourceId?: string, table?: string) =>
    apiPost<TableColumn[]>('/table/getAllColumns', { datasourceId, table }),
};

export const groupService = {
  list: () => apiPost<ApiGroup[]>('/group/getAll'),
  create: (name: string) => apiPost<unknown>('/group/create', { name }),
  remove: (id: string) => apiPost<unknown>(`/group/delete/${id}`),
};

function idsQuery(ids: string[]): string {
  return `ids=${encodeURIComponent(ids.join(','))}`;
}

export function downloadBlob(blob: Blob, filename: string) {
  const link = document.createElement('a');
  link.href = URL.createObjectURL(blob);
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(link.href);
}

export const apiConfigService = {
  list: () => apiPost<ApiConfig[]>('/apiConfig/getAll'),
  search: (input: { keyword?: string; field?: string; groupId?: string }) =>
    apiPost<ApiConfig[]>('/apiConfig/search', input),
  tree: () => apiPost<ApiTreeNode[]>('/apiConfig/getApiTree'),
  detail: (id: string) => apiPost<ApiConfig | null>(`/apiConfig/detail/${id}`),
  create: (input: ApiConfig) => apiPost<unknown>('/apiConfig/add', input),
  update: (input: ApiConfig) => apiPost<unknown>('/apiConfig/update', input),
  remove: (id: string) => apiPost<unknown>(`/apiConfig/delete/${id}`),
  online: (id: string) => apiPost<unknown>(`/apiConfig/online/${id}`),
  offline: (id: string) => apiPost<unknown>(`/apiConfig/offline/${id}`),
  exportConfig: (ids: string[]) =>
    apiDownload(`/apiConfig/downloadConfig?${idsQuery(ids)}`, { method: 'POST' }),
  exportDocs: (ids: string[]) =>
    apiDownload(`/apiConfig/apiDocs?${idsQuery(ids)}`, { method: 'POST' }),
  importConfig: (file: File) => apiUpload<unknown>('/apiConfig/import', file),
  exportGroups: (ids: string[]) =>
    apiDownload(`/apiConfig/downloadGroupConfig?${idsQuery(ids)}`, { method: 'POST' }),
  importGroups: (file: File) => apiUpload<unknown>('/apiConfig/importGroup', file),
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
  topApi: (input: Record<string, unknown>) =>
    apiPost<Array<Record<string, unknown>>>('/access/top5api', input),
  topApp: (input: Record<string, unknown>) =>
    apiPost<Array<Record<string, unknown>>>('/access/top5app', input),
  topIp: (input: Record<string, unknown>) =>
    apiPost<Array<Record<string, unknown>>>('/access/topNIP', input),
  topDuration: (input: Record<string, unknown>) =>
    apiPost<Array<Record<string, unknown>>>('/access/top5duration', input),
};

export async function callUserApi(
  path: string,
  body: Record<string, unknown>,
  contentType: string,
  token?: string,
  method: ApiMethod = 'POST',
): Promise<unknown> {
  const normalizedMethod = method || 'POST';
  const cleanPath = `/api/${path.replace(/^\/+/, '')}`;
  const headers: Record<string, string> = {};
  if (token) headers.Authorization = token;

  if (normalizedMethod === 'GET' || normalizedMethod === 'DELETE') {
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(body)) {
      if (Array.isArray(value)) {
        for (const item of value) query.append(key, String(item));
      } else if (value !== undefined && value !== null && value !== '') {
        query.set(key, String(value));
      }
    }
    const suffix = query.toString() ? `?${query.toString()}` : '';
    return apiRequest(`${cleanPath}${suffix}`, { method: normalizedMethod, headers });
  }

  headers['Content-Type'] = contentType;
  const requestBody =
    contentType === 'application/x-www-form-urlencoded'
      ? new URLSearchParams(body as Record<string, string>).toString()
      : typeof body === 'string'
        ? body
        : JSON.stringify(body);
  return apiRequest(cleanPath, {
    method: normalizedMethod,
    headers,
    body: requestBody,
  });
}
