use serde::{Deserialize, Serialize};
use crate::data::db::Database;
use crate::utils::error::AppError;

/// §19.3.8 评测用例质量审计

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditVerdict {
    /// 用例正常
    Ok,
    /// 用例损坏
    Broken { categories: Vec<BrokenCategory> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BrokenCategory {
    /// 过严测试
    OverlyStrict,
    /// 欠明确 prompt
    AmbiguousPrompt,
    /// 低覆盖测试
    LowCoverage,
    /// 误导 prompt
    MisleadingPrompt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseAuditReport {
    pub case_id: String,
    pub verdict: AuditVerdict,
    pub reason: String,
}

/// 用例审计器
pub struct CaseAuditor {
    db: Database,
}

impl CaseAuditor {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 审计单条用例
    pub async fn audit_case(&self, case_id: &str) -> Result<CaseAuditReport, AppError> {
        // 读取用例
        let case = sqlx::query_as::<_, CaseRow>(
            "SELECT id, query, expected_answer FROM rag_eval_cases WHERE id = ?"
        )
        .bind(case_id)
        .fetch_optional(&self.db.pool)
        .await?
        .ok_or_else(|| AppError::Internal(format!("用例不存在: {case_id}")))?;

        let mut categories = Vec::new();
        let mut reasons = Vec::new();

        // 检查 1: prompt 是否明确
        if case.query.trim().len() < 5 {
            categories.push(BrokenCategory::AmbiguousPrompt);
            reasons.push("查询词过短，可能不明确".to_string());
        }

        // 检查 2: 期望答案是否合理
        if case.expected_answer.trim().len() < 10 {
            categories.push(BrokenCategory::LowCoverage);
            reasons.push("期望答案过短，覆盖不足".to_string());
        }

        // 检查 3: 是否有误导性
        if case.query.contains("不是") || case.query.contains("不应该") {
            categories.push(BrokenCategory::MisleadingPrompt);
            reasons.push("包含否定词，可能误导".to_string());
        }

        let verdict = if categories.is_empty() {
            AuditVerdict::Ok
        } else {
            AuditVerdict::Broken { categories }
        };

        let reason = if reasons.is_empty() {
            "用例质量正常".to_string()
        } else {
            reasons.join("; ")
        };

        Ok(CaseAuditReport {
            case_id: case_id.to_string(),
            verdict,
            reason,
        })
    }

    /// 审计所有用例
    pub async fn audit_all(&self) -> Result<Vec<CaseAuditReport>, AppError> {
        let cases = sqlx::query_as::<_, CaseRow>(
            "SELECT id, query, expected_answer FROM rag_eval_cases"
        )
        .fetch_all(&self.db.pool)
        .await?;

        let mut reports = Vec::new();
        for case in cases {
            let report = self.audit_case(&case.id).await?;
            reports.push(report);
        }

        Ok(reports)
    }

    /// 更新用例审计结果
    pub async fn update_verdict(&self, case_id: &str, verdict: &AuditVerdict) -> Result<(), AppError> {
        let verdict_str = serde_json::to_string(verdict).unwrap_or_default();
        sqlx::query("UPDATE rag_eval_cases SET audit_verdict = ? WHERE id = ?")
            .bind(&verdict_str)
            .bind(case_id)
            .execute(&self.db.pool)
            .await?;
        Ok(())
    }

    /// 获取 broken 用例列表（从汇总中排除）
    pub async fn get_broken_cases(&self) -> Result<Vec<String>, AppError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM rag_eval_cases WHERE audit_verdict LIKE '%Broken%'"
        )
        .fetch_all(&self.db.pool)
        .await?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct CaseRow {
    id: String,
    query: String,
    expected_answer: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_verdict_serialize() {
        let verdict = AuditVerdict::Broken {
            categories: vec![BrokenCategory::AmbiguousPrompt],
        };
        let json = serde_json::to_string(&verdict).unwrap();
        assert!(json.contains("Broken"));
        assert!(json.contains("AmbiguousPrompt"));
    }
}
