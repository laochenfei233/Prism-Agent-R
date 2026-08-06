<script lang="ts">
	import { listen, invoke } from '$lib/api/client';

	let visible = $state(false);
	let request = $state<any>(null);

	listen('tool:approval-request', (event: { payload: any }) => {
		request = event.payload;
		visible = true;
	});

	async function respond(response: string) {
		await invoke('tool_approval_respond', {
			callId: request.call_id,
			response,
		});
		visible = false;
		request = null;
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape' && visible) {
			respond('Rejected');
		}
	}
</script>

<svelte:window on:keydown={handleKeydown} />

{#if visible && request}
	<div class="overlay" role="presentation">
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="dialog glass" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1" aria-label="Tool Approval">
			<h3 class="title">Tool Approval</h3>
			<p class="agent-name">Agent "{request.agent_id}" requests:</p>

			<div class="tool-info">
				<span class="tool-name">Tool: {request.tool_name}</span>
				<span class="risk-badge risk-{request.risk_level?.toLowerCase() || 'low'}">Risk: {request.risk_level}</span>
			</div>

			<div class="params">
				<h4>Parameters</h4>
				<pre class="params-pre">{JSON.stringify(request.arguments, null, 2)}</pre>
			</div>

			{#if request.description}
				<p class="description">{request.description}</p>
			{/if}

			<div class="dialog-actions">
				<button class="btn-approve" onclick={() => respond('Approved')}>&#10003; Approve</button>
				<button class="btn-reject" onclick={() => respond('Rejected')}>&#10007; Reject</button>
				<button class="btn-always" onclick={() => respond('AlwaysApprove')}>Always Approve</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.45);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1100;
		animation: fadeIn var(--duration-fast) ease;
	}

	.dialog {
		border-radius: var(--radius-xl);
		min-width: 380px;
		max-width: 520px;
		padding: var(--space-6);
		animation: scaleIn var(--duration-normal) var(--spring);
		background: var(--color-bg);
		border: 1px solid var(--color-separator);
		box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
	}

	.title {
		margin: 0 0 var(--space-3);
		font-size: var(--text-lg);
		font-weight: 600;
		color: var(--color-fg);
	}

	.agent-name {
		margin: 0 0 var(--space-4);
		font-size: var(--text-sm);
		color: var(--color-fg-secondary);
	}

	.tool-info {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-3) var(--space-4);
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		margin-bottom: var(--space-4);
	}

	.tool-name {
		font-family: monospace;
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-fg);
	}

	.risk-badge {
		padding: 2px 10px;
		border-radius: var(--radius-sm);
		font-size: 12px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.3px;
	}

	.risk-low {
		background: rgba(52, 199, 89, 0.15);
		color: #34C759;
	}

	.risk-medium {
		background: rgba(255, 149, 0, 0.15);
		color: #FF9500;
	}

	.risk-high {
		background: rgba(255, 59, 48, 0.15);
		color: #FF3B30;
	}

	.params {
		margin-bottom: var(--space-4);
	}

	.params h4 {
		margin: 0 0 var(--space-2);
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.params-pre {
		background: var(--color-bg-secondary);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		font-family: monospace;
		font-size: 13px;
		color: var(--color-fg);
		overflow-x: auto;
		margin: 0;
		max-height: 200px;
		overflow-y: auto;
	}

	.description {
		margin: 0 0 var(--space-4);
		font-size: var(--text-sm);
		color: var(--color-fg-secondary);
		line-height: 1.5;
	}

	.dialog-actions {
		display: flex;
		gap: var(--space-2);
		margin-top: var(--space-4);
	}

	.btn-approve {
		flex: 1;
		padding: 10px 16px;
		border-radius: var(--radius-md);
		border: none;
		background: #34C759;
		color: #fff;
		font-size: 15px;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.btn-approve:hover { background: #2DB84E; }
	.btn-approve:active { transform: scale(0.97); }

	.btn-reject {
		flex: 1;
		padding: 10px 16px;
		border-radius: var(--radius-md);
		border: none;
		background: #FF3B30;
		color: #fff;
		font-size: 15px;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.btn-reject:hover { background: #E0342B; }
	.btn-reject:active { transform: scale(0.97); }

	.btn-always {
		flex: 1;
		padding: 10px 16px;
		border-radius: var(--radius-md);
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.btn-always:hover { background: var(--color-bg-tertiary); }
	.btn-always:active { transform: scale(0.97); }

	@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
	@keyframes scaleIn { from { opacity: 0; transform: scale(0.95); } to { opacity: 1; transform: scale(1); } }
</style>
