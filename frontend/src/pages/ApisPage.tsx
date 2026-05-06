import {
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  FileTextOutlined,
  FolderOutlined,
  PlayCircleOutlined,
  PlusOutlined,
  ReloadOutlined,
  UploadOutlined,
} from '@ant-design/icons';
import { App, Button, Input, Modal, Popconfirm, Select, Space, Table, Tag, Tree, Typography } from 'antd';
import type { Key } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { apiConfigService, downloadBlob, groupService } from '../api/services';
import type { ApiConfig, ApiGroup, ApiTreeNode } from '../api/types';

const SEARCH_FIELDS = [
  { value: 'name', label: '名称' },
  { value: 'path', label: '路径' },
  { value: 'note', label: '备注' },
];

export default function ApisPage() {
  const { message } = App.useApp();
  const navigate = useNavigate();
  const [rows, setRows] = useState<ApiConfig[]>([]);
  const [groups, setGroups] = useState<ApiGroup[]>([]);
  const [tree, setTree] = useState<ApiTreeNode[]>([]);
  const [keyword, setKeyword] = useState('');
  const [field, setField] = useState<string | undefined>();
  const [groupId, setGroupId] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);
  const [groupModalOpen, setGroupModalOpen] = useState(false);
  const [newGroupName, setNewGroupName] = useState('');
  const [exportOpen, setExportOpen] = useState(false);
  const [exportMode, setExportMode] = useState<'api' | 'docs'>('api');
  const [checkedApiIds, setCheckedApiIds] = useState<Key[]>([]);
  const [groupExportOpen, setGroupExportOpen] = useState(false);
  const [checkedGroupIds, setCheckedGroupIds] = useState<string[]>([]);
  const apiImportRef = useRef<HTMLInputElement>(null);
  const groupImportRef = useRef<HTMLInputElement>(null);

  async function load(nextFilters?: { keyword?: string; field?: string; groupId?: string }) {
    const nextKeyword = nextFilters && 'keyword' in nextFilters ? nextFilters.keyword ?? '' : keyword;
    const nextField = nextFilters && 'field' in nextFilters ? nextFilters.field : field;
    const nextGroupId = nextFilters && 'groupId' in nextFilters ? nextFilters.groupId : groupId;
    setLoading(true);
    try {
      const hasFilter = Boolean(nextKeyword || nextField || nextGroupId);
      const [nextRows, nextGroups] = await Promise.all([
        hasFilter
          ? apiConfigService.search({ keyword: nextKeyword, field: nextField, groupId: nextGroupId })
          : apiConfigService.list(),
        groupService.list(),
      ]);
      setRows(nextRows);
      setGroups(nextGroups);
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  const groupOptions = useMemo(
    () => groups.filter((group) => group.id).map((group) => ({ value: group.id!, label: group.name || group.id })),
    [groups],
  );

  const groupNameById = useMemo(
    () => new Map(groups.filter((group) => group.id).map((group) => [group.id!, group.name || group.id!])),
    [groups],
  );

  const exportTreeData = useMemo(
    () =>
      tree.map((node) => ({
        key: `group:${node.id ?? node.name}`,
        title: node.name,
        selectable: false,
        children: (node.children || [])
          .filter((api) => api.id)
          .map((api) => ({ key: api.id!, title: `${api.name || api.id} ${api.path || ''}` })),
      })),
    [tree],
  );

  const exportableApiIds = useMemo(
    () => new Set(tree.flatMap((node) => (node.children || []).map((api) => api.id).filter(Boolean) as string[])),
    [tree],
  );

  async function setStatus(row: ApiConfig, online: boolean) {
    if (!row.id) return;
    if (online) await apiConfigService.online(row.id);
    else await apiConfigService.offline(row.id);
    await load();
  }

  async function openExport(mode: 'api' | 'docs') {
    try {
      setExportMode(mode);
      setCheckedApiIds([]);
      setTree(await apiConfigService.tree());
      setExportOpen(true);
    } catch (error) {
      message.error(String(error));
    }
  }

  async function confirmExport() {
    const ids = checkedApiIds.map(String).filter((id) => exportableApiIds.has(id));
    if (!ids.length) {
      message.warning('请选择 API');
      return;
    }
    try {
      const blob =
        exportMode === 'api' ? await apiConfigService.exportConfig(ids) : await apiConfigService.exportDocs(ids);
      downloadBlob(blob, exportMode === 'api' ? 'api_config.json' : 'API Doc.md');
      setExportOpen(false);
    } catch (error) {
      message.error(String(error));
    }
  }

  async function importApis(file: File | undefined) {
    if (!file) return;
    try {
      await apiConfigService.importConfig(file);
      message.success('导入成功');
      await load();
    } catch (error) {
      message.error(String(error));
    } finally {
      if (apiImportRef.current) apiImportRef.current.value = '';
    }
  }

  async function importGroups(file: File | undefined) {
    if (!file) return;
    try {
      await apiConfigService.importGroups(file);
      message.success('导入成功');
      await load();
    } catch (error) {
      message.error(String(error));
    } finally {
      if (groupImportRef.current) groupImportRef.current.value = '';
    }
  }

  async function createGroup() {
    const name = newGroupName.trim();
    if (!name) return;
    try {
      await groupService.create(name);
      setNewGroupName('');
      await load();
    } catch (error) {
      message.error(String(error));
    }
  }

  async function removeGroup(id: string) {
    try {
      await groupService.remove(id);
      await load();
    } catch (error) {
      message.error(String(error));
    }
  }

  async function confirmGroupExport() {
    if (!checkedGroupIds.length) {
      message.warning('请选择分组');
      return;
    }
    try {
      const blob = await apiConfigService.exportGroups(checkedGroupIds);
      downloadBlob(blob, 'api_group_config.json');
      setGroupExportOpen(false);
    } catch (error) {
      message.error(String(error));
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <Typography.Title level={3} className="!mb-1">
            API
          </Typography.Title>
          <Typography.Text type="secondary">SQL API 和 QueryBuilder API 统一管理。</Typography.Text>
        </div>
        <Space>
          <Select
            allowClear
            className="min-w-40"
            placeholder="分组"
            value={groupId}
            options={groupOptions}
            onChange={(value) => {
              setGroupId(value);
              void load({ groupId: value });
            }}
          />
          <Input.Search
            allowClear
            addonBefore={
              <Select
                allowClear
                className="w-24"
                placeholder="字段"
                value={field}
                options={SEARCH_FIELDS}
                onChange={setField}
              />
            }
            placeholder="搜索名称 / 路径"
            value={keyword}
            onChange={(event) => setKeyword(event.target.value)}
            onSearch={() => void load()}
          />
          <Button icon={<ReloadOutlined />} onClick={() => void load()}>
            刷新
          </Button>
          <Button icon={<FolderOutlined />} onClick={() => setGroupModalOpen(true)}>
            分组
          </Button>
          <Button icon={<FileTextOutlined />} onClick={() => void openExport('docs')}>
            导出文档
          </Button>
          <Button icon={<DownloadOutlined />} onClick={() => void openExport('api')}>
            导出 API
          </Button>
          <Button icon={<UploadOutlined />} onClick={() => apiImportRef.current?.click()}>
            导入 API
          </Button>
          <Button
            icon={<DownloadOutlined />}
            onClick={() => {
              setCheckedGroupIds([]);
              setGroupExportOpen(true);
            }}
          >
            导出分组
          </Button>
          <Button icon={<UploadOutlined />} onClick={() => groupImportRef.current?.click()}>
            导入分组
          </Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => navigate('/apis/new')}>
            创建 API
          </Button>
          <input
            ref={apiImportRef}
            type="file"
            accept=".json"
            className="hidden"
            onChange={(event) => void importApis(event.target.files?.[0])}
          />
          <input
            ref={groupImportRef}
            type="file"
            accept=".json"
            className="hidden"
            onChange={(event) => void importGroups(event.target.files?.[0])}
          />
        </Space>
      </div>
      <Table<ApiConfig>
        rowKey={(row) => row.id ?? row.path ?? Math.random().toString()}
        loading={loading}
        dataSource={rows}
        columns={[
          { title: '名称', dataIndex: 'name' },
          { title: '路径', dataIndex: 'path' },
          {
            title: '分组',
            render: (_, row) => (row.groupId ? groupNameById.get(row.groupId) || row.groupId : '-'),
          },
          {
            title: '模式',
            render: (_, row) => {
              const plugin = row.sqlList?.[0]?.transformPlugin;
              return <Tag color={plugin === 'queryBuilder' ? 'cyan' : 'blue'}>{plugin || 'sql'}</Tag>;
            },
          },
          {
            title: '状态',
            render: (_, row) =>
              row.status === 1 ? <Tag color="green">在线</Tag> : <Tag>离线</Tag>,
          },
          { title: '更新时间', dataIndex: 'updateTime', width: 180 },
          {
            title: '操作',
            width: 320,
            render: (_, row) => (
              <Space>
                <Button size="small" icon={<EditOutlined />} onClick={() => navigate(`/apis/${row.id}/edit`)}>
                  编辑
                </Button>
                <Button size="small" icon={<PlayCircleOutlined />} onClick={() => navigate(`/apis/${row.id}/request`)}>
                  测试
                </Button>
                <Button size="small" onClick={() => setStatus(row, row.status !== 1)}>
                  {row.status === 1 ? '下线' : '上线'}
                </Button>
                <Popconfirm
                  title="确认删除 API？"
                  onConfirm={async () => {
                    await apiConfigService.remove(row.id!);
                    await load();
                  }}
                >
                  <Button size="small" danger icon={<DeleteOutlined />} />
                </Popconfirm>
              </Space>
            ),
          },
        ]}
      />
      <Modal title="API 分组" open={groupModalOpen} onCancel={() => setGroupModalOpen(false)} footer={null}>
        <Space.Compact className="mb-4 w-full">
          <Input
            value={newGroupName}
            placeholder="新分组名称"
            onChange={(event) => setNewGroupName(event.target.value)}
            onPressEnter={() => void createGroup()}
          />
          <Button type="primary" onClick={() => void createGroup()}>
            创建
          </Button>
        </Space.Compact>
        <Space wrap>
          {groups.map((group) => (
            <Tag
              key={group.id}
              closable={Boolean(group.id)}
              onClose={(event) => {
                event.preventDefault();
                if (group.id) void removeGroup(group.id);
              }}
            >
              {group.name || group.id}
            </Tag>
          ))}
        </Space>
      </Modal>
      <Modal
        title={exportMode === 'api' ? '导出 API' : '导出 API 文档'}
        open={exportOpen}
        onCancel={() => setExportOpen(false)}
        onOk={confirmExport}
      >
        <Tree
          checkable
          treeData={exportTreeData}
          checkedKeys={checkedApiIds}
          onCheck={(keys) => setCheckedApiIds(Array.isArray(keys) ? keys : keys.checked)}
        />
      </Modal>
      <Modal
        title="导出 API 分组"
        open={groupExportOpen}
        onCancel={() => {
          setCheckedGroupIds([]);
          setGroupExportOpen(false);
        }}
        onOk={confirmGroupExport}
      >
        <Select
          mode="multiple"
          className="w-full"
          placeholder="选择分组"
          value={checkedGroupIds}
          options={groupOptions}
          onChange={setCheckedGroupIds}
        />
      </Modal>
    </div>
  );
}
