-- §10.3 会议状态机：idle → recording ⇄ paused → transcribing → ready / cancelled
ALTER TABLE meetings ADD COLUMN status TEXT NOT NULL DEFAULT 'idle';
