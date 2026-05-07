import { ConfigProvider, App as AntApp, theme } from 'antd';
import { Navigate, Outlet, Route, Routes, useLocation } from 'react-router-dom';
import { isAuthenticated } from './auth/session';
import AppLayout from './layout/AppLayout';
import ApiEditorPage from './pages/ApiEditorPage';
import ApiRequestPage from './pages/ApiRequestPage';
import ApisPage from './pages/ApisPage';
import DatasourcesPage from './pages/DatasourcesPage';
import LoginPage from './pages/LoginPage';
import MonitorPage from './pages/MonitorPage';
import TokensPage from './pages/TokensPage';

function RequireAuth() {
  const location = useLocation();
  if (!isAuthenticated()) {
    return <Navigate to="/login" replace state={{ from: location }} />;
  }
  return <Outlet />;
}

export default function App() {
  return (
    <ConfigProvider
      theme={{
        algorithm: theme.defaultAlgorithm,
        token: {
          colorPrimary: '#2f8f68',
          borderRadius: 8,
        },
      }}
    >
      <AntApp>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route element={<RequireAuth />}>
            <Route element={<AppLayout />}>
              <Route index element={<Navigate to="/apis" replace />} />
              <Route path="/datasources" element={<DatasourcesPage />} />
              <Route path="/apis" element={<ApisPage />} />
              <Route path="/apis/new" element={<ApiEditorPage />} />
              <Route path="/apis/:id/edit" element={<ApiEditorPage />} />
              <Route path="/apis/:id/request" element={<ApiRequestPage />} />
              <Route path="/tokens" element={<TokensPage />} />
              <Route path="/monitor" element={<MonitorPage />} />
              <Route path="*" element={<Navigate to="/apis" replace />} />
            </Route>
          </Route>
        </Routes>
      </AntApp>
    </ConfigProvider>
  );
}
