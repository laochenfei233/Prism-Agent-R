<script lang="ts">
	import type { KanbanCard, KanbanData } from '$lib/stores/dashboard.svelte';
	import type { TaskItem } from '$lib/api';

	interface Props {
		data: KanbanData | null;
		tasks?: TaskItem[];
		onStartChat: (agentId: string) => void;
		onCreateAgent: () => void;
		onDeleteAgent?: (agentId: string) => void;
	}

	let { data, tasks = [], onStartChat, onCreateAgent, onDeleteAgent }: Props = $props();

	const agentColumns = [
		{ key: 'idle' as const, title: '空闲', dotVar: 'var(--color-muted)', bgColor: 'rgba(128, 128, 128, 0.06)', borderColor: 'rgba(128, 128, 128, 0.15)' },
		{ key: 'running' as const, title: '运行中', dotVar: 'var(--color-green)', bgColor: 'rgba(52, 199, 89, 0.06)', borderColor: 'rgba(52, 199, 89, 0.18)' },
		{ key: 'done' as const, title: '已完成', dotVar: 'var(--color-accent)', bgColor: 'rgba(0, 122, 255, 0.06)', borderColor: 'rgba(0, 122, 255, 0.18)' },
		{ key: 'failed' as const, title: '失败', dotVar: 'var(--color-red)', bgColor: 'rgba(255, 59, 48, 0.06)', borderColor: 'rgba(255, 59, 48, 0.18)' },
	];

	const taskColumns = [
		{ key: 'todo' as const, title: '待处理', dotVar: 'var(--color-muted)', bgColor: 'rgba(128, 128, 128, 0.06)', borderColor: 'rgba(128, 128, 128, 0.15)' },
		{ key: 'doing' as const, title: '进行中', dotVar: 'var(--color-accent)', bgColor: 'rgba(0, 122, 255, 0.06)', borderColor: 'rgba(0, 122, 255, 0.18)' },
		{ key: 'done' as const, title: '已完成', dotVar: 'var(--color-green)', bgColor: 'rgba(52, 199, 89, 0.06)', borderColor: 'rgba(52, 199, 89, 0.18)' },
	];

	function cards(colKey: 'idle' | 'running' | 'done' | 'failed'): KanbanCard[] {
		return data?.[colKey] ?? [];
	}

	function tasksByStatus(status: string): TaskItem[] {
		return tasks.filter(t => t.status === status);
	}

	function formatRelative(ts: number | null): string {
		if (!ts) return '从未使用';
		const diff = Date.now() - ts;
		const mins = Math.floor(diff / 60_000);
		if (mins < 1) return '刚刚';
		if (mins < 60) return `${mins} 分钟前`;
		const hours = Math.floor(mins / 60);
		if (hours < 24) return `${hours} 小时前`;
		const days = Math.floor(hours / 24);
		return `${days} 天前`;
	}
</script>

