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
      // Regex, not a bare '/s' prefix: a plain string match also catches
      // '/src/...', which is how Vite serves its own source modules in dev —
      // that made every request for main.tsx 404 through this proxy and the
      // app never mounted. The backend nests these routes under /api/s/*
      // (see backend-rust/src/routes/mod.rs); production Caddy rewrites
      // /s/* -> /api/s/* (see Caddyfile.prod), which this mirrors so `vite
      // dev` talking directly to `cargo run` (no Caddy in front) works too.
      '^/s/': {
        target: 'http://localhost:5000',
        changeOrigin: true,
        rewrite: (path) => `/api${path}`,
      },
    },
  },
  build: { outDir: 'dist', assetsDir: 'attend-assets' },
  assetsInclude: ['**/*.tflite'],
  optimizeDeps: {
    exclude: ['@mediapipe/tasks-vision'],
  },
});
