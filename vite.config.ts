import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vite'

// Tauri expects a fixed dev port.
const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  // Prevent Vite from obscuring Rust errors.
  clearScreen: false,
  server: {
    // Tauri requires a fixed port.
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Don't watch .rust-target (wasteful & noisy).
      ignored: ['**/.rust-target/**', '**/target/**', '**/crates/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    // Rust backend expects app files in $FRONTEND_DIST
    outDir: 'dist',
    target: 'es2022',
    minify: process.env.TAURI_ENV_DEBUG ? false : 'esbuild',
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
})
