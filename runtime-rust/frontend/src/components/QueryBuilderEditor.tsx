import { MinusCircleOutlined, PlusOutlined, ReloadOutlined } from '@ant-design/icons';
import { Alert, Button, Card, Checkbox, Collapse, Form, Input, InputNumber, Select, Space, Typography } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import {
  QueryBuilder,
  ValueEditor,
  type Field,
  type FullOption,
  type RuleGroupType,
  type ValueEditorProps,
} from 'react-querybuilder';
import { QueryBuilderAntD } from '@react-querybuilder/antd';
import { tableService } from '../api/services';
import type { QueryBuilderDsl, TableColumn } from '../api/types';

const emptyRules: RuleGroupType = { combinator: 'and', rules: [] };
const fallbackColumns: TableColumn[] = [
  { name: 'id', type: 'number' },
  { name: 'name', type: 'string' },
  { name: 'status', type: 'string' },
  { name: 'note', type: 'string' },
];

const operators: FullOption[] = [
  { name: '=', value: '=', label: '等于' },
  { name: '!=', value: '!=', label: '不等于' },
  { name: '>', value: '>', label: '大于' },
  { name: '>=', value: '>=', label: '大于等于' },
  { name: '<', value: '<', label: '小于' },
  { name: '<=', value: '<=', label: '小于等于' },
  { name: 'contains', value: 'contains', label: '包含' },
  { name: 'begins_with', value: 'begins_with', label: '开头是' },
  { name: 'ends_with', value: 'ends_with', label: '结尾是' },
  { name: 'in', value: 'in', label: '属于列表 in' },
  { name: 'not_in', value: 'not_in', label: '不属于列表 not in' },
  { name: 'null', value: 'null', label: '为空' },
  { name: 'not_null', value: 'not_null', label: '不为空' },
];

const valueSources: FullOption[] = [
  { name: 'value', value: 'value', label: '固定值' },
  { name: 'param', value: 'param', label: '请求参数' },
];

export interface QueryBuilderEditorProps {
  value?: QueryBuilderDsl;
  datasourceId?: string;
  onChange: (value: QueryBuilderDsl) => void;
}

