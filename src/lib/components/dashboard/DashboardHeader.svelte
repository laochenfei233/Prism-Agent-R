<script lang="ts">
	let { agentCount = 0, onSearch }: {
		agentCount?: number;
		onSearch?: (query: string) => void;
	} = $props();

	let searchQuery = $state('');

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && onSearch) {
			onSearch(searchQuery);
		}
	}
</script>

<header class="dashboard-header">
	<div class="greeting">
		<h1>Prism Agent</h1>
		<p>{agentCount > 0 ? `已配置 ${agentCount} 个 Agent，开始对话吧` : '欢迎使用 Prism Agent'}</p>
	</div>
	<div class="search-box">
		<svg class="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
		</svg>
		<input
			type="text"
			placeholder="搜索 Agent、会话、技能..."
			bind:value={searchQuery}
			onkeydown={handleKeydown}
		/>
	</div>
</header>

<style>
	.dashboard-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 20px 24px 16px;
		gap: 16px;
	}

	.greeting h1 {
		font-size: var(--text-title1);
		font-weight: 700;
		color: var(--color-fg);
		margin: 0;
		letter-spacing: -0.5px;
	}

	.greeting p {
		font-size: var(--text-subheadline);
		color: var(--color-fg-secondary);
		margin: 2px 0 0;
	}

	.search-box {
		position: relative;
		width: 280px;
		flex-shrink: 0;
	}

	.search-icon {
		position: absolute;
		left: 12px;
		top: 50%;
		transform: translateY(-50%);
		color: var(--color-fg-tertiary);
		pointer-events: none;
	}

	input {
		width: 100%;
		padding: 8px 12px 8px 36px;
		border-radius: var(--radius-md);
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: var(--text-subheadline);
		outline: none;
		transition: border-color 0.15s ease;
	}

	input:focus {
		border-color: var(--color-accent);
		background: var(--color-bg);
	}

	input::placeholder {
		color: var(--color-fg-tertiary);
	}
</style>
