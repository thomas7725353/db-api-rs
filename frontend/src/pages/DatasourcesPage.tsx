import { DeleteOutlined, EditOutlined, PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import { App, Button, Form, Input, Modal, Popconfirm, Select, Space, Table, Typography } from 'antd';
import { useEffect, useState } from 'react';
import { datasourceService } from '../api/services';
import type { DataSource } from '../api/types';

export default function DatasourcesPage() {
  const { message } = App.useApp();
  const [rows, setRows] = useState<DataSource[]>([]);
  const [loading, setLoading] = useState(false);
  const [editing, setEditing] = useState<DataSource | null>(null);
  const [open, setOpen] = useState(false);
  const [form] = Form.useForm<DataSource>();

  async function load() {
    setLoading(true);
    try {
      setRows(await datasourceService.list());
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  function startEdit(row?: DataSource) {
    setEditing(row ?? null);
    form.setFieldsValue(
      row ?? {
        type: 'sqlite',
        url: 'sqlite://../data.db',
      },
    );
    setOpen(true);
  }

  async function save() {
    const values = await form.validateFields();
    const payload = { ...editing, ...values, edit_password: true };
    if (editing?.id) {
      await datasourceService.update(payload);
    } else {
      await datasourceService.create(payload);
    }
    message.success('保存成功');
    setOpen(false);
    await load();
  }

  async function testConnection() {
    const values = await form.validateFields();
    await datasourceService.connect(values);
    message.success('连接成功');
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <Typography.Title level={3} className="!mb-1">
            数据源
          </Typography.Title>
          <Typography.Text type="secondary">Rust 单机版当前支持 SQLite / MySQL / PostgreSQL。</Typography.Text>
        </div>
        <Space>
          <Button icon={<ReloadOutlined />} onClick={load}>
            刷新
          </Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => startEdit()}>
            新建数据源
          </Button>
        </Space>
      </div>
      <Table<DataSource>
        rowKey={(row) => row.id ?? row.name ?? Math.random().toString()}
        loading={loading}
        dataSource={rows}
        columns={[
          { title: '名称', dataIndex: 'name' },
          { title: '类型', dataIndex: 'type', width: 120 },
          { title: 'URL', dataIndex: 'url', ellipsis: true },
          { title: '更新时间', dataIndex: 'updateTime', width: 180 },
          {
            title: '操作',
            width: 160,
            render: (_, row) => (
              <Space>
                <Button size="small" icon={<EditOutlined />} onClick={() => startEdit(row)}>
                  编辑
                </Button>
                <Popconfirm
                  title="确认删除数据源？"
                  onConfirm={async () => {
                    await datasourceService.remove(row.id!);
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
      <Modal
        title={editing?.id ? '编辑数据源' : '新建数据源'}
        open={open}
        onCancel={() => setOpen(false)}
        onOk={save}
        width={720}
        okText="保存"
      >
        <Form form={form} layout="vertical">
          <Form.Item name="name" label="名称" rules={[{ required: true }]}>
            <Input />
          </Form.Item>
          <Form.Item name="type" label="类型" rules={[{ required: true }]}>
            <Select
              options={[
                { value: 'sqlite', label: 'SQLite' },
                { value: 'mysql', label: 'MySQL' },
                { value: 'postgres', label: 'PostgreSQL' },
              ]}
            />
          </Form.Item>
          <Form.Item name="url" label="URL" rules={[{ required: true }]}>
            <Input placeholder="sqlite://../data.db" />
          </Form.Item>
          <Form.Item name="username" label="用户名">
            <Input />
          </Form.Item>
          <Form.Item name="password" label="密码">
            <Input.Password />
          </Form.Item>
          <Form.Item name="note" label="备注">
            <Input.TextArea rows={3} />
          </Form.Item>
          <Button onClick={testConnection}>测试连接</Button>
        </Form>
      </Modal>
    </div>
  );
}
