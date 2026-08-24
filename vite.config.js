import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import UnoCSS from 'unocss/vite'
import { resolve } from 'path'

const isDev = process.env.NODE_ENV === 'development'
const isTauriDebug = process.env.TAURI_DEBUG === 'true'

export default defineConfig({
  root: 'src',
  clearScreen: false,

  server: {
    port: 1421,
    strictPort: true,
    fs: {
      allow: [
        resolve(__dirname, '.'),
        resolve(__dirname, 'node_modules'),
        resolve(__dirname, 'src'),
      ],
    },
  },

  envPrefix: ['VITE_', 'TAURI_'],

  resolve: {
    preserveSymlinks: true,
    alias: {
      '@': resolve(__dirname, 'src'),
      '@shared': resolve(__dirname, 'src/shared'),
      '@windows': resolve(__dirname, 'src/windows'),
      'uno.css': 'virtual:uno.css',
    },
  },

  plugins: [
    UnoCSS({
      mode: 'global',
      inspector: false,
    }),
    react({
      babel: {
        plugins: [
          ['babel-plugin-react-compiler', {}],
        ],
      },
    }),
  ],

  build: {
    outDir: '../dist',
    target: process.env.TAURI_PLATFORM === 'windows'
      ? 'chrome105'
      : 'safari16',

    minify: isDev || isTauriDebug ? false : 'esbuild',

    esbuild: isDev || isTauriDebug
      ? {}
      : {
          drop: ['debugger'],
          pure: ['console.log', 'console.info', 'console.debug'],
        },

    sourcemap: isDev || isTauriDebug,
    cssCodeSplit: true,

    rollupOptions: {
      input: {
          main: resolve(__dirname, 'src/windows/main/index.html'),
          settings: resolve(__dirname, 'src/windows/settings/index.html'),
          quickpaste: resolve(__dirname, 'src/windows/quickpaste/index.html'),
          community: resolve(__dirname, 'src/windows/community/index.html'),
          textEditor: resolve(__dirname, 'src/windows/textEditor/index.html'),
          contextMenu: resolve(__dirname, 'src/plugins/context_menu/contextMenu.html'),
          inputDialog: resolve(__dirname, 'src/plugins/input_dialog/inputDialog.html'),
          pinImage: resolve(__dirname, 'src/windows/pinImage/pinImage.html'),
          preview: resolve(__dirname, 'src/windows/preview/index.html'),
          transferShelf: resolve(__dirname, 'src/windows/transferShelf/index.html'),
          receiveBox: resolve(__dirname, 'src/windows/receiveBox/index.html'),
          updater: resolve(__dirname, 'src/windows/updater/index.html'),
      },

      output: {
        assetFileNames: 'assets/[name]-[hash][extname]',
        chunkFileNames: 'js/[name]-[hash].js',
        entryFileNames: 'js/[name]-[hash].js',

        manualChunks(id) {
          if (id.includes('node_modules')) return 'vendor'
          if (id.includes('/shared/') || id.includes('\\shared\\')) return 'shared'
          return undefined
        },
      },
    },
  },
})
