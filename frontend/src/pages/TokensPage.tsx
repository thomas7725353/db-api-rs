import { DeleteOutlined, ReloadOutlined } from '@ant-design/icons';
import { App, Button, Card, Form, Input, Modal, Popconfirm, Select, Space, Table, Typography } from 'antd';
import { useEffect, useState } from 'react';
import { appService, groupService } from '../api/services';
import type { ApiGroup, AppInfo } from '../api/types';

export default function TokensPage() {
  const { message } = App.useApp();
  const [apps, setApps] = useState<AppInfo[]>([]);
  const [groups, setGroups] = useState<ApiGroup[]>([]);
  const [open, setOpen] = useState(false);
  const [form] = Form.useForm();

  async function load() {
    const [nextApps, nextGroups] = await Promise.all([appService.list(), groupService.list()]);
    setApps(nextApps);
    setGroups(nextGroups);
  }

  useEffect(() => {
    void load();
  }, []);

  async function create() {
    const values = await form.validateFields();
    await appService.create(values);
    message.success('创建成功');
    setOpen(false);
    form.resetFields();
    await load();
  }

  async function authorize(appId: string, groupIds: string[]) {
    await appService.authorize(appId, groupIds);
    message.success('授权成功');
  }

  async function generateToken(row: AppInfo) {
    if (!row.id || !row.secret) return;
    const token = await appService.token(row.id, row.secret);
    message.success(token.token);
    await load();
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <Typography.Title level={3} className="!mb-1">
            权限 / Token
          </Typography.Title>
          <Typography.Text type="secondary">创建应用，授权 API 分组，然后生成访问 token。</Typography.Text>
        </div>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={load}>
            刷新
          </Button>
          <Button type="primary" onClick={() => setOpen(true)}>
            创建应用
          </Button>
        </Space>
      </div>
      <Card>
        <Table<AppInfo>
          rowKey={(row) => row.id ?? Math.random().toString()}
          dataSource={apps}
          expandable={{
            expandedRowRender: (row) => (
              <Space direction="vertical" className="w-full">
                <Typography.Text copyable>appid: {row.id}</Typography.Text>
                <Typography.Text copyable>secret: {row.secret}</Typography.Text>
                <Typography.Text copyable>token: {row.token || '-'}</Typography.Text>
                <Select
                  mode="multiple"
                  className="w-full"
                  placeholder="授权分组"
                  options={groups.map((group) => ({ value: group.id, label: group.name || group.id }))}
                  onChange={(ids) => authorize(row.id!, ids)}
                />
              </Space>
            ),
          }}
          columns={[
            { title: '名称', dataIndex: 'name' },
            { title: '过期策略', dataIndex: 'expireDesc', width: 120 },
            { title: '备注', dataIndex: 'note' },
            {
              title: '操作',
              width: 220,
              render: (_, row) => (
                <Space>
                  <Button size="small" onClick={() => generateToken(row)}>
                    生成 token
                  </Button>
                  <Popconfirm
                    title="确认删除应用？"
                    onConfirm={async () => {
                      await appService.remove(row.id!);
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
      </Card>

      <Modal title="创建应用" open={open} onCancel={() => setOpen(false)} onOk={create}>
        <Form form={form} layout="vertical" initialValues={{ expireDesc: 'forever' }}>
          <Form.Item name="name" label="名称" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="expireDesc" label="过期策略">
            <Select
              options={[
                { value: 'forever', label: '永久' },
                { value: '5min', label: '5 分钟' },
                { value: '1hour', label: '1 小时' },
                { value: '1day', label: '1 天' },
                { value: '30day', label: '30 天' },
                { value: 'once', label: '一次性' },
              ]}
            />
          </Form.Item>
          <Form.Item name="note" label="备注">
            <Input.TextArea rows={3} />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}
