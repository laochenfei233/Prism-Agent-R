<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { meetingApi, asrApi, type MeetingDto, type AsrBackendInfoDto, type AsrModelInfoDto, type InstalledAsrModelDto, type AsrConfigInputDto, type AsrConfigDto } from '$lib/api';
	import { listen } from '$lib/api/client';

	let meetings = $state<MeetingDto[]>([]);
	let selectedMeeting = $state<MeetingDto | null>(null);
	let showCreate = $state(false);
	let newTitle = $state('');
	let newParticipants = $state('');
	let loading = $state(false);
	let backends = $state<AsrBackendInfoDto[]>([]);
	let activeTab = $state<'meetings' | 'asr'>('meetings');

	// ASR 配置 + 模型管理
	let catalog = $state<AsrModelInfoDto[]>([]);
	let installed = $state<InstalledAsrModelDto[]>([]);
	let downloadProgress = $state<Record<string, number>>({});
	let configs = $state<AsrConfigDto[]>([]);
	let showAddConfig = $state(false);
	let newConfig = $state<AsrConfigInputDto>({
		name: '本地 SenseVoice', kind: 'SherpaOnnx', is_default: false,
		model_path: '', lang: 'zh'
	});
	let modelPathInput = $state('');
	let recording = $state(false);

	// 录音
	let audioCtx: AudioContext | null = null;
	let mediaStream: MediaStream | null = null;
	let recorderNode: AudioWorkletNode | null = null;
	let micStream: MediaStreamAudioSourceNode | null = null;
	let workletLoaded = false;

	onMount(async () => {
		loading = true;
		try {
			meetings = await meetingApi.list();
			backends = await asrApi.backends();
			await loadAsrModels();
		} catch (e) { console.error(e); }
		loading = false;
		// 下载进度事件
		const un = await listen<{ model_id: string; progress: number; message: string }>('asr:model-download-progress', (e) => {
			downloadProgress[e.model_id] = e.progress;
			if (e.progress >= 1) loadAsrModels();
		});
		cleanup = un;
	});
	onDestroy(() => { cleanup?.(); stopRecording(); });

	let cleanup: (() => void) | null = null;

	async function loadAsrModels() {
		try {
			[catalog, installed, configs] = await Promise.all([
				asrApi.modelCatalog(), asrApi.modelInstalled(), asrApi.listConfigs()
			]);
		} catch (e) { console.error(e); }
	}

	async function downloadModel(id: string) {
		try { await asrApi.modelDownload(id); } catch (e) { console.error(e); }
	}

	async function removeModel(id: string) {
		if (!confirm('删除模型？')) return;
		try { await asrApi.modelRemove(id); await loadAsrModels(); } catch (e) { console.error(e); }
	}

	async function testConfig() {
		try {
			const res = await asrApi.test({ ...newConfig, model_path: modelPathInput || undefined });
			alert(res.ok ? `连接正常（${res.latency_ms}ms）` : `失败：${res.error}`);
		} catch (e) { console.error(e); }
	}

	async function saveConfig() {
		try {
			await asrApi.saveConfig({ ...newConfig, model_path: modelPathInput || undefined, api_key: newConfig.api_key || undefined });
			showAddConfig = false;
			await loadAsrModels();
		} catch (e) { console.error(e); }
	}

	async function deleteConfig(id: string) {
		try { await asrApi.deleteConfig(id); await loadAsrModels(); } catch (e) { console.error(e); }
	}

	// ── 录音 ──────────────────────────────────────────────
	async function startRecording() {
		if (!selectedMeeting) return;
		try {
			// 1. 通知后端先建 stream（时序规避核心）
			const cfg = { ...newConfig, model_path: modelPathInput || undefined, api_key: newConfig.api_key || undefined };
			await asrApi.startRecording(selectedMeeting.id, cfg);

			// 2. 前端采集
			mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
			audioCtx = new AudioContext({ sampleRate: 16000 });
			await loadWorklet();
			micStream = audioCtx.createMediaStreamSource(mediaStream);
			recorderNode = new AudioWorkletNode(audioCtx, 'pcm-recorder');
			const mid = selectedMeeting.id;
			recorderNode.port.onmessage = (ev) => {
				const pcmBase64 = ev.data as string;
				if (pcmBase64) asrApi.audioChunk(mid, pcmBase64);
			};
			micStream.connect(recorderNode);
			recording = true;
		} catch (e) { console.error('录音启动失败:', e); alert('录音启动失败：' + e); }
	}

	async function loadWorklet() {
		if (workletLoaded) return;
		// 16kHz PCM 采集 worklet（AudioWorklet 不阻塞主线程）
		const code = `
			class PcmRecorder extends AudioWorkletProcessor {
				process(inputs) {
					const input = inputs[0];
					if (!input || !input[0]) return true;
					const samples = input[0];
					// 下采样到 16kHz（AudioContext 已是 16kHz 时原样）
					const int16 = new Int16Array(samples.length);
					for (let i = 0; i < samples.length; i++) {
						const s = Math.max(-1, Math.min(1, samples[i]));
						int16[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
					}
					const bytes = new Uint8Array(int16.buffer);
					let binary = '';
					const chunk = 0x8000;
					for (let i = 0; i < bytes.length; i += chunk) {
						binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
					}
					this.port.postMessage(btoa(binary));
					return true;
				}
			}
			registerProcessor('pcm-recorder', PcmRecorder);`;
		const blob = new Blob([code], { type: 'application/javascript' });
		const url = URL.createObjectURL(blob);
		await audioCtx!.audioWorklet.addModule(url);
		workletLoaded = true;
	}

	async function stopRecording() {
		recording = false;
		recorderNode?.disconnect();
		micStream?.disconnect();
		mediaStream?.getTracks().forEach(t => t.stop());
		recorderNode = null; micStream = null; mediaStream = null;
		if (audioCtx) { audioCtx.close(); audioCtx = null; }
		if (selectedMeeting) {
			try { await asrApi.stopRecording(selectedMeeting.id); } catch (e) { console.error(e); }
		}
	}

	async function createMeeting() {
		if (!newTitle.trim()) return;
		const participants = newParticipants.split(',').map(s => s.trim()).filter(Boolean);
		try {
			const m = await meetingApi.create(newTitle.trim(), participants.length ? participants : undefined);
			newTitle = ''; newParticipants = ''; showCreate = false;
			meetings = [m, ...meetings];
			selectedMeeting = m;
		} catch (e) { console.error(e); }
	}

	async function selectMeeting(m: MeetingDto) {
		selectedMeeting = m;
	}

	async function deleteMeeting(id: string) {
		if (!confirm('确定删除此会议？')) return;
		try {
			await meetingApi.delete(id);
			meetings = meetings.filter(m => m.id !== id);
			if (selectedMeeting?.id === id) selectedMeeting = null;
		} catch (e) { console.error(e); }
	}

	async function generateSummary() {
		if (!selectedMeeting) return;
		try {
			const summary = await meetingApi.summary(selectedMeeting.id);
			selectedMeeting = { ...selectedMeeting, summary };
		} catch (e) { console.error(e); }
	}

	async function cleanTranscript() {
		if (!selectedMeeting) return;
		try {
			const cleaned = await meetingApi.clean(selectedMeeting.id);
			selectedMeeting = { ...selectedMeeting, transcript: cleaned };
		} catch (e) { console.error(e); }
	}

	async function exportMeeting(format: string) {
		if (!selectedMeeting) return;
		try {
			const content = await meetingApi.export(selectedMeeting.id, format, true, false);
			const blob = new Blob([content], { type: 'text/plain' });
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url; a.download = `${selectedMeeting.title}.${format === 'text' ? 'txt' : 'md'}`; a.click();
			URL.revokeObjectURL(url);
		} catch (e) { console.error(e); }
	}

	async function translateTranscript() {
		if (!selectedMeeting) return;
		try {
			const path = await meetingApi.exportTranslation(selectedMeeting.id, 'en');
			alert(`翻译稿已保存：${path}`);
		} catch (e) { console.error(e); }
	}
