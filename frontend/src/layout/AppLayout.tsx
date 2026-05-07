import {
  ApiOutlined,
  BarChartOutlined,
  DatabaseOutlined,
  KeyOutlined,
  LogoutOutlined,
} from '@ant-design/icons';
import { Button, Layout, Menu, Tag, Typography } from 'antd';
import { useEffect, useMemo, useState } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { systemService } from '../api/services';
import { clearAuthSession } from '../auth/session';

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

  function logout() {
    clearAuthSession();
    navigate('/login', { replace: true });
  }

  return (
    <Layout className="min-h-screen">
      <Header className="app-header">
        <div className="app-brand">
          <DatabaseOutlined className="app-brand-icon" />
          <div className="app-brand-text">
            <Typography.Text className="app-brand-title">DBAPI</Typography.Text>
            <Typography.Text className="app-brand-version">{version}</Typography.Text>
          </div>
        </div>
        <Menu
          mode="horizontal"
          selectedKeys={[selectedKey]}
          className="app-nav"
          onClick={(item) => navigate(item.key)}
          items={[
            { key: '/datasources', icon: <DatabaseOutlined />, label: '数据源' },
            { key: '/apis', icon: <ApiOutlined />, label: 'API' },
            { key: '/tokens', icon: <KeyOutlined />, label: '权限 / Token' },
            { key: '/monitor', icon: <BarChartOutlined />, label: '监控' },
          ]}
        />
        <div className="app-header-actions">
          <Tag color="green" className="app-mode-tag">
            {mode}
          </Tag>
          <Button icon={<LogoutOutlined />} onClick={logout}>
            退出
          </Button>
        </div>
      </Header>
      <Content className="p-5">
        <Outlet />
      </Content>
    </Layout>
  );
}
