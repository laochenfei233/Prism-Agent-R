<script lang="ts">
	import { monitorStore } from '$lib/stores/monitor.svelte';

	const budget = $derived(monitorStore.budget);
	const exceptions = $derived(monitorStore.exceptions);
	const loading = $derived(monitorStore.loading);

	function tokenPercent(): number {
		if (!budget) return 0;
		return Math.min(100, Math.round((budget.daily_tokens_used / budget.daily_tokens_limit) * 100));
	}

	function costPercent(): number {
		if (!budget) return 0;
		return Math.min(100, Math.round((budget.daily_cost_used / budget.daily_cost_limit) * 100));
	}

	function severityColor(severity: string): string {
		switch (severity) {
			case 'critical': return 'text-red-400 bg-red-400/10';
			case 'high': return 'text-orange-400 bg-orange-400/10';
			case 'medium': return 'text-yellow-400 bg-yellow-400/10';
			default: return 'text-slate-400 bg-slate-400/10';
		}
	}

	function formatTime(ts: number): string {
		const d = new Date(ts);
		const now = new Date();
		const diff = now.getTime() - ts;
		if (diff < 60000) return '刚刚';
		if (diff < 3600000) return `${Math.floor(diff / 60000)} 分钟前`;
		if (diff < 86400000) return `${Math.floor(diff / 3600000)} 小时前`;
		return d.toLocaleDateString();
	}

	$effect(() => {
		monitorStore.refresh();
		const interval = setInterval(() => monitorStore.refresh(), 10000);
		return () => clearInterval(interval);
	});
</script>

<div class="monitor-panel h-full overflow-y-auto p-4 space-y-4">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<h2 class="text-lg font-semibold text-slate-100">监控面板</h2>
		<button
			class="text-xs text-slate-400 hover:text-slate-200 transition-colors"
			onclick={() => monitorStore.refresh()}
			disabled={loading}
		>
			{loading ? '刷新中...' : '刷新'}
		</button>
	</div>

	<!-- Budget Overview -->
	<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4 space-y-3">
		<h3 class="text-sm font-medium text-slate-300">预算概览</h3>

		{#if budget}
			<!-- Token Usage -->
			<div class="space-y-1">
				<div class="flex justify-between text-xs">
					<span class="text-slate-400">Token 使用</span>
					<span class="text-slate-300">{(budget.daily_tokens_used / 1000).toFixed(1)}k / {(budget.daily_tokens_limit / 1000).toFixed(0)}k</span>
				</div>
				<div class="h-2 bg-slate-700 rounded-full overflow-hidden">
					<div
						class="h-full rounded-full transition-all duration-500"
						class:bg-emerald-500={tokenPercent() < 60}
						class:bg-yellow-500={tokenPercent() >= 60 && tokenPercent() < 80}
						class:bg-red-500={tokenPercent() >= 80}
						style="width: {tokenPercent()}%"
					></div>
				</div>
				<div class="text-right text-xs text-slate-500">{tokenPercent()}%</div>
			</div>

			<!-- Cost Usage -->
			<div class="space-y-1">
				<div class="flex justify-between text-xs">
					<span class="text-slate-400">费用使用</span>
					<span class="text-slate-300">${budget.daily_cost_used.toFixed(2)} / ${budget.daily_cost_limit.toFixed(2)}</span>
				</div>
				<div class="h-2 bg-slate-700 rounded-full overflow-hidden">
					<div
						class="h-full rounded-full transition-all duration-500"
						class:bg-emerald-500={costPercent() < 60}
						class:bg-yellow-500={costPercent() >= 60 && costPercent() < 80}
						class:bg-red-500={costPercent() >= 80}
						style="width: {costPercent()}%"
					></div>
				</div>
				<div class="text-right text-xs text-slate-500">{costPercent()}%</div>
			</div>

			<!-- Active Workflows -->
			<div class="flex justify-between text-xs pt-1">
				<span class="text-slate-400">活跃工作流</span>
				<span class="text-slate-300">{budget.active_workflows}</span>
			</div>
		{:else}
			<div class="text-sm text-slate-500">加载中...</div>
		{/if}
	</div>

	<!-- Recent Exceptions -->
	<div class="rounded-lg bg-slate-800/50 border border-slate-700/50 p-4 space-y-3">
		<h3 class="text-sm font-medium text-slate-300">最近异常</h3>

		{#if exceptions.length === 0}
			<div class="text-sm text-slate-500">暂无异常</div>
		{:else}
			<div class="space-y-2 max-h-60 overflow-y-auto">
				{#each exceptions as exc (exc.id)}
					<div class="flex items-start gap-2 text-xs p-2 rounded bg-slate-900/50">
						<span class="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium shrink-0 {severityColor(exc.severity)}">
							{exc.severity}
						</span>
						<div class="min-w-0 flex-1">
							<div class="text-slate-300 truncate">{exc.message}</div>
							<div class="text-slate-500 mt-0.5">{exc.agent_id} · {formatTime(exc.created_at)}</div>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
