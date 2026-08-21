<script lang="ts">
	import { composeStore, type ComposeTask, type ComposeStage } from '$lib/stores/compose.svelte';

	let specExpanded = $state(true);
	let reviewExpanded = $state(true);

	const stageLabels: Record<ComposeStage, { label: string; icon: string }> = {
		idle: { label: '空闲', icon: '○' },
		brainstorming: { label: '思考中', icon: '💭' },
		designing: { label: '设计中', icon: '📐' },
		implementing: { label: '实现中', icon: '⚙️' },
		verifying: { label: '验证中', icon: '🔍' },
		reviewing: { label: '评审中', icon: '📝' },
		completed: { label: '已完成', icon: '✅' },
		failed: { label: '失败', icon: '❌' }
	};

	function taskStatusColor(status: string): string {
		switch (status) {
			case 'completed': return 'var(--color-green)';
			case 'running': return 'var(--color-accent)';
			case 'failed': return 'var(--color-red)';
			default: return 'var(--color-muted)';
		}
	}

	function taskStatusLabel(status: string): string {
		switch (status) {
			case 'completed': return '已完成';
			case 'running': return '运行中';
			case 'failed': return '失败';
			case 'pending': return '待处理';
			default: return status;
		}
	}

	function handleStop() {
		composeStore.stopCompose();
	}

	function handlePause() {
		if (composeStore.paused) {
			composeStore.resumeCompose();
		} else {
			composeStore.pauseCompose();
		}
	}
</script>

