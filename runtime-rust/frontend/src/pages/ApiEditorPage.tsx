import { App, Button, Card, Form, Input, Radio, Select, Space, Tabs, Typography } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { apiConfigService, datasourceService, groupService } from '../api/services';
import type { ApiConfig, ApiEngine, ApiGroup, DataSource, ParamSpec, QueryBuilderDsl } from '../api/types';
import QueryBuilderEditor from '../components/QueryBuilderEditor';

const defaultDsl: QueryBuilderDsl = {
  type: 'queryBuilder',
  table: 'demo_items',
  select: ['id', 'name', 'status', 'note'],
  rules: {
    combinator: 'and',
    rules: [{ field: 'status', operator: '=', value: 'active' }],
  },
  orderBy: [{ field: 'id', direction: 'desc' }],
  limit: { param: 'limit', default: 20, max: 100 },
  offset: { param: 'offset', default: 0 },
  count: true,
};

export default function ApiEditorPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { message } = App.useApp();
  const [form] = Form.useForm<ApiConfig>();
  const [datasources, setDatasources] = useState<DataSource[]>([]);
  const [groups, setGroups] = useState<ApiGroup[]>([]);
  const [engine, setEngine] = useState<ApiEngine>('queryBuilder');
  const [sqlText, setSqlText] = useState('select * from demo_items limit $limit offset $offset');
  const [dsl, setDsl] = useState<QueryBuilderDsl>(defaultDsl);
  const [paramsJson, setParamsJson] = useState('[{"name":"limit","type":"number"},{"name":"offset","type":"number"}]');

  const isEdit = Boolean(id);

  useEffect(() => {
    void Promise.all([datasourceService.list(), groupService.list()]).then(([ds, gs]) => {
      setDatasources(ds);
      setGroups(gs);
    });
  }, []);

  useEffect(() => {
    if (!id) return;
    void apiConfigService.detail(id).then((detail) => {
      if (!detail) return;
      form.setFieldsValue(detail);
      setParamsJson(detail.params || '[]');
      const firstSql = detail.sqlList?.[0];
      if (firstSql?.transformPlugin === 'queryBuilder') {
        setEngine('queryBuilder');
        try {
          setDsl(JSON.parse(firstSql.sqlText || '{}') as QueryBuilderDsl);
        } catch {
          setDsl(defaultDsl);
        }
      } else {
        setEngine('sql');
        setSqlText(firstSql?.sqlText || '');
      }
    });
  }, [form, id]);

  const datasourceOptions = useMemo(
    () => datasources.map((item) => ({ value: item.id, label: `${item.name} (${item.type})` })),
    [datasources],
  );

  const groupOptions = useMemo(
    () => groups.map((item) => ({ value: item.id, label: item.name || item.id })),
    [groups],
  );

  async function save() {
    const values = await form.validateFields();
    const sqlList =
      engine === 'queryBuilder'
        ? [
            {
              sqlText: JSON.stringify(dsl, null, 2),
              transformPlugin: 'queryBuilder',
              transformPluginParams: '',
            },
          ]
        : [{ sqlText, transformPlugin: 'sql', transformPluginParams: '' }];
    const payload: ApiConfig = {
      ...values,
      id,
      params: normalizeParams(paramsJson),
      sqlList,
      contentType: values.contentType || 'application/json',
      previlege: values.previlege ?? 1,
      openTrans: values.openTrans ?? 0,
    };
    if (isEdit) await apiConfigService.update(payload);
    else await apiConfigService.create(payload);
    message.success('保存成功');
    navigate('/apis');
  }

  return (
    <div className="mx-auto max-w-6xl space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <Typography.Title level={3} className="!mb-1">
            {isEdit ? '编辑 API' : '创建 API'}
          </Typography.Title>
          <Typography.Text type="secondary">QueryBuilder 模式适合普通查询 API，SQL 模式保留兼容。</Typography.Text>
        </div>
        <Space>
          <Button onClick={() => navigate('/apis')}>返回</Button>
          <Button type="primary" onClick={save}>
            保存
          </Button>
        </Space>
      </div>

      <Card>
        <Form form={form} layout="vertical">
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
            <Form.Item name="name" label="名称" rules={[{ required: true }]}>
              <Input />
            </Form.Item>
            <Form.Item name="path" label="路径" rules={[{ required: true }]}>
              <Input placeholder="/api/demo/items/list 或 demo/items/list" />
            </Form.Item>
            <Form.Item name="datasourceId" label="数据源" rules={[{ required: true }]}>
              <Select options={datasourceOptions} />
            </Form.Item>
            <Form.Item name="groupId" label="分组">
              <Select allowClear options={groupOptions} />
            </Form.Item>
            <Form.Item name="contentType" label="Content-Type" initialValue="application/json">
              <Select
                options={[
                  { value: 'application/json', label: 'application/json' },
                  {
                    value: 'application/x-www-form-urlencoded',
                    label: 'application/x-www-form-urlencoded',
                  },
                ]}
              />
            </Form.Item>
            <Form.Item name="previlege" label="权限" initialValue={1}>
              <Radio.Group
                options={[
                  { value: 1, label: '公开' },
                  { value: 0, label: '需要 Token' },
                ]}
              />
            </Form.Item>
          </div>
          <Form.Item name="note" label="备注">
            <Input.TextArea rows={2} />
          </Form.Item>
        </Form>
      </Card>

      <Card>
        <Tabs
          activeKey={engine}
          onChange={(key) => setEngine(key as ApiEngine)}
          items={[
            {
              key: 'queryBuilder',
              label: 'QueryBuilder',
              children: <QueryBuilderEditor value={dsl} onChange={setDsl} />,
            },
            {
              key: 'sql',
              label: 'SQL',
              children: (
                <Input.TextArea
                  rows={16}
                  value={sqlText}
                  onChange={(event) => setSqlText(event.target.value)}
                />
              ),
            },
          ]}
        />
      </Card>

      <Card title="请求参数定义">
        <Input.TextArea
          rows={8}
          value={paramsJson}
          onChange={(event) => setParamsJson(event.target.value)}
        />
      </Card>
    </div>
  );
}

function normalizeParams(raw: string): string {
  if (!raw.trim()) return '[]';
  const parsed = JSON.parse(raw) as ParamSpec[];
  if (!Array.isArray(parsed)) throw new Error('params 必须是数组 JSON');
  return JSON.stringify(parsed);
}
