import { DeleteOutlined, PlusOutlined, SendOutlined } from '@ant-design/icons';
import { App, Button, Card, Empty, Form, Input, Space, Tag, Typography } from 'antd';
import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { apiConfigService, callUserApi, systemService } from '../api/services';
import type { ApiConfig, ParamSpec } from '../api/types';
import { normalizeParamType, parseParamSpecs } from '../components/ParamEditor';

type RequestParam = ParamSpec & {
  value?: unknown;
  values?: Array<{ va: unknown }>;
};

export default function ApiRequestPage() {
  const { id } = useParams();
  const { message } = App.useApp();
  const [detail, setDetail] = useState<ApiConfig | null>(null);
  const [address, setAddress] = useState('127.0.0.1:8520');
  const [token, setToken] = useState('');
  const [jsonBody, setJsonBody] = useState('{}');
  const [params, setParams] = useState<RequestParam[]>([]);
  const [result, setResult] = useState('');

  const contentType = detail?.contentType || 'application/json';
  const isJson = contentType.startsWith('application/json');

  useEffect(() => {
    if (!id) return;
    void apiConfigService.detail(id).then((next) => {
      setDetail(next);
      const specs = parseParamSpecs(next?.params);
      setParams(specs.map(withEmptyValue));
      setJsonBody(next?.jsonParam?.trim() || JSON.stringify(sampleBody(specs), null, 2));
    });
    void systemService.ip().then(setAddress);
  }, [id]);

  async function send() {
    if (!detail?.path) return;
    try {
      const body = isJson ? JSON.parse(jsonBody || '{}') : paramsToBody(params);
      const response = await callUserApi(detail.path, body, contentType, token);
      setResult(JSON.stringify(response, null, 2));
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    }
  }

  function updateParam(index: number, patch: Partial<RequestParam>) {
    setParams((current) => current.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row)));
  }

  function updateArrayValue(rowIndex: number, valueIndex: number, value: string) {
    setParams((current) =>
      current.map((row, currentRowIndex) => {
        if (currentRowIndex !== rowIndex) return row;
        const values = ensureArrayValues(row).map((item, currentValueIndex) =>
          currentValueIndex === valueIndex ? { va: value } : item,
        );
        return { ...row, values };
      }),
    );
  }

  function addArrayValue(rowIndex: number) {
    setParams((current) =>
      current.map((row, currentRowIndex) =>
        currentRowIndex === rowIndex ? { ...row, values: [...ensureArrayValues(row), { va: '' }] } : row,
      ),
    );
  }

  function removeArrayValue(rowIndex: number, valueIndex: number) {
    setParams((current) =>
      current.map((row, currentRowIndex) => {
        if (currentRowIndex !== rowIndex) return row;
        const values = ensureArrayValues(row).filter((_, currentValueIndex) => currentValueIndex !== valueIndex);
        return { ...row, values: values.length ? values : [{ va: '' }] };
      }),
    );
  }

  return (
    <div className="mx-auto max-w-5xl space-y-4">
      <Typography.Title level={3}>{detail?.name || '请求测试'}</Typography.Title>
      <Card>
        <Form layout="vertical">
          <Form.Item label="URL">
            <Input value={`http://${address}/api/${detail?.path || ''}`.replace('/api//', '/api/')} readOnly />
          </Form.Item>
          <Form.Item label="Content-Type">
            <Input value={contentType} readOnly />
          </Form.Item>
          <Form.Item label="Authorization">
            <Input value={token} onChange={(event) => setToken(event.target.value)} />
          </Form.Item>

          <Form.Item label={isJson ? '请求参数 JSON' : '请求参数'}>
            {isJson ? (
              <Input.TextArea rows={10} value={jsonBody} onChange={(event) => setJsonBody(event.target.value)} />
            ) : params.length ? (
              <div className="request-param-list">
                {params.map((row, rowIndex) => (
                  <div className="request-param-row" key={`${row.name}-${rowIndex}`}>
                    <div className="request-param-meta">
                      <span className="request-param-name">{row.name}</span>
                      <Tag className="request-param-type">{normalizeParamType(row.type)}</Tag>
                      {row.note ? <Typography.Text type="secondary">{row.note}</Typography.Text> : null}
                    </div>
                    {isArrayType(row.type) ? (
                      <Space direction="vertical" className="request-param-array" size={8}>
                        {ensureArrayValues(row).map((item, valueIndex) => (
                          <Space.Compact className="request-param-array-item" key={`${row.name}-${valueIndex}`}>
                            <Input
                              value={String(item.va ?? '')}
                              placeholder="数组元素"
                              onChange={(event) => updateArrayValue(rowIndex, valueIndex, event.target.value)}
                            />
                            <Button icon={<DeleteOutlined />} onClick={() => removeArrayValue(rowIndex, valueIndex)} />
                          </Space.Compact>
                        ))}
                        <Button icon={<PlusOutlined />} onClick={() => addArrayValue(rowIndex)}>
                          添加值
                        </Button>
                      </Space>
                    ) : (
                      <Input
                        value={String(row.value ?? '')}
                        placeholder={placeholderFor(row.type)}
                        onChange={(event) => updateParam(rowIndex, { value: event.target.value })}
                      />
                    )}
                  </div>
                ))}
              </div>
            ) : (
              <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="当前 API 没有请求参数" />
            )}
          </Form.Item>
          <Button type="primary" icon={<SendOutlined />} onClick={send}>
            发送请求
          </Button>
        </Form>
      </Card>
      <Card title="返回结果">
        <Input.TextArea rows={14} value={result} readOnly />
      </Card>
    </div>
  );
}

function withEmptyValue(param: ParamSpec): RequestParam {
  if (isArrayType(param.type)) return { ...param, values: [{ va: '' }] };
  return { ...param, value: '' };
}

function sampleBody(params: ParamSpec[]): Record<string, unknown> {
  return Object.fromEntries(
    params.map((param) => [param.name, isArrayType(param.type) ? [] : sampleValue(param.type)]),
  );
}

function paramsToBody(params: RequestParam[]): Record<string, string> {
  return Object.fromEntries(
    params
      .filter((param) => param.name)
      .map((param) => {
        if (isArrayType(param.type)) {
          const values = ensureArrayValues(param)
            .map((item) => coerceValue(String(item.va ?? ''), param.type))
            .filter((value) => value !== '');
          return [param.name, values.join(',')];
        }
        return [param.name, String(coerceValue(String(param.value ?? ''), param.type))];
      }),
  );
}

function ensureArrayValues(param: RequestParam): Array<{ va: unknown }> {
  return param.values?.length ? param.values : [{ va: '' }];
}

function isArrayType(type: string | undefined): boolean {
  return normalizeParamType(type).startsWith('Array<');
}

function placeholderFor(type: string | undefined): string {
  const normalized = normalizeParamType(type);
  if (normalized === 'bigint') return '整数，如 1';
  if (normalized === 'double') return '数字，如 3.14';
  if (normalized === 'date') return '日期，如 2026-05-05';
  return '请输入参数值';
}

function sampleValue(type: string | undefined): unknown {
  const normalized = normalizeParamType(type);
  if (normalized === 'bigint' || normalized === 'double') return 0;
  if (normalized === 'date') return '2026-05-05';
  return '';
}

function coerceValue(value: string, type: string | undefined): string | number {
  const normalized = normalizeParamType(type);
  const trimmed = value.trim();
  if (trimmed === '') return '';
  if (normalized === 'bigint') return Number.parseInt(trimmed, 10);
  if (normalized === 'double') return Number.parseFloat(trimmed);
  return value;
}
