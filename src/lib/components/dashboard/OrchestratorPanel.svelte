<script lang="ts">
	import { orchestratorStore } from '$lib/stores/orchestrator.svelte';

	const session = $derived(orchestratorStore.session);
	const events = $derived(orchestratorStore.events);
	const loading = $derived(orchestratorStore.loading);
	const error = $derived(orchestratorStore.error);

	let userInput = $state('');
	let activeTab = $state<'input' | 'spec' | 'execution' | 'review'>('input');

	async function startOrchestration() {
		if (!userInput.trim()) return;
		const s = await orchestratorStore.startSession(userInput.trim());
		if (s) {
			activeTab = 'spec';
		}
	}

	function statusLabel(status: string): string {
		switch (status) {
			case 'spec_generating': return '正在分析需求...';
			case 'spec_reviewing': return '等待确认 SPEC';
			case 'plan_generating': return '正在生成执行计划...';
			case 'executing': return '正在执行任务';
			case 'reviewing': return '正在审查结果';
			case 'repairing': return '正在修复失败任务';
			case 'completed': return '全部完成';
			case 'paused': return '已暂停';
			case 'budget_exhausted': return '预算耗尽';
			case 'failed': return '执行失败';
			default: return status;
		}
	}

	function statusColor(status: string): string {
		switch (status) {
			case 'completed': return 'text-emerald-400 bg-emerald-400/10';
			case 'executing': return 'text-blue-400 bg-blue-400/10';
			case 'reviewing': return 'text-yellow-400 bg-yellow-400/10';
			case 'failed': case 'budget_exhausted': return 'text-red-400 bg-red-400/10';
			default: return 'text-slate-400 bg-slate-400/10';
		}
	}

	function complexityColor(c: string): string {
		switch (c) {
			case 'high': return 'text-red-400';
			case 'medium': return 'text-yellow-400';
			default: return 'text-emerald-400';
		}
	}

	function formatTime(ts: number): string {
		const d = new Date(ts);
		return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
	}
</script>

