import {
  ApiOutlined,
  BarChartOutlined,
  DatabaseOutlined,
  KeyOutlined,
} from '@ant-design/icons';
import { Layout, Menu, Space, Tag, Typography } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { systemService } from '../api/services';

const { Header, Content } = Layout;

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const [version, setVersion] = useState('3.3.0-rust');
  const [mode, setMode] = useState('standalone');

  useEffect(() => {
    void Promise.allSettled([systemService.version(), systemService.mode()]).then((results) => {
      if (results[0].status === 'fulfilled') setVersion(String(results[0].value));
      if (results[1].status === 'fulfilled') setMode(String(results[1].value));
    });
  }, []);

  const selectedKey = useMemo(() => {
    if (location.pathname.startsWith('/datasources')) return '/datasources';
    if (location.pathname.startsWith('/tokens')) return '/tokens';
    if (location.pathname.startsWith('/monitor')) return '/monitor';
    return '/apis';
  }, [location.pathname]);

  return (
    <Layout className="min-h-screen">
      <Header className="sticky top-0 z-10 flex items-center gap-5 bg-[#4caf7a] px-5">
        <Space className="min-w-[190px]" align="center">
          <DatabaseOutlined className="text-3xl text-white" />
          <div>
            <Typography.Text className="block text-lg font-bold text-white">DBAPI</Typography.Text>
            <Typography.Text className="text-xs text-white/85">{version}</Typography.Text>
          </div>
        </Space>
        <Menu
          mode="horizontal"
          selectedKeys={[selectedKey]}
          className="flex-1 border-0 bg-transparent text-white"
          onClick={(item) => navigate(item.key)}
          items={[
            { key: '/datasources', icon: <DatabaseOutlined />, label: '数据源' },
            { key: '/apis', icon: <ApiOutlined />, label: 'API' },
            { key: '/tokens', icon: <KeyOutlined />, label: '权限 / Token' },
            { key: '/monitor', icon: <BarChartOutlined />, label: '监控' },
          ]}
        />
        <Tag color="green" className="m-0 bg-white/15 text-white">
          {mode}
        </Tag>
      </Header>
      <Content className="p-5">
        <Outlet />
      </Content>
    </Layout>
  );
}
