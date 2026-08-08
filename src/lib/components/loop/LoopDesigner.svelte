<script lang="ts">
	import { loopApi, type AgentLoop, type LoopKind } from '$lib/api';

	let loops = $state<AgentLoop[]>([]);
	let loading = $state(false);
	let msg = $state('');

	let loopKind = $state<LoopKind>('Goal');
	let maxRounds = $state(5);
	let intervalSecs = $state(60);
	let goalDesc = $state('');

	async function loadLoops() {
		try {
			loops = await loopApi.list();
		} catch (e) {
			msg = '加载失败: ' + String(e);
		}
	}

	async function createLoop() {
		loading = true;
		msg = '';
		try {
			await loopApi.start({
				kind: loopKind,
				max_rounds: maxRounds,
				interval_secs: loopKind === 'Timer' ? intervalSecs : undefined,
				goal: loopKind === 'Goal' ? { description: goalDesc } : undefined,
			});
			msg = '✓ Loop 已创建';
			await loadLoops();
		} catch (e) {
			msg = '创建失败: ' + String(e);
		} finally {
			loading = false;
		}
	}

	async function stopLoop(id: string) {
		try {
			await loopApi.stop(id);
			await loadLoops();
		} catch (e) {
			msg = '停止失败: ' + String(e);
		}
	}

	function getStatusVariant(status: string): string {
		switch (status) {
			case 'Running': return 'primary';
			case 'Completed': return 'success';
			case 'Failed': return 'error';
			case 'Paused': return 'warning';
			default: return 'secondary';
		}
	}

	$effect(() => {
		loadLoops();
	});
</script>

<div class="loop-designer">
	<h3 class="section-title">自动化 Loop</h3>
	<p class="section-desc">创建 Goal/Timer/Maker-Checker 循环，自动化重复任务。</p>

	<div class="create-form">
		<div class="form-row">
			<div class="form-item">
				<label>Loop 类型</label>
				<select bind:value={loopKind}>
					<option value="Goal">Goal 循环</option>
					<option value="Timer">Timer 定时</option>
					<option value="MakerChecker">Maker-Checker</option>
				</select>
			</div>
			<div class="form-item">
				<label>最大轮次</label>
				<input type="number" bind:value={maxRounds} min="1" max="50" />
			</div>
			{#if loopKind === 'Timer'}
				<div class="form-item">
					<label>间隔（秒）</label>
					<input type="number" bind:value={intervalSecs} min="60" max="3600" />
				</div>
			{/if}
			{#if loopKind === 'Goal'}
				<div class="form-item full-width">
					<label>目标描述</label>
					<input type="text" bind:value={goalDesc} placeholder="描述需要达成的目标..." />
				</div>
			{/if}
		</div>
		<button class="btn btn-primary" onclick={createLoop} disabled={loading}>
			{loading ? '创建中...' : '创建 Loop'}
		</button>
	</div>

	{#if msg}
		<p class="message" class:success={msg.startsWith('✓')}>{msg}</p>
	{/if}

	{#if loops.length > 0}
		<div class="loop-list">
			<h4 class="list-title">活跃 Loop ({loops.length})</h4>
			{#each loops as loop_}
				<div class="loop-item">
					<div class="loop-header">
						<span class="badge badge-{getStatusVariant(loop_.status)}">{loop_.status}</span>
						<span class="loop-kind">{loop_.kind}</span>
						<span class="loop-id">#{loop_.id.slice(0, 8)}</span>
						{#if loop_.status === 'Running'}
							<button class="btn btn-sm btn-danger" onclick={() => stopLoop(loop_.id)}>
								停止
							</button>
						{/if}
					</div>
					<div class="loop-progress">
						<div class="progress-bar">
							<div class="progress-fill" style:width="{(loop_.current_round / loop_.max_rounds) * 100}%"></div>
						</div>
						<span class="progress-text">{loop_.current_round}/{loop_.max_rounds} 轮</span>
					</div>
				</div>
			{/each}
		</div>
	{:else}
		<p class="empty">暂无活跃 Loop</p>
	{/if}
</div>

<style>
	.loop-designer { padding: 1rem; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-bg-elevated); }
	.section-title { font-size: 1rem; font-weight: 600; margin: 0 0 0.25rem; }
	.section-desc { font-size: 0.875rem; color: var(--color-fg-tertiary); margin: 0 0 1rem; }
	.create-form { padding: 1rem; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-bg); }
	.form-row { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 1rem; margin-bottom: 1rem; }
	.form-item { display: flex; flex-direction: column; gap: 0.375rem; }
	.form-item.full-width { grid-column: 1 / -1; }
	.form-item label { font-size: 0.8125rem; font-weight: 500; color: var(--color-fg-secondary); }
	.message { font-size: 0.8125rem; margin-top: 0.5rem; color: var(--color-fg-secondary); }
	.message.success { color: var(--color-green); }
	.btn { padding: 0.5rem 1rem; border-radius: var(--radius-sm); font-size: 0.875rem; cursor: pointer; }
	.btn-primary { background: var(--color-accent); color: white; border: none; }
	.btn-danger { background: var(--color-red); color: white; border: none; }
	.btn-sm { padding: 0.25rem 0.5rem; font-size: 0.75rem; }
	.btn:disabled { opacity: 0.5; cursor: not-allowed; }
	.loop-list { margin-top: 1.5rem; }
	.list-title { font-size: 0.875rem; font-weight: 600; margin: 0 0 0.75rem; }
	.loop-item { padding: 0.75rem; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-bg); margin-bottom: 0.5rem; }
	.loop-header { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem; }
	.badge { padding: 0.125rem 0.375rem; border-radius: var(--radius-sm); font-size: 0.75rem; }
	.badge-primary { background: var(--color-accent); color: white; }
	.badge-success { background: var(--color-green); color: white; }
	.badge-error { background: var(--color-red); color: white; }
	.badge-warning { background: var(--color-orange); color: white; }
	.badge-secondary { background: var(--color-fg-tertiary); color: white; }
	.loop-kind { font-size: 0.8125rem; color: var(--color-fg-secondary); }
	.loop-id { font-size: 0.75rem; color: var(--color-fg-tertiary); font-family: var(--font-mono); margin-left: auto; }
	.loop-progress { display: flex; align-items: center; gap: 0.75rem; }
	.progress-bar { flex: 1; height: 4px; background: var(--color-bg-hover); border-radius: 2px; overflow: hidden; }
	.progress-fill { height: 100%; background: var(--color-accent); transition: width 0.3s; }
	.progress-text { font-size: 0.75rem; color: var(--color-fg-tertiary); white-space: nowrap; }
	.empty { font-size: 0.875rem; color: var(--color-fg-tertiary); text-align: center; padding: 2rem; }
</style>