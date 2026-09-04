/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_LEDGER_API_BASE?: string;
  readonly VITE_LEDGER_DATA_MODE?: 'mock' | 'http';
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
