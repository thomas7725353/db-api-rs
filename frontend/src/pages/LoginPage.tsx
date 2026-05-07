import { LockOutlined, UserOutlined } from '@ant-design/icons';
import { App, Button, Card, Form, Input, Typography } from 'antd';
import { Navigate, useLocation, useNavigate } from 'react-router-dom';
import { authService } from '../api/services';
import { isAuthenticated, saveAuthToken } from '../auth/session';

interface LoginForm {
  username: string;
  password: string;
}

interface LocationState {
  from?: {
    pathname?: string;
  };
}

export default function LoginPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { message } = App.useApp();
  const state = location.state as LocationState | null;
  const targetPath = state?.from?.pathname && state.from.pathname !== '/login' ? state.from.pathname : '/apis';

  if (isAuthenticated()) {
    return <Navigate to="/apis" replace />;
  }

  async function submit(values: LoginForm) {
    const token = await authService.login(values.username, values.password);
    saveAuthToken(token);
    message.success('登录成功');
    navigate(targetPath, { replace: true });
  }

  return (
    <main className="login-page">
      <Card className="login-card">
        <div className="login-heading">
          <Typography.Title level={2} className="login-title">
            DBAPI
          </Typography.Title>
          <Typography.Text className="login-subtitle">管理控制台</Typography.Text>
        </div>
        <Form<LoginForm>
          layout="vertical"
          initialValues={{ username: 'admin' }}
          onFinish={submit}
          requiredMark={false}
        >
          <Form.Item
            label="用户名"
            name="username"
            rules={[{ required: true, message: '请输入用户名' }]}
          >
            <Input prefix={<UserOutlined />} autoComplete="username" />
          </Form.Item>
          <Form.Item
            label="密码"
            name="password"
            rules={[{ required: true, message: '请输入密码' }]}
          >
            <Input.Password prefix={<LockOutlined />} autoComplete="current-password" />
          </Form.Item>
          <Button type="primary" htmlType="submit" block size="large">
            登录
          </Button>
        </Form>
      </Card>
    </main>
  );
}
