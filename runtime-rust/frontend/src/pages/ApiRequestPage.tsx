import { App, Button, Card, Form, Input, Typography } from 'antd';
import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { apiConfigService, callUserApi, systemService } from '../api/services';
import type { ApiConfig } from '../api/types';

export default function ApiRequestPage() {
  const { id } = useParams();
  const { message } = App.useApp();
  const [detail, setDetail] = useState<ApiConfig | null>(null);
  const [address, setAddress] = useState('127.0.0.1:8520');
  const [token, setToken] = useState('');
  const [body, setBody] = useState('{}');
  const [result, setResult] = useState('');

  useEffect(() => {
    if (!id) return;
    void apiConfigService.detail(id).then((next) => {
      setDetail(next);
      const params = next?.params ? JSON.parse(next.params) : [];
      const bodyObject = Object.fromEntries(params.map((item: { name: string }) => [item.name, '']));
      setBody(JSON.stringify(bodyObject, null, 2));
    });
    void systemService.ip().then(setAddress);
  }, [id]);

  async function send() {
    if (!detail?.path) return;
    try {
      const parsed = detail.contentType === 'application/json' ? JSON.parse(body || '{}') : JSON.parse(body || '{}');
      const response = await callUserApi(detail.path, parsed, detail.contentType || 'application/json', token);
      setResult(JSON.stringify(response, null, 2));
    } catch (error) {
      message.error(String(error));
    }
  }

  return (
    <div className="mx-auto max-w-5xl space-y-4">
      <Typography.Title level={3}>{detail?.name || '请求测试'}</Typography.Title>
      <Card>
        <Form layout="vertical">
          <Form.Item label="URL">
            <Input value={`http://${address}/api/${detail?.path || ''}`.replace('/api//', '/api/')} readOnly />
          </Form.Item>
          <Form.Item label="Authorization">
            <Input value={token} onChange={(event) => setToken(event.target.value)} />
          </Form.Item>
          <Form.Item label="请求参数 JSON">
            <Input.TextArea rows={10} value={body} onChange={(event) => setBody(event.target.value)} />
          </Form.Item>
          <Button type="primary" onClick={send}>
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