export default function QueryBuilderEditor({ value, datasourceId, onChange }: QueryBuilderEditorProps) {
  const [tables, setTables] = useState<string[]>([]);
  const [columns, setColumns] = useState<TableColumn[]>(fallbackColumns);
  const [schemaError, setSchemaError] = useState<string>();

  const dsl = value ?? {
    type: 'queryBuilder',
    table: '',
    select: [],
    rules: emptyRules,
    orderBy: [],
    limit: { param: 'limit', default: 20, max: 100 },
    offset: { param: 'offset', default: 0 },
    count: true,
  };

  useEffect(() => {
    if (!datasourceId) return;
    void tableService
      .tables(datasourceId)
      .then((items) => {
        setTables(items);
        setSchemaError(undefined);
      })
      .catch((error: Error) => setSchemaError(error.message));
  }, [datasourceId]);

  useEffect(() => {
    if (!datasourceId || !dsl.table) return;
    void tableService
      .columns(datasourceId, dsl.table)
      .then((items) => {
        setColumns(items.length > 0 ? items : fallbackColumns);
        setSchemaError(undefined);
      })
      .catch((error: Error) => setSchemaError(error.message));
  }, [datasourceId, dsl.table]);

  const effectiveColumns = columns.length > 0 ? columns : fallbackColumns;
  const columnNames = effectiveColumns.map((column) => column.name);

  const effectiveSelect = useMemo(() => {
    const configured = dsl.select?.filter(Boolean) ?? [];
    return configured.length > 0 ? configured : columnNames;
  }, [columnNames.join('|'), dsl.select]);

  const fields = useMemo<Field[]>(
    () =>
      effectiveColumns.map((column) => ({
        name: column.name,
        label: column.type ? `${column.name} (${column.type})` : column.name,
        inputType: inferInputType(column.name, column.type),
      })),
    [effectiveColumns],
  );

  function patch(next: Partial<QueryBuilderDsl>) {
    onChange({ ...dsl, ...next });
  }

  function reloadSchema() {
    if (!datasourceId) return;
    void tableService.tables(datasourceId).then(setTables);
    if (dsl.table) void tableService.columns(datasourceId, dsl.table).then(setColumns);
  }

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Form.Item label="表名">
          <Select
            showSearch
            value={dsl.table || undefined}
            options={tables.map((table) => ({ value: table, label: table }))}
            onChange={(table) => patch({ table, select: [] })}
            placeholder={datasourceId ? '从真实 schema 选择表' : '请先选择数据源'}
            notFoundContent={datasourceId ? '暂无表；可刷新或检查数据源' : '请先选择数据源'}
            dropdownRender={(menu) => (
              <div>
                {menu}
                <Button type="link" icon={<ReloadOutlined />} onClick={reloadSchema}>
                  刷新 schema
                </Button>
              </div>
            )}
          />
        </Form.Item>
        <Form.Item label="选择字段">
          <Select
            mode="multiple"
            value={effectiveSelect}
            options={effectiveColumns.map((column) => ({
              value: column.name,
              label: column.type ? `${column.name} (${column.type})` : column.name,
            }))}
            onChange={(select) => patch({ select })}
            placeholder="从真实字段中选择"
          />
        </Form.Item>
      </div>

      {schemaError ? <Alert type="warning" showIcon message="Schema 加载失败" description={schemaError} /> : null}

      <Card
        className="query-builder-card"
        title="过滤条件可视化编辑器"
        extra={<Typography.Text type="secondary">字段来自真实 schema；值可绑定请求参数</Typography.Text>}
      >
        <Alert
          className="mb-4"
          type="info"
          showIcon
          message="选择“固定值”会把值写入模板；选择“请求参数”会在运行时从请求体/query/form 中取值。"
        />
        <QueryBuilderAntD>
          <QueryBuilder
            fields={fields}
            query={dsl.rules ?? emptyRules}
            operators={operators}
            getValueSources={() => valueSources as never}
            getValueEditorType={() => 'text'}
            showCombinatorsBetweenRules={false}
            controlClassnames={{ queryBuilder: 'dbapi-query-builder' }}
            controlElements={{ valueEditor: DbApiValueEditor }}
            onQueryChange={(rules) => patch({ rules })}
          />
        </QueryBuilderAntD>
      </Card>

      <Form.Item label="排序">
        <div className="space-y-2">
          {(dsl.orderBy ?? []).map((item, index) => (
            <Space key={`${item.field}-${index}`}>
              <Select
                className="w-48"
                value={item.field}
                options={effectiveSelect.map((field) => ({ value: field, label: field }))}
                onChange={(field) => {
                  const orderBy = [...(dsl.orderBy ?? [])];
                  orderBy[index] = { ...orderBy[index], field };
                  patch({ orderBy });
                }}
              />
              <Select
                className="w-32"
                value={item.direction}
                options={[
                  { value: 'asc', label: '升序' },
                  { value: 'desc', label: '降序' },
                ]}
                onChange={(direction) => {
                  const orderBy = [...(dsl.orderBy ?? [])];
                  orderBy[index] = { ...orderBy[index], direction };
                  patch({ orderBy });
                }}
              />
              <Button
                icon={<MinusCircleOutlined />}
                onClick={() => patch({ orderBy: (dsl.orderBy ?? []).filter((_, i) => i !== index) })}
              />
            </Space>
          ))}
          <Button
            icon={<PlusOutlined />}
            onClick={() => patch({ orderBy: [...(dsl.orderBy ?? []), { field: effectiveSelect[0] ?? '', direction: 'desc' }] })}
          >
            添加排序
          </Button>
        </div>
      </Form.Item>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <Form.Item label="limit 参数名">
          <Input value={dsl.limit?.param} onChange={(event) => patch({ limit: { ...dsl.limit, param: event.target.value } })} />
        </Form.Item>
        <Form.Item label="limit 默认值">
          <InputNumber className="w-full" value={dsl.limit?.default} min={0} onChange={(next) => patch({ limit: { ...dsl.limit, default: Number(next ?? 20) } })} />
        </Form.Item>
        <Form.Item label="limit 最大值">
          <InputNumber className="w-full" value={dsl.limit?.max} min={1} onChange={(next) => patch({ limit: { ...dsl.limit, max: Number(next ?? 100) } })} />
        </Form.Item>
        <Form.Item label="offset 参数名">
          <Input value={dsl.offset?.param} onChange={(event) => patch({ offset: { ...dsl.offset, param: event.target.value } })} />
        </Form.Item>
        <Form.Item label="offset 默认值">
          <InputNumber className="w-full" value={dsl.offset?.default} min={0} onChange={(next) => patch({ offset: { ...dsl.offset, default: Number(next ?? 0) } })} />
        </Form.Item>
        <Form.Item label="返回 count">
          <Checkbox checked={dsl.count} onChange={(event) => patch({ count: event.target.checked })}>
            count
          </Checkbox>
        </Form.Item>
      </div>

      <Collapse
        items={[
          {
            key: 'preview',
            label: '高级预览：生成的 DSL JSON / SQL 规则',
            children: (
              <div className="space-y-4">
                <Form.Item label="DSL JSON 预览">
                  <Input.TextArea rows={10} value={JSON.stringify(dsl, null, 2)} readOnly />
                </Form.Item>
              </div>
            ),
          },
        ]}
      />
    </div>
  );
}

