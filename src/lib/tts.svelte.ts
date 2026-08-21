// §10.3.9 TTS 播报 — Web Speech API 封装（系统 TTS，零依赖离线可用）
// 长文分段队列：逐段播放，支持暂停/恢复/停止/语速。

export const ttsState = $state({
  playing: false,
  paused: false,
  queue: [] as string[],
  current: 0,
  rate: 1,
  supported: typeof window !== 'undefined' && 'speechSynthesis' in window,
});

let utterance: SpeechSynthesisUtterance | null = null;
// 代际保护：stop/语速重启会使旧 utterance 的 onend/onerror 过期，防止并发推进队列（双音/跳段）
let generation = 0;

function speakSegment(text: string, onDone: () => void) {
  if (!ttsState.supported || !window.speechSynthesis) return;
  const gen = generation;
  utterance = new SpeechSynthesisUtterance(text);
  utterance.lang = 'zh-CN';
  utterance.rate = ttsState.rate;
  utterance.onend = () => {
    if (gen === generation) onDone();
  };
  utterance.onerror = () => {
    if (gen === generation) onDone();
  };
  window.speechSynthesis.speak(utterance);
}

/** 播放文本（内部按句子队列；也可直接传入服务端分段数组） */
export function ttsSpeak(text: string, rate = 1) {
  if (!ttsState.supported) return;
  ttsStop();
  ttsState.queue = text
    .split(/(?<=[。！？.!?；;])/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  ttsState.current = 0;
  ttsState.rate = rate;
  playNext();
}

/** 播放服务端分段数组（tts:speak 返回的 segments） */
export function ttsSpeakSegments(segments: string[], rate = 1) {
  if (!ttsState.supported) return;
  ttsStop();
  ttsState.queue = segments.filter((s) => s.trim().length > 0);
  ttsState.current = 0;
  ttsState.rate = rate;
  playNext();
}

function playNext() {
  if (!ttsState.supported) return;
  if (ttsState.current >= ttsState.queue.length) {
    ttsState.playing = false;
    ttsState.paused = false;
    return;
  }
  ttsState.playing = true;
  ttsState.paused = false;
  speakSegment(ttsState.queue[ttsState.current], () => {
    ttsState.current += 1;
    playNext();
  });
}

export function ttsPause() {
  if (!ttsState.supported || !ttsState.playing) return;
  window.speechSynthesis?.pause();
  ttsState.paused = true;
}

export function ttsResume() {
  if (!ttsState.supported || !ttsState.paused) return;
  window.speechSynthesis?.resume();
  ttsState.paused = false;
}

export function ttsStop() {
  if (!ttsState.supported) return;
  generation += 1; // 使在途 utterance 回调过期
  window.speechSynthesis?.cancel();
  utterance = null;
  ttsState.playing = false;
  ttsState.paused = false;
  ttsState.queue = [];
  ttsState.current = 0;
}

export function ttsSetRate(rate: number) {
  ttsState.rate = Math.min(2, Math.max(0.5, rate));
  if (ttsState.playing && !ttsState.paused) {
    // 重启当前段以应用语速（新代次，旧回调丢弃）
    const seg = ttsState.queue[ttsState.current];
    if (seg) {
      generation += 1;
      window.speechSynthesis?.cancel();
      utterance = null;
      speakSegment(seg, () => {
        ttsState.current += 1;
        playNext();
      });
    }
  }
}

/** 系统可用音色（Web Speech API） */
export function ttsVoices(): SpeechSynthesisVoice[] {
  if (!ttsState.supported) return [];
  return window.speechSynthesis?.getVoices() ?? [];
}

/** 从会议摘要提取「待办事项/行动项」小节（§10.3.9 播报待办数据源） */
export function extractActionItems(summary: string): string | null {
  if (!summary) return null;
  const lines = summary.split('\n');
  let capture = false;
  const out: string[] = [];
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) {
      if (capture) break; // 小节结束（空行）
      continue;
    }
    if (/^#{1,6}\s*(待办事项|行动项|待办)/.test(line)) {
      capture = true;
      continue;
    }
    if (capture) {
      out.push(line.replace(/^[-*]\s*/, ''));
    }
  }
  if (!capture) return null;
  const text = out.join('，');
  return text.length > 0 ? text : null;
}
