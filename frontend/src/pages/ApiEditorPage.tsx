import { Alert, App, Button, Card, Form, Input, Radio, Select, Space, Tabs, Typography } from 'antd';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { apiConfigService, datasourceService, groupService, tableService } from '../api/services';
import type { ApiConfig, ApiEngine, ApiGroup, DataSource, ParamSpec, QueryBuilderDsl, TableColumn } from '../api/types';
import ParamEditor, { parseParamSpecs, stringifyParamSpecs } from '../components/ParamEditor';
import QueryBuilderEditor from '../components/QueryBuilderEditor';
import {
  buildViewSqlList,
  responseModeRequiresCountSql,
  resultTypeParams,
  type ApiResponseMode,
} from '../components/apiEditorPayload';
import { inferQueryBuilderPageParams, sanitizeQueryBuilderDsl } from '../components/queryBuilderPreview';
import {
  columnParamOptions,
  hasFixedIdParamContract,
  inferSqlTableName,
  syncParamTypesFromColumns,
} from '../components/sqlParamSchema';
import { renderViewSqlPreview } from '../components/viewSqlPreview';

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

const defaultViewSqlText =
  'select [[ columns | ident_list ]] from demo_items a where a.status = $status order by [[ order_by | ident ]] desc limit [[ limit | int(default=10,max=1000) ]] offset [[ offset | int(default=0) ]]';

const defaultViewCountSqlText = 'select count(*) as total from demo_items a where a.status = $status';

const defaultViewPreviewParams = JSON.stringify(
  {
    columns: ['a.id', 'a.name', 'a.status'],
    order_by: 'a.id',
    limit: 10,
    offset: 0,
    status: 'active',
  },
  null,
  2,
);

type QueryBuilderResponseMode = ApiResponseMode;

export default function ApiEditorPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { message } = App.useApp();
  const [form] = Form.useForm<ApiConfig>();
  const [datasources, setDatasources] = useState<DataSource[]>([]);
  const [groups, setGroups] = useState<ApiGroup[]>([]);
  const [engine, setEngine] = useState<ApiEngine>('queryBuilder');
  const [sqlText, setSqlText] = useState('select * from demo_items limit $limit offset $offset');
  const [viewSqlText, setViewSqlText] = useState(defaultViewSqlText);
  const [viewCountSqlText, setViewCountSqlText] = useState(defaultViewCountSqlText);
  const [viewPreviewParams, setViewPreviewParams] = useState(defaultViewPreviewParams);
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
      } else if (firstSql?.transformPlugin === 'viewSql') {
        setEngine('viewSql');
        setViewSqlText(firstSql.sqlText || '');
        setViewCountSqlText(detail.sqlList?.find((item) => item.transformPlugin === 'viewSqlCount')?.sqlText || '');
        setResponseMode(parseResponseMode(firstSql.transformPluginParams));
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

  const viewPreview = useMemo(() => renderViewPreviewText(viewSqlText, viewPreviewParams), [viewPreviewParams, viewSqlText]);

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
    const viewSqlTab = {
      key: 'viewSql',
      label: 'View SQL',
      children: (
        <div className="space-y-3">
          <Alert
            type="info"
            showIcon
            message="结构片段使用 [[ columns | ident_list ]]、[[ order_by | ident ]]、[[ limit | int(default=10,max=1000) ]]；普通值继续使用 $param 绑定。"
          />
          <Form.Item label="列表 SQL 模板">
            <Input.TextArea rows={14} value={viewSqlText} onChange={(event) => setViewSqlText(event.target.value)} />
          </Form.Item>
          {responseModeRequiresCountSql(responseMode) ? (
            <Form.Item label="Count SQL 模板">
              <Input.TextArea
                rows={5}
                value={viewCountSqlText}
                placeholder="page/count 模式需要 count SQL 模板，例如 select count(*) as total from demo_items where status = $status"
                onChange={(event) => setViewCountSqlText(event.target.value)}
              />
            </Form.Item>
          ) : null}
          <Form.Item label="预览参数">
            <Input.TextArea
              rows={6}
              value={viewPreviewParams}
              placeholder='预览参数，例如 {"columns":["a.id"],"order_by":"a.id","limit":10,"offset":0}'
              onChange={(event) => setViewPreviewParams(event.target.value)}
            />
          </Form.Item>
          <Form.Item label="列表 SQL 预览">
            <Input.TextArea rows={8} readOnly value={viewPreview} />
          </Form.Item>
        </div>
      ),
    };
    if (!isEdit) return [queryBuilderTab, sqlTab, viewSqlTab];
    if (engine === 'queryBuilder') return [queryBuilderTab];
    if (engine === 'viewSql') return [viewSqlTab];
    return [sqlTab];
  }, [
    dsl,
    engine,
    isEdit,
    responseMode,
    selectedDatasourceId,
    sqlSchemaError,
    sqlTable,
    sqlTables,
    sqlText,
    updateSqlText,
    viewCountSqlText,
    viewPreview,
    viewPreviewParams,
    viewSqlText,
  ]);

  async function save() {
    const values = await form.validateFields();
    const queryBuilderDsl = sanitizeQueryBuilderDsl(dsl);
    if (engine === 'viewSql' && responseModeRequiresCountSql(responseMode) && !viewCountSqlText.trim()) {
      message.error('page/count 模式需要 count SQL 模板');
      return;
    }
    const sqlList =
      engine === 'queryBuilder'
        ? [
            {
              sqlText: JSON.stringify(queryBuilderDsl, null, 2),
              transformPlugin: 'queryBuilder',
              transformPluginParams: resultTypeParams(responseMode),
            },
          ]
        : engine === 'viewSql'
          ? buildViewSqlList(viewSqlText, viewCountSqlText, responseMode)
        : [{ sqlText, transformPlugin: 'sql', transformPluginParams: '' }];
    const payload: ApiConfig = {
      ...values,
      id,
      params:
        engine === 'queryBuilder'
          ? stringifyParamSpecs(inferQueryBuilderPageParams(queryBuilderDsl))
          : engine === 'viewSql'
            ? stringifyParamSpecs(params)
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
          <Typography.Text type="secondary">QueryBuilder 模式适合普通查询 API，View SQL 适合复杂查询，SQL 模式保留兼容。</Typography.Text>
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
        {engine === 'queryBuilder' || engine === 'viewSql' ? (
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
                if (engine === 'queryBuilder') {
                  setDsl((current) => ({ ...current, count: mode === 'page' || mode === 'count' }));
                }
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
        ) : engine === 'viewSql' ? (
          <ParamEditor value={params} onChange={setParams} />
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
function parseResponseMode(raw?: string): QueryBuilderResponseMode {
  const value = raw?.match(/result_?type=([^&]+)/i)?.[1]?.toLowerCase();
  if (value === 'object' || value === 'count' || value === 'list' || value === 'page') return value;
  return 'page';
}

function renderViewPreviewText(template: string, rawParams: string): string {
  try {
    const params = JSON.parse(rawParams) as Record<string, unknown>;
    return renderViewSqlPreview(template, params).sql;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}
