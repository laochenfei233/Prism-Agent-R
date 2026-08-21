<script lang="ts">
  let {
    value = $bindable(''),
    options = [],
    placeholder = 'Select...',
    disabled = false,
  }: {
    value?: string;
    options: { label: string; value: string }[];
    placeholder?: string;
    disabled?: boolean;
  } = $props();
</script>

<select class="select" bind:value {disabled}>
  <option value="" disabled>{placeholder}</option>
  {#each options as opt (opt.value)}
    <option value={opt.value}>{opt.label}</option>
  {/each}
</select>

<style>
  .select {
    width: 100%;
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    border: 1px solid var(--color-separator);
    background: var(--color-bg);
    color: var(--color-fg);
    font-size: var(--text-base);
    font-family: var(--font-sans);
    outline: none;
    cursor: pointer;
    transition: border-color var(--duration-fast);
  }
  .select:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 3px var(--color-focus-ring);
  }
  .select:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
