<script lang="ts">
	import { onMount } from 'svelte';

	interface Props {
		content?: string;
		streaming?: boolean;
	}

	let { content = '', streaming = false }: Props = $props();

	let copiedIndex = $state<number | null>(null);
	let md: any = null;

	onMount(async () => {
		const MarkdownIt = (await import('markdown-it')).default;
		const { createHighlighter } = await import('shiki');

		const highlighter = await createHighlighter({
			themes: ['github-light', 'github-dark'],
			langs: [
				'javascript', 'typescript', 'python', 'rust', 'go', 'java',
				'html', 'css', 'json', 'yaml', 'bash', 'sql', 'markdown',
				'svelte', 'vue', 'jsx', 'tsx', 'c', 'cpp', 'ruby', 'php',
				'swift', 'kotlin', 'shell', 'dockerfile', 'toml',
			],
		});

		md = new MarkdownIt({
			html: false,
			linkify: true,
			breaks: true,
			highlight: (str: string, lang: string) => {
				if (lang && highlighter.getLanguage(lang)) {
					try {
						return highlighter.codeToHtml(str, {
							lang,
							theme: document.documentElement.classList.contains('dark')
								? 'github-dark'
								: 'github-light',
						});
					} catch {}
				}
				return `<pre class="shiki"><code>${md.utils.escapeHtml(str)}</code></pre>`;
			},
		});

		// Add task list support
		md.core.ruler.after('inline', 'task-lists', (state: any) => {
			const tokens = state.tokens;
			for (let i = 0; i < tokens.length; i++) {
				if (tokens[i].type !== 'inline') continue;
				const content = tokens[i].content;
				if (/^\[[ x]\]\s/.test(content)) {
					const checked = content.startsWith('[x]');
					const text = content.replace(/^\[[ x]\]\s/, '');
					tokens[i].content = text;
					tokens[i].children = [
						{ type: 'html_inline', content: `<input type="checkbox" disabled ${checked ? 'checked' : ''}> ` },
						...tokens[i].children || [],
					];
				}
			}
		});
	});

	function renderMarkdown(src: string): string {
		if (!md) return escapeHtml(src);

		// In streaming mode, auto-close unclosed code fences
		let text = src.replace(/\r\n/g, '\n');
		if (streaming) {
			const fenceCount = (text.match(/^```/gm) || []).length;
			if (fenceCount % 2 === 1) text += '\n```';
		}

		return md.render(text);
	}

	function escapeHtml(s: string): string {
		return s
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;')
			.replace(/"/g, '&quot;');
	}

	const renderedHtml = $derived(renderMarkdown(content));

	async function copyCode(code: string) {
		try {
			await navigator.clipboard.writeText(code);
		} catch {
			const ta = document.createElement('textarea');
			ta.value = code;
			ta.style.position = 'fixed';
			ta.style.opacity = '0';
			document.body.appendChild(ta);
			ta.select();
			document.execCommand('copy');
			document.body.removeChild(ta);
		}
		copiedIndex = 0;
		setTimeout(() => { copiedIndex = null; }, 1500);
	}

	function handleCopy(e: Event) {
		const btn = (e.target as HTMLElement).closest('.copy-btn');
		if (!btn) return;
		const code = btn.getAttribute('data-code');
		if (code) copyCode(decodeURIComponent(code));
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="markdown-viewer" onclick={handleCopy}>
	{#if !(streaming && content.trim() === '')}
		{@html renderedHtml}
	{/if}
</div>

<style>
	.markdown-viewer {
		font-size: var(--text-subheadline);
		line-height: 1.7;
		color: var(--color-fg);
		word-break: break-word;
		min-width: 0;
	}

	.markdown-viewer :global(p) {
		margin: 0 0 0.6em;
	}
	.markdown-viewer :global(p:last-child) {
		margin-bottom: 0;
	}

	.markdown-viewer :global(h1),
	.markdown-viewer :global(h2),
	.markdown-viewer :global(h3),
	.markdown-viewer :global(h4),
	.markdown-viewer :global(h5),
	.markdown-viewer :global(h6) {
		margin: 1em 0 0.5em;
		font-weight: var(--font-weight-semibold);
		color: var(--color-fg);
	}
	.markdown-viewer :global(h1) { font-size: 1.4em; }
	.markdown-viewer :global(h2) { font-size: 1.25em; }
	.markdown-viewer :global(h3) { font-size: 1.15em; }
	.markdown-viewer :global(h4) { font-size: 1.05em; }

	.markdown-viewer :global(ul),
	.markdown-viewer :global(ol) {
		margin: 0.4em 0;
		padding-left: 1.6em;
	}
	.markdown-viewer :global(li) {
		margin: 0.2em 0;
	}
	.markdown-viewer :global(li > input[type="checkbox"]) {
		margin-right: 0.4em;
	}

	.markdown-viewer :global(a) {
		color: var(--color-accent);
		text-decoration: none;
	}
	.markdown-viewer :global(a:hover) {
		text-decoration: underline;
	}

	.markdown-viewer :global(blockquote) {
		margin: 0.5em 0;
		padding: 0.4em 1em;
		border-left: 3px solid var(--color-accent);
		background: var(--color-bg-secondary);
		border-radius: var(--radius-sm);
		color: var(--color-fg-secondary);
	}

	.markdown-viewer :global(table) {
		border-collapse: collapse;
		margin: 0.6em 0;
		width: 100%;
		overflow-x: auto;
	}
	.markdown-viewer :global(th),
	.markdown-viewer :global(td) {
		border: 1px solid var(--color-separator);
		padding: 6px 12px;
		text-align: left;
		font-size: var(--text-caption1);
	}
	.markdown-viewer :global(th) {
		background: var(--color-bg-hover);
		font-weight: var(--font-weight-semibold);
	}
	.markdown-viewer :global(tr:nth-child(even)) {
		background: var(--color-bg-secondary);
	}

	.markdown-viewer :global(hr) {
		border: none;
		border-top: 1px solid var(--color-separator);
		margin: 1em 0;
	}

	.markdown-viewer :global(del) {
		color: var(--color-fg-tertiary);
	}

	.markdown-viewer :global(code.inline-code) {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-separator);
		border-radius: 4px;
		padding: 0.15em 0.4em;
		font-family: var(--font-mono);
		font-size: 0.9em;
	}

	.markdown-viewer :global(pre.shiki) {
		margin: 0.6em 0;
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		overflow: hidden;
		background: var(--color-bg-secondary) !important;
	}

	.markdown-viewer :global(pre.shiki code) {
		display: block;
		padding: 12px 16px;
		overflow-x: auto;
		font-family: var(--font-mono);
		font-size: var(--text-footnote);
		line-height: 1.5;
	}

	.markdown-viewer :global(pre.shiki code .line) {
		display: inline-block;
		width: 100%;
	}

	.markdown-viewer :global(img) {
		max-width: 100%;
		border-radius: var(--radius-sm);
	}

	.markdown-viewer :global(.task-list-item) {
		list-style: none;
		margin-left: -1.6em;
	}
</style>
