<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    icon = '',
    title = '',
    description = '',
    action,
    children,
  }: {
    icon?: string;
    title?: string;
    description?: string;
    action?: Snippet;
    children?: Snippet;
  } = $props();
</script>

<div class="empty-state">
  {#if icon}
    <div class="icon">{icon}</div>
  {/if}
  {#if title}
    <h3>{title}</h3>
  {/if}
  {#if description}
    <p>{description}</p>
  {/if}
  {#if action}
    <div class="action">
      {@render action()}
    </div>
  {/if}
  {#if children}
    <div class="extra">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--space-12) var(--space-6);
    text-align: center;
    gap: var(--space-3);
  }
  .icon {
    font-size: 48px;
    opacity: 0.5;
  }
  h3 {
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--color-fg);
    margin: 0;
  }
  p {
    font-size: var(--text-base);
    color: var(--color-fg-secondary);
    margin: 0;
    max-width: 320px;
  }
  .action {
    margin-top: var(--space-2);
  }
  .extra {
    margin-top: var(--space-2);
  }
</style>
