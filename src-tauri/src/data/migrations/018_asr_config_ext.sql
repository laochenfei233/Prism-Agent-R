-- ASR 配置扩展：本地模型路径 + 额外参数（自定义后端扩展，不框死字段）
ALTER TABLE asr_configs ADD COLUMN model_path TEXT;
ALTER TABLE asr_configs ADD COLUMN extra TEXT;
