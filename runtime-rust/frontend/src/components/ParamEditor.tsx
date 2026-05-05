import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Empty, Input, Select, Table } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import type { ParamSpec } from '../api/types';

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
  emptyText?: string;
}

export default function ParamEditor({
  value,
  onChange,
  readonly = false,
  lockNames = false,
  emptyText = '暂无参数',
}: ParamEditorProps) {
  const rows = normalizeParams(value);

  function update(index: number, patch: Partial<ParamSpec>) {
    onChange?.(rows.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row)));
  }

  function addRow() {
    onChange?.([...rows, { name: '', type: 'string', note: '' }]);
  }

  function removeRow(index: number) {
    onChange?.(rows.filter((_, rowIndex) => rowIndex !== index));
  }

  const columns: ColumnsType<ParamSpec & { key: string }> = [
    {
      title: '参数名',
      dataIndex: 'name',
      width: 260,
      render: (name: string, _row, index) =>
        readonly || lockNames ? (
          <span className="param-name">{name || '-'}</span>
        ) : (
          <Input value={name} placeholder="如 id / status" onChange={(event) => update(index, { name: event.target.value })} />
        ),
    },
    {
      title: '类型',
      dataIndex: 'type',
      width: 220,
      render: (type: string, _row, index) =>
        readonly ? (
          <span className="param-type-tag">{type || 'string'}</span>
        ) : (
          <Select
            className="w-full"
            value={normalizeParamType(type)}
            options={PARAM_TYPE_OPTIONS}
            onChange={(nextType) => update(index, { type: nextType })}
          />
        ),
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

  if (!readonly) {
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
      {!readonly ? (
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
