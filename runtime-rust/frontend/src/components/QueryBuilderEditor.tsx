import { MinusCircleOutlined, PlusOutlined } from '@ant-design/icons';
import { Alert, Button, Card, Checkbox, Collapse, Form, Input, InputNumber, Select, Space, Typography } from 'antd';
import { useMemo } from 'react';
import {
  defaultOperators,
  formatQuery,
  QueryBuilder,
  type Field,
  type RuleGroupType,
} from 'react-querybuilder';
import { QueryBuilderAntD } from '@react-querybuilder/antd';
import type { QueryBuilderDsl } from '../api/types';

const emptyRules: RuleGroupType = { combinator: 'and', rules: [] };
const fallbackFields = ['id', 'name', 'status', 'note'];

export interface QueryBuilderEditorProps {
  value?: QueryBuilderDsl;
  onChange: (value: QueryBuilderDsl) => void;
}

export default function QueryBuilderEditor({ value, onChange }: QueryBuilderEditorProps) {
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

  const effectiveSelect = useMemo(() => {
    const configured = dsl.select?.filter(Boolean) ?? [];
    return configured.length > 0 ? configured : fallbackFields;
  }, [dsl.select]);

  const fields = useMemo<Field[]>(
    () =>
      effectiveSelect.map((field) => ({
        name: field,
        label: field,
        inputType: inferInputType(field),
      })),
    [effectiveSelect],
  );

  function patch(next: Partial<QueryBuilderDsl>) {
    onChange({ ...dsl, ...next });
  }

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Form.Item label="表名">
          <Input value={dsl.table} onChange={(event) => patch({ table: event.target.value })} />
        </Form.Item>
        <Form.Item label="选择字段">
          <Select
            mode="tags"
            value={effectiveSelect}
            tokenSeparators={[',', ' ']}
            onChange={(select) => patch({ select })}
            placeholder="id, name, status"
          />
        </Form.Item>
      </div>

      <Card
        className="query-builder-card"
        title="过滤条件可视化编辑器"
        extra={<Typography.Text type="secondary">用 +Rule / +Group 组合查询条件</Typography.Text>}
      >
        <Alert
          className="mb-4"
          type="info"
          showIcon
          message="这里是实际的 react-querybuilder 规则组件，不需要手写 JSON。字段来自上方“选择字段”，可直接新增字段。"
        />
        <QueryBuilderAntD>
          <QueryBuilder
            fields={fields}
            query={dsl.rules ?? emptyRules}
            operators={defaultOperators}
            showCombinatorsBetweenRules={false}
            controlClassnames={{ queryBuilder: 'dbapi-query-builder' }}
            onQueryChange={(rules) => patch({ rules })}
          />
        </QueryBuilderAntD>
      </Card>

      <Form.Item label="排序">
        <Form.List name="orderBy" initialValue={dsl.orderBy ?? []}>
          {(_, { add, remove }) => (
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
                      { value: 'asc', label: 'asc' },
                      { value: 'desc', label: 'desc' },
                    ]}
                    onChange={(direction) => {
                      const orderBy = [...(dsl.orderBy ?? [])];
                      orderBy[index] = { ...orderBy[index], direction };
                      patch({ orderBy });
                    }}
                  />
                  <Button
                    icon={<MinusCircleOutlined />}
                    onClick={() => {
                      remove(index);
                      patch({ orderBy: (dsl.orderBy ?? []).filter((_, i) => i !== index) });
                    }}
                  />
                </Space>
              ))}
              <Button
                icon={<PlusOutlined />}
                onClick={() => {
                  add();
                  patch({
                    orderBy: [
                      ...(dsl.orderBy ?? []),
                      { field: effectiveSelect[0] ?? '', direction: 'desc' },
                    ],
                  });
                }}
              >
                添加排序
              </Button>
            </div>
          )}
        </Form.List>
      </Form.Item>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <Form.Item label="limit 参数名">
          <Input
            value={dsl.limit?.param}
            onChange={(event) =>
              patch({ limit: { ...dsl.limit, param: event.target.value } })
            }
          />
        </Form.Item>
        <Form.Item label="limit 默认值">
          <InputNumber
            className="w-full"
            value={dsl.limit?.default}
            min={0}
            onChange={(next) => patch({ limit: { ...dsl.limit, default: Number(next ?? 20) } })}
          />
        </Form.Item>
        <Form.Item label="limit 最大值">
          <InputNumber
            className="w-full"
            value={dsl.limit?.max}
            min={1}
            onChange={(next) => patch({ limit: { ...dsl.limit, max: Number(next ?? 100) } })}
          />
        </Form.Item>
        <Form.Item label="offset 参数名">
          <Input
            value={dsl.offset?.param}
            onChange={(event) =>
              patch({ offset: { ...dsl.offset, param: event.target.value } })
            }
          />
        </Form.Item>
        <Form.Item label="offset 默认值">
          <InputNumber
            className="w-full"
            value={dsl.offset?.default}
            min={0}
            onChange={(next) => patch({ offset: { ...dsl.offset, default: Number(next ?? 0) } })}
          />
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
                <Form.Item label="规则预览">
                  <Input.TextArea rows={3} value={formatQuery(dsl.rules, 'sql')} readOnly />
                </Form.Item>
              </div>
            ),
          },
        ]}
      />
    </div>
  );
}

function inferInputType(field: string): Field['inputType'] {
  const normalized = field.toLowerCase();
  if (normalized === 'id' || normalized.endsWith('_id') || normalized.includes('count')) return 'number';
  if (normalized.includes('age') || normalized.includes('price') || normalized.includes('amount')) return 'number';
  if (normalized.includes('date') || normalized.includes('time')) return 'datetime-local';
  return 'text';
}