{#if !composeStore.session || composeStore.stage === 'idle'}
	<div class="empty-state">
		<div class="empty-icon">
			<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
				<path d="M12 2L2 7l10 5 10-5-10-5z"/>
				<path d="M2 17l10 5 10-5"/>
				<path d="M2 12l10 5 10-5"/>
			</svg>
		</div>
		<span class="empty-title">暂无 Compose 会话</span>
		<span class="empty-desc">在输入框输入 /compose 开始编排模式</span>
	</div>
{:else}
	<div class="compose-panel">
		<!-- Stage indicator -->
		<div class="stage-section">
			<div class="stage-indicator" class:active={composeStore.active}>
				<span class="stage-icon">{stageLabels[composeStore.stage].icon}</span>
				<span class="stage-label">{stageLabels[composeStore.stage].label}</span>
				{#if composeStore.paused}
					<span class="paused-badge">已暂停</span>
				{/if}
			</div>
		</div>

		<!-- Progress bar -->
		{#if composeStore.totalTaskCount() > 0}
			<div class="progress-section">
				<div class="progress-info">
					<span class="progress-label">进度</span>
					<span class="progress-count">
						{composeStore.completedTaskCount()} / {composeStore.totalTaskCount()}
					</span>
				</div>
				<div class="progress-bar-track">
					<div
						class="progress-bar-fill"
						style:width="{composeStore.progressPercent()}%"
					></div>
				</div>
			</div>
		{/if}

		<!-- User request -->
		{#if composeStore.session}
			<div class="request-section">
				<span class="section-title">请求</span>
				<p class="request-text">{composeStore.session.user_request}</p>
			</div>
		{/if}

		<!-- Spec summary -->
		{#if composeStore.session?.spec}
			<div class="section">
				<button
					class="section-header"
					onclick={() => (specExpanded = !specExpanded)}
				>
					<span class="section-title">规格文档</span>
					<span class="chevron">{specExpanded ? '▾' : '▸'}</span>
				</button>
				{#if specExpanded}
					<div class="spec-content">
						<p class="spec-summary">{composeStore.session.spec.summary}</p>
						{#if composeStore.session.spec.tasks.length > 0}
							<span class="spec-task-count">
								{composeStore.session.spec.tasks.length} 个任务
							</span>
						{/if}
					</div>
				{/if}
			</div>
		{/if}

		<!-- Task list -->
		{#if composeStore.session?.tasks && composeStore.session.tasks.length > 0}
			<div class="section">
				<div class="section-header static">
					<span class="section-title">任务列表</span>
					<span class="section-count">{composeStore.session.tasks.length}</span>
				</div>
				<div class="task-list">
					{#each composeStore.session.tasks as task (task.id)}
						<div class="task-item">
							<div class="task-header">
								<span
									class="task-status-dot"
									style:background={taskStatusColor(task.status)}
								></span>
								<span class="task-desc">{task.description}</span>
							</div>
							<div class="task-meta">
								<span class="task-status-label" style:color={taskStatusColor(task.status)}>
									{taskStatusLabel(task.status)}
								</span>
								{#if task.error}
									<span class="task-error">{task.error}</span>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<!-- Review results -->
		{#if composeStore.session?.review}
			<div class="section">
				<button
					class="section-header"
					onclick={() => (reviewExpanded = !reviewExpanded)}
				>
					<span class="section-title">评审结果</span>
					{#if composeStore.session.review.ready_to_merge}
						<span class="ready-badge">可合并</span>
					{/if}
					<span class="chevron">{reviewExpanded ? '▾' : '▸'}</span>
				</button>
				{#if reviewExpanded}
					<div class="review-content">
						{#if composeStore.session.review.critical.length > 0}
							<div class="review-group">
								<span class="review-label critical">严重</span>
								{#each composeStore.session.review.critical as issue}
									<div class="review-item">{issue}</div>
								{/each}
							</div>
						{/if}
						{#if composeStore.session.review.important.length > 0}
							<div class="review-group">
								<span class="review-label important">重要</span>
								{#each composeStore.session.review.important as issue}
									<div class="review-item">{issue}</div>
								{/each}
							</div>
						{/if}
						{#if composeStore.session.review.minor.length > 0}
							<div class="review-group">
								<span class="review-label minor">建议</span>
								{#each composeStore.session.review.minor as issue}
									<div class="review-item">{issue}</div>
								{/each}
							</div>
						{/if}
					</div>
				{/if}
			</div>
		{/if}

		<!-- Error -->
		{#if composeStore.error}
			<div class="error-section">
				<span class="error-text">{composeStore.error}</span>
			</div>
		{/if}

		<!-- Action buttons -->
		{#if composeStore.active}
			<div class="actions">
				<button class="action-btn pause" onclick={handlePause}>
					{composeStore.paused ? '继续' : '暂停'}
				</button>
				<button class="action-btn stop" onclick={handleStop}>
					停止
				</button>
			</div>
		{/if}
	</div>
{/if}

<style>
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
		padding: 40px 16px;
		text-align: center;
	}

	.empty-icon {
		color: var(--color-fg-tertiary);
		opacity: 0.5;
	}

	.empty-title {
		font-size: 14px;
		font-weight: 600;
		color: var(--color-fg-secondary);
	}

	.empty-desc {
		font-size: 12px;
		color: var(--color-fg-tertiary);
	}

	.compose-panel {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	/* ── Stage ─────────────────────────────── */

	.stage-section {
		padding: 10px 12px;
		border-radius: var(--radius-md);
		background: var(--glass-solid-bg, var(--color-bg));
		backdrop-filter: var(--glass-solid-blur, blur(16px));
		border: 1px solid var(--color-separator);
	}

	.stage-indicator {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 14px;
		font-weight: 600;
		color: var(--color-fg);
	}

	.stage-indicator.active {
		color: var(--color-accent);
	}

	.stage-icon {
		font-size: 16px;
	}

	.paused-badge {
		margin-left: auto;
		padding: 2px 8px;
		border-radius: 10px;
		background: var(--color-muted);
		color: var(--color-fg-secondary);
		font-size: 11px;
		font-weight: 500;
	}

	/* ── Progress ──────────────────────────── */

	.progress-section {
		padding: 10px 12px;
		border-radius: var(--radius-md);
		background: var(--glass-solid-bg, var(--color-bg));
		backdrop-filter: var(--glass-solid-blur, blur(16px));
		border: 1px solid var(--color-separator);
	}

	.progress-info {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 8px;
	}

	.progress-label {
		font-size: 12px;
		color: var(--color-fg-secondary);
		font-weight: 500;
	}

	.progress-count {
		font-size: 12px;
		color: var(--color-fg-tertiary);
		font-family: var(--font-mono);
	}

	.progress-bar-track {
		height: 4px;
		border-radius: 2px;
		background: var(--color-separator);
		overflow: hidden;
	}

	.progress-bar-fill {
		height: 100%;
		border-radius: 2px;
		background: var(--color-accent);
		transition: width 0.3s ease;
	}

	/* ── Request ───────────────────────────── */

	.request-section {
		padding: 10px 12px;
		border-radius: var(--radius-md);
		background: var(--glass-solid-bg, var(--color-bg));
		backdrop-filter: var(--glass-solid-blur, blur(16px));
		border: 1px solid var(--color-separator);
	}

	.request-text {
		margin: 6px 0 0;
		font-size: 13px;
		color: var(--color-fg);
		line-height: 1.5;
		word-break: break-word;
	}

	/* ── Sections ──────────────────────────── */

	.section {
		border-radius: var(--radius-md);
		background: var(--glass-solid-bg, var(--color-bg));
		backdrop-filter: var(--glass-solid-blur, blur(16px));
		border: 1px solid var(--color-separator);
		overflow: hidden;
	}

	.section-header {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 10px 12px;
		border: none;
		background: transparent;
		cursor: pointer;
		color: var(--color-fg);
		font-size: 13px;
		font-weight: 600;
		text-align: left;
		transition: background 0.15s ease;
	}

	.section-header:hover {
		background: var(--color-bg-tertiary);
	}

	.section-header.static {
		cursor: default;
	}

	.section-title {
		flex: 1;
		font-size: 12px;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.3px;
	}

	.section-count {
		padding: 1px 6px;
		border-radius: 8px;
		background: var(--color-separator);
		font-size: 11px;
		color: var(--color-fg-tertiary);
		font-weight: 500;
	}

	.chevron {
		font-size: 10px;
		color: var(--color-fg-tertiary);
	}

	.ready-badge {
		padding: 2px 8px;
		border-radius: 10px;
		background: var(--color-green);
		color: #fff;
		font-size: 11px;
		font-weight: 500;
	}

	/* ── Spec ──────────────────────────────── */

	.spec-content {
		padding: 0 12px 10px;
	}

	.spec-summary {
		margin: 0;
		font-size: 13px;
		color: var(--color-fg);
		line-height: 1.5;
	}

	.spec-task-count {
		display: inline-block;
		margin-top: 6px;
		font-size: 12px;
		color: var(--color-fg-tertiary);
	}

	/* ── Tasks ─────────────────────────────── */

	.task-list {
		max-height: 300px;
		overflow-y: auto;
	}

	.task-item {
		padding: 8px 12px;
		border-top: 1px solid var(--color-separator);
	}

	.task-header {
		display: flex;
		align-items: flex-start;
		gap: 8px;
	}

	.task-status-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		flex-shrink: 0;
		margin-top: 4px;
	}

	.task-desc {
		font-size: 13px;
		color: var(--color-fg);
		line-height: 1.4;
	}

	.task-meta {
		margin-left: 16px;
		margin-top: 4px;
	}

	.task-status-label {
		font-size: 11px;
		font-weight: 500;
	}

	.task-error {
		display: block;
		margin-top: 2px;
		font-size: 11px;
		color: var(--color-red);
	}

	/* ── Review ────────────────────────────── */

	.review-content {
		padding: 0 12px 10px;
	}

	.review-group {
		margin-top: 8px;
	}

	.review-group:first-child {
		margin-top: 0;
	}

	.review-label {
		display: inline-block;
		padding: 1px 6px;
		border-radius: 6px;
		font-size: 11px;
		font-weight: 600;
		margin-bottom: 4px;
	}

	.review-label.critical {
		background: rgba(239, 68, 68, 0.15);
		color: var(--color-red);
	}

	.review-label.important {
		background: rgba(245, 158, 11, 0.15);
		color: var(--color-accent);
	}

	.review-label.minor {
		background: var(--color-separator);
		color: var(--color-fg-tertiary);
	}

	.review-item {
		padding: 4px 8px;
		font-size: 12px;
		color: var(--color-fg);
		line-height: 1.4;
		border-left: 2px solid var(--color-separator);
		margin-left: 4px;
	}

	/* ── Error ─────────────────────────────── */

	.error-section {
		padding: 10px 12px;
		border-radius: var(--radius-md);
		background: rgba(239, 68, 68, 0.08);
		border: 1px solid rgba(239, 68, 68, 0.2);
	}

	.error-text {
		font-size: 13px;
		color: var(--color-red);
		line-height: 1.4;
	}

	/* ── Actions ───────────────────────────── */

	.actions {
		display: flex;
		gap: 8px;
		padding-top: 4px;
	}

	.action-btn {
		flex: 1;
		padding: 8px 12px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg-secondary);
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
	}

	.action-btn:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-fg);
	}

	.action-btn.stop {
		border-color: rgba(239, 68, 68, 0.3);
		color: var(--color-red);
	}

	.action-btn.stop:hover {
		background: rgba(239, 68, 68, 0.08);
		border-color: rgba(239, 68, 68, 0.5);
	}

	.action-btn.pause {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: #fff;
	}

	.action-btn.pause:hover {
		background: var(--color-accent-hover);
	}
</style>
