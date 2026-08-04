declare global {
	interface Window {
		__TAURI__: {
			core: {
				invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
			};
			event: {
				listen: <T>(event: string, handler: (payload: { payload: T }) => void) => Promise<() => void>;
			};
		};
	}
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	return window.__TAURI__.core.invoke<T>(cmd, args);
}

export async function listen<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
	return window.__TAURI__.event.listen<T>(event, (e) => handler(e.payload));
}
