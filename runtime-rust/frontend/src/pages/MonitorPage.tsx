import { ReloadOutlined } from '@ant-design/icons';
import { App, Button, Card, DatePicker, Space, Statistic, Table, Typography } from 'antd';
import dayjs, { Dayjs } from 'dayjs';
import { useEffect, useMemo, useState } from 'react';
import { monitorService } from '../api/services';
import type { AccessLog } from '../api/types';

export default function MonitorPage() {
  const { message } = App.useApp();
  const [range, setRange] = useState<[Dayjs, Dayjs]>([dayjs().subtract(7, 'day'), dayjs()]);
  const [logs, setLogs] = useState<AccessLog[]>([]);
  const [ratio, setRatio] = useState<Record<string, unknown>>({});

  const payload = useMemo(
    () => ({
      start: range[0].unix(),
      end: range[1].add(1, 'day').unix(),
    }),
    [range],
  );

  async function load() {
    try {
      const [nextLogs, nextRatio] = await Promise.all([
        monitorService.search(payload),
        monitorService.successRatio(payload),
      ]);
      setLogs(nextLogs);
      setRatio(nextRatio);
    } catch (error) {
      message.error(String(error));
    }
  }

  useEffect(() => {
    void load();
  }, []);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <Typography.Title level={3} className="!mb-1">
          监控
        </Typography.Title>
        <Space>
          <DatePicker.RangePicker
            value={range}
            onChange={(next) => {
              if (next?.[0] && next[1]) setRange([next[0], next[1]]);
            }}
          />
          <Button icon={<ReloadOutlined />} onClick={load}>
            查询
          </Button>
        </Space>
      </div>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
        <Card>
          <Statistic title="Success" value={Number(ratio.successNum ?? 0)} />
        </Card>
        <Card>
          <Statistic title="Failed" value={Number(ratio.failNum ?? 0)} />
        </Card>
        <Card>
          <Statistic title="Total" value={Number(ratio.successNum ?? 0) + Number(ratio.failNum ?? 0)} />
        </Card>
      </div>
      <Card title="访问记录">
        <Table<AccessLog>
          rowKey={(row) => row.id ?? Math.random().toString()}
          dataSource={logs}
          columns={[
            { title: 'URL', dataIndex: 'url' },
            { title: '状态', dataIndex: 'status', width: 100 },
            { title: '耗时(ms)', dataIndex: 'duration', width: 120 },
            { title: 'IP', dataIndex: 'ip', width: 150 },
            { title: '错误', dataIndex: 'error', ellipsis: true },
          ]}
        />
      </Card>
    </div>
  );
}
