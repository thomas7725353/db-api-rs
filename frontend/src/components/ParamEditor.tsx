import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Empty, Input, Select, Table, Tag } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import type { ParamSpec } from '../api/types';
import {
  findColumnParamOption,
  firstUnusedColumnOption,
  mergeParamOptions,
  type ColumnParamOption,
} from './sqlParamSchema';

export const PARAM_TYPE_OPTIONS = [
  { value: 'string', label: 'string' },
  { value: 'bigint', label: 'bigint' },
  { value: 'double', label: 'double' },
  { value: 'date', label: 'date' },
  { value: 'Array<string>', label: 'Array<string>' },
  { value: 'Array<bigint>', label: 'Array<bigint>' },
  { value: 'Array<double>', label: 'Array<double>' },
  { value: 'Array<date>', label: 'Array<date>' },
];

interface ParamEditorProps {
  value: ParamSpec[];
  onChange?: (value: ParamSpec[]) => void;
  readonly?: boolean;
  lockNames?: boolean;
  lockTypes?: boolean;
  disableAdd?: boolean;
  disableRemove?: boolean;
  fieldOptions?: ColumnParamOption[];
  emptyText?: string;
}

export default function ParamEditor({
  value,
  onChange,
  readonly = false,
  lockNames = false,
  lockTypes = false,
  disableAdd = false,
  disableRemove = false,
  fieldOptions,
  emptyText = '暂无参数',
}: ParamEditorProps) {
  const rows = normalizeParams(value);
  const schemaOptions = fieldOptions ? mergeParamOptions(rows, fieldOptions) : undefined;

  function update(index: number, patch: Partial<ParamSpec>) {
    onChange?.(rows.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row)));
  }

  function addRow() {
    const nextOption = schemaOptions ? firstUnusedColumnOption(schemaOptions, rows) : undefined;
    onChange?.([
      ...rows,
      {
        name: nextOption?.value ?? '',
        type: nextOption?.type ?? 'string',
        note: '',
      },
    ]);
  }

  function removeRow(index: number) {
    onChange?.(rows.filter((_, rowIndex) => rowIndex !== index));
  }

  const columns: ColumnsType<ParamSpec & { key: string }> = [
    {
      title: '参数名',
      dataIndex: 'name',
      width: 260,
      render: (name: string, _row, index) => {
        if (readonly || lockNames) return <span className="param-name">{name || '-'}</span>;
        if (!schemaOptions) {
          return (
            <Input
              value={name}
              placeholder="如 id / status"
              onChange={(event) => update(index, { name: event.target.value })}
            />
          );
        }
        return (
          <Select
            showSearch
            className="w-full"
            value={name || undefined}
            placeholder="选择表字段"
            optionFilterProp="label"
            options={schemaOptions.map((option) => ({
              value: option.value,
              label: option.label,
              disabled: option.missing,
            }))}
            onChange={(nextName) => {
              const option = findColumnParamOption(schemaOptions, nextName);
              update(index, { name: nextName, type: option?.type ?? 'string' });
            }}
          />
        );
      },
    },
    {
      title: '类型',
      dataIndex: 'type',
      width: 220,
      render: (type: string, row, index) => {
        const option = schemaOptions ? findColumnParamOption(schemaOptions, row.name) : undefined;
        if (readonly || lockTypes || schemaOptions) {
          return (
            <span className="param-type-tag">
              {normalizeParamType(option?.type ?? type) || 'string'}
              {option?.missing ? (
                <Tag className="ml-2" color="warning">
                  字段不存在
                </Tag>
              ) : null}
            </span>
          );
        }
        return (
          <Select
            className="w-full"
            value={normalizeParamType(type)}
            options={PARAM_TYPE_OPTIONS}
            onChange={(nextType) => update(index, { type: nextType })}
          />
        );
      },
    },
    {
      title: '说明',
      dataIndex: 'note',
      render: (note: string, _row, index) =>
        readonly ? (
          <span className="param-note">{note || '-'}</span>
        ) : (
          <Input value={note} placeholder="给调用方看的参数说明" onChange={(event) => update(index, { note: event.target.value })} />
        ),
    },
  ];

  if (!readonly && !disableRemove) {
    columns.push({
      title: '操作',
      width: 88,
      render: (_value, _row, index) => (
        <Button danger icon={<DeleteOutlined />} onClick={() => removeRow(index)} />
      ),
    });
  }

  return (
    <div className="param-editor">
      <Table
        rowKey="key"
        size="small"
        pagination={false}
        columns={columns}
        dataSource={rows.map((row, index) => ({ ...row, key: `${row.name || 'param'}-${index}` }))}
        locale={{ emptyText: <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={emptyText} /> }}
      />
      {!readonly && !disableAdd ? (
        <Button className="param-add-button" icon={<PlusOutlined />} onClick={addRow}>
          添加参数
        </Button>
      ) : null}
    </div>
  );
}

export function normalizeParams(value: ParamSpec[] | undefined): ParamSpec[] {
  return (Array.isArray(value) ? value : []).map((item) => ({
    ...item,
    name: item.name ?? '',
    type: normalizeParamType(item.type),
    note: item.note ?? '',
  }));
}

export function normalizeParamType(type: string | undefined): ParamSpec['type'] {
  if (type === 'number') return 'double';
  if (type === 'array') return 'Array<string>';
  if (PARAM_TYPE_OPTIONS.some((option) => option.value === type)) return type ?? 'string';
  return 'string';
}

export function parseParamSpecs(raw: string | undefined): ParamSpec[] {
  if (!raw?.trim()) return [];
  const parsed = JSON.parse(raw) as ParamSpec[];
  if (!Array.isArray(parsed)) throw new Error('params 必须是数组 JSON');
  return normalizeParams(parsed);
}

export function stringifyParamSpecs(params: ParamSpec[]): string {
  return JSON.stringify(
    normalizeParams(params).map(({ name, type, note }) => ({ name, type, ...(note ? { note } : {}) })),
  );
}
