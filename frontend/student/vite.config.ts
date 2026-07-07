/// <reference types="vitest" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: 8080,
    proxy: {
      '/api': { target: 'http://localhost:5000', changeOrigin: true },
      '/s':   { target: 'http://localhost:5000', changeOrigin: true },
    },
  },
  build: { outDir: 'dist', assetsDir: 'attend-assets' },
});
