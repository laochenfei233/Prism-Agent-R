<script lang="ts">
	let hydeEnabled = $state(true);
	let rrfEnabled = $state(true);
	let cliffCutoffEnabled = $state(true);
	let webSearchEnabled = $state(false);
	let minGap = $state(0.15);
	let relDrop = $state(0.25);
</script>

<div class="rag-config">
	<h3 class="section-title">RAG 检索增强</h3>
	<p class="section-desc">配置高级检索策略，提升知识库问答质量。</p>

	<div class="cards-grid">
		<div class="config-card">
			<div class="card-header">
				<h4 class="card-title">HyDE 假设文档检索</h4>
				<input type="checkbox" bind:checked={hydeEnabled} />
			</div>
			<p class="card-desc">短查询时先让 LLM 生成假设答案，再用假设答案检索，提升语义对齐度。</p>
			<div class="card-status">
				<span class="status-dot" class:active={hydeEnabled}></span>
				<span>{hydeEnabled ? '已启用' : '已禁用'}</span>
			</div>
		</div>

		<div class="config-card">
			<div class="card-header">
				<h4 class="card-title">RRF 多路融合</h4>
				<input type="checkbox" bind:checked={rrfEnabled} />
			</div>
			<p class="card-desc">三路并发检索（混合/HyDE/网络）后用 RRF 算法融合排名，公平合并异构来源。</p>
			<div class="card-status">
				<span class="status-dot" class:active={rrfEnabled}></span>
				<span>{rrfEnabled ? '已启用' : '已禁用'}</span>
			</div>
		</div>

		<div class="config-card">
			<div class="card-header">
				<h4 class="card-title">断崖截断</h4>
				<input type="checkbox" bind:checked={cliffCutoffEnabled} />
			</div>
			<p class="card-desc">检测分数断崖自动截断，避免无关文档硬塞进上下文。</p>
			{#if cliffCutoffEnabled}
				<div class="card-fields">
					<div class="field-row">
						<label>min_gap</label>
						<input type="number" bind:value={minGap} min="0.01" max="1" step="0.01" />
					</div>
					<div class="field-row">
						<label>rel_drop</label>
						<input type="number" bind:value={relDrop} min="0.01" max="1" step="0.01" />
					</div>
				</div>
			{/if}
		</div>

		<div class="config-card">
			<div class="card-header">
				<h4 class="card-title">网络搜索补充</h4>
				<input type="checkbox" bind:checked={webSearchEnabled} />
			</div>
			<p class="card-desc">本地检索不足时自动补充网络搜索结果。需先在设置中配置搜索 Provider。</p>
			<div class="card-status">
				<span class="status-dot" class:active={webSearchEnabled}></span>
				<span>{webSearchEnabled ? '已启用' : '已禁用'}</span>
			</div>
		</div>
	</div>

	<div class="pipeline-visual">
		<h4 class="visual-title">检索链路</h4>
		<div class="pipeline">
			<div class="pipeline-node">查询</div>
			<div class="pipeline-arrow">→</div>
			<div class="pipeline-branch">
				<div class="branch-path" class:disabled={!rrfEnabled}>
					<span class="path-label">A 混合检索</span>
					<span class="path-info">top-150</span>
				</div>
				<div class="branch-path" class:disabled={!hydeEnabled || !rrfEnabled}>
					<span class="path-label">B HyDE</span>
					<span class="path-info">top-150</span>
				</div>
				<div class="branch-path" class:disabled={!webSearchEnabled || !rrfEnabled}>
					<span class="path-label">C 网络</span>
					<span class="path-info">top-10</span>
				</div>
			</div>
			<div class="pipeline-arrow">→</div>
			<div class="pipeline-node" class:disabled={!rrfEnabled}>RRF 融合</div>
			<div class="pipeline-arrow">→</div>
			<div class="pipeline-node" class:disabled={!cliffCutoffEnabled}>断崖截断</div>
			<div class="pipeline-arrow">→</div>
			<div class="pipeline-node">注入</div>
		</div>
	</div>
</div>

<style>
	.rag-config { padding: 1rem; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-bg-elevated); }
	.section-title { font-size: 1rem; font-weight: 600; margin: 0 0 0.25rem; }
	.section-desc { font-size: 0.875rem; color: var(--color-fg-tertiary); margin: 0 0 1rem; }
	.cards-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 1rem; }
	.config-card { padding: 1rem; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-bg); }
	.card-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }
	.card-title { font-size: 0.875rem; font-weight: 600; margin: 0; }
	.card-desc { font-size: 0.75rem; color: var(--color-fg-tertiary); margin: 0 0 0.75rem; line-height: 1.4; }
	.card-status { display: flex; align-items: center; gap: 0.375rem; font-size: 0.75rem; color: var(--color-fg-secondary); }
	.status-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--color-fg-tertiary); }
	.status-dot.active { background: var(--color-green); }
	.card-fields { display: flex; flex-direction: column; gap: 0.5rem; margin-top: 0.75rem; }
	.field-row { display: flex; align-items: center; gap: 0.5rem; }
	.field-row label { font-size: 0.75rem; color: var(--color-fg-secondary); min-width: 60px; }
	.pipeline-visual { margin-top: 1.5rem; padding: 1rem; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-bg); }
	.visual-title { font-size: 0.875rem; font-weight: 600; margin: 0 0 1rem; }
	.pipeline { display: flex; align-items: center; gap: 0.5rem; overflow-x: auto; padding: 0.5rem 0; }
	.pipeline-node { padding: 0.5rem 0.75rem; border: 1px solid var(--color-border-strong); border-radius: var(--radius-sm); font-size: 0.75rem; white-space: nowrap; background: var(--color-bg-elevated); }
	.pipeline-node.disabled { opacity: 0.5; background: var(--color-bg-hover); }
	.pipeline-arrow { color: var(--color-fg-tertiary); font-size: 0.875rem; }
	.pipeline-branch { display: flex; flex-direction: column; gap: 0.25rem; }
	.branch-path { display: flex; align-items: center; gap: 0.375rem; padding: 0.25rem 0.5rem; border: 1px solid var(--color-border); border-radius: var(--radius-sm); font-size: 0.6875rem; background: var(--color-bg-elevated); }
	.branch-path.disabled { opacity: 0.5; background: var(--color-bg-hover); }
	.path-label { font-weight: 500; }
	.path-info { color: var(--color-fg-tertiary); }
</style>