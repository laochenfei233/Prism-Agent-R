<script lang="ts">
	interface Block {
		type: 'text' | 'code';
		html: string;
		code?: string;
		lang?: string;
	}

	let {
		content = '',
		streaming = false
	}: { content?: string; streaming?: boolean } = $props();

	let copiedIndex = $state<number | null>(null);

	function escapeHtml(s: string): string {
		return s
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;')
			.replace(/"/g, '&quot;');
	}

	function inlineMarkdown(src: string): string {
		const parts = src.split(/`([^`]+)`/g);
		return parts
			.map((part, i) => {
				if (i % 2 === 1) return `<code class="inline-code">${escapeHtml(part)}</code>`;
				let s = escapeHtml(part);
				s = s.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
				s = s.replace(/(^|[^*])\*([^*\n]+)\*/g, '$1<em>$2</em>');
				s = s.replace(
					/\[([^\]]+)\]\(([^)\s]+)\)/g,
					'<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>'
				);
				return s;
			})
			.join('');
	}

	function renderTextLines(lines: string[]): string {
		let html = '';
		let i = 0;
		while (i < lines.length) {
			const line = lines[i];

			const heading = line.match(/^(#{1,4})\s+(.*)$/);
			if (heading) {
				const level = heading[1].length;
				html += `<h${level}>${inlineMarkdown(heading[2])}</h${level}>`;
				i++;
				continue;
			}

			if (line.startsWith('>')) {
				const quotes: string[] = [];
				while (i < lines.length && lines[i].startsWith('>')) {
					quotes.push(inlineMarkdown(lines[i].replace(/^>\s?/, '')));
					i++;
				}
				html += `<blockquote>${quotes.join('<br>')}</blockquote>`;
				continue;
			}

			const ul = line.match(/^[-*+]\s+(.*)$/);
			if (ul) {
				const items: string[] = [];
				while (i < lines.length) {
					const m = lines[i].match(/^[-*+]\s+(.*)$/);
					if (!m) break;
					items.push(`<li>${inlineMarkdown(m[1])}</li>`);
					i++;
				}
				html += `<ul>${items.join('')}</ul>`;
				continue;
			}

			const ol = line.match(/^\d+[.)]\s+(.*)$/);
			if (ol) {
				const items: string[] = [];
				while (i < lines.length) {
					const m = lines[i].match(/^\d+[.)]\s+(.*)$/);
					if (!m) break;
					items.push(`<li>${inlineMarkdown(m[1])}</li>`);
					i++;
				}
				html += `<ol>${items.join('')}</ol>`;
				continue;
			}

			const para: string[] = [];
			while (
				i < lines.length &&
				lines[i].trim() !== '' &&
				!/^(#{1,4})\s|^>|^[-*+]\s|^\d+[.)]\s/.test(lines[i])
			) {
				para.push(lines[i]);
				i++;
			}
			if (para.length > 0) html += `<p>${inlineMarkdown(para.join(' '))}</p>`;
		}
		return html;
	}

	function parseMarkdown(src: string): Block[] {
		// In streaming mode an odd fence count means the closing fence hasn't
		// arrived yet: pseudo-close it so the completed part still renders as a
		// code block instead of swallowing the rest of the stream as raw text.
		let text = src.replace(/\r\n/g, '\n');
		if (streaming) {
			const fenceCount = text
				.split('\n')
				.filter((line) => /^```([\w+-]*)\s*$/.test(line)).length;
			if (fenceCount % 2 === 1) text += '\n```';
		}
		const lines = text.split('\n');
		const blocks: Block[] = [];
		let i = 0;
		while (i < lines.length) {
			const fence = lines[i].match(/^```([\w+-]*)\s*$/);
			if (fence) {
				const lang = fence[1] || 'text';
				const codeLines: string[] = [];
				i++;
				while (i < lines.length && !/^```\s*$/.test(lines[i])) {
					codeLines.push(lines[i]);
					i++;
				}
				i++; // skip closing fence
				blocks.push({ type: 'code', html: '', code: codeLines.join('\n'), lang });
				continue;
			}

			const textLines: string[] = [];
			while (i < lines.length) {
				// A complete fence starts a new code block; a partial one (e.g.
				// "```pyt" cut mid-stream) falls through as plain text so the
				// renderer never spins on it.
				if (/^```([\w+-]*)\s*$/.test(lines[i])) break;
				if (lines[i].trim() === '') {
					if (textLines.length > 0) break;
					i++;
					continue;
				}
				textLines.push(lines[i]);
				i++;
			}
			if (textLines.length > 0) blocks.push({ type: 'text', html: renderTextLines(textLines) });
		}
		return blocks;
	}

	const blocks = $derived(parseMarkdown(content));

	function codeLines(code: string): string[] {
		return code.split('\n');
	}

	async function copyCode(index: number, code: string) {
		try {
			await navigator.clipboard.writeText(code);
		} catch {
			// fallback for webviews without clipboard permission
			const ta = document.createElement('textarea');
			ta.value = code;
			ta.style.position = 'fixed';
			ta.style.opacity = '0';
			document.body.appendChild(ta);
			ta.select();
			document.execCommand('copy');
			document.body.removeChild(ta);
		}
		copiedIndex = index;
		setTimeout(() => {
			if (copiedIndex === index) copiedIndex = null;
		}, 1500);
	}
</script>

<div class="markdown-viewer">
	{#if !(streaming && content.trim() === '')}
		{#each blocks as block, i}
		{#if block.type === 'code'}
			<div class="code-block">
				<div class="code-header">
					<span class="code-lang">{block.lang}</span>
					<button class="copy-btn" onclick={() => copyCode(i, block.code ?? '')}>
						{copiedIndex === i ? '已复制' : '复制'}
					</button>
				</div>
				<pre class="code-body"><code>{#each codeLines(block.code ?? '') as line, li}<span class="code-line"><span class="line-num">{li + 1}</span><span class="line-content">{line}</span></span>{/each}</code></pre>
			</div>
		{:else}
			{@html block.html}
		{/if}
		{/each}
	{/if}
</div>

<style>
	.markdown-viewer {
		font-size: var(--text-subheadline);
		line-height: 1.6;
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
	.markdown-viewer :global(h4) {
		margin: 0.8em 0 0.4em;
		font-weight: var(--font-weight-semibold);
		color: var(--color-fg);
	}
	.markdown-viewer :global(h1) { font-size: 1.35em; }
	.markdown-viewer :global(h2) { font-size: 1.2em; }
	.markdown-viewer :global(h3) { font-size: 1.1em; }
	.markdown-viewer :global(h4) { font-size: 1em; }

	.markdown-viewer :global(ul),
	.markdown-viewer :global(ol) {
		margin: 0.4em 0;
		padding-left: 1.4em;
	}
	.markdown-viewer :global(li) {
		margin: 0.2em 0;
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
		padding: 0.3em 0.8em;
		border-left: 3px solid var(--color-accent);
		background: var(--color-bg-secondary);
		border-radius: var(--radius-sm);
		color: var(--color-fg-secondary);
	}

	.markdown-viewer :global(code.inline-code) {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-separator);
		border-radius: 4px;
		padding: 0.1em 0.35em;
		font-family: var(--font-mono);
		font-size: 0.9em;
	}

	.code-block {
		margin: 0.6em 0;
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		overflow: hidden;
		background: var(--color-bg-secondary);
	}

	.code-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 6px 12px;
		background: rgba(127, 127, 127, 0.08);
		border-bottom: 1px solid var(--color-separator);
	}

	.code-lang {
		font-size: var(--text-caption1);
		font-family: var(--font-mono);
		color: var(--color-fg-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.copy-btn {
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: var(--text-caption1);
		cursor: pointer;
		padding: 2px 8px;
		border-radius: 4px;
	}
	.copy-btn:hover {
		background: rgba(127, 127, 127, 0.12);
		color: var(--color-fg);
	}

	.code-body {
		margin: 0;
		padding: 10px 12px;
		overflow-x: auto;
	}

	.code-line {
		display: block;
	}

	.line-num {
		display: inline-block;
		width: 2.2em;
		text-align: right;
		margin-right: 1em;
		color: var(--color-fg-tertiary);
		user-select: none;
	}

	.line-content {
		white-space: pre;
		font-family: var(--font-mono);
		font-size: var(--text-footnote);
		color: var(--color-fg);
	}
</style>
