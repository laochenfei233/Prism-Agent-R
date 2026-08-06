use crate::core::autoagents::workflow::{Workflow, WorkflowStage, TaskInput, InputKind};
use crate::data::models::{WorkflowDto, WorkflowRow};
use crate::data::Database;
use crate::utils::error::AppError;

// ── 工作流服务 ────────────────────────────────────────────

pub struct WorkflowService {
    db: Database,
}

impl WorkflowService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 列出所有工作流
    pub async fn list(&self) -> Result<Vec<WorkflowDto>, AppError> {
        let rows = sqlx::query_as::<_, WorkflowRow>(
            "SELECT id, name, description, definition, created_at, updated_at FROM workflows ORDER BY created_at"
        )
        .fetch_all(&self.db.pool)
        .await?;

        Ok(rows.into_iter().map(|r| WorkflowDto {
            id: r.id,
            name: r.name,
            description: r.description,
            definition: serde_json::from_str(&r.definition).unwrap_or(serde_json::json!({})),
        }).collect())
    }

    /// 保存工作流
    pub async fn save(
        &self,
        name: String,
        description: Option<String>,
        definition: serde_json::Value,
    ) -> Result<WorkflowDto, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let def_json = serde_json::to_string(&definition).unwrap_or_else(|_| "{}".into());

        sqlx::query(
            "INSERT INTO workflows (id, name, description, definition, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&name)
        .bind(&description)
        .bind(&def_json)
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;

        Ok(WorkflowDto {
            id,
            name,
            description,
            definition,
        })
    }

    /// 获取预置工作流定义
    pub fn builtin_workflows() -> Vec<Workflow> {
        vec![
            Self::deep_research_workflow(),
            Self::code_review_workflow(),
            Self::brainstorm_workflow(),
            Self::translate_proofread_workflow(),
        ]
    }

    /// 深度研究工作流
    fn deep_research_workflow() -> Workflow {
        Workflow {
            id: "deep-research".to_string(),
            name: "深度研究".to_string(),
            description: Some("搜索资料 → 分析综合 → 生成报告".to_string()),
            inputs: vec![
                TaskInput {
                    key: "topic".to_string(),
                    label: "研究主题".to_string(),
                    kind: InputKind::Text,
                    default: None,
                    required: true,
                },
                TaskInput {
                    key: "depth".to_string(),
                    label: "深度".to_string(),
                    kind: InputKind::Select,
                    default: Some(serde_json::json!("标准")),
                    required: false,
                },
            ],
            stages: vec![
                WorkflowStage {
                    id: "stage1".to_string(),
                    name: "资料搜集".to_string(),
                    role: "researcher".to_string(),
                    prompt_template: "研究主题：{{topic}}\n深度：{{depth}}\n\n请使用网络搜索工具全面搜集与该主题相关的资料，覆盖：背景、关键概念、主要参与方、最新进展。输出带来源链接的资料汇编（Markdown）。".to_string(),
                    tools: vec!["web_search".to_string(), "read_file".to_string()],
                    max_iterations: 15,
                    depends_on: vec![],
                },
                WorkflowStage {
                    id: "stage2".to_string(),
                    name: "分析综合".to_string(),
                    role: "analyst".to_string(),
                    prompt_template: "基于以下资料进行深度分析：\n\n{{stage1.output}}\n\n请从多角度交叉验证，指出观点分歧、数据矛盾，给出综合结论与关键洞察。输出结构化分析（Markdown，含「关键结论」「争议点」「证据强度」小节）。".to_string(),
                    tools: vec!["read_file".to_string()],
                    max_iterations: 10,
                    depends_on: vec!["stage1".to_string()],
                },
                WorkflowStage {
                    id: "stage3".to_string(),
                    name: "成文".to_string(),
                    role: "writer".to_string(),
                    prompt_template: "基于分析撰写最终研究报告：\n\n{{stage2.output}}\n\n要求：结构清晰（摘要/正文/结论/参考）、语言精炼、保留关键来源。输出完整 Markdown 报告。".to_string(),
                    tools: vec![],
                    max_iterations: 5,
                    depends_on: vec!["stage2".to_string()],
                },
            ],
        }
    }

    /// 代码审查工作流
    fn code_review_workflow() -> Workflow {
        Workflow {
            id: "code-review".to_string(),
            name: "代码审查".to_string(),
            description: Some("通读代码 → 审查问题 → 修复建议".to_string()),
            inputs: vec![
                TaskInput {
                    key: "workdir".to_string(),
                    label: "目标目录".to_string(),
                    kind: InputKind::Text,
                    default: None,
                    required: true,
                },
            ],
            stages: vec![
                WorkflowStage {
                    id: "stage1".to_string(),
                    name: "代码通读".to_string(),
                    role: "reader".to_string(),
                    prompt_template: "通读工作目录 {{workdir}} 的代码（重点：入口、核心模块、变更文件）。输出：项目结构概览 + 关键文件清单（含行数与职责）。".to_string(),
                    tools: vec!["read_file".to_string(), "list_dir".to_string()],
                    max_iterations: 20,
                    depends_on: vec![],
                },
                WorkflowStage {
                    id: "stage2".to_string(),
                    name: "问题审查".to_string(),
                    role: "reviewer".to_string(),
                    prompt_template: "审查以下代码：\n\n{{stage1.output}}\n\n逐文件列出：严重度（critical/major/minor）、问题描述、位置（file:line）、修复建议。输出 Markdown 审查报告。".to_string(),
                    tools: vec!["read_file".to_string()],
                    max_iterations: 20,
                    depends_on: vec!["stage1".to_string()],
                },
            ],
        }
    }

    /// 头脑风暴工作流
    fn brainstorm_workflow() -> Workflow {
        Workflow {
            id: "brainstorm".to_string(),
            name: "头脑风暴".to_string(),
            description: Some("发散创意 → 收敛筛选 → 批判评估".to_string()),
            inputs: vec![
                TaskInput {
                    key: "topic".to_string(),
                    label: "主题".to_string(),
                    kind: InputKind::Text,
                    default: None,
                    required: true,
                },
            ],
            stages: vec![
                WorkflowStage {
                    id: "stage1".to_string(),
                    name: "发散".to_string(),
                    role: "diverge".to_string(),
                    prompt_template: "针对「{{topic}}」发散出至少 10 个候选方案/创意，覆盖不同角度（技术、商业、用户体验）。不评判优劣，只列点子。".to_string(),
                    tools: vec![],
                    max_iterations: 8,
                    depends_on: vec![],
                },
                WorkflowStage {
                    id: "stage2".to_string(),
                    name: "收敛".to_string(),
                    role: "converge".to_string(),
                    prompt_template: "对以下候选进行收敛归类：\n\n{{stage1.output}}\n\n合并相似项，按可行性/价值/风险三维度初筛，保留 Top 5 并说明理由。".to_string(),
                    tools: vec![],
                    max_iterations: 5,
                    depends_on: vec!["stage1".to_string()],
                },
            ],
        }
    }

    /// 翻译校对工作流
    fn translate_proofread_workflow() -> Workflow {
        Workflow {
            id: "translate-proofread".to_string(),
            name: "翻译校对".to_string(),
            description: Some("初译 → 校对修正".to_string()),
            inputs: vec![
                TaskInput {
                    key: "text".to_string(),
                    label: "待翻译文本".to_string(),
                    kind: InputKind::Textarea,
                    default: None,
                    required: true,
                },
                TaskInput {
                    key: "target".to_string(),
                    label: "目标语言".to_string(),
                    kind: InputKind::Text,
                    default: Some(serde_json::json!("en")),
                    required: true,
                },
            ],
            stages: vec![
                WorkflowStage {
                    id: "stage1".to_string(),
                    name: "初译".to_string(),
                    role: "translator".to_string(),
                    prompt_template: "将以下文本翻译为 {{target}}，保留代码与专有名词，只输出译文：\n\n{{text}}".to_string(),
                    tools: vec![],
                    max_iterations: 3,
                    depends_on: vec![],
                },
                WorkflowStage {
                    id: "stage2".to_string(),
                    name: "校对".to_string(),
                    role: "proofreader".to_string(),
                    prompt_template: "校对以下译文，修正术语不一致、漏译、生硬表达，输出终稿：\n\n{{stage1.output}}".to_string(),
                    tools: vec![],
                    max_iterations: 3,
                    depends_on: vec!["stage1".to_string()],
                },
            ],
        }
    }
}
