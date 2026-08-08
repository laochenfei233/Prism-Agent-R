<script lang="ts">
	import type { TaskStageDef } from '$lib/stores/task.svelte';
	import { taskStore } from '$lib/stores/task.svelte';

	let { stage, onClose }: {
		stage: TaskStageDef;
		onClose: () => void;
	} = $props();

	let name = $state(stage.name);
	let role = $state(stage.role);
	let agentId = $state(stage.agent_id ?? '');
	let prompt = $state(stage.prompt_template);
	let toolsStr = $state(stage.tools.join(', '));
	let maxIter = $state(stage.max_iterations);
	let modelHint = $state(stage.model_hint ?? '');
	let outputSpec = $state(stage.output_spec ?? '');

	function apply() {
		taskStore.updateStage(stage.id, {
			name,
			role,
			agent_id: agentId || null,
			prompt_template: prompt,
			tools: toolsStr.split(',').map((t) => t.trim()).filter(Boolean),
			max_iterations: maxIter,
			model_hint: modelHint || null,
			output_spec: outputSpec || null,
		});
	}
</script>

<div class="inspector-backdrop" onclick={onClose} role="presentation">
	<div class="inspector" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-label="阶段属性" tabindex="-1">
		<div class="inspector-header">
			<h3>阶段属性</h3>
			<button class="close-btn" onclick={onClose} aria-label="关闭">&times;</button>
		</div>

		<div class="inspector-body">
			<div class="field">
				<label for="si-name">名称</label>
				<input id="si-name" bind:value={name} oninput={apply} />
			</div>

			<div class="field">
				<label for="si-role">角色</label>
				<select id="si-role" bind:value={role} onchange={apply}>
					<option value="assistant">Assistant</option>
					<option value="user">User</option>
					<option value="system">System</option>
				</select>
			</div>

			<div class="field">
				<label for="si-agent">Agent ID（可选）</label>
				<input id="si-agent" bind:value={agentId} oninput={apply} placeholder="留空使用默认" />
			</div>

			<div class="field">
				<label for="si-model">模型提示（可选）</label>
				<input id="si-model" bind:value={modelHint} oninput={apply} placeholder="如 gpt-4o" />
			</div>

			<div class="field">
				<label for="si-prompt">Prompt 模板</label>
				<textarea id="si-prompt" bind:value={prompt} oninput={apply} rows="4" placeholder="支持 input.key 变量引用"></textarea>
			</div>

			<div class="field">
				<label for="si-tools">工具（逗号分隔）</label>
				<input id="si-tools" bind:value={toolsStr} oninput={apply} placeholder="web_search, code_exec" />
			</div>

			<div class="field">
				<label for="si-iter">最大迭代次数</label>
				<input id="si-iter" type="number" bind:value={maxIter} oninput={apply} min="1" max="20" />
			</div>

			<div class="field">
				<label for="si-output">输出规范（可选）</label>
				<textarea id="si-output" bind:value={outputSpec} oninput={apply} rows="2" placeholder="描述期望的输出格式"></textarea>
			</div>
		</div>
	</div>
</div>

<style>
	.inspector-backdrop {
		position: fixed;
		inset: 0;
		background: var(--color-overlay);
		z-index: 100;
		display: flex;
		justify-content: flex-end;
	}

	.inspector {
		width: 380px;
		max-width: 90vw;
		background: var(--color-bg-elevated);
		border-left: 1px solid var(--color-separator);
		display: flex;
		flex-direction: column;
		overflow-y: auto;
	}

	.inspector-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--spacing-md);
		border-bottom: 1px solid var(--color-separator);
	}

	.inspector-header h3 {
		font-size: var(--text-headline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.close-btn {
		border: none;
		background: none;
		color: var(--color-fg-secondary);
		cursor: pointer;
		font-size: 20px;
		padding: 4px 8px;
		border-radius: var(--radius-sm);
	}

	.close-btn:hover {
		background: var(--color-bg-secondary);
	}

	.inspector-body {
		padding: var(--spacing-md);
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.field label {
		font-size: var(--text-caption2);
		font-weight: 500;
		color: var(--color-fg-secondary);
	}

	.field input,
	.field select,
	.field textarea {
		padding: 8px 12px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: var(--text-base);
		font-family: var(--font-sans);
		outline: none;
	}

	.field input:focus,
	.field select:focus,
	.field textarea:focus {
		border-color: var(--color-accent);
	}

	.field textarea {
		resize: vertical;
		min-height: 60px;
	}
</style>
