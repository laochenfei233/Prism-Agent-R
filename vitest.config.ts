import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Standalone vitest config — intentionally separate from vite.config.ts so the
// Tauri build pipeline never picks up test-only plugins/environments.
export default defineConfig({
  plugins: [svelte()],
  esbuild: {
    // Standalone tsconfig (no extends .svelte-kit/tsconfig.json) so tests run
    // even before `svelte-kit sync` generated the project tsconfig.
    tsconfigRaw: JSON.stringify({
      compilerOptions: {
        target: 'ES2022',
        module: 'ESNext',
        moduleResolution: 'bundler',
        lib: ['ES2022', 'DOM', 'DOM.Iterable'],
        allowJs: true,
        checkJs: false,
      },
    }),
  },
  resolve: {
    // svelte exports a browser (client) build under the `browser` condition;
    // without it vitest resolves the SSR build and Svelte.mount fails.
    conditions: ['browser'],
    alias: {
      $lib: new URL('./src/lib', import.meta.url).pathname,
    },
  },
  test: {
    environment: 'jsdom',
    globals: false,
    include: ['src/**/*.test.{ts,tsx,svelte.ts}'],
    setupFiles: ['./src/test/setup.ts'],
  },
});
