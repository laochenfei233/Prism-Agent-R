import { describe, it, expect, afterEach, vi } from 'vitest';

type ThemeStore = { theme: string; init: () => void; toggle: () => void };

function mockMatchMedia(matches: boolean) {
  return vi.fn().mockImplementation((query: string) => ({
    matches,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    onchange: null,
    dispatchEvent: vi.fn(),
  }));
}

// themeStore is a module singleton initialized at import time; reset modules and
// re-import per test so the stored preference / matchMedia state is deterministic.
async function loadStore(): Promise<ThemeStore> {
  vi.resetModules();
  const mod = await import('./theme.svelte');
  return mod.themeStore as ThemeStore;
}

describe('themeStore', () => {
  const originalMatchMedia = window.matchMedia;

  afterEach(() => {
    localStorage.clear();
    window.matchMedia = originalMatchMedia;
  });

  it('resolves dark when system prefers dark and nothing stored', async () => {
    window.matchMedia = mockMatchMedia(true);
    const store = await loadStore();
    expect(store.theme).toBe('dark');
  });

  it('resolves light when system prefers light and nothing stored', async () => {
    window.matchMedia = mockMatchMedia(false);
    const store = await loadStore();
    expect(store.theme).toBe('light');
  });

  it('prefers stored preference over system preference', async () => {
    window.matchMedia = mockMatchMedia(false);
    localStorage.setItem('prism-theme', 'dark');
    const store = await loadStore();
    expect(store.theme).toBe('dark');
  });

  it('toggles between light and dark and persists', async () => {
    window.matchMedia = mockMatchMedia(false);
    const store = await loadStore();
    store.toggle();
    expect(store.theme).toBe('dark');
    expect(localStorage.getItem('prism-theme')).toBe('dark');
    store.toggle();
    expect(store.theme).toBe('light');
    expect(localStorage.getItem('prism-theme')).toBe('light');
  });
});
