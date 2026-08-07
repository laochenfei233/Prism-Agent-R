type Theme = 'light' | 'dark';

function createThemeStore() {
	const STORAGE_KEY = 'prism-theme';

	function systemPrefersDark(): boolean {
		return typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches;
	}

	function storedTheme(): Theme | null {
		try {
			const v = localStorage.getItem(STORAGE_KEY);
			return v === 'light' || v === 'dark' ? v : null;
		} catch {
			return null;
		}
	}

	function apply(theme: Theme) {
		document.documentElement.classList.toggle('dark', theme === 'dark');
		document.documentElement.style.colorScheme = theme;
	}

	function resolve(): Theme {
		return storedTheme() ?? (systemPrefersDark() ? 'dark' : 'light');
	}

	let theme = $state<Theme>(resolve());

	function init() {
		apply(theme);
		const mq = window.matchMedia('(prefers-color-scheme: dark)');
		mq.addEventListener('change', () => {
			if (!storedTheme()) {
				theme = systemPrefersDark() ? 'dark' : 'light';
				apply(theme);
			}
		});
	}

	function toggle() {
		theme = theme === 'dark' ? 'light' : 'dark';
		try {
			localStorage.setItem(STORAGE_KEY, theme);
		} catch {
			// 无痕模式下忽略持久化失败
		}
		apply(theme);
	}

	return {
		get theme() {
			return theme;
		},
		init,
		toggle
	};
}

export const themeStore = createThemeStore();
