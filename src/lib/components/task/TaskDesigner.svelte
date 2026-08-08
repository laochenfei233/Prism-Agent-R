<script lang="ts">
	import { taskStore } from '$lib/stores/task.svelte';
	import TaskTemplateCard from './TaskTemplateCard.svelte';
	import TaskCanvas from './TaskCanvas.svelte';
	import TaskRunPanel from './TaskRunPanel.svelte';

	const TABS = [
		{ label: '模板', key: 'template' as const },
		{ label: '设计', key: 'design' as const },
		{ label: '运行', key: 'run' as const },
	];

	let activeTab = $derived(TABS.findIndex((t) => t.key === taskStore.viewMode));

	function handleTabChange(index: number) {
		taskStore.viewMode = TABS[index].key;
	}

	const builtinTemplates: { id: string; name: string; description: string; stage_count: number; icon: string; definition: any }[] = [
		{
			id: 'builtin-research',
			name: '深度研究',
			description: '搜索 → 分析 → 综合报告',
			stage_count: 3,
			icon: 'M21 21l-4.35-4.35M17 11a6 6 0 1 1-12 0 6 6 0 0 1 12 0z',
			definition: {
				name: '深度研究',
				description: '多阶段研究工作流',
				inputs: [
					{ key: 'topic', label: '研究主题', kind: 'Text' as const, options: null, default: '', required: true },
					{ key: 'depth', label: '研究深度', kind: 'Select' as const, options: ['快速', '标准', '深度'], default: '标准', required: true },
				],
				stages: [
					{ id: '1', name: '搜索', role: '研究员', agent_id: null, prompt_template: '请搜索关于「{{topic}}」的全面信息：', tools: ['web_search'], max_iterations: 5, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: '分析', role: '分析师', agent_id: null, prompt_template: '请分析以下搜索结果，提取关键发现：\n{{搜索}}', tools: [], max_iterations: 3, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: '报告', role: '写手', agent_id: null, prompt_template: '请基于分析结果撰写一份综合报告：\n{{分析}}', tools: [], max_iterations: 2, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
		{
			id: 'builtin-translate',
			name: '翻译校对',
			description: '翻译 → 审校 → 终稿',
			stage_count: 3,
			icon: 'M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
			definition: {
				name: '翻译校对',
				description: '专业翻译工作流',
				inputs: [
					{ key: 'source_text', label: '原文', kind: 'Textarea' as const, options: null, default: '', required: true },
					{ key: 'target_lang', label: '目标语言', kind: 'Select' as const, options: ['英语', '日语', '韩语', '法语', '德语'], default: '英语', required: true },
				],
				stages: [
					{ id: '1', name: '初翻', role: '翻译', agent_id: null, prompt_template: '请将以下文本翻译为{{target_lang}}：\n{{source_text}}', tools: [], max_iterations: 1, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: '审校', role: '审校员', agent_id: null, prompt_template: '请审校以下翻译，检查准确性和流畅度：\n{{初翻}}', tools: [], max_iterations: 2, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: '终稿', role: '编辑', agent_id: null, prompt_template: '请根据审校意见输出最终译文：\n原文：{{source_text}}\n初翻：{{初翻}}\n审校意见：{{审校}}', tools: [], max_iterations: 1, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
		{
			id: 'builtin-code-review',
			name: '代码审查',
			description: '代码分析 → 问题发现 → 改进建议',
			stage_count: 3,
			icon: 'M9.75 17 9 20l-1 1h8l-1-1-.75-3M5.6 20.18a9 9 0 1 1 12.8 0M8 14h8M12 8v6',
			definition: {
				name: '代码审查',
				description: '自动化代码审查工作流',
				inputs: [
					{ key: 'code', label: '代码', kind: 'Textarea' as const, options: null, default: '', required: true },
					{ key: 'language', label: '编程语言', kind: 'Select' as const, options: ['TypeScript', 'Python', 'Rust', 'Go', 'Java'], default: 'TypeScript', required: true },
				],
				stages: [
					{ id: '1', name: '分析', role: '分析师', agent_id: null, prompt_template: '请分析以下{{language}}代码的结构和功能：\n{{code}}', tools: [], max_iterations: 1, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: '找问题', role: '审查员', agent_id: null, prompt_template: '请根据代码分析结果，找出潜在的 Bug 和问题：\n{{分析}}', tools: [], max_iterations: 2, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: '建议', role: '顾问', agent_id: null, prompt_template: '请针对发现的问题给出具体改进建议：\n问题列表：{{找问题}}\n原始代码：\n{{code}}', tools: [], max_iterations: 1, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
		{
			id: 'builtin-brainstorm',
			name: '头脑风暴',
			description: '生成创意 → 评估筛选 → 输出方案',
			stage_count: 3,
			icon: 'M9 18h6M10 22h4M12 2a7 7 0 0 0-4 12.7c.6.5 1 1.4 1 2.3h6c0-.9.4-1.8 1-2.3A7 7 0 0 0 12 2z',
			definition: {
				name: '头脑风暴',
				description: '结构化创意工作流',
				inputs: [
					{ key: 'topic', label: '主题', kind: 'Text' as const, options: null, default: '', required: true },
				],
				stages: [
					{ id: '1', name: '发散', role: '创意官', agent_id: null, prompt_template: '请围绕「{{topic}}」进行头脑风暴，尽可能多地产出创意：', tools: [], max_iterations: 3, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: '评估', role: '评审官', agent_id: null, prompt_template: '请从可行性和影响力两个维度评估以下创意：\n{{发散}}', tools: [], max_iterations: 2, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: '精选', role: '策略师', agent_id: null, prompt_template: '请从评估结果中选出最佳创意并细化：\n{{评估}}', tools: [], max_iterations: 1, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
	];

	$effect(() => {
		taskStore.loadTemplates();
	});
</script>

<div class="task-designer">
	<div class="designer-tabs" role="tablist" tabindex="0" onkeydown={(e) => {
		const dir = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
		if (dir) {
			e.preventDefault();
			handleTabChange((activeTab + dir + TABS.length) % TABS.length);
		}
	}}>
		{#each TABS as tab, i}
			<button
				class="tab"
				class:active={activeTab === i}
				role="tab"
				aria-selected={activeTab === i}
				tabindex={activeTab === i ? 0 : -1}
				onclick={() => handleTabChange(i)}
			>
				{tab.label}
			</button>
		{/each}
	</div>

	<div class="designer-content">
		{#if taskStore.viewMode === 'template'}
			<div class="template-panel">
				<div class="template-grid">
					{#each builtinTemplates as tpl}
						<TaskTemplateCard template={tpl} onSelect={() => taskStore.loadTemplate(tpl.definition)} />
					{/each}
					{#each taskStore.templates as tpl (tpl.id)}
						<TaskTemplateCard template={tpl} onSelect={(t) => taskStore.loadTemplate(t)} />
					{/each}
				</div>
				<button class="start-blank" onclick={taskStore.newDefinition}>
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
					从零开始
				</button>
			</div>

		{:else if taskStore.viewMode === 'design'}
			<TaskCanvas />

		{:else if taskStore.viewMode === 'run'}
			<TaskRunPanel />
		{/if}
	</div>
</div>

<style>
	.task-designer {
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-separator);
		border-radius: 12px;
		box-shadow: var(--shadow-sm);
		display: flex;
		flex-direction: column;
		height: 480px;
		overflow: hidden;
	}

	.designer-tabs {
		display: flex;
		gap: 0;
		border-bottom: 1px solid var(--color-separator);
		padding: 0 4px;
	}

	.tab {
		padding: 10px 16px;
		border: none;
		background: none;
		cursor: pointer;
		font-size: 13px;
		font-weight: 500;
		color: var(--color-muted);
		border-bottom: 2px solid transparent;
		transition: color 0.15s, border-color 0.15s;
	}
	.tab:hover { color: var(--color-fg); }
	.tab.active {
		color: var(--color-fg);
		border-bottom-color: var(--color-fg);
	}

	.designer-content {
		flex: 1;
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}

	/* ── Template Panel ───────────────────────── */
	.template-panel {
		padding: 20px;
		display: flex;
		flex-direction: column;
		gap: 16px;
		overflow-y: auto;
	}

	.template-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 10px;
	}

	.start-blank {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		padding: 10px;
		border: 1px dashed var(--color-border-strong);
		border-radius: 10px;
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.15s;
	}
	.start-blank:hover {
		border-color: var(--color-accent);
		color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 6%, transparent);
	}

	@media (max-width: 600px) {
		.template-grid { grid-template-columns: 1fr; }
	}
</style>
