import { ReloadOutlined } from '@ant-design/icons';
import { App, Button, Card, DatePicker, Empty, Space, Statistic, Table, Typography } from 'antd';
import dayjs, { Dayjs } from 'dayjs';
import { useEffect, useMemo, useState } from 'react';
import { monitorService } from '../api/services';
import type { AccessLog } from '../api/types';

type MetricRow = Record<string, unknown>;

export default function MonitorPage() {
  const { message } = App.useApp();
  const [range, setRange] = useState<[Dayjs, Dayjs]>([dayjs().subtract(7, 'day'), dayjs()]);
  const [logs, setLogs] = useState<AccessLog[]>([]);
  const [ratio, setRatio] = useState<MetricRow>({});
  const [trend, setTrend] = useState<MetricRow[]>([]);
  const [topApi, setTopApi] = useState<MetricRow[]>([]);
  const [topApp, setTopApp] = useState<MetricRow[]>([]);
  const [topIp, setTopIp] = useState<MetricRow[]>([]);
  const [topDuration, setTopDuration] = useState<MetricRow[]>([]);

  const payload = useMemo(
    () => ({
      start: range[0].unix(),
      end: range[1].add(1, 'day').unix(),
    }),
    [range],
  );

  const successNum = numberOf(ratio, 'successNum');
  const failNum = numberOf(ratio, 'failNum');
  const totalNum = successNum + failNum;

  async function load() {
    const [nextLogs, nextRatio, nextTrend, nextTopApi, nextTopApp, nextTopIp, nextTopDuration] =
      await Promise.all([
        keepOnError(monitorService.search(payload), logs),
        keepOnError(monitorService.successRatio(payload), ratio),
        keepOnError(monitorService.countByDay(payload), trend),
        keepOnError(monitorService.topApi(payload), topApi),
        keepOnError(monitorService.topApp(payload), topApp),
        keepOnError(monitorService.topIp(payload), topIp),
        keepOnError(monitorService.topDuration(payload), topDuration),
      ]);
    setLogs(nextLogs);
    setRatio(nextRatio);
    setTrend(nextTrend);
    setTopApi(nextTopApi);
    setTopApp(nextTopApp);
    setTopIp(nextTopIp);
    setTopDuration(nextTopDuration);
  }

  async function keepOnError<T>(request: Promise<T>, fallback: T): Promise<T> {
    try {
      return await request;
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
      return fallback;
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
          <Statistic title="Success" value={successNum} />
        </Card>
        <Card>
          <Statistic title="Failed" value={failNum} />
        </Card>
        <Card>
          <Statistic title="Total" value={totalNum} />
        </Card>
      </div>
      <div className="monitor-grid">
        <Card title="访问趋势" className="monitor-chart-wide">
          <TrendChart rows={trend} />
        </Card>
        <Card title="成功占比">
          <RatioChart success={successNum} failed={failNum} />
        </Card>
        <TopBarChart title="Top API" rows={topApi} labelKey="url" valueKey="num" />
        <TopBarChart title="Top IP" rows={topIp} labelKey="ip" valueKey="num" />
        <TopBarChart title="Top App" rows={topApp} labelKey="app_id" valueKey="num" emptyText="暂无 App 访问" />
        <TopBarChart title="平均耗时 Top" rows={topDuration} labelKey="url" valueKey="duration" suffix=" ms" />
      </div>
      <Card title="访问记录">
        <Table<AccessLog>
          rowKey={(row, index) => row.id ?? `${row.url}-${index}`}
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

function TrendChart({ rows }: { rows: MetricRow[] }) {
  if (!rows.length) return <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无访问趋势" />;

  const success = rows.map((row) => numberOf(row, 'successNum'));
  const failed = rows.map((row) => numberOf(row, 'failNum'));
  const max = Math.max(1, ...success, ...failed);

  return (
    <div className="trend-chart">
      <svg viewBox="0 0 100 48" role="img" aria-label="访问趋势">
        <polyline className="trend-grid-line" points="0,44 100,44" />
        <polyline className="trend-grid-line" points="0,24 100,24" />
        <polyline className="trend-line trend-line-success" points={pointsFor(success, max)} />
        <polyline className="trend-line trend-line-failed" points={pointsFor(failed, max)} />
      </svg>
      <div className="trend-labels">
        <span>{String(rows[0]?.date ?? '')}</span>
        <span>{String(rows[rows.length - 1]?.date ?? '')}</span>
      </div>
      <div className="trend-legend">
        <span><i className="legend-dot legend-success" />Success</span>
        <span><i className="legend-dot legend-failed" />Failed</span>
      </div>
    </div>
  );
}

function RatioChart({ success, failed }: { success: number; failed: number }) {
  const total = success + failed;
  if (!total) return <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="暂无访问数据" />;
  const successPercent = Math.round((success / total) * 100);

  return (
    <div className="ratio-chart">
      <div className="ratio-number">{successPercent}%</div>
      <div className="ratio-track">
        <span className="ratio-success" style={{ width: `${successPercent}%` }} />
      </div>
      <div className="ratio-details">
        <span>Success {success}</span>
        <span>Failed {failed}</span>
      </div>
    </div>
  );
}

function TopBarChart({
  title,
  rows,
  labelKey,
  valueKey,
  suffix = '',
  emptyText = '暂无数据',
}: {
  title: string;
  rows: MetricRow[];
  labelKey: string;
  valueKey: string;
  suffix?: string;
  emptyText?: string;
}) {
  const max = Math.max(1, ...rows.map((row) => numberOf(row, valueKey)));
  return (
    <Card title={title}>
      {rows.length ? (
        <div className="top-bar-list">
          {rows.map((row, index) => {
            const value = numberOf(row, valueKey);
            return (
              <div className="top-bar-row" key={`${String(row[labelKey])}-${index}`}>
                <Typography.Text className="top-bar-label" ellipsis={{ tooltip: String(row[labelKey] ?? '-') }}>
                  {String(row[labelKey] ?? '-')}
                </Typography.Text>
                <div className="top-bar-track">
                  <span style={{ width: `${Math.max(4, (value / max) * 100)}%` }} />
                </div>
                <span className="top-bar-value">{value}{suffix}</span>
              </div>
            );
          })}
        </div>
      ) : (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={emptyText} />
      )}
    </Card>
  );
}

function pointsFor(values: number[], max: number): string {
  if (values.length === 1) return `0,${yFor(values[0], max)} 100,${yFor(values[0], max)}`;
  return values
    .map((value, index) => {
      const x = (index / (values.length - 1)) * 100;
      return `${x.toFixed(2)},${yFor(value, max)}`;
    })
    .join(' ');
}

function yFor(value: number, max: number): string {
  return (44 - (value / max) * 36).toFixed(2);
}

function numberOf(row: MetricRow, key: string): number {
  const value = row[key];
  if (typeof value === 'number') return value;
  if (typeof value === 'string') return Number(value) || 0;
  return 0;
}
