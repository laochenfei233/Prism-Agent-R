<script lang="ts">
  let {
    value = 0,
    max = 100,
    variant = 'default',
  }: {
    value?: number;
    max?: number;
    variant?: 'default' | 'success' | 'warning' | 'error';
  } = $props();

  let percent = $derived(Math.min(100, Math.max(0, (value / max) * 100)));
  let colorVar = $derived(
    variant === 'success'
      ? 'var(--color-green)'
      : variant === 'warning'
        ? 'var(--color-orange)'
        : variant === 'error'
          ? 'var(--color-red)'
          : 'var(--color-accent)',
  );
</script>

<div class="progress">
  <div class="bar" style:width="{percent}%" style:background={colorVar}></div>
</div>

<style>
  .progress {
    width: 100%;
    height: 6px;
    border-radius: 3px;
    background: var(--color-bg-secondary);
    overflow: hidden;
  }
  .bar {
    height: 100%;
    border-radius: 3px;
    transition: width var(--duration-base) var(--ease-in-out);
  }
</style>
