<script lang="ts">
  let {
    checked = $bindable(false),
    disabled = false,
  }: {
    checked?: boolean;
    disabled?: boolean;
  } = $props();

  function toggle() {
    if (!disabled) checked = !checked;
  }
</script>

<button
  class="checkbox"
  class:checked
  {disabled}
  onclick={toggle}
  role="checkbox"
  aria-checked={checked}
>
  {#if checked}
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
      <path
        d="M2 6L5 9L10 3"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  {/if}
</button>

<style>
  .checkbox {
    width: 20px;
    height: 20px;
    border-radius: var(--radius-sm);
    border: 2px solid var(--color-separator);
    background: var(--color-bg);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    transition: all var(--duration-fast);
    flex-shrink: 0;
  }
  .checkbox.checked {
    background: var(--color-accent);
    border-color: var(--color-accent);
  }
  .checkbox:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