<div class="orchestrator-panel h-full flex flex-col">
	<!-- Header -->
	<div class="flex items-center justify-between p-4 border-b border-slate-700/50">
		<h2 class="text-lg font-semibold text-slate-100">自主编排</h2>
		{#if session}
			<div class="flex items-center gap-2">
				<span class="inline-flex items-center px-2 py-1 rounded text-xs font-medium {statusColor(session.status)}">
					{statusLabel(session.status)}
				</span>
				<button
					class="text-xs text-slate-400 hover:text-slate-200 transition-colors"
					onclick={() => { orchestratorStore.reset(); activeTab = 'input'; }}
				>
					新建
				</button>
			</div>
		{/if}
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-hidden">
		{#if !session}
			<!-- Input View -->
			<div class="h-full flex flex-col items-center justify-center p-8">
				<div class="w-full max-w-2xl space-y-6">
					<div class="text-center space-y-2">
						<h3 class="text-xl font-semibold text-slate-100">描述你的需求</h3>
						<p class="text-sm text-slate-400">输入模糊需求，AI 将自动生成计划、分配 Agent 并执行</p>
					</div>

					<textarea
						class="w-full h-32 p-4 rounded-lg bg-slate-800/50 border border-slate-700/50 text-slate-200 text-sm placeholder-slate-500 resize-none focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500/50"
						placeholder="例如：帮我实现一个用户认证系统，包含登录、注册、JWT token 刷新、权限中间件"
						bind:value={userInput}
						onkeydown={(e) => {
							if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
								startOrchestration();
							}
						}}
					></textarea>

					{#if error}
						<div class="text-sm text-red-400 bg-red-400/10 rounded p-3">{error}</div>
					{/if}

					<button
						class="w-full py-3 rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
						onclick={startOrchestration}
						disabled={loading || !userInput.trim()}
					>
						{loading ? '分析中...' : '开始编排'}
					</button>

					<p class="text-xs text-slate-500 text-center">
						按 Ctrl+Enter 快速开始
					</p>
				</div>
			</div>
		{:else}
			<!-- Tabs -->
			<div class="flex border-b border-slate-700/50">
				<button
					class="px-4 py-2 text-xs font-medium transition-colors"
					class:text-blue-400={activeTab === 'input'}
					class:text-slate-400={activeTab !== 'input'}
					class:hover:text-slate-200={activeTab !== 'input'}
					class:border-b-2={activeTab === 'input'}
					class:border-blue-400={activeTab === 'input'}
					onclick={() => activeTab = 'input'}
				>
					输入
				</button>
				<button
					class="px-4 py-2 text-xs font-medium transition-colors"
					class:text-blue-400={activeTab === 'spec'}
					class:text-slate-400={activeTab !== 'spec'}
					class:hover:text-slate-200={activeTab !== 'spec'}
					class:border-b-2={activeTab === 'spec'}
					class:border-blue-400={activeTab === 'spec'}
					onclick={() => activeTab = 'spec'}
				>
					SPEC
				</button>
				<button
					class="px-4 py-2 text-xs font-medium transition-colors"
					class:text-blue-400={activeTab === 'execution'}
					class:text-slate-400={activeTab !== 'execution'}
					class:hover:text-slate-200={activeTab !== 'execution'}
					class:border-b-2={activeTab === 'execution'}
					class:border-blue-400={activeTab === 'execution'}
					onclick={() => activeTab = 'execution'}
				>
					执行
				</button>
				<button
					class="px-4 py-2 text-xs font-medium transition-colors"
					class:text-blue-400={activeTab === 'review'}
					class:text-slate-400={activeTab !== 'review'}
					class:hover:text-slate-200={activeTab !== 'review'}
					class:border-b-2={activeTab === 'review'}
					class:border-blue-400={activeTab === 'review'}
					onclick={() => activeTab = 'review'}
				>
					审查
				</button>
			</div>

			<!-- Tab Content -->
			<div class="h-full overflow-y-auto p-4">
				{#if activeTab === 'input'}
					<div class="space-y-4">
						<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4">
							<h4 class="text-sm font-medium text-slate-300 mb-2">需求</h4>
							<p class="text-sm text-slate-200 whitespace-pre-wrap">{session.user_request}</p>
						</div>
						<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4">
							<h4 class="text-sm font-medium text-slate-300 mb-2">进度</h4>
							<div class="text-xs text-slate-400 space-y-1">
								<div>循环次数: {session.cycle_count} / {session.max_cycles}</div>
								<div>状态: {statusLabel(session.status)}</div>
								<div>事件数: {events.length}</div>
							</div>
						</div>
					</div>

				{:else if activeTab === 'spec'}
					{#if session.spec}
						<div class="space-y-4">
							<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4">
								<h4 class="text-sm font-medium text-slate-300 mb-2">需求摘要</h4>
								<p class="text-sm text-slate-200">{session.spec.summary}</p>
							</div>

							<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4">
								<h4 class="text-sm font-medium text-slate-300 mb-3">任务清单</h4>
								<div class="space-y-2">
									{#each session.spec.tasks as task (task.id)}
										<div class="flex items-start gap-3 p-3 rounded bg-slate-900/50">
											<span class="text-xs font-mono text-slate-500 mt-0.5">{task.id}</span>
											<div class="flex-1 min-w-0">
												<div class="flex items-center gap-2">
													<span class="text-sm text-slate-200">{task.title}</span>
													<span class="text-[10px] font-medium {complexityColor(task.estimated_complexity)}">
														{task.estimated_complexity}
													</span>
												</div>
												<p class="text-xs text-slate-400 mt-1">{task.description}</p>
												{#if task.acceptance.length > 0}
													<div class="mt-2 text-[10px] text-slate-500">
														验收: {task.acceptance.join(' | ')}
													</div>
												{/if}
											</div>
										</div>
									{/each}
								</div>
							</div>

							{#if Object.keys(session.spec.dependencies).length > 0}
								<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4">
									<h4 class="text-sm font-medium text-slate-300 mb-2">依赖关系</h4>
									<div class="text-xs text-slate-400 space-y-1">
										{#each Object.entries(session.spec.dependencies) as [taskId, deps]}
											<div>{taskId} → {deps.join(', ')}</div>
										{/each}
									</div>
								</div>
							{/if}
						</div>
					{:else}
						<div class="text-sm text-slate-500 text-center py-8">
							{session.status === 'spec_generating' ? '正在生成 SPEC...' : 'SPEC 尚未生成'}
						</div>
					{/if}

				{:else if activeTab === 'execution'}
					{#if session.plan}
						<div class="space-y-4">
							<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4">
								<h4 class="text-sm font-medium text-slate-300 mb-3">执行计划</h4>
								<div class="space-y-3">
									{#each session.plan.groups as group, gi (group.id)}
										<div class="rounded bg-slate-900/50 p-3">
											<div class="text-xs text-slate-400 mb-2">
												第 {gi + 1} 组
												({group.kind === 'parallel' ? '并行' : '顺序'})
											</div>
											<div class="space-y-1">
												{#each group.tasks as task (task.spec_task_id)}
													<div class="flex items-center gap-2 text-xs">
														<span class="text-slate-500">{task.spec_task_id}</span>
														<span class="text-slate-300">{task.agent_config.role}</span>
														<span class="text-slate-500">→</span>
														<span class="text-slate-400">{task.agent_config.model_id}</span>
													</div>
												{/each}
											</div>
										</div>
									{/each}
								</div>
							</div>

							<!-- Live Events -->
							{#if events.length > 0}
								<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4">
									<h4 class="text-sm font-medium text-slate-300 mb-3">实时事件</h4>
									<div class="space-y-1 max-h-60 overflow-y-auto">
										{#each events.slice(0, 20) as event (event.timestamp)}
											<div class="flex items-start gap-2 text-xs">
												<span class="text-slate-500 shrink-0">{formatTime(event.timestamp)}</span>
												<span class="text-slate-300">{event.message}</span>
											</div>
										{/each}
									</div>
								</div>
							{/if}
						</div>
					{:else}
						<div class="text-sm text-slate-500 text-center py-8">
							{session.status === 'plan_generating' ? '正在生成执行计划...' : '执行计划尚未生成'}
						</div>
					{/if}

				{:else if activeTab === 'review'}
					<div class="space-y-4">
						<!-- All Events (Review Log) -->
						<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4">
							<h4 class="text-sm font-medium text-slate-300 mb-3">审查日志</h4>
							{#if events.length === 0}
								<div class="text-sm text-slate-500">暂无事件</div>
							{:else}
								<div class="space-y-1 max-h-96 overflow-y-auto">
									{#each events as event (event.timestamp)}
										<div class="flex items-start gap-2 text-xs py-1 border-b border-slate-700/30 last:border-0">
											<span class="text-slate-500 shrink-0 w-16">{formatTime(event.timestamp)}</span>
											<span class="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium shrink-0"
												class:bg-blue-400/10={event.event_type.includes('generat')}
												class:text-blue-400={event.event_type.includes('generat')}
												class:bg-emerald-400/10={event.event_type.includes('completed') || event.event_type.includes('passed')}
												class:text-emerald-400={event.event_type.includes('completed') || event.event_type.includes('passed')}
												class:bg-yellow-400/10={event.event_type.includes('executing') || event.event_type.includes('reviewing')}
												class:text-yellow-400={event.event_type.includes('executing') || event.event_type.includes('reviewing')}
												class:bg-red-400/10={event.event_type.includes('failed') || event.event_type.includes('exhausted')}
												class:text-red-400={event.event_type.includes('failed') || event.event_type.includes('exhausted')}
											>
												{event.event_type}
											</span>
											<span class="text-slate-300">{event.message}</span>
										</div>
									{/each}
								</div>
							{/if}
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>
