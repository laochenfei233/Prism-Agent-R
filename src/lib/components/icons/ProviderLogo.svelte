<script lang="ts">
	import Anthropic from './providers/Anthropic.svelte';
	import Baichuan from './providers/Baichuan.svelte';
	import Dashscope from './providers/Dashscope.svelte';
	import Deepseek from './providers/Deepseek.svelte';
	import Doubao from './providers/Doubao.svelte';
	import Google from './providers/Google.svelte';
	import Groq from './providers/Groq.svelte';
	import Minimax from './providers/Minimax.svelte';
	import Mimo from './providers/Mimo.svelte';
	import Mistral from './providers/Mistral.svelte';
	import Moonshot from './providers/Moonshot.svelte';
	import Ollama from './providers/Ollama.svelte';
	import Openai from './providers/Openai.svelte';
	import Openrouter from './providers/Openrouter.svelte';
	import Silicon from './providers/Silicon.svelte';
	import Zhipu from './providers/Zhipu.svelte';

	let { kind, size = 20 }: { kind: string; size?: number } = $props();

	const LOGOS: Record<string, typeof Openai> = {
		openai: Openai,
		anthropic: Anthropic,
		google: Google,
		dashscope: Dashscope,
		mimo: Mimo,
		ollama: Ollama,
		deepseek: Deepseek,
		zhipu: Zhipu,
		moonshot: Moonshot,
		doubao: Doubao,
		minimax: Minimax,
		baichuan: Baichuan,
		silicon: Silicon,
		groq: Groq,
		openrouter: Openrouter,
		mistral: Mistral,
	};
</script>

{#if LOGOS[kind]}
	{@const Logo = LOGOS[kind]}
	<Logo {size} />
{:else}
	<span class="fallback">{kind.slice(0, 1).toUpperCase()}</span>
{/if}

<style>
	.fallback {
		width: 20px;
		height: 20px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: 6px;
		background: var(--color-bg-tertiary);
		color: var(--color-fg-secondary);
		font-size: 12px;
		font-weight: 600;
	}
</style>
