import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Cổng 1420 cố định theo tauri.conf.json (Task 17).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: 'es2022', outDir: 'dist', emptyOutDir: true },
  test: { environment: 'jsdom', setupFiles: ['src/test/setup.ts'], css: false },
});
