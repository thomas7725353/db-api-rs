import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    outDir: '../static',
    emptyOutDir: true,
    sourcemap: true,
  },
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:8520',
      '/apiConfig': 'http://127.0.0.1:8520',
      '/datasource': 'http://127.0.0.1:8520',
      '/group': 'http://127.0.0.1:8520',
      '/app': 'http://127.0.0.1:8520',
      '/token': 'http://127.0.0.1:8520',
      '/access': 'http://127.0.0.1:8520',
      '/system': 'http://127.0.0.1:8520',
      '/table': 'http://127.0.0.1:8520',
      '/queryBuilder': 'http://127.0.0.1:8520',
    },
  },
});
