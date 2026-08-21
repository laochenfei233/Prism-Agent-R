<script lang="ts">
  import { invoke } from '$lib/api/client';
  import { contextStore } from '$lib/stores/context.svelte';
  import type { AgentContext } from '$lib/stores/context.svelte';

  let { data }: { data: AgentContext } = $props();

  const servers = $derived(data.lsp);
  const workdir = $derived(data.workspace.current_dir);

  let busyId = $state<string | null>(null);
  let detecting = $state(false);
  let error = $state<string | null>(null);

  function statusColor(status: string): string {
    if (status === 'running') return 'var(--color-green)';
    if (status === 'error') return 'var(--color-red)';
    return 'var(--color-fg-secondary)';
  }

  async function startServer(serverId: string) {
    if (!workdir || busyId) return;
    busyId = serverId;
    error = null;
    try {
      await invoke('lsp_start', { serverId, workdir });
      await contextStore.refresh();
    } catch (e) {
      error = errMessage(e);
    } finally {
      busyId = null;
    }
  }

  async function stopServer(serverId: string) {
    if (busyId) return;
    busyId = serverId;
    error = null;
    try {
      await invoke('lsp_stop', { serverId });
      await contextStore.refresh();
    } catch (e) {
      error = errMessage(e);
    } finally {
      busyId = null;
    }
  }

  async function redetect() {
    if (!workdir || detecting) return;
    detecting = true;
    error = null;
    try {
      await invoke('lsp_detect', { workdir });
      await contextStore.refresh();
    } catch (e) {
      error = errMessage(e);
    } finally {
      detecting = false;
    }
  }

  function errMessage(e: unknown): string {
    if (typeof e === 'string') return e;
    if (e && typeof e === 'object' && 'message' in e)
      return String((e as { message: unknown }).message);
    return String(e);
  }
</script>

<div class="lsp-panel">
  <div class="panel-header">
    <span class="panel-title">语言服务器</span>
    <button
      class="refresh-btn"
      onclick={() => void redetect()}
      disabled={detecting || !workdir}
      title="重新检测"
    >
      <svg
        width="13"
        height="13"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <polyline points="23 4 23 10 17 10" />
        <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
      </svg>
      {detecting ? '检测中…' : '重新检测'}
    </button>
  </div>

  {#if error}
    <div class="error-msg">{error}</div>
  {/if}

  {#if !workdir}
    <div class="empty">
      <span>未设置工作目录，无法启动 LSP</span>
    </div>
  {:else if servers.length === 0}
    <div class="empty">
      <span>无 LSP 服务器</span>
    </div>
  {:else}
    <div class="server-list">
      {#each servers as server (server.id)}
        <div class="server-item">
          <div class="server-header">
            <div class="status-dot" style:background={statusColor(server.status)}></div>
            <div class="server-info">
              <span class="server-name">{server.id}</span>
              <span class="server-cmd">{server.cmd}</span>
            </div>
            {#if server.status === 'running'}
              <button
                class="btn-stop"
                onclick={() => void stopServer(server.id)}
                disabled={busyId !== null}
              >
                {busyId === server.id ? '停止中…' : '停止'}
              </button>
            {:else}
              <button
                class="btn-start"
                onclick={() => void startServer(server.id)}
                disabled={busyId !== null}
              >
                {busyId === server.id ? '启动中…' : '启动'}
              </button>
            {/if}
          </div>

          <div class="server-detail">
            <div class="lang-tags">
              {#each server.langs as lang, i (i)}
                <span class="lang-tag">{lang}</span>
              {/each}
            </div>

            <div class="detail-row">
              <span class="detail-label">状态</span>
              <span class="detail-value">{server.status}</span>
            </div>

            {#if server.index_file_count !== null}
              <div class="detail-row">
                <span class="detail-label">索引文件</span>
                <span class="detail-value">{server.index_file_count}</span>
              </div>
            {/if}

            {#if server.last_error}
              <div class="error-msg">{server.last_error}</div>
            {/if}

            {#if server.install_hint}
              <div class="install-hint">
                <span class="install-title">未安装</span>
                <code>{server.install_hint}</code>
              </div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .lsp-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .panel-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-fg-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .refresh-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    border-radius: 6px;
    border: 1px solid var(--color-separator);
    background: var(--color-bg);
    color: var(--color-fg-secondary);
    font-size: 11px;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }
  .refresh-btn:hover {
    background: var(--color-bg-tertiary);
    color: var(--color-fg);
  }
  .refresh-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px 0;
    font-size: 13px;
    color: var(--color-fg-secondary);
  }

  .server-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .server-item {
    background: var(--color-bg);
    border-radius: 8px;
    border: 1px solid var(--color-separator);
    overflow: hidden;
  }

  .server-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .server-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .server-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-fg);
  }

  .server-cmd {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-fg-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .btn-start,
  .btn-stop {
    padding: 4px 10px;
    border-radius: 6px;
    border: none;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    flex-shrink: 0;
    transition: opacity 0.15s ease;
  }
  .btn-start:hover,
  .btn-stop:hover {
    opacity: 0.9;
  }
  .btn-start:disabled,
  .btn-stop:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-start {
    background: var(--color-accent);
    color: #fff;
  }

  .btn-stop {
    background: color-mix(in srgb, var(--color-red) 12%, transparent);
    color: var(--color-red);
  }

  .server-detail {
    padding: 0 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .lang-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .lang-tag {
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 500;
    background: var(--color-bg-secondary);
    color: var(--color-fg-secondary);
    border: 1px solid var(--color-separator);
  }

  .detail-row {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
  }

  .detail-label {
    color: var(--color-fg-secondary);
  }

  .detail-value {
    color: var(--color-fg);
    font-weight: 500;
  }

  .error-msg {
    font-size: 12px;
    color: var(--color-red);
    padding: 6px 8px;
    background: color-mix(in srgb, var(--color-red) 8%, transparent);
    border-radius: 4px;
    word-break: break-all;
  }

  .install-hint {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11px;
    color: var(--color-fg-secondary);
    padding: 6px 8px;
    background: var(--color-bg-secondary);
    border-radius: 4px;
  }

  .install-title {
    font-weight: 600;
  }

  .install-hint code {
    font-family: var(--font-mono);
    font-size: 11px;
    word-break: break-all;
    color: var(--color-fg);
  }
</style>
