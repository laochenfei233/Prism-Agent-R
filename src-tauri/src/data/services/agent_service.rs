use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::data::models::{AgentDto, AgentRow};
use crate::utils::error::AppError;

pub struct AgentService {
    pub pool: SqlitePool,
}

// ── 内置 OPC Agent（一人公司常用场景，参考 prism-agent builtinAgentsSeeder）──

struct BuiltinAgentDef {
    name: &'static str,
    description: &'static str,
    system_prompt: &'static str,
}

const BUILTIN_AGENTS: &[BuiltinAgentDef] = &[
    BuiltinAgentDef {
        name: "短视频脚本师",
        description: "专精于口播、剧情、Vlog 等类型的短视频脚本创作",
        system_prompt: "你是一位短视频脚本专家，专精于口播、剧情、Vlog 等类型的内容创作。你熟悉抖音、B站、小红书等平台的调性和算法偏好。\n\n## 你的能力\n- 根据选题快速生成 15s/30s/60s/3min 的脚本\n- 设计黄金 3 秒开头的钩子（好奇、冲突、反常识）\n- 控制节奏：每 3-5 秒一个信息点或情绪点\n- 适配品牌调性（专业/幽默/温情/犀利）\n- 输出格式：标题|时长|画面描述|口播内容|BGM建议|注意事项",
    },
    BuiltinAgentDef {
        name: "文案优化师",
        description: "精通社交媒体文案优化，提升标题/简介/评论的点击率和互动率",
        system_prompt: "你是一位资深社交媒体文案编辑，精通各平台的内容风格和用户心理。\n\n## 你的能力\n- 优化标题/简介/评论区文案，提升点击率和互动率\n- 适配不同平台风格\n- 植入 SEO 关键词和热门标签\n- 保持品牌调性统一\n- 输出：原文|优化后|改动说明",
    },
    BuiltinAgentDef {
        name: "品牌定位顾问",
        description: "梳理品牌核心价值、差异化定位，输出品牌手册",
        system_prompt: "你是一位品牌战略顾问，专注于帮助一人公司/小团队建立清晰的品牌定位。\n\n## 你的能力\n- 梳理品牌核心价值、使命愿景、差异化定位\n- 输出品牌手册\n- 基于 STP 框架和品牌金字塔模型\n- 分析竞争对手的品牌策略",
    },
    BuiltinAgentDef {
        name: "竞品分析师",
        description: "系统分析竞品内容策略、受众画像、变现模式",
        system_prompt: "你是一位竞争情报分析师，擅长系统性地分析竞争对手。\n\n## 你的能力\n- 识别直接竞品和间接竞品\n- 分析竞品的内容策略、受众画像、变现模式\n- 用 SWOT 框架对比优劣势\n- 找出差异化机会",
    },
    BuiltinAgentDef {
        name: "合同审查员",
        description: "审查合同条款，标注风险点和不合理条款",
        system_prompt: "你是一位合同审查助手，帮助用户快速识别合同中的风险和问题。\n\n## 你的能力\n- 识别合同中的常见风险条款\n- 标注不合理条款和模糊表达\n- 给出修改建议和谈判话术\n- 特别关注：知识产权归属、竞业限制、保密条款",
    },
    BuiltinAgentDef {
        name: "法务顾问",
        description: "解答著作权、肖像权、商标、合同等法律知识",
        system_prompt: "你是一位法律知识助手，帮助用户了解与创业经营相关的基础法律知识。\n\n## 你的能力\n- 解答著作权、商标、专利等知识产权问题\n- 解读合同法、劳动法、广告法\n- 提供法律风险防范建议\n- 用通俗易懂的语言解释法律概念",
    },
    BuiltinAgentDef {
        name: "税务计算器",
        description: "估算个税、增值税、企业所得税",
        system_prompt: "你是一位税务估算助手。\n\n## 你的能力\n- 解释个税、增值税、企业所得税的基本规则\n- 根据收入估算应缴税额\n- 解释小微企业税收优惠政策\n- 对比不同纳税主体的税负差异",
    },
    BuiltinAgentDef {
        name: "周报月报生成器",
        description: "基于工作记录自动生成周报/月报",
        system_prompt: "你是一位汇报助手。\n\n## 你的能力\n- 从工作记录中提取关键工作项\n- 结构化汇报：本周完成|数据亮点|遇到的问题|下周计划|需要的资源\n- 量化工作成果\n- 适配不同汇报对象",
    },
];

