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

	const builtinTemplates: { id: string; name: string; description: string; stage_count: number; input_count: number; definition: any }[] = [
		{
			id: 'builtin-translate',
			name: '翻译工作流',
			description: '多阶段翻译：初翻 → 审校 → 终稿',
			stage_count: 3,
			input_count: 2,
			definition: {
				name: '翻译工作流',
				description: '多阶段翻译：初翻 → 审校 → 终稿',
				inputs: [
					{ key: 'source_text', label: '原文', kind: 'Textarea' as const, options: null, default: '', required: true },
					{ key: 'target_lang', label: '目标语言', kind: 'Select' as const, options: ['英文', '日文', '韩文', '法文'], default: '英文', required: true },
				],
				stages: [
					{ id: '1', name: '初翻', role: 'assistant', agent_id: null, prompt_template: '请将以下文本翻译为{{target_lang}}：\n{{source_text}}', tools: [], max_iterations: 1, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: '审校', role: 'assistant', agent_id: null, prompt_template: '请审校以下翻译，指出问题并给出改进建议：\n{{初翻}}', tools: [], max_iterations: 2, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: '终稿', role: 'assistant', agent_id: null, prompt_template: '根据审校意见，输出最终翻译：\n原文：{{source_text}}\n初翻：{{初翻}}\n审校意见：{{审校}}', tools: [], max_iterations: 1, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
		{
			id: 'builtin-review',
			name: '内容审核',
			description: '内容安全检测 + 分类 + 摘要',
			stage_count: 3,
			input_count: 1,
			definition: {
				name: '内容审核',
				description: '内容安全检测 + 分类 + 摘要',
				inputs: [
					{ key: 'content', label: '待审核内容', kind: 'Textarea' as const, options: null, default: '', required: true },
				],
				stages: [
					{ id: '1', name: '安全检测', role: 'assistant', agent_id: null, prompt_template: '检测以下内容是否包含违规信息：\n{{content}}', tools: [], max_iterations: 1, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: '内容分类', role: 'assistant', agent_id: null, prompt_template: '对以下内容进行分类：\n{{content}}', tools: [], max_iterations: 1, depends_on: [], model_hint: null, output_spec: null },
					{ id: '3', name: '生成摘要', role: 'assistant', agent_id: null, prompt_template: '为以下内容生成摘要：\n{{content}}\n分类结果：{{内容分类}}', tools: [], max_iterations: 1, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
		{
			id: 'builtin-code-review',
			name: '代码审查',
			description: '代码分析 → 问题发现 → 改进建议',
			stage_count: 3,
			input_count: 2,
			definition: {
				name: '代码审查',
				description: '代码分析 → 问题发现 → 改进建议',
				inputs: [
					{ key: 'code', label: '代码', kind: 'Textarea' as const, options: null, default: '', required: true },
					{ key: 'language', label: '编程语言', kind: 'Select' as const, options: ['TypeScript', 'Python', 'Rust', 'Go', 'Java'], default: 'TypeScript', required: true },
				],
				stages: [
					{ id: '1', name: '代码分析', role: 'assistant', agent_id: null, prompt_template: '分析以下{{language}}代码的结构和功能：\n```{{language}}\n{{code}}\n```', tools: [], max_iterations: 1, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: '问题发现', role: 'assistant', agent_id: null, prompt_template: '根据代码分析结果，找出潜在问题：\n{{代码分析}}', tools: [], max_iterations: 2, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: '改进建议', role: 'assistant', agent_id: null, prompt_template: '基于问题发现，给出具体改进建议：\n问题列表：{{问题发现}}\n原始代码：\n```{{language}}\n{{code}}\n```', tools: [], max_iterations: 1, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
	];

	$effect(() => {
		taskStore.loadTemplates();
	});
</script>

<div class="task-designer">
	<div class="designer-tabs" role="tablist">
		{#each TABS as tab, i}
			<button
				class="tab"
				class:active={activeTab === i}
				role="tab"
				aria-selected={activeTab === i}
				onclick={() => handleTabChange(i)}
			>
				{tab.label}
			</button>
		{/each}
	</div>

	<div class="designer-content">
		{#if taskStore.viewMode === 'template'}
			<div class="template-panel">
				<div class="panel-header">
					<h3>选择模板</h3>
					<button class="btn-sm btn-primary" onclick={taskStore.newDefinition}>从零开始</button>
				</div>

				<div class="template-grid">
					{#each builtinTemplates as tpl}
						{@const def = tpl.definition}
						<TaskTemplateCard template={tpl} onSelect={() => taskStore.loadTemplate(def)} />
					{/each}

					{#each taskStore.templates as tpl (tpl.id)}
						<TaskTemplateCard template={tpl} onSelect={(t) => taskStore.loadTemplate(t)} />
					{/each}
				</div>

				{#if taskStore.templatesLoading}
					<div class="loading-text">加载中...</div>
				{/if}

				{#if !builtinTemplates.length && !taskStore.templates.length && !taskStore.templatesLoading}
					<div class="empty-templates">
						<p>暂无模板，点击"从零开始"创建</p>
					</div>
				{/if}
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
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.designer-tabs {
		display: flex;
		gap: 0;
		border-bottom: 1px solid var(--color-separator);
		padding: 0 var(--spacing-md);
	}

	.tab {
		padding: var(--spacing-sm) var(--spacing-md);
		border: none;
		background: none;
		cursor: pointer;
		font-size: var(--text-subheadline);
		font-weight: 500;
		color: var(--color-fg-secondary);
		border-bottom: 2px solid transparent;
		transition: color 0.15s ease, border-color 0.15s ease;
	}

	.tab:hover {
		color: var(--color-fg);
	}

	.tab.active {
		color: var(--color-accent);
		border-bottom-color: var(--color-accent);
	}

	.designer-content {
		flex: 1;
		overflow-y: auto;
	}

	.template-panel {
		padding: var(--spacing-md);
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.panel-header h3 {
		font-size: var(--text-headline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.btn-sm {
		padding: 6px 12px;
		border-radius: var(--radius-full);
		border: none;
		cursor: pointer;
		font-size: var(--text-caption1);
		font-weight: 500;
	}

	.btn-primary {
		background: var(--color-accent);
		color: #fff;
	}

	.template-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: var(--spacing-sm);
	}

	.loading-text {
		text-align: center;
		color: var(--color-fg-tertiary);
		font-size: var(--text-subheadline);
		padding: var(--spacing-md);
	}

	.empty-templates {
		text-align: center;
		color: var(--color-fg-tertiary);
		font-size: var(--text-subheadline);
		padding: var(--spacing-xxl);
	}
</style>
