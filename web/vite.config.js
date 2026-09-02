import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

const apiTarget = process.env.API_PROXY || 'http://localhost:5201'

export default defineConfig({
    plugins: [vue()],
    server: {
        port: 5173,
        proxy: {
            '/api': { target: apiTarget, changeOrigin: true },
            '/r': { target: apiTarget, changeOrigin: true },
        },
    },
    build: {
        outDir: 'dist',
        emptyOutDir: true,
    },
})
