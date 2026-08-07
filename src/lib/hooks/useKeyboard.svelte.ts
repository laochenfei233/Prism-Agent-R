// Svelte 5 rune 风格键盘快捷键 hook
import { untrack } from 'svelte';

export function useKeyboard() {
	const shortcuts = $state<Map<string, () => void>>(new Map());

	function register(key: string, handler: () => void) {
		shortcuts.set(key, handler);
	}

	function unregister(key: string) {
		shortcuts.delete(key);
	}

	$effect(() => {
		const listener = (e: KeyboardEvent) => {
			const mod = (e.metaKey || e.ctrlKey) ? 'cmd' : '';
			const shift = e.shiftKey ? '+shift' : '';
			const key = `${mod}${shift}+${e.key.toLowerCase()}`;
			// 不拦截输入框内操作（除了 Esc）
			const target = e.target as HTMLElement;
			if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)) {
				if (e.key !== 'Escape') return;
			}
			const handler = shortcuts.get(key);
			if (handler) {
				e.preventDefault();
				handler();
			}
		};
		window.addEventListener('keydown', listener);
		return () => window.removeEventListener('keydown', listener);
	});

	return { register, unregister };
}