<div class="glass-container">
	<div class="header-row">
		<h2 class="header-title">Agents</h2>
		<button class="new-btn" onclick={onCreateAgent}>
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
				<line x1="12" y1="5" x2="12" y2="19" />
				<line x1="5" y1="12" x2="19" y2="12" />
			</svg>
			新建 Agent
		</button>
	</div>

	<div class="board">
		{#each agentColumns as col (col.key)}
			<div class="column" style="background: {col.bgColor}; border: 1px solid {col.borderColor}; border-radius: var(--radius-md); padding: 12px;">
				<div class="column-header">
					<span class="col-dot" style="background: {col.dotVar}"></span>
					<span class="col-title">{col.title}</span>
					<span class="col-count" style="color: {col.dotVar}; background: {col.bgColor};">{cards(col.key).length}</span>
				</div>

				<div class="column-body">
					{#each cards(col.key) as card (card.agent_id)}
						<div class="card">
							<div class="card-top">
								<div class="avatar">
									{#if card.agent_avatar}
										<img src={card.agent_avatar} alt={card.agent_name} />
									{:else}
										<span class="avatar-fallback">{card.agent_name.charAt(0)}</span>
									{/if}
								</div>
								<div class="card-info">
									<span class="agent-name">{card.agent_name}</span>
									{#if card.model_name}
										<span class="tag model-tag">{card.model_name}</span>
									{/if}
								</div>
								{#if onDeleteAgent}
									<button
										class="menu-btn"
										title="删除"
										onclick={(e) => { e.stopPropagation(); if (confirm(`确定删除 Agent「${card.agent_name}」？此操作不可撤销。`)) { onDeleteAgent(card.agent_id); } }}
									>
										<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
											<polyline points="3 6 5 6 21 6" />
											<path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
											<line x1="10" y1="11" x2="10" y2="17" />
											<line x1="14" y1="11" x2="14" y2="17" />
										</svg>
									</button>
								{/if}
							</div>

							{#if card.session_title}
								<span class="session-title">{card.session_title}</span>
							{/if}

							<div class="meta-row">
								<span class="last-used">{formatRelative(card.session_updated_at)}</span>
								{#if card.message_count > 0}
									<span class="msg-count">{card.message_count} 条消息</span>
								{/if}
								{#if card.lifecycle === 'Paused'}
									<span class="paused-badge">已暂停</span>
								{/if}
							</div>

							<button class="start-btn" onclick={(e) => { e.stopPropagation(); onStartChat(card.agent_id); }}>
								对话
							</button>
						</div>
					{:else}
						<div class="empty-col">暂无</div>
					{/each}
				</div>
			</div>
		{/each}
	</div>

	<!-- Agent-managed Tasks Board -->
	{#if tasks.length > 0}
		<div class="tasks-section">
			<div class="tasks-header">
				<h3 class="tasks-title">Tasks</h3>
				<span class="tasks-count">{tasks.length} 个任务</span>
			</div>
			<div class="tasks-board">
				{#each taskColumns as col (col.key)}
					<div class="task-column" style="background: {col.bgColor}; border: 1px solid {col.borderColor}; border-radius: var(--radius-md); padding: 12px;">
						<div class="column-header">
							<span class="col-dot" style="background: {col.dotVar}"></span>
							<span class="col-title">{col.title}</span>
							<span class="col-count" style="color: {col.dotVar}; background: {col.bgColor};">{tasksByStatus(col.key).length}</span>
						</div>
						<div class="task-col-body">
							{#each tasksByStatus(col.key) as task (task.id)}
								<div class="task-card">
									<div class="task-subject">{task.subject}</div>
									{#if task.owner}
										<div class="task-owner">@{task.owner}</div>
									{/if}
									<div class="task-time">{formatRelative(task.updated_at)}</div>
								</div>
							{:else}
								<div class="empty-col">暂无</div>
							{/each}
						</div>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</div>

<style>
	/* ── Glass container ──────────────────────────── */
	.glass-container {
		background: var(--glass-solid-bg);
		backdrop-filter: var(--glass-solid-blur);
		-webkit-backdrop-filter: var(--glass-solid-blur);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--glass-edge-highlight), var(--shadow-sm);
		padding: 20px;
	}

	.header-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
	}
	.header-title {
		font-size: 15px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}
	.new-btn {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 5px 12px;
		border: 1px solid var(--color-separator);
		border-radius: 8px;
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: var(--text-caption1);
		font-weight: 500;
		cursor: pointer;
		transition: border-color 0.15s ease, color 0.15s ease;
	}
	.new-btn:hover {
		border-color: var(--color-accent);
		color: var(--color-accent);
	}

	/* ── Board (4 columns) ────────────────────────── */
	.board {
		display: flex;
		gap: 12px;
		overflow-x: auto;
	}

	.column {
		flex: 1;
		min-width: 220px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.column-header {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 0 4px;
	}
	.col-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		flex-shrink: 0;
	}
	.col-title {
		font-size: var(--text-caption1);
		font-weight: 600;
		color: var(--color-fg-secondary);
	}
	.col-count {
		font-size: var(--text-caption2);
		color: var(--color-muted);
		background: color-mix(in srgb, var(--color-muted) 12%, transparent);
		padding: 1px 7px;
		border-radius: 9999px;
	}

	.column-body {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	/* ── Card ─────────────────────────────────────── */
	.card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		padding: 12px;
		display: flex;
		flex-direction: column;
		gap: 8px;
		transition: transform 0.15s var(--ease-default), box-shadow 0.15s ease;
	}
	.card:hover {
		transform: translateY(-2px);
		box-shadow: var(--shadow-md);
	}

	.card-top {
		display: flex;
		align-items: flex-start;
		gap: 10px;
	}

	.avatar {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		overflow: hidden;
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.avatar img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.avatar-fallback {
		font-size: 16px;
		font-weight: 600;
		color: var(--color-accent);
	}

	.card-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.agent-name {
		font-size: 14px;
		font-weight: 600;
		color: var(--color-fg);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.menu-btn {
		width: 24px;
		height: 24px;
		display: flex;
		align-items: center;
		justify-content: center;
		border: none;
		background: transparent;
		color: var(--color-muted);
		border-radius: 6px;
		cursor: pointer;
		transition: color 0.15s ease, background 0.15s ease;
		flex-shrink: 0;
	}
	.menu-btn:hover {
		color: var(--color-fg);
		background: color-mix(in srgb, var(--color-fg) 8%, transparent);
	}

	/* ── Tags ──────────────────────────────────────── */
	.tag {
		padding: 2px 8px;
		border-radius: 9999px;
		font-size: var(--text-caption2);
		font-weight: 500;
		display: inline-block;
		width: fit-content;
	}
	.model-tag {
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
		color: var(--color-accent);
	}

	/* ── Session info ──────────────────────────────── */
	.session-title {
		font-size: var(--text-caption2);
		color: var(--color-fg-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta-row {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.last-used {
		font-size: var(--text-caption2);
		color: var(--color-muted);
	}
	.msg-count {
		font-size: var(--text-caption2);
		color: var(--color-fg-secondary);
	}
	.paused-badge {
		font-size: var(--text-caption2);
		color: var(--color-red);
		background: color-mix(in srgb, var(--color-red) 12%, transparent);
		padding: 1px 7px;
		border-radius: 9999px;
	}

	/* ── Start button ──────────────────────────────── */
	.start-btn {
		background: var(--color-accent);
		color: #fff;
		border: none;
		border-radius: 9999px;
		padding: 5px 14px;
		font-size: var(--text-caption1);
		font-weight: 600;
		cursor: pointer;
		align-self: flex-start;
		transition: opacity 0.15s ease;
	}
	.start-btn:hover {
		opacity: 0.85;
	}

	/* ── Empty column ──────────────────────────────── */
	.empty-col {
		font-size: var(--text-caption2);
		color: var(--color-muted);
		text-align: center;
		padding: 16px 8px;
	}

	/* ── Tasks Section ─────────────────────────────── */
	.tasks-section {
		margin-top: 16px;
		border-top: 1px solid var(--color-separator);
		padding-top: 16px;
	}

	.tasks-header {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 12px;
	}
	.tasks-title {
		font-size: 15px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}
	.tasks-count {
		font-size: var(--text-caption2);
		color: var(--color-muted);
		background: color-mix(in srgb, var(--color-muted) 12%, transparent);
		padding: 1px 7px;
		border-radius: 9999px;
	}

	.tasks-board {
		display: flex;
		gap: 12px;
		overflow-x: auto;
	}

	.task-column {
		flex: 1;
		min-width: 180px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.task-col-body {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.task-card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-sm);
		padding: 10px 12px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.task-subject {
		font-size: 13px;
		font-weight: 500;
		color: var(--color-fg);
		line-height: 1.4;
	}

	.task-owner {
		font-size: 12px;
		color: var(--color-fg-secondary);
	}

	.task-time {
		font-size: 12px;
		color: var(--color-muted);
	}

	/* ── Responsive ────────────────────────────────── */
	@media (max-width: 600px) {
		.board, .tasks-board {
			overflow-x: auto;
		}
		.column, .task-column {
			min-width: 200px;
		}
	}
</style>
