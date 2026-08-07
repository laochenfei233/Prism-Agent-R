<script lang="ts">
	import { taskStore } from '$lib/stores/task.svelte';
	import TaskTemplateCard from './TaskTemplateCard.svelte';
	import TaskCanvas from './TaskCanvas.svelte';
	import TaskRunPanel from './TaskRunPanel.svelte';

	const TABS = [
		{ label: 'Templates', key: 'template' as const },
		{ label: 'Design', key: 'design' as const },
		{ label: 'Run', key: 'run' as const },
	];

	let activeTab = $derived(TABS.findIndex((t) => t.key === taskStore.viewMode));

	function handleTabChange(index: number) {
		taskStore.viewMode = TABS[index].key;
	}

	const builtinTemplates: { id: string; name: string; description: string; stage_count: number; icon: string; definition: any }[] = [
		{
			id: 'builtin-research',
			name: 'Deep Research',
			description: 'Multi-stage research: search → analyze → synthesize',
			stage_count: 3,
			icon: '🔍',
			definition: {
				name: 'Deep Research',
				description: 'Comprehensive research workflow',
				inputs: [
					{ key: 'topic', label: 'Research Topic', kind: 'Text' as const, options: null, default: '', required: true },
					{ key: 'depth', label: 'Depth', kind: 'Select' as const, options: ['Quick', 'Standard', 'Deep'], default: 'Standard', required: true },
				],
				stages: [
					{ id: '1', name: 'Search', role: 'Researcher', agent_id: null, prompt_template: 'Search for comprehensive information about: {{topic}}', tools: ['web_search'], max_iterations: 5, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: 'Analyze', role: 'Analyst', agent_id: null, prompt_template: 'Analyze the following research findings:\n{{Search}}', tools: [], max_iterations: 3, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: 'Report', role: 'Writer', agent_id: null, prompt_template: 'Write a comprehensive report based on:\n{{Analyze}}', tools: [], max_iterations: 2, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
		{
			id: 'builtin-translate',
			name: 'Translation',
			description: 'Translate → review → finalize',
			stage_count: 3,
			icon: '🌐',
			definition: {
				name: 'Translation Pipeline',
				description: 'Professional translation workflow',
				inputs: [
					{ key: 'source_text', label: 'Source Text', kind: 'Textarea' as const, options: null, default: '', required: true },
					{ key: 'target_lang', label: 'Target Language', kind: 'Select' as const, options: ['English', 'Japanese', 'Korean', 'French', 'German'], default: 'English', required: true },
				],
				stages: [
					{ id: '1', name: 'Translate', role: 'Translator', agent_id: null, prompt_template: 'Translate the following to {{target_lang}}:\n{{source_text}}', tools: [], max_iterations: 1, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: 'Review', role: 'Reviewer', agent_id: null, prompt_template: 'Review this translation for accuracy and fluency:\n{{Translate}}', tools: [], max_iterations: 2, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: 'Finalize', role: 'Editor', agent_id: null, prompt_template: 'Produce the final polished translation:\nSource: {{source_text}}\nDraft: {{Translate}}\nReview: {{Review}}', tools: [], max_iterations: 1, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
		{
			id: 'builtin-code-review',
			name: 'Code Review',
			description: 'Analyze → find issues → suggest improvements',
			stage_count: 3,
			icon: '💻',
			definition: {
				name: 'Code Review',
				description: 'Automated code review workflow',
				inputs: [
					{ key: 'code', label: 'Code', kind: 'Textarea' as const, options: null, default: '', required: true },
					{ key: 'language', label: 'Language', kind: 'Select' as const, options: ['TypeScript', 'Python', 'Rust', 'Go', 'Java'], default: 'TypeScript', required: true },
				],
				stages: [
					{ id: '1', name: 'Analyze', role: 'Analyst', agent_id: null, prompt_template: 'Analyze this {{language}} code:\n{{code}}', tools: [], max_iterations: 1, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: 'Find Issues', role: 'Reviewer', agent_id: null, prompt_template: 'Find bugs and issues in this analysis:\n{{Analyze}}', tools: [], max_iterations: 2, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: 'Suggest', role: 'Advisor', agent_id: null, prompt_template: 'Suggest improvements for:\n{{Find Issues}}\nOriginal code:\n{{code}}', tools: [], max_iterations: 1, depends_on: ['2'], model_hint: null, output_spec: null },
				],
			},
		},
		{
			id: 'builtin-brainstorm',
			name: 'Brainstorm',
			description: 'Generate ideas → evaluate → select best',
			stage_count: 3,
			icon: '💡',
			definition: {
				name: 'Brainstorm',
				description: 'Structured brainstorming workflow',
				inputs: [
					{ key: 'topic', label: 'Topic', kind: 'Text' as const, options: null, default: '', required: true },
				],
				stages: [
					{ id: '1', name: 'Generate', role: 'Creative', agent_id: null, prompt_template: 'Generate diverse ideas for: {{topic}}', tools: [], max_iterations: 3, depends_on: [], model_hint: null, output_spec: null },
					{ id: '2', name: 'Evaluate', role: 'Critic', agent_id: null, prompt_template: 'Evaluate these ideas on feasibility and impact:\n{{Generate}}', tools: [], max_iterations: 2, depends_on: ['1'], model_hint: null, output_spec: null },
					{ id: '3', name: 'Select', role: 'Strategist', agent_id: null, prompt_template: 'Select and refine the best ideas:\n{{Evaluate}}', tools: [], max_iterations: 1, depends_on: ['2'], model_hint: null, output_spec: null },
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
					Start from scratch
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
		background: #fff;
		border: 1px solid rgba(0, 0, 0, 0.06);
		border-radius: 12px;
		display: flex;
		flex-direction: column;
		height: 480px;
		overflow: hidden;
	}

	.designer-tabs {
		display: flex;
		gap: 0;
		border-bottom: 1px solid rgba(0, 0, 0, 0.06);
		padding: 0 4px;
	}

	.tab {
		padding: 10px 16px;
		border: none;
		background: none;
		cursor: pointer;
		font-size: 13px;
		font-weight: 500;
		color: #a0a0a0;
		border-bottom: 2px solid transparent;
		transition: color 0.15s, border-color 0.15s;
	}
	.tab:hover { color: #171717; }
	.tab.active {
		color: #171717;
		border-bottom-color: #171717;
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
		border: 1px dashed rgba(0, 0, 0, 0.12);
		border-radius: 10px;
		background: transparent;
		color: #6b6b6b;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.15s;
	}
	.start-blank:hover {
		border-color: #FF6900;
		color: #FF6900;
		background: #fff8f0;
	}

	@media (max-width: 600px) {
		.template-grid { grid-template-columns: 1fr; }
	}
</style>
