// 设置注册表：全部可配置设置项的定义（key / 分组 / 类型 / 默认值 / 描述）
//
// 注册表是设置中心的唯一权威清单：前端 settings_get_all 拉取后按分组渲染，
// settings_set 按 key 写 preferences（类型校验 + 范围校验）。
// 新增设置项只需在此追加一条 spec。

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingGroup {
    /// 模型服务：Provider / 模型 / 翻译模型 / TTS
    ModelService,
    /// Agent：默认参数 / 反思 / 目标监控
    Agent,
    /// 记忆：索引重建等
    Memory,
    /// 工具：MCP / 技能
    Tools,
    /// RAG：分块 / 检索 / 混合权重
    Rag,
    /// 会议：ASR / TTS
    Meeting,
    /// 安全：护栏
    Security,
    /// 高级：Token 预算 / 保留策略 / 工作区
    Advanced,
}

impl SettingGroup {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ModelService => "模型服务",
            Self::Agent => "Agent",
            Self::Memory => "记忆",
            Self::Tools => "工具",
            Self::Rag => "RAG",
            Self::Meeting => "会议",
            Self::Security => "安全",
            Self::Advanced => "高级",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingKind {
    Bool,
    Int,
    Float,
    String,
    Select,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub group: SettingGroup,
    pub kind: SettingKind,
    pub default: serde_json::Value,
    pub description: &'static str,
    /// Select 类型的候选项
    pub options: Option<Vec<&'static str>>,
    /// 数值范围与步进
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

/// 全部可配置设置项（注册表）
pub fn specs() -> Vec<SettingSpec> {
    vec![
        // ── Agent ───────────────────────────────────────
        SettingSpec {
            key: "agent.default.temperature",
            label: "默认温度",
            group: SettingGroup::Agent,
            kind: SettingKind::Float,
            default: serde_json::json!(0.7),
            description: "新建 Agent 的默认采样温度（0–2，越高越随机）",
            options: None,
            min: Some(0.0),
            max: Some(2.0),
            step: Some(0.1),
        },
        SettingSpec {
            key: "agent.default.max_tokens",
            label: "默认最大输出 Token",
            group: SettingGroup::Agent,
            kind: SettingKind::Int,
            default: serde_json::json!(8192),
            description: "新建 Agent 的默认最大输出 token 数",
            options: None,
            min: Some(256.0),
            max: Some(128_000.0),
            step: Some(256.0),
        },
        SettingSpec {
            key: "reflection.enabled",
            label: "反思循环",
            group: SettingGroup::Agent,
            kind: SettingKind::Bool,
            default: serde_json::json!(false),
            description: "启用后 Agent 生成输出会进入「生成-评审」子循环，用于高精度场景",
            options: None,
            min: None,
            max: None,
            step: None,
        },
        SettingSpec {
            key: "reflection.max_iterations",
            label: "反思最大轮数",
            group: SettingGroup::Agent,
            kind: SettingKind::Int,
            default: serde_json::json!(3),
            description: "反思循环的最大评审-重试轮数",
            options: None,
            min: Some(1.0),
            max: Some(10.0),
            step: Some(1.0),
        },
        SettingSpec {
            key: "goal.achieved_threshold",
            label: "目标达成阈值",
            group: SettingGroup::Agent,
            kind: SettingKind::Float,
            default: serde_json::json!(0.8),
            description: "目标监控判定达成的最低加权得分（0–1）",
            options: None,
            min: Some(0.0),
            max: Some(1.0),
            step: Some(0.05),
        },
        // ── RAG ─────────────────────────────────────────
        SettingSpec {
            key: "rag.chunk_size",
            label: "分块大小（字符）",
            group: SettingGroup::Rag,
            kind: SettingKind::Int,
            default: serde_json::json!(1000),
            description: "文档切块的目标长度（字符），越小检索越细、上下文越碎",
            options: None,
            min: Some(200.0),
            max: Some(2000.0),
            step: Some(100.0),
        },
        SettingSpec {
            key: "rag.chunk_overlap",
            label: "分块重叠（字符）",
            group: SettingGroup::Rag,
            kind: SettingKind::Int,
            default: serde_json::json!(200),
            description: "相邻分块的重叠长度，用于保持跨块语义连续",
            options: None,
            min: Some(0.0),
            max: Some(500.0),
            step: Some(50.0),
        },
        SettingSpec {
            key: "rag.top_k",
            label: "检索返回条数",
            group: SettingGroup::Rag,
            kind: SettingKind::Int,
            default: serde_json::json!(5),
            description: "RAG 检索默认返回的分块数量",
            options: None,
            min: Some(1.0),
            max: Some(20.0),
            step: Some(1.0),
        },
        SettingSpec {
            key: "rag.vector_weight",
            label: "向量检索权重",
            group: SettingGroup::Rag,
            kind: SettingKind::Float,
            default: serde_json::json!(0.7),
            description: "混合检索中向量得分的权重，BM25 权重自动为 1 − 该项",
            options: None,
            min: Some(0.0),
            max: Some(1.0),
            step: Some(0.1),
        },
        // ── 安全 ────────────────────────────────────────
        SettingSpec {
            key: "guardrail.max_chars",
            label: "输入长度上限（字符）",
            group: SettingGroup::Security,
            kind: SettingKind::Int,
            default: serde_json::json!(100_000),
            description: "超出后护栏对输入给出警告",
            options: None,
            min: Some(1000.0),
            max: Some(1_000_000.0),
            step: Some(1000.0),
        },
        SettingSpec {
            key: "guardrail.injection_enabled",
            label: "注入检测",
            group: SettingGroup::Security,
            kind: SettingKind::Bool,
            default: serde_json::json!(true),
            description: "拦截「忽略之前指令」等提示注入模式",
            options: None,
            min: None,
            max: None,
            step: None,
        },
        // ── 高级 ────────────────────────────────────────
        SettingSpec {
            key: "token_budget.chat",
            label: "对话 Token 预算",
            group: SettingGroup::Advanced,
            kind: SettingKind::Int,
            default: serde_json::json!(100_000),
            description: "单轮对话的 Token 预算，用于工具输出裁剪与上下文压力计算",
            options: None,
            min: Some(10_000.0),
            max: Some(500_000.0),
            step: Some(10_000.0),
        },
        SettingSpec {
            key: "trace.retain",
            label: "轨迹保留条数",
            group: SettingGroup::Advanced,
            kind: SettingKind::Int,
            default: serde_json::json!(1000),
            description: "Agent 执行轨迹（agent_traces）最多保留的条数，超出自动清理",
            options: None,
            min: Some(100.0),
            max: Some(10_000.0),
            step: Some(100.0),
        },
        SettingSpec {
            key: "meeting.audio_buffer_secs",
            label: "音频缓冲保留时长（秒）",
            group: SettingGroup::Advanced,
            kind: SettingKind::Int,
            default: serde_json::json!(30),
            description: "会议录音在内存中的缓冲时长，超过后丢弃最旧片段",
            options: None,
            min: Some(10.0),
            max: Some(120.0),
            step: Some(5.0),
        },
        // ── 会议 / TTS ──────────────────────────────────
        SettingSpec {
            key: "tts.lang",
            label: "TTS 播报语言",
            group: SettingGroup::Meeting,
            kind: SettingKind::String,
            default: serde_json::json!("zh-CN"),
            description: "语音播报的语言代码，如 zh-CN / en-US",
            options: None,
            min: None,
            max: None,
            step: None,
        },
        SettingSpec {
            key: "tts.rate",
            label: "TTS 播报语速",
            group: SettingGroup::Meeting,
            kind: SettingKind::Float,
            default: serde_json::json!(1.0),
            description: "语音播报语速倍率（0.5–2）",
            options: None,
            min: Some(0.5),
            max: Some(2.0),
            step: Some(0.1),
        },
    ]
}

/// 按 key 查找注册表项
pub fn spec_by_key(key: &str) -> Option<SettingSpec> {
    specs().into_iter().find(|s| s.key == key)
}

/// 校验 value 是否符合该项的类型与范围；返回规范化后的 JSON 值
pub fn validate(spec: &SettingSpec, value: &serde_json::Value) -> Result<serde_json::Value, String> {
    match spec.kind {
        SettingKind::Bool => {
            let v = value.as_bool().ok_or_else(|| format!("{} 需要布尔值", spec.label))?;
            Ok(serde_json::json!(v))
        }
        SettingKind::Int => {
            let v = value.as_i64().ok_or_else(|| format!("{} 需要整数", spec.label))?;
            let (min, max) = (spec.min.unwrap_or(0.0), spec.max.unwrap_or(i64::MAX as f64));
            if (v as f64) < min || (v as f64) > max {
                return Err(format!("{} 超出范围 [{min}, {max}]", spec.label));
            }
            Ok(serde_json::json!(v))
        }
        SettingKind::Float => {
            let v = value.as_f64().ok_or_else(|| format!("{} 需要数值", spec.label))?;
            if let Some(min) = spec.min {
                if v < min { return Err(format!("{} 小于下限 {min}", spec.label)); }
            }
            if let Some(max) = spec.max {
                if v > max { return Err(format!("{} 大于上限 {max}", spec.label)); }
            }
            Ok(serde_json::json!(v))
        }
        SettingKind::String | SettingKind::Select => {
            let v = value.as_str().ok_or_else(|| format!("{} 需要字符串", spec.label))?;
            if let Some(opts) = &spec.options {
                if !opts.contains(&v) {
                    return Err(format!("{} 的值不在允许选项内", spec.label));
                }
            }
            Ok(serde_json::json!(v))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_all_groups() {
        let groups: std::collections::HashSet<SettingGroup> =
            specs().into_iter().map(|s| s.group).collect();
        assert_eq!(groups.len(), 5); // Agent / Rag / Security / Advanced / Meeting
        for s in specs() {
            assert!(!s.key.is_empty() && !s.label.is_empty());
        }
    }

    #[test]
    fn keys_are_unique() {
        let mut keys: Vec<&str> = specs().into_iter().map(|s| s.key).collect();
        let len = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), len, "注册表存在重复 key");
    }

    #[test]
    fn validate_checks_type_and_range() {
        let spec = spec_by_key("rag.chunk_size").unwrap();
        assert!(validate(&spec, &serde_json::json!(1500)).is_ok());
        assert!(validate(&spec, &serde_json::json!(100)).is_err(), "低于下限应拒绝");
        assert!(validate(&spec, &serde_json::json!("x")).is_err(), "非整数应拒绝");

        let bool_spec = spec_by_key("reflection.enabled").unwrap();
        assert!(validate(&bool_spec, &serde_json::json!(true)).is_ok());
        assert!(validate(&bool_spec, &serde_json::json!(1)).is_err());
    }

    #[test]
    fn defaults_pass_validation() {
        // settings_get_all 依赖此不变量：所有默认值必须通过自身校验，
        // 否则前端首屏拉取即报错
        for spec in specs() {
            let result = validate(&spec, &spec.default);
            assert!(
                result.is_ok(),
                "设置项 {} 的默认值 {} 未通过校验: {:?}",
                spec.key,
                spec.default,
                result.err()
            );
        }
    }
}
