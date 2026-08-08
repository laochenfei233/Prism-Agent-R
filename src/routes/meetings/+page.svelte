<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { meetingApi, asrApi, type MeetingDto } from '$lib/api';
	import { invoke, listen } from '$lib/api/client';
	import Speaker from '$lib/components/meeting/Speaker.svelte';
	import { ttsSpeakSegments, extractActionItems, ttsState } from '$lib/tts.svelte';

	// ── 数据 ──────────────────────────────────────────────
	let meetings = $state<MeetingDto[]>([]);
	let selectedMeeting = $state<MeetingDto | null>(null);
	let loading = $state(false);
	let searchQuery = $state('');
	let editingTitle = $state(false);
	let titleValue = $state('');
	let generatingTitle = $state(false);

	// ── 详情 tab ─────────────────────────────────────────
	let activeTab = $state<'summary' | 'transcript'>('summary');
	let summarizing = $state(false);
	let cleaningTranscript = $state(false);

	// ── Q&A ──────────────────────────────────────────────
	let question = $state('');
	let answer = $state('');
	let asking = $state(false);

	// ── 录音 ─────────────────────────────────────────────
	let recording = $state(false);
	let recordingDuration = $state(0);
	let recordingTimer: ReturnType<typeof setInterval> | null = null;
	let liveTranscript = $state('');
	let audioCtx: AudioContext | null = null;
	let mediaStream: MediaStream | null = null;
	let recorderNode: AudioWorkletNode | null = null;
	let micStream: MediaStreamAudioSourceNode | null = null;
	let workletLoaded = false;
	let unlistenTranscript: (() => void) | null = null;

	// ── TTS 播报 ─────────────────────────────────────────
	let broadcastError = $state<string | null>(null);
	let broadcastBusy = $state(false);

	const filteredMeetings = $derived.by(() => {
		if (!searchQuery.trim()) return meetings;
		const q = searchQuery.toLowerCase();
		return meetings.filter((m) =>
			m.title.toLowerCase().includes(q) ||
			(m.summary || '').toLowerCase().includes(q)
		);
	});

	function formatRelativeDate(dateStr: string): string {
		const d = new Date(dateStr);
		const diffMs = Date.now() - d.getTime();
		const mins = Math.floor(diffMs / 60000);
		if (mins < 1) return '刚刚';
		if (mins < 60) return `${mins}分钟前`;
		if (mins < 1440) return `${Math.floor(mins / 60)}小时前`;
		if (mins < 10080) return `${Math.floor(mins / 1440)}天前`;
		return d.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' });
	}

	function formatDuration(seconds: number): string {
		if (!seconds) return '0:00';
		const mins = Math.floor(seconds / 60);
		const secs = seconds % 60;
		return `${mins}:${secs.toString().padStart(2, '0')}`;
	}

	function summaryPreview(m: MeetingDto): string {
		if (!m.summary) return '';
		const line = m.summary.split('\n').find((l) => l.trim() && !l.startsWith('#'));
		return (line || '').trim().slice(0, 40);
	}

	onMount(async () => {
		loading = true;
		try {
			meetings = await meetingApi.list();
			if (meetings.length > 0) {
				selectedMeeting = await meetingApi.get(meetings[0].id);
			}
		} catch (e) { console.error(e); }
		loading = false;
	});
	onDestroy(() => { stopRecording(); unlistenTranscript?.(); });

	// ── 会议操作 ─────────────────────────────────────────
	async function createMeeting() {
		const title = `会议 ${new Date().toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}`;
		try {
			const m = await meetingApi.create(title);
			meetings = [m, ...meetings];
			await selectMeeting(m);
		} catch (e) { console.error(e); }
	}

	async function selectMeeting(m: MeetingDto) {
		try {
			selectedMeeting = await meetingApi.get(m.id);
			titleValue = selectedMeeting.title;
			answer = '';
			liveTranscript = '';
		} catch (e) { console.error(e); }
	}

	async function deleteMeeting(m: MeetingDto) {
		if (!confirm(`确定删除「${m.title}」？`)) return;
		try {
			await meetingApi.delete(m.id);
			meetings = meetings.filter(x => x.id !== m.id);
			if (selectedMeeting?.id === m.id) {
				selectedMeeting = meetings[0] ? await meetingApi.get(meetings[0].id) : null;
				if (selectedMeeting) titleValue = selectedMeeting.title;
			}
		} catch (e) { console.error(e); }
	}

	async function generateTitle() {
		if (!selectedMeeting?.transcript || generatingTitle) return;
		generatingTitle = true;
		try {
			const t = await meetingApi.qa(selectedMeeting.id, '请用一句话（20字以内）概括这个会议的主题，只输出标题，不要引号。');
			const newTitle = t.trim();
			if (newTitle) {
				titleValue = newTitle;
				selectedMeeting = { ...selectedMeeting, title: newTitle };
				meetings = meetings.map(x => x.id === selectedMeeting?.id ? { ...x, title: newTitle } : x);
			}
		} catch (e) { console.error(e); }
		generatingTitle = false;
	}

	async function saveTitle() {
		editingTitle = false;
		// TODO: 后端无重命名命令，仅本地更新
		if (titleValue.trim() && selectedMeeting && titleValue !== selectedMeeting.title) {
			selectedMeeting = { ...selectedMeeting, title: titleValue.trim() };
			meetings = meetings.map(x => x.id === selectedMeeting?.id ? { ...x, title: titleValue.trim() } : x);
		}
	}

	// ── 摘要 / 转录 ──────────────────────────────────────
	async function generateSummary() {
		if (!selectedMeeting) return;
		summarizing = true;
		try {
			const summary = await meetingApi.summary(selectedMeeting.id);
			selectedMeeting = { ...selectedMeeting, summary };
			meetings = meetings.map(x => x.id === selectedMeeting?.id ? { ...x, summary } : x);
		} catch (e) { console.error(e); }
		summarizing = false;
	}

	async function cleanTranscript() {
		if (!selectedMeeting) return;
		cleaningTranscript = true;
		try {
			const cleaned = await meetingApi.clean(selectedMeeting.id);
			selectedMeeting = { ...selectedMeeting, transcript: cleaned };
		} catch (e) { console.error(e); }
		cleaningTranscript = false;
	}

	async function exportContent(content: string, filename: string) {
		const blob = new Blob([content], { type: 'text/markdown;charset=utf-8' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url; a.download = `${filename}.md`; a.click();
		URL.revokeObjectURL(url);
	}

	// ── Q&A ──────────────────────────────────────────────
	async function handleAsk() {
		if (!question.trim() || !selectedMeeting) return;
		asking = true;
		answer = '';
		try {
			answer = await meetingApi.qa(selectedMeeting.id, question.trim());
		} catch (e) { console.error(e); answer = '回答生成失败'; }
		asking = false;
	}

	// ── 录音 ─────────────────────────────────────────────
	async function toggleRecording() {
		if (recording) {
			await stopRecording();
			return;
		}
		if (!selectedMeeting) return;
		try {
			// 1. 通知后端先建 stream（使用设置页配置的默认 ASR 后端）
			await asrApi.startRecording(selectedMeeting.id);

			// 2. 监听实时转录事件
			unlistenTranscript = await listen<{ meeting_id: string; index: number; text: string; is_final: boolean }>(
				'meeting:transcript', (e) => {
					if (e.meeting_id === selectedMeeting?.id) {
						liveTranscript += e.text;
					}
				}
			);

			// 3. 前端采集
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
			recordingDuration = 0;
			recordingTimer = setInterval(() => { recordingDuration += 1; }, 1000);
		} catch (e) {
			console.error('录音启动失败:', e);
			alert('录音启动失败：' + e);
		}
	}

	async function loadWorklet() {
		if (workletLoaded) return;
		const code = `
			class PcmRecorder extends AudioWorkletProcessor {
				process(inputs) {
					const input = inputs[0];
					if (!input || !input[0]) return true;
					const samples = input[0];
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
		if (recordingTimer) { clearInterval(recordingTimer); recordingTimer = null; }
		recorderNode?.disconnect();
		micStream?.disconnect();
		mediaStream?.getTracks().forEach(t => t.stop());
		recorderNode = null; micStream = null; mediaStream = null;
		if (audioCtx) { audioCtx.close(); audioCtx = null; }
		if (selectedMeeting) {
			try {
				const res = await asrApi.stopRecording(selectedMeeting.id);
				// 落库后刷新会议数据，合并实时转录
				const updated = await meetingApi.get(selectedMeeting.id);
				selectedMeeting = updated;
				meetings = meetings.map(x => x.id === updated.id ? updated : x);
				liveTranscript = '';
			} catch (e) { console.error(e); }
		}
		unlistenTranscript?.();
		unlistenTranscript = null;
	}

	// ── TTS 播报 ─────────────────────────────────────────
	async function broadcastActionItems() {
		if (!selectedMeeting?.summary) {
			broadcastError = '暂无摘要，请先生成摘要';
			return;
		}
		const items = extractActionItems(selectedMeeting.summary);
		if (!items) {
			broadcastError = '摘要中未找到「待办事项/行动项」小节';
			return;
		}
		broadcastError = null;
		broadcastBusy = true;
		try {
			const res = await invoke<{ backend: string; segments: string[] }>('tts_speak', {
				text: items,
				lang: 'zh-CN',
				rate: 1
			});
			ttsSpeakSegments(res.segments, 1);
		} catch (e) {
			broadcastError = String(e);
		} finally {
			broadcastBusy = false;
		}
	}
</script>

<div class="meeting-shell">
	<!-- 左栏：会议列表 -->
	<aside class="meeting-list-pane">
		<div class="list-header">
			<input
				class="search-input"
				placeholder="搜索会议..."
				bind:value={searchQuery}
			/>
			<button class="add-btn" onclick={createMeeting} title="新建会议">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
			</button>
		</div>
		<div class="list-body">
			{#if loading}
				<div class="empty">加载中...</div>
			{:else if filteredMeetings.length === 0}
				<div class="empty">
					<p>暂无会议</p>
					<button class="text-btn" onclick={createMeeting}>新建</button>
				</div>
			{:else}
				{#each filteredMeetings as m}
					<div
						class="list-item"
						class:selected={selectedMeeting?.id === m.id}
						onclick={() => selectMeeting(m)}
						role="button"
						tabindex="0"
						onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectMeeting(m); } }}
					>
						<div class="item-icon">
							<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/></svg>
						</div>
						<div class="item-body">
							<div class="item-title-row">
								<span class="item-title">{m.title}</span>
								{#if m.recording_duration > 0}
									<span class="item-duration">{formatDuration(m.recording_duration)}</span>
								{/if}
							</div>
							<div class="item-meta">
								<span>{formatRelativeDate(m.date)}</span>
								{#if m.summary}
									<span class="badge-summarized">已摘要</span>
								{/if}
							</div>
							{#if summaryPreview(m)}
								<div class="item-preview">{summaryPreview(m)}</div>
							{/if}
						</div>
						<button
							class="delete-btn"
							onclick={(e) => { e.stopPropagation(); deleteMeeting(m); }}
							title="删除"
							aria-label="删除会议"
						>
							<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
						</button>
					</div>
				{/each}
			{/if}
		</div>
	</aside>

	<!-- 右栏：详情 -->
	<main class="meeting-detail-pane">
		{#if selectedMeeting}
			{@const m = selectedMeeting}
			<!-- 头部 -->
			<div class="detail-header">
				<div class="header-left">
					<div class="title-row">
						{#if editingTitle}
							<input
								class="title-input"
								bind:value={titleValue}
								onblur={saveTitle}
								onkeydown={(e) => {
									if (e.key === 'Enter') saveTitle();
									if (e.key === 'Escape') { titleValue = m.title; editingTitle = false; }
								}}
							/>
						{:else}
							<button class="title" onclick={() => { editingTitle = true; titleValue = m.title; }} title="点击编辑标题">
								{m.title}
							</button>
						{/if}
						{#if m.transcript && !editingTitle}
							<button class="icon-btn" onclick={generateTitle} title="自动生成标题" disabled={generatingTitle}>
								<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 3v18"/><path d="M5 7l7 7 7-7"/><path d="M5 17h14"/></svg>
							</button>
						{/if}
					</div>
					<div class="meta-row">
						<span>{new Date(m.date).toLocaleString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric', hour: '2-digit', minute: '2-digit' })}</span>
						{#if m.recording_duration > 0}
							<span>·</span>
							<span>录音 {formatDuration(m.recording_duration)}</span>
						{/if}
						{#if m.participants.length > 0}
							<span>·</span>
							<span>{m.participants.join(', ')}</span>
						{/if}
					</div>
				</div>
				<div class="header-actions">
					<button
						class="rec-btn"
						class:recording={recording}
						onclick={toggleRecording}
					>
						{#if recording}
							<span class="rec-dot"></span>
							{formatDuration(recordingDuration)}
						{:else}
							<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/></svg>
							录音
						{/if}
					</button>
					<button class="ghost-btn" onclick={() => exportContent(m.summary || m.transcript, `${m.title}_导出`)}>导出</button>
				</div>
			</div>

			<!-- tab 切换 -->
			<div class="tabs">
				<button class="tab" class:active={activeTab === 'summary'} onclick={() => activeTab = 'summary'}>摘要</button>
				<button class="tab" class:active={activeTab === 'transcript'} onclick={() => activeTab = 'transcript'}>转录</button>
			</div>

			<!-- 内容 -->
			<div class="detail-body">
				{#if activeTab === 'summary'}
					{#if m.summary}
						<div class="content-head">
							<span class="char-count">{m.summary.length} 字</span>
							<div class="head-actions">
								<button class="mini-btn" onclick={generateSummary} disabled={summarizing}>
									{summarizing ? '生成中...' : '重新生成'}
								</button>
								<button class="mini-btn" onclick={broadcastActionItems} disabled={broadcastBusy}>🔊 播报待办</button>
								<button class="mini-btn" onclick={() => exportContent(m.summary, `${m.title}_摘要`)}>导出</button>
							</div>
						</div>
						{#if broadcastError}
							<div class="broadcast-error">{broadcastError}</div>
						{/if}
						<div class="content-box markdown">{m.summary}</div>
						{#if ttsState.supported && ttsState.queue.length > 0}
							<div class="speaker-host"><Speaker /></div>
						{/if}
					{:else}
						<div class="empty-state">
							<p>暂无摘要</p>
							{#if m.transcript}
								<button class="primary-btn" onclick={generateSummary} disabled={summarizing}>
									{summarizing ? '生成中...' : '生成摘要'}
								</button>
							{/if}
						</div>
					{/if}
				{:else}
					<!-- 转录 tab -->
					{#if m.transcript || liveTranscript || recording}
						<div class="content-head">
							<span class="char-count">
								{(m.transcript.length + liveTranscript.length)} 字
								{#if recording}<span class="rec-live"> 录制中...</span>{/if}
							</span>
							<div class="head-actions">
								{#if !recording && m.transcript}
									<button class="mini-btn" onclick={cleanTranscript} disabled={cleaningTranscript}>
										{cleaningTranscript ? '整理中...' : '整理转录'}
									</button>
								{/if}
								<button class="mini-btn" onclick={() => exportContent(m.transcript, `${m.title}_转录`)}>导出</button>
							</div>
						</div>
						<div class="content-box markdown transcript">{m.transcript}{liveTranscript}</div>
					{:else}
						<div class="empty-state">
							<p>暂无转录内容</p>
							<button class="primary-btn" onclick={toggleRecording}>开始录音</button>
						</div>
					{/if}
				{/if}
			</div>

			<!-- Q&A 底部 -->
			{#if m.transcript}
				<div class="qa-bar">
					<input
						class="qa-input"
						placeholder="对会议内容提问..."
						bind:value={question}
						onkeydown={(e) => e.key === 'Enter' && handleAsk()}
						disabled={asking}
					/>
					<button class="qa-btn" onclick={handleAsk} disabled={asking || !question.trim()} aria-label="发送提问">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m22 2-7 20-4-9-9-4Z"/><path d="M22 2 11 13"/></svg>
					</button>
				</div>
				{#if answer}
					<div class="qa-answer">{answer}</div>
				{/if}
			{/if}
		{:else}
			<div class="empty-state center">
				<p>暂无会议</p>
				<button class="primary-btn" onclick={createMeeting}>新建会议</button>
			</div>
		{/if}
	</main>
</div>

<style>
	.meeting-shell { display: flex; height: 100vh; }

	/* ── 左栏列表 ─────────────────────────────── */
	.meeting-list-pane {
		width: 280px;
		min-width: 280px;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-secondary);
		border-right: 1px solid var(--color-separator);
	}
	.list-header { display: flex; gap: 8px; padding: 12px; border-bottom: 1px solid var(--color-separator); }
	.search-input {
		flex: 1;
		padding: 6px 10px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 13px;
		outline: none;
	}
	.search-input:focus { border-color: var(--color-accent); }
	.add-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border-radius: 8px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		cursor: pointer;
		flex-shrink: 0;
	}
	.add-btn:hover { opacity: 0.9; }
	.list-body { flex: 1; overflow-y: auto; padding: 8px; }
	.empty { text-align: center; padding: 40px 12px; color: var(--color-fg-secondary); font-size: 13px; }
	.text-btn { border: none; background: none; color: var(--color-accent); cursor: pointer; font-size: 13px; margin-top: 8px; }
	.list-item {
		display: flex;
		align-items: flex-start;
		gap: 10px;
		padding: 10px;
		border-radius: 10px;
		cursor: pointer;
		position: relative;
		transition: background 0.15s;
	}
	.list-item:hover { background: var(--color-bg-tertiary); }
	.list-item.selected { background: var(--color-bg-tertiary); box-shadow: inset 2px 0 0 var(--color-accent); }
	.item-icon {
		width: 28px;
		height: 28px;
		border-radius: 8px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-bg-tertiary);
		color: var(--color-fg-secondary);
		flex-shrink: 0;
	}
	.list-item.selected .item-icon { background: var(--color-accent); color: #fff; }
	.item-body { flex: 1; min-width: 0; }
	.item-title-row { display: flex; align-items: center; gap: 6px; }
	.item-title { font-size: 13px; font-weight: 500; color: var(--color-fg); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
	.item-duration { font-size: 11px; color: var(--color-fg-secondary); flex-shrink: 0; }
	.item-meta { display: flex; align-items: center; gap: 6px; margin-top: 2px; font-size: 11px; color: var(--color-fg-secondary); }
	.badge-summarized { color: var(--color-accent); font-size: 10px; }
	.item-preview { margin-top: 4px; font-size: 11px; color: var(--color-fg-tertiary, #8b93a7); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
	.delete-btn {
		position: absolute;
		top: 8px;
		right: 8px;
		display: none;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
	}
	.list-item:hover .delete-btn { display: flex; }
	.delete-btn:hover { background: #ff4444; color: #fff; }

	/* ── 右栏详情 ─────────────────────────────── */
	.meeting-detail-pane { flex: 1; min-width: 0; display: flex; flex-direction: column; overflow: hidden; }
	.detail-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 16px;
		padding: 16px 24px;
		border-bottom: 1px solid var(--color-separator);
	}
	.header-left { min-width: 0; }
	.title-row { display: flex; align-items: center; gap: 8px; }
	.title { font-size: 20px; font-weight: 600; color: var(--color-fg); margin: 0; cursor: text; background: none; border: none; padding: 0; text-align: left; }
	.title:hover { opacity: 0.75; }
	.title-input { font-size: 20px; font-weight: 600; color: var(--color-fg); border: none; border-bottom: 2px solid var(--color-accent); background: transparent; outline: none; padding: 0; }
	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
	}
	.icon-btn:hover { background: var(--color-bg-tertiary); }
	.meta-row { display: flex; align-items: center; gap: 6px; margin-top: 4px; font-size: 12px; color: var(--color-fg-secondary); flex-wrap: wrap; }
	.header-actions { display: flex; gap: 8px; flex-shrink: 0; }
	.rec-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 14px;
		border-radius: 999px;
		border: none;
		font-size: 12px;
		font-weight: 500;
		cursor: pointer;
		background: var(--color-accent);
		color: #fff;
		transition: background 0.15s;
	}
	.rec-btn.recording { background: #ff4444; }
	.rec-dot { width: 8px; height: 8px; border-radius: 50%; background: #fff; animation: pulse 1s infinite; }
	@keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.3; } }
	.ghost-btn {
		padding: 6px 14px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: 12px;
		cursor: pointer;
	}
	.ghost-btn:hover { background: var(--color-bg-tertiary); }

	/* ── tab ──────────────────────────────────── */
	.tabs { display: flex; gap: 4px; padding: 0 24px; border-bottom: 1px solid var(--color-separator); }
	.tab {
		padding: 10px 4px;
		margin-right: 16px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		position: relative;
	}
	.tab.active { color: var(--color-fg); }
	.tab.active::after {
		content: '';
		position: absolute;
		inset-inline: 0;
		bottom: -1px;
		height: 2px;
		background: var(--color-accent);
	}

	/* ── 内容 ──────────────────────────────────── */
	.detail-body { flex: 1; overflow-y: auto; padding: 16px 24px; }
	.content-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; }
	.char-count { font-size: 12px; color: var(--color-fg-secondary); }
	.head-actions { display: flex; gap: 6px; }
	.mini-btn {
		padding: 4px 10px;
		border-radius: 6px;
		border: 1px solid var(--color-separator);
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: 11px;
		cursor: pointer;
	}
	.mini-btn:hover { background: var(--color-bg-tertiary); }
	.mini-btn:disabled { opacity: 0.5; cursor: not-allowed; }
	.content-box {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-separator);
		border-radius: 10px;
		padding: 16px;
		font-size: 14px;
		color: var(--color-fg);
		white-space: pre-wrap;
		line-height: 1.7;
	}
	.transcript { max-height: calc(100vh - 320px); overflow-y: auto; }
	.broadcast-error { color: #ff453a; font-size: 12px; margin-bottom: 8px; }
	.speaker-host { margin-top: 10px; }
	.empty-state { text-align: center; padding: 48px; color: var(--color-fg-secondary); }
	.empty-state.center { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; }
	.primary-btn {
		margin-top: 12px;
		padding: 8px 20px;
		border-radius: 10px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: 13px;
		cursor: pointer;
	}
	.primary-btn:disabled { opacity: 0.5; cursor: not-allowed; }
	.rec-live { color: #ff453a; }

	/* ── Q&A ──────────────────────────────────── */
	.qa-bar {
		display: flex;
		gap: 8px;
		padding: 12px 24px;
		border-top: 1px solid var(--color-separator);
	}
	.qa-input {
		flex: 1;
		padding: 9px 14px;
		border-radius: 10px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: 13px;
		outline: none;
	}
	.qa-input:focus { border-color: var(--color-accent); }
	.qa-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		border-radius: 10px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		cursor: pointer;
		flex-shrink: 0;
	}
	.qa-btn:disabled { opacity: 0.5; cursor: not-allowed; }
	.qa-answer {
		margin: 0 24px 12px;
		padding: 12px;
		border-radius: 10px;
		background: var(--color-bg-secondary);
		font-size: 13px;
		line-height: 1.7;
		color: var(--color-fg);
	}
</style>