impl AgentService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 幂等补齐内置 OPC Agent：按名称去重插入（参考 WorkflowService::ensure_builtin_workflows 的懒加载模式）
    pub async fn ensure_builtin_agents(&self) -> Result<(), AppError> {
        Self::ensure_source_column(&self.pool).await?;

        // 查询默认模型 ID（is_default = 1）
        let default_model_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM models WHERE is_default = 1 LIMIT 1")
                .fetch_optional(&self.pool)
                .await?;

        // 给没有 model_id 的 Agent 绑定默认模型（包括已有内置 Agent）
        if let Some(ref mid) = default_model_id {
            sqlx::query("UPDATE agents SET model_id = ? WHERE model_id IS NULL OR model_id = ''")
                .bind(mid)
                .execute(&self.pool)
                .await?;
        }

        let existing: Vec<String> = sqlx::query_scalar("SELECT name FROM agents")
            .fetch_all(&self.pool)
            .await?;
        let existing_names: std::collections::HashSet<&str> =
            existing.iter().map(|s| s.as_str()).collect();

        // 查询默认模型 ID（is_default = 1）
        let default_model_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM models WHERE is_default = 1 LIMIT 1")
                .fetch_optional(&self.pool)
                .await?;

        let mut order_key: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(order_key), 0) + 1 FROM agents")
                .fetch_one(&self.pool)
                .await?;

        let now = chrono::Utc::now().timestamp_millis();
        for def in BUILTIN_AGENTS {
            if existing_names.contains(def.name) {
                continue;
            }
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO agents (id, name, description, system_prompt, model_id, temperature, max_tokens, disabled_tools, configuration, order_key, created_at, updated_at, source) VALUES (?, ?, ?, ?, ?, 0.7, 8192, '[]', '{}', ?, ?, ?, 'builtin')"
            )
            .bind(&id)
            .bind(def.name)
            .bind(def.description)
            .bind(def.system_prompt)
            .bind(&default_model_id)
            .bind(order_key)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
            order_key += 1;
        }
        Ok(())
    }

    async fn ensure_source_column(pool: &SqlitePool) -> Result<(), AppError> {
        let cols = sqlx::query("PRAGMA table_info('agents')")
            .fetch_all(pool)
            .await?;
        let has_source = cols.iter().any(|r| r.get::<String, _>("name") == "source");
        if !has_source {
            sqlx::query("ALTER TABLE agents ADD COLUMN source TEXT NOT NULL DEFAULT 'user'")
                .execute(pool)
                .await?;
        }
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<AgentDto>, AppError> {
        let rows = sqlx::query_as::<_, AgentRow>(
            "SELECT id, name, description, avatar, system_prompt, model_id, plan_model_id, small_model_id, temperature, max_tokens, disabled_tools, configuration, order_key, created_at, updated_at FROM agents ORDER BY order_key"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn get(&self, id: &str) -> Result<AgentDto, AppError> {
        let row = sqlx::query_as::<_, AgentRow>(
            "SELECT id, name, description, avatar, system_prompt, model_id, plan_model_id, small_model_id, temperature, max_tokens, disabled_tools, configuration, order_key, created_at, updated_at FROM agents WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::AgentNotFound(id.to_string()))?;

        Ok(row.into())
    }

    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
        system_prompt: Option<&str>,
        model_id: Option<&str>,
    ) -> Result<AgentDto, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        // 新建 Agent 默认参数（可配置，回退 0.7 / 8192）
        let temperature =
            crate::data::settings::prefs::get_f64(&self.pool, "agent.default.temperature", 0.7)
                .await
                .clamp(0.0, 2.0);
        let max_tokens =
            crate::data::settings::prefs::get_i64(&self.pool, "agent.default.max_tokens", 8192)
                .await
                .clamp(256, 128_000);

        // 如果未指定 model_id，自动绑定默认模型（支持思考的模型）
        let effective_model_id = match model_id {
            Some(m) if !m.is_empty() => Some(m.to_string()),
            _ => {
                sqlx::query_scalar::<_, String>(
                    "SELECT id FROM models WHERE is_default = 1 LIMIT 1",
                )
                .fetch_optional(&self.pool)
                .await?
            }
        };

        sqlx::query(
            "INSERT INTO agents (id, name, description, system_prompt, model_id, temperature, max_tokens, disabled_tools, configuration, order_key, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, '[]', '{}', 0, ?, ?)"
        )
        .bind(&id)
        .bind(name)
        .bind(description)
        .bind(system_prompt)
        .bind(&effective_model_id)
        .bind(temperature)
        .bind(max_tokens)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.get(&id).await
    }

    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        system_prompt: Option<&str>,
        model_id: Option<&str>,
    ) -> Result<AgentDto, AppError> {
        let now = chrono::Utc::now().timestamp_millis();

        if let Some(n) = name {
            sqlx::query("UPDATE agents SET name = ?, updated_at = ? WHERE id = ?")
                .bind(n)
                .bind(now)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        if let Some(d) = description {
            sqlx::query("UPDATE agents SET description = ?, updated_at = ? WHERE id = ?")
                .bind(d)
                .bind(now)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        if let Some(sp) = system_prompt {
            sqlx::query("UPDATE agents SET system_prompt = ?, updated_at = ? WHERE id = ?")
                .bind(sp)
                .bind(now)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        if let Some(m) = model_id {
            sqlx::query("UPDATE agents SET model_id = ?, updated_at = ? WHERE id = ?")
                .bind(m)
                .bind(now)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }

        self.get(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

impl From<AgentRow> for AgentDto {
    fn from(r: AgentRow) -> Self {
        let disabled_tools: Vec<String> =
            serde_json::from_str(&r.disabled_tools).unwrap_or_default();
        Self {
            id: r.id,
            name: r.name,
            description: r.description,
            avatar: r.avatar,
            system_prompt: r.system_prompt,
            model_id: r.model_id,
            temperature: r.temperature,
            max_tokens: r.max_tokens,
            disabled_tools,
            order_key: r.order_key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_db() -> (crate::data::Database, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("prism_agent_{}", Uuid::new_v4()));
        let db = crate::data::db::Database::new(&dir).await.unwrap();
        (db, dir)
    }

    /// 空库：首次 ensure 插入全部 8 个内置 OPC Agent
    #[tokio::test]
    async fn ensure_builtin_seeds_eight_agents() {
        let (db, dir) = temp_db().await;
        let svc = AgentService::new(db.pool.clone());
        svc.ensure_builtin_agents().await.unwrap();

        let agents = svc.list().await.unwrap();
        assert_eq!(agents.len(), 8, "应插入 8 个内置 OPC Agent");
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"短视频脚本师"));
        assert!(names.contains(&"周报月报生成器"));
        // 内置 Agent 应带完整 system_prompt
        let scriptwriter = agents.iter().find(|a| a.name == "短视频脚本师").unwrap();
        assert!(scriptwriter
            .system_prompt
            .as_deref()
            .unwrap_or("")
            .contains("短视频脚本专家"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 幂等：二次 ensure 不重复插入；已有同名用户 Agent 不覆盖
    #[tokio::test]
    async fn ensure_builtin_is_idempotent() {
        let (db, dir) = temp_db().await;
        let svc = AgentService::new(db.pool.clone());

        svc.ensure_builtin_agents().await.unwrap();
        svc.ensure_builtin_agents().await.unwrap();
        let agents = svc.list().await.unwrap();
        assert_eq!(agents.len(), 8, "二次 ensure 不应重复插入");

        // 用户先创建同名 Agent（如用户自建「文案优化师」），ensure 不应再插入
        let (db2, dir2) = temp_db().await;
        let svc2 = AgentService::new(db2.pool.clone());
        svc2.create("文案优化师", Some("用户自定义"), Some("自定义提示词"), None)
            .await
            .unwrap();
        svc2.ensure_builtin_agents().await.unwrap();
        let agents = svc2.list().await.unwrap();
        assert_eq!(agents.len(), 8, "同名不重复，其余 7 个内置补齐");
        let dup = agents.iter().find(|a| a.name == "文案优化师").unwrap();
        assert_eq!(
            dup.system_prompt.as_deref(),
            Some("自定义提示词"),
            "不应覆盖用户已有 Agent"
        );

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }
}
