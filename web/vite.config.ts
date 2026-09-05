import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', 'VITE_');
  const mock = env.VITE_LEDGER_DATA_MODE === 'mock';
  return {
  plugins: [react()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  // A built preview is static acceptance, never an implicit live-ledger proxy.
  preview: { host: '127.0.0.1', strictPort: true, proxy: {} },
  server: {
    host: '127.0.0.1',
    port: 47128,
    strictPort: true,
    proxy: mock ? undefined : {
      '/v1': 'http://127.0.0.1:47127',
    },
  },
  };
});