function DbApiValueEditor(props: ValueEditorProps) {
  if (['null', 'not_null'].includes(props.operator)) return null;
  if (String(props.valueSource) === 'param') {
    const paramValue = normalizeParamValue(props.value);
    return (
      <Space.Compact>
        <Input
          className="w-44"
          placeholder="参数名，如 statusList"
          value={paramValue.param}
          onChange={(event) => props.handleOnChange({ ...paramValue, param: event.target.value })}
        />
        <Input
          className="w-48"
          placeholder="默认值 JSON（可空）"
          value={paramValue.defaultText}
          onChange={(event) => {
            const defaultText = event.target.value;
            props.handleOnChange({ ...paramValue, defaultText, default: parseDefault(defaultText) });
          }}
        />
      </Space.Compact>
    );
  }
  if (['in', 'not_in'].includes(props.operator)) {
    const value = Array.isArray(props.value) ? props.value.map(String) : splitList(String(props.value ?? ''));
    return <Select mode="tags" className="min-w-64" value={value} tokenSeparators={[',']} onChange={props.handleOnChange} />;
  }
  return <ValueEditor {...props} />;
}

function normalizeParamValue(value: unknown): { param: string; defaultText: string; default?: unknown } {
  if (typeof value === 'string') return { param: value, defaultText: '' };
  if (value && typeof value === 'object' && 'param' in value) {
    const objectValue = value as { param?: unknown; default?: unknown; defaultText?: unknown };
    return {
      param: typeof objectValue.param === 'string' ? objectValue.param : '',
      defaultText:
        typeof objectValue.defaultText === 'string'
          ? objectValue.defaultText
          : objectValue.default === undefined
            ? ''
            : JSON.stringify(objectValue.default),
      default: objectValue.default,
    };
  }
  return { param: '', defaultText: '' };
}

function parseDefault(raw: string): unknown {
  if (!raw.trim()) return undefined;
  try {
    return JSON.parse(raw);
  } catch {
    return raw;
  }
}

function splitList(raw: string): string[] {
  return raw
    .split(',')
    .map((value) => value.trim())
    .filter(Boolean);
}

function inferInputType(field: string, type?: string): Field['inputType'] {
  const normalized = `${field} ${type ?? ''}`.toLowerCase();
  if (normalized.includes('bool')) return 'checkbox';
  if (normalized === 'id' || normalized.includes('_id') || normalized.includes('int')) return 'number';
  if (normalized.includes('decimal') || normalized.includes('number') || normalized.includes('amount')) return 'number';
  if (normalized.includes('date') || normalized.includes('time')) return 'datetime-local';
  return 'text';
}
