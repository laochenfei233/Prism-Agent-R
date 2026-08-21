<script lang="ts">
  let {
    checked = $bindable(false),
    disabled = false,
    label = '开关',
    onchange,
  }: {
    checked?: boolean;
    disabled?: boolean;
    label?: string;
    onchange?: (checked: boolean) => void;
  } = $props();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  class="switch"
  class:active={checked}
  {disabled}
  onclick={toggle}
  role="switch"
  aria-checked={checked}
  aria-label={label}
>
  <span class="thumb"></span>
</button>

<style>
  .switch {
    width: 44px;
    height: 26px;
    border-radius: 13px;
    border: none;
    background: var(--color-bg-tertiary);
    cursor: pointer;
    position: relative;
    transition: background var(--duration-fast);
    flex-shrink: 0;
  }
  .switch.active {
    background: var(--color-green);
  }
  .switch:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: #fff;
    transition: transform var(--duration-fast) var(--spring);
    box-shadow: var(--shadow-sm);
  }
  .switch.active .thumb {
    transform: translateX(18px);
  }
</style>
