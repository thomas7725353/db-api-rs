import { Alert, App, Button, Card, Form, Input, Radio, Select, Space, Tabs, Typography } from 'antd';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { apiConfigService, datasourceService, groupService, tableService } from '../api/services';
import type { ApiConfig, ApiEngine, ApiGroup, DataSource, ParamSpec, QueryBuilderDsl, TableColumn } from '../api/types';
import ParamEditor, { parseParamSpecs, stringifyParamSpecs } from '../components/ParamEditor';
import QueryBuilderEditor from '../components/QueryBuilderEditor';
import { inferQueryBuilderPageParams, sanitizeQueryBuilderDsl } from '../components/queryBuilderPreview';
import {
  columnParamOptions,
  hasFixedIdParamContract,
  inferSqlTableName,
  syncParamTypesFromColumns,
} from '../components/sqlParamSchema';

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

type QueryBuilderResponseMode = 'list' | 'page' | 'object' | 'count';

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
  const [responseMode, setResponseMode] = useState<QueryBuilderResponseMode>('page');
  const [params, setParams] = useState<ParamSpec[]>([
    { name: 'limit', type: 'bigint' },
    { name: 'offset', type: 'bigint' },
  ]);
  const [jsonParam, setJsonParam] = useState('');
  const [sqlTables, setSqlTables] = useState<string[]>([]);
  const [sqlTable, setSqlTable] = useState('');
  const [sqlColumns, setSqlColumns] = useState<TableColumn[]>([]);
  const [sqlSchemaError, setSqlSchemaError] = useState<string>();

  const selectedDatasourceId = Form.useWatch('datasourceId', form);
  const contentType = Form.useWatch('contentType', form) || 'application/json';
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
      setParams(parseParamSpecs(detail.params));
      setJsonParam(detail.jsonParam || '');
      const firstSql = detail.sqlList?.[0];
      if (firstSql?.transformPlugin === 'queryBuilder') {
        setEngine('queryBuilder');
        try {
          setDsl(JSON.parse(firstSql.sqlText || '{}') as QueryBuilderDsl);
          setResponseMode(parseResponseMode(firstSql.transformPluginParams));
        } catch {
          setDsl(defaultDsl);
        }
      } else {
        setEngine('sql');
        setSqlText(firstSql?.sqlText || '');
        setSqlTable(inferSqlTableName(firstSql?.sqlText || ''));
      }
    });
  }, [form, id]);

  useEffect(() => {
    let ignore = false;
    setSqlTables([]);
    setSqlColumns([]);
    setSqlSchemaError(undefined);
    if (!selectedDatasourceId || engine !== 'sql') return () => { ignore = true; };
    void tableService
      .tables(selectedDatasourceId)
      .then((items) => {
        if (ignore) return;
        setSqlTables(items);
        setSqlSchemaError(undefined);
      })
      .catch((error: Error) => {
        if (ignore) return;
        setSqlSchemaError(error.message);
      });
    return () => { ignore = true; };
  }, [engine, selectedDatasourceId]);

  useEffect(() => {
    if (engine !== 'sql' || sqlTable) return;
    const inferredTable = inferSqlTableName(sqlText);
    if (inferredTable) setSqlTable(inferredTable);
  }, [engine, sqlTable, sqlText]);

  useEffect(() => {
    let ignore = false;
    setSqlColumns([]);
    setSqlSchemaError(undefined);
    if (!selectedDatasourceId || engine !== 'sql' || !sqlTable) return () => { ignore = true; };
    void tableService
      .columns(selectedDatasourceId, sqlTable)
      .then((items) => {
        if (ignore) return;
        setSqlColumns(items);
        setParams((current) => syncParamTypesFromColumns(current, items));
        setSqlSchemaError(undefined);
      })
      .catch((error: Error) => {
        if (ignore) return;
        setSqlSchemaError(error.message);
      });
    return () => { ignore = true; };
  }, [engine, selectedDatasourceId, sqlTable]);

  const datasourceOptions = useMemo(
    () => datasources.map((item) => ({ value: item.id, label: `${item.name} (${item.type})` })),
    [datasources],
  );

  const groupOptions = useMemo(
    () => groups.map((item) => ({ value: item.id, label: item.name || item.id })),
    [groups],
  );

  const sqlFieldOptions = useMemo(() => columnParamOptions(sqlColumns), [sqlColumns]);
  const fixedSqlParams = useMemo(() => hasFixedIdParamContract(sqlText, params), [params, sqlText]);

  const updateSqlText = useCallback((nextSqlText: string) => {
    setSqlText(nextSqlText);
    const inferredTable = inferSqlTableName(nextSqlText);
    if (inferredTable && inferredTable !== sqlTable) setSqlTable(inferredTable);
  }, [sqlTable]);

  const editorTabs = useMemo(() => {
    const queryBuilderTab = {
      key: 'queryBuilder',
      label: 'QueryBuilder',
      children: <QueryBuilderEditor value={dsl} datasourceId={selectedDatasourceId} onChange={setDsl} />,
    };
    const sqlTab = {
      key: 'sql',
      label: 'SQL',
      children: (
        <div className="space-y-3">
          <Select
            showSearch
            allowClear
            className="max-w-sm"
            value={sqlTable || undefined}
            options={sqlTables.map((table) => ({ value: table, label: table }))}
            placeholder={selectedDatasourceId ? '从真实 schema 选择表' : '请先选择数据源'}
            notFoundContent={selectedDatasourceId ? '暂无表；可检查数据源' : '请先选择数据源'}
            onChange={(table) => setSqlTable(table || '')}
          />
          {sqlSchemaError ? <Alert type="warning" showIcon message={sqlSchemaError} /> : null}
          <Input.TextArea
            rows={16}
            value={sqlText}
            onChange={(event) => updateSqlText(event.target.value)}
          />
        </div>
      ),
    };
    if (!isEdit) return [queryBuilderTab, sqlTab];
    return engine === 'queryBuilder' ? [queryBuilderTab] : [sqlTab];
  }, [dsl, engine, isEdit, selectedDatasourceId, sqlSchemaError, sqlTable, sqlTables, sqlText, updateSqlText]);

  async function save() {
    const values = await form.validateFields();
    const queryBuilderDsl = sanitizeQueryBuilderDsl(dsl);
    const sqlList =
      engine === 'queryBuilder'
        ? [
            {
              sqlText: JSON.stringify(queryBuilderDsl, null, 2),
              transformPlugin: 'queryBuilder',
              transformPluginParams: resultTypeParams(responseMode),
            },
          ]
        : [{ sqlText, transformPlugin: 'sql', transformPluginParams: '' }];
    const payload: ApiConfig = {
      ...values,
      id,
      params:
        engine === 'queryBuilder'
          ? stringifyParamSpecs(inferQueryBuilderPageParams(queryBuilderDsl))
          : contentType === 'application/json'
            ? '[]'
            : stringifyParamSpecs(params),
      sqlList,
      contentType: values.contentType || 'application/json',
      jsonParam: contentType === 'application/json' ? jsonParam : undefined,
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
        {engine === 'queryBuilder' ? (
          <Form.Item label="返回模式">
            <Select
              className="max-w-sm"
              value={responseMode}
              options={[
                { value: 'list', label: 'list：返回数组' },
                { value: 'page', label: 'page：返回 list + total' },
                { value: 'object', label: 'object：返回单个对象' },
                { value: 'count', label: 'count：只返回总数' },
              ]}
              onChange={(mode: QueryBuilderResponseMode) => {
                setResponseMode(mode);
                setDsl((current) => ({ ...current, count: mode === 'page' || mode === 'count' }));
              }}
            />
          </Form.Item>
        ) : null}
        <Tabs
          activeKey={engine}
          onChange={(key) => {
            if (!isEdit) setEngine(key as ApiEngine);
          }}
          items={editorTabs}
        />
      </Card>

      <Card title="请求参数定义">
        {engine === 'queryBuilder' ? (
          <ParamEditor value={inferQueryBuilderPageParams(sanitizeQueryBuilderDsl(dsl))} readonly emptyText="当前 QueryBuilder 没有分页参数" />
        ) : contentType === 'application/json' ? (
          <Input.TextArea
            rows={8}
            value={jsonParam}
            placeholder='例如 {"id": 1, "status": "active"}'
            onChange={(event) => setJsonParam(event.target.value)}
          />
        ) : (
          <ParamEditor
            value={params}
            onChange={setParams}
            fieldOptions={sqlFieldOptions.length > 0 ? sqlFieldOptions : undefined}
            lockTypes={sqlFieldOptions.length > 0}
            lockNames={fixedSqlParams}
            disableAdd={fixedSqlParams}
            disableRemove={fixedSqlParams}
          />
        )}
      </Card>
    </div>
  );
}



function resultTypeParams(mode: QueryBuilderResponseMode): string {
  return `resultType=${mode}`;
}

function parseResponseMode(raw?: string): QueryBuilderResponseMode {
  const value = raw?.match(/result_?type=([^&]+)/i)?.[1]?.toLowerCase();
  if (value === 'object' || value === 'count' || value === 'list' || value === 'page') return value;
  return 'page';
}
