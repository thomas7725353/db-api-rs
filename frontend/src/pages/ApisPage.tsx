import {
  DeleteOutlined,
  EditOutlined,
  PlayCircleOutlined,
  PlusOutlined,
  ReloadOutlined,
} from '@ant-design/icons';
import { App, Button, Input, Popconfirm, Space, Table, Tag, Typography } from 'antd';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { apiConfigService } from '../api/services';
import type { ApiConfig } from '../api/types';

export default function ApisPage() {
  const { message } = App.useApp();
  const navigate = useNavigate();
  const [rows, setRows] = useState<ApiConfig[]>([]);
  const [keyword, setKeyword] = useState('');
  const [loading, setLoading] = useState(false);

  async function load() {
    setLoading(true);
    try {
      setRows(keyword ? await apiConfigService.search({ keyword }) : await apiConfigService.list());
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function setStatus(row: ApiConfig, online: boolean) {
    if (!row.id) return;
    if (online) await apiConfigService.online(row.id);
    else await apiConfigService.offline(row.id);
    await load();
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
          <Input.Search
            allowClear
            placeholder="搜索名称 / 路径"
            value={keyword}
            onChange={(event) => setKeyword(event.target.value)}
            onSearch={load}
          />
          <Button icon={<ReloadOutlined />} onClick={load}>
            刷新
          </Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => navigate('/apis/new')}>
            创建 API
          </Button>
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
    </div>
  );
}
