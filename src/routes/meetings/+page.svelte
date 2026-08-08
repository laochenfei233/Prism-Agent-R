<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { meetingApi, asrApi, type MeetingDto } from '$lib/api';
	import { invoke } from '$lib/api/client';
	import Speaker from '$lib/components/meeting/Speaker.svelte';
	import { ttsSpeakSegments, extractActionItems, ttsState } from '$lib/tts.svelte';

	let meetings = $state<MeetingDto[]>([]);
	let selectedMeeting = $state<MeetingDto | null>(null);
	let showCreate = $state(false);
	let newTitle = $state('');
	let newParticipants = $state('');
	let loading = $state(false);
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
		} catch (e) { console.error(e); }
		loading = false;
	});
	onDestroy(() => { stopRecording(); });

	// ── 录音 ──────────────────────────────────────────────
	async function startRecording() {
		if (!selectedMeeting) return;
		try {
			// 1. 通知后端先建 stream（使用设置页配置的默认 ASR 后端）
			await asrApi.startRecording(selectedMeeting.id);

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

	// ── TTS 播报（§10.3.9） ───────────────────────────────
	let broadcastError = $state<string | null>(null);
	let broadcastBusy = $state(false);

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
			// 服务端分段（长文按句切分），前端 Web Speech API 顺序播放
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

<div class="page">
	<header class="page-header">
		<h1>会议纪要</h1>
	</header>

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
						<div class="section-head">
							<h3>摘要</h3>
							<button class="btn-ghost btn-sm" onclick={broadcastActionItems} disabled={broadcastBusy}>
								🔊 播报待办
							</button>
						</div>
						{#if broadcastError}
							<div class="broadcast-error">{broadcastError}</div>
						{/if}
						<div class="content-box">{selectedMeeting.summary}</div>
						{#if ttsState.supported && ttsState.queue.length > 0}
							<div class="speaker-host">
								<Speaker />
							</div>
						{/if}
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
</div>

<style>
	.page { padding: 24px 32px; max-width: 1400px; margin: 0 auto; }
	.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px; }
	.page-header h1 { font-size: 24px; font-weight: 600; color: var(--color-fg); margin: 0; }
	.btn-primary { padding: 8px 16px; border-radius: 8px; border: none; background: var(--color-accent); color: #fff; font-size: 14px; font-weight: 500; cursor: pointer; }
	.btn-sm { padding: 5px 12px; font-size: 12px; }
	.btn-ghost { padding: 8px 16px; border-radius: 8px; border: 1px solid var(--color-separator); background: transparent; color: var(--color-fg-secondary); font-size: 14px; cursor: pointer; }
	.btn-danger-sm { padding: 4px 8px; border-radius: 6px; border: none; background: #ff4444; color: #fff; font-size: 12px; cursor: pointer; }
	.create-form { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 16px; margin: 16px 0; display: flex; flex-direction: column; gap: 8px; }
	.create-form input { padding: 8px 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 14px; outline: none; }
	.create-form input:focus { border-color: var(--color-accent); }
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
	.section-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
	.section-head h3 { margin: 0; }
	.broadcast-error { color: var(--color-red, #ff453a); font-size: 12px; margin-bottom: 8px; }
	.speaker-host { margin-top: 10px; }
	.content-box { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 10px; padding: 16px; font-size: 14px; color: var(--color-fg); white-space: pre-wrap; line-height: 1.6; }
	.transcript { max-height: 400px; overflow-y: auto; }
</style>
