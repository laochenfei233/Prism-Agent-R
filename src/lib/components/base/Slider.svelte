<script lang="ts">
  let {
    value = $bindable(0),
    min = 0,
    max = 100,
    step = 1,
    disabled = false,
  }: {
    value?: number;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
  } = $props();

  let percent = $derived(((value - min) / (max - min)) * 100);
</script>

<input
  type="range"
  class="slider"
  {min}
  {max}
  {step}
  {disabled}
  bind:value
  style:--percent="{percent}%"
/>

<style>
  .slider {
    width: 100%;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: linear-gradient(
      to right,
      var(--color-accent) 0%,
      var(--color-accent) var(--percent),
      var(--color-bg-secondary) var(--percent),
      var(--color-bg-secondary) 100%
    );
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }
  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--color-accent);
    border: 2px solid #fff;
    box-shadow: var(--shadow-sm);
  }
  .slider:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
