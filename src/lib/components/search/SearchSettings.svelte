<script lang="ts">
  import { searchApi, type SearchConfigResult, type SearchTestResult } from '$lib/api';

  let config = $state<SearchConfigResult | null>(null);
  let testResult = $state<SearchTestResult | null>(null);
  let loading = $state(false);
  let testing = $state(false);
  let msg = $state('');

  let provider = $state('noop');
  let apiKey = $state('');
  let searxngUrl = $state('');
  let fallbackProvider = $state('');

  async function loadConfig() {
    try {
      config = await searchApi.config();
      provider = config.provider;
      searxngUrl = config.searxng_url || '';
      fallbackProvider = config.fallback_provider || '';
    } catch (e) {
      msg = '加载配置失败: ' + String(e);
    }
  }

  async function saveConfig() {
    loading = true;
    msg = '';
    try {
      await searchApi.saveConfig({
        provider,
        api_key: apiKey || undefined,
        searxng_url: searxngUrl || undefined,
        fallback_provider: fallbackProvider || undefined,
      });
      apiKey = '';
      msg = '✓ 配置已保存';
      await loadConfig();
    } catch (e) {
      msg = '保存失败: ' + String(e);
    } finally {
      loading = false;
    }
  }

  async function testConnection() {
    testing = true;
    testResult = null;
    try {
      testResult = await searchApi.test();
    } catch (e) {
      msg = '测试失败: ' + String(e);
    } finally {
      testing = false;
    }
  }

  $effect(() => {
    loadConfig();
  });
</script>

<div class="search-settings">
  <h3 class="section-title">网络搜索</h3>
  <p class="section-desc">配置网络搜索 Provider，供 Agent 使用 web_search 工具时调用。</p>

  <div class="config-grid">
    <div class="config-item">
      <label class="label" for="search-provider">Provider</label>
      <select id="search-provider" bind:value={provider}>
        <option value="noop">未配置</option>
        <option value="tavily">Tavily</option>
        <option value="serper">Serper (Google)</option>
        <option value="searxng">Searxng (自建)</option>
      </select>
    </div>

    {#if provider === 'tavily' || provider === 'serper'}
      <div class="config-item">
        <label class="label" for="search-api-key">API Key</label>
        <input
          id="search-api-key"
          type="password"
          bind:value={apiKey}
          placeholder={config?.api_key_set ? '••••••••' : '输入 API Key'}
        />
        {#if config?.api_key_set}
          <span class="hint">已配置</span>
        {/if}
      </div>
    {/if}

    {#if provider === 'searxng'}
      <div class="config-item">
        <label class="label" for="search-searxng-url">Searxng 实例地址</label>
        <input
          id="search-searxng-url"
          type="url"
          bind:value={searxngUrl}
          placeholder="http://localhost:8888"
        />
      </div>
    {/if}

    <div class="config-item">
      <label class="label" for="search-fallback">备用 Provider</label>
      <select id="search-fallback" bind:value={fallbackProvider}>
        <option value="">无</option>
        {#if provider !== 'tavily'}
          <option value="tavily">Tavily</option>
        {/if}
        {#if provider !== 'serper'}
          <option value="serper">Serper</option>
        {/if}
        {#if provider !== 'searxng'}
          <option value="searxng">Searxng</option>
        {/if}
      </select>
    </div>
  </div>

  <div class="actions">
    <button class="btn btn-primary" onclick={saveConfig} disabled={loading}>
      {loading ? '保存中...' : '保存配置'}
    </button>
    <button
      class="btn btn-outline"
      onclick={testConnection}
      disabled={testing || provider === 'noop'}
    >
      {testing ? '测试中...' : '测试连接'}
    </button>
  </div>

  {#if msg}
    <p class="message" class:success={msg.startsWith('✓')}>{msg}</p>
  {/if}

  {#if testResult}
    <div class="test-result" class:success={testResult.success} class:error={!testResult.success}>
      <div class="test-header">
        <span
          class="badge"
          class:badge-success={testResult.success}
          class:badge-error={!testResult.success}
        >
          {testResult.success ? '成功' : '失败'}
        </span>
        <span class="provider">{testResult.provider}</span>
        <span class="latency">{testResult.elapsed_ms}ms</span>
      </div>
      {#if testResult.first_result_title}
        <p class="result-title">首条结果: {testResult.first_result_title}</p>
      {/if}
      {#if testResult.error}
        <p class="result-error">{testResult.error}</p>
      {/if}
    </div>
  {/if}

  <p class="footer-note">结果缓存 1 小时（避免重复计费）</p>
</div>

<style>
  .search-settings {
    padding: 1rem;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-elevated);
  }
  .section-title {
    font-size: 1rem;
    font-weight: 600;
    margin: 0 0 0.25rem;
  }
  .section-desc {
    font-size: 0.875rem;
    color: var(--color-fg-tertiary);
    margin: 0 0 1rem;
  }
  .config-grid {
    display: grid;
    gap: 1rem;
  }
  .config-item {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }
  .label {
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--color-fg-secondary);
  }
  .hint {
    font-size: 0.75rem;
    color: var(--color-green);
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 1rem;
  }
  .message {
    font-size: 0.8125rem;
    margin-top: 0.5rem;
    color: var(--color-fg-secondary);
  }
  .message.success {
    color: var(--color-green);
  }
  .btn {
    padding: 0.5rem 1rem;
    border-radius: var(--radius-sm);
    font-size: 0.875rem;
    cursor: pointer;
  }
  .btn-primary {
    background: var(--color-accent);
    color: white;
    border: none;
  }
  .btn-outline {
    background: transparent;
    border: 1px solid var(--color-border);
    color: var(--color-fg);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .test-result {
    margin-top: 1rem;
    padding: 0.75rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
  }
  .test-result.success {
    border-color: var(--color-green);
    background: rgba(0, 200, 0, 0.05);
  }
  .test-result.error {
    border-color: var(--color-red);
    background: rgba(200, 0, 0, 0.05);
  }
  .test-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .badge {
    padding: 0.125rem 0.375rem;
    border-radius: var(--radius-sm);
    font-size: 0.75rem;
  }
  .badge-success {
    background: var(--color-green);
    color: white;
  }
  .badge-error {
    background: var(--color-red);
    color: white;
  }
  .provider {
    font-size: 0.8125rem;
    color: var(--color-fg-secondary);
  }
  .latency {
    font-size: 0.75rem;
    color: var(--color-fg-tertiary);
    margin-left: auto;
  }
  .result-title {
    font-size: 0.8125rem;
    margin: 0.5rem 0 0;
  }
  .result-error {
    font-size: 0.8125rem;
    color: var(--color-red);
    margin: 0.5rem 0 0;
  }
  .footer-note {
    font-size: 0.75rem;
    color: var(--color-fg-tertiary);
    margin-top: 1rem;
  }
</style>