</script>

<div class="page">
	<header class="page-header">
		<h1>会议纪要</h1>
		<div class="tabs">
			<button class="tab" class:active={activeTab === 'meetings'} onclick={() => activeTab = 'meetings'}>会议</button>
			<button class="tab" class:active={activeTab === 'asr'} onclick={() => activeTab = 'asr'}>ASR 设置</button>
		</div>
	</header>

	{#if activeTab === 'meetings'}
		{#if !selectedMeeting}
			<button class="btn-primary" onclick={() => showCreate = true}>新建会议</button>

			{#if showCreate}
				<div class="create-form">
					<input placeholder="会议标题" bind:value={newTitle} />
					<input placeholder="参会人（逗号分隔）" bind:value={newParticipants} />
					<div class="form-actions">
						<button class="btn-ghost" onclick={() => showCreate = false}>取消</button>
						<button class="btn-primary" onclick={createMeeting}>创建</button>
					</div>
				</div>
			{/if}

			{#if loading}
				<div class="empty">加载中...</div>
			{:else if meetings.length === 0}
				<div class="empty"><p>暂无会议</p></div>
			{:else}
				<div class="list">
					{#each meetings as m}
						<div class="list-item" onclick={() => selectMeeting(m)}>
							<div class="item-main">
								<h3>{m.title}</h3>
								<span class="meta">{m.date} · {m.participants.length} 人参会 · {m.recording_duration}s</span>
							</div>
							<button class="btn-danger-sm" onclick={(e) => { e.stopPropagation(); deleteMeeting(m.id); }}>删除</button>
						</div>
					{/each}
				</div>
			{/if}
		{:else}
			<!-- 会议详情 -->
			<div class="detail">
				<div class="detail-header">
					<button class="btn-ghost" onclick={() => selectedMeeting = null}>← 返回</button>
					<h2>{selectedMeeting.title}</h2>
					<div class="detail-actions">
						<button class="btn-primary" onclick={recording ? stopRecording : startRecording}>
							{recording ? '■ 停止录音' : '● 开始录音'}
						</button>
						<button class="btn-ghost" onclick={generateSummary}>生成摘要</button>
						<button class="btn-ghost" onclick={cleanTranscript}>清洗转写</button>
						<button class="btn-ghost" onclick={() => exportMeeting('md')}>导出 MD</button>
						<button class="btn-ghost" onclick={() => exportMeeting('text')}>导出 TXT</button>
						<button class="btn-ghost" onclick={translateTranscript}>翻译稿</button>
					</div>
				</div>

				<div class="meta-bar">
					<span>日期: {selectedMeeting.date}</span>
					<span>参会人: {selectedMeeting.participants.join(', ') || '未设置'}</span>
					{#if recording}<span class="rec-badge">● 录音中</span>{/if}
				</div>

				{#if selectedMeeting.summary}
					<div class="section">
						<h3>摘要</h3>
						<div class="content-box">{selectedMeeting.summary}</div>
					</div>
				{/if}

				<div class="section">
					<h3>转写内容</h3>
					<div class="content-box transcript">
						{selectedMeeting.transcript || '暂无转写内容，点击「开始录音」实时转写'}
					</div>
				</div>
			</div>
		{/if}
	{:else}
		<!-- ASR 设置页 -->
		<div class="asr-page">
			<div class="section">
				<h3>可用后端</h3>
				<div class="backend-list">
					{#each backends as b}
						<div class="backend-item">
							<strong>{b.name}</strong>
							<span>{b.description}</span>
							<span class="langs">{b.languages.join(', ')}</span>
						</div>
					{/each}
				</div>
			</div>

			<div class="section">
				<h3>模型管理</h3>
				<div class="model-list">
					{#each catalog as m}
						<div class="model-item">
							<div class="model-main">
								<strong>{m.name}</strong>
								<span class="model-meta">{m.backend} · {m.size_mb}MB · {m.lang.join(',')}</span>
							</div>
							{#if downloadProgress[m.id] !== undefined && downloadProgress[m.id] < 1}
								<span class="progress">{(downloadProgress[m.id] * 100).toFixed(0)}%</span>
							{:else if installed.some(i => i.id === m.id)}
								<button class="btn-danger-sm" onclick={() => removeModel(m.id)}>删除</button>
							{:else}
								<button class="btn-primary btn-sm" onclick={() => downloadModel(m.id)}>下载</button>
							{/if}
						</div>
					{/each}
				</div>
			</div>

			<div class="section">
				<h3>ASR 配置</h3>
				<button class="btn-ghost" onclick={() => showAddConfig = !showAddConfig}>+ 新建配置</button>
				{#if showAddConfig}
					<div class="config-form">
						<input placeholder="名称（如 本地 SenseVoice）" bind:value={newConfig.name} />
						<select bind:value={newConfig.kind}>
							{#each backends as b}<option value={b.kind}>{b.name}</option>{/each}
						</select>
						<input placeholder="模型路径（本地后端，如 asr_models/sherpa-sensevoice-small）" bind:value={modelPathInput} />
						{#if newConfig.kind.includes('Http') || newConfig.kind === 'Custom' || newConfig.kind === 'WhisperApi'}
							<input placeholder="API Key" bind:value={newConfig.api_key} />
						{/if}
						<div class="form-actions">
							<button class="btn-ghost" onclick={testConfig}>测试连接</button>
							<button class="btn-primary" onclick={saveConfig}>保存</button>
						</div>
					</div>
				{/if}
				<div class="config-list">
					{#each configs as c}
						<div class="config-item">
							<span><strong>{c.name}</strong> · {c.kind}</span>
							{#if c.model_path}<span class="meta">📁 {c.model_path}</span>{/if}
							<button class="btn-danger-sm" onclick={() => deleteConfig(c.id)}>删</button>
						</div>
					{/each}
					{#if configs.length === 0}<div class="hint">暂无配置</div>{/if}
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
	.page { padding: 24px 32px; max-width: 960px; margin: 0 auto; }
	.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px; }
	.page-header h1 { font-size: 24px; font-weight: 600; color: var(--color-fg); margin: 0; }
	.tabs { display: flex; gap: 4px; background: var(--color-bg-secondary); border-radius: 8px; padding: 3px; }
	.tab { padding: 6px 14px; border: none; border-radius: 6px; background: transparent; color: var(--color-fg-secondary); font-size: 13px; cursor: pointer; }
	.tab.active { background: var(--color-accent); color: #fff; }
	.btn-primary { padding: 8px 16px; border-radius: 8px; border: none; background: var(--color-accent); color: #fff; font-size: 14px; font-weight: 500; cursor: pointer; }
	.btn-sm { padding: 5px 12px; font-size: 12px; }
	.btn-ghost { padding: 8px 16px; border-radius: 8px; border: 1px solid var(--color-separator); background: transparent; color: var(--color-fg-secondary); font-size: 14px; cursor: pointer; }
	.btn-danger-sm { padding: 4px 8px; border-radius: 6px; border: none; background: #ff4444; color: #fff; font-size: 12px; cursor: pointer; }
	.create-form { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 16px; margin: 16px 0; display: flex; flex-direction: column; gap: 8px; }
	.create-form input, .config-form input, .config-form select { padding: 8px 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 14px; outline: none; }
	.create-form input:focus, .config-form input:focus, .config-form select:focus { border-color: var(--color-accent); }
	.form-actions { display: flex; gap: 8px; justify-content: flex-end; }
	.empty { text-align: center; padding: 48px; color: var(--color-fg-secondary); }
	.list { display: flex; flex-direction: column; gap: 8px; margin-top: 16px; }
	.list-item { display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 10px; cursor: pointer; transition: border-color 0.15s; }
	.list-item:hover { border-color: var(--color-accent); }
	.item-main h3 { margin: 0; font-size: 15px; color: var(--color-fg); }
	.meta { font-size: 12px; color: var(--color-fg-secondary); }
	.detail-header { display: flex; align-items: center; gap: 12px; margin-bottom: 16px; flex-wrap: wrap; }
	.detail-header h2 { margin: 0; font-size: 20px; color: var(--color-fg); flex: 1; }
	.detail-actions { display: flex; gap: 8px; }
	.meta-bar { display: flex; gap: 16px; font-size: 13px; color: var(--color-fg-secondary); margin-bottom: 20px; flex-wrap: wrap; }
	.rec-badge { color: #ff4444; font-weight: 600; }
	.section { margin-top: 20px; }
	.section h3 { font-size: 14px; color: var(--color-fg-secondary); margin: 0 0 8px; text-transform: uppercase; letter-spacing: 0.5px; }
	.content-box { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 10px; padding: 16px; font-size: 14px; color: var(--color-fg); white-space: pre-wrap; line-height: 1.6; }
	.transcript { max-height: 400px; overflow-y: auto; }

	/* ASR 设置 */
	.backend-list, .model-list, .config-list { display: flex; flex-direction: column; gap: 6px; }
	.backend-item, .model-item, .config-item { display: flex; align-items: center; gap: 8px; padding: 8px 12px; background: var(--color-bg-secondary); border-radius: 8px; font-size: 13px; }
	.backend-item strong, .model-item strong { color: var(--color-fg); }
	.backend-item span, .model-item span { color: var(--color-fg-secondary); }
	.langs { font-size: 11px; background: var(--color-accent); color: #fff; padding: 2px 6px; border-radius: 4px; }
	.model-main { flex: 1; display: flex; flex-direction: column; }
	.model-meta { font-size: 11px; }
	.progress { color: var(--color-accent); font-size: 12px; font-weight: 600; }
	.config-form { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 10px; padding: 14px; margin: 10px 0; display: flex; flex-direction: column; gap: 8px; }
	.config-item span { flex: 1; }
	.config-item .meta { font-size: 11px; font-family: monospace; }
	.hint { font-size: 12px; color: var(--color-fg-secondary); padding: 8px 0; }
</style>
