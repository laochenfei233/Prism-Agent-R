use crate::data::db::Database;
use crate::data::models::{RagChunkRow, RagDocumentRow};
use crate::utils::error::AppError;

/// Insert a new document record and return its ID.
pub async fn insert_document(
    db: &Database,
    wiki_id: &str,
    name: &str,
    mime_type: &str,
    size: i64,
) -> Result<String, AppError> {
    insert_document_with_meta(db, wiki_id, name, mime_type, size, None, None).await
}

/// 项目级索引（§10.2.1）：额外记录 file_path（相对工作目录）+ fingerprint（mtime:size）
///
/// §16.6 幂等导入：
/// - 有 file_path + fingerprint 时，检查是否已存在
/// - 存在且指纹相同 → 跳过（返回既有 doc_id）
/// - 存在且指纹不同 → 删除旧文档后重新入库
/// - 不存在 → 正常入库
/// - 无 file_path 的导入不触发幂等（保持原行为）
pub async fn insert_document_with_meta(
    db: &Database,
    wiki_id: &str,
    name: &str,
    mime_type: &str,
    size: i64,
    file_path: Option<&str>,
    fingerprint: Option<&str>,
) -> Result<String, AppError> {
    // 幂等检查：有 file_path + fingerprint 时才触发
    if let (Some(fp), Some(finger)) = (file_path, fingerprint) {
        if let Some(existing_id) = find_document_by_path(db, wiki_id, fp).await? {
            let existing_fp = fingerprint_of_document(db, &existing_id).await?;
            match existing_fp {
                Some(ref efp) if efp == finger => {
                    // 指纹相同，跳过导入
                    return Ok(existing_id);
                }
                Some(_) => {
                    // 指纹不同，删除旧文档后重新入库
                    delete_document_by_path(db, wiki_id, fp).await?;
                }
                None => {
                    // 旧文档无指纹，删除后重新入库
                    delete_document_by_path(db, wiki_id, fp).await?;
                }
            }
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO rag_documents (id, wiki_id, name, mime_type, size, chunk_count, status, file_path, fingerprint, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 0, 'pending', ?6, ?7, ?8, ?8)",
    )
    .bind(&id)
    .bind(wiki_id)
    .bind(name)
    .bind(mime_type)
    .bind(size)
    .bind(file_path)
    .bind(fingerprint)
    .bind(now)
    .execute(&db.pool)
    .await?;
    Ok(id)
}

/// Update document processing status.
pub async fn update_document_status(
    db: &Database,
    doc_id: &str,
    status: &str,
    error_msg: Option<&str>,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "UPDATE rag_documents SET status = ?1, error_msg = ?2, updated_at = ?3 WHERE id = ?4",
    )
    .bind(status)
    .bind(error_msg)
    .bind(now)
    .bind(doc_id)
    .execute(&db.pool)
    .await?;
    Ok(())
}

/// Insert chunks for a document and update chunk_count. Returns number of chunks inserted.
pub async fn insert_chunks(
    db: &Database,
    doc_id: &str,
    wiki_id: &str,
    chunks: &[(String, Option<Vec<u8>>)], // (content, optional embedding bytes)
    contexts: Option<&[String]>,          // 每 chunk 的上下文说明（§10.2.2，可与 chunks 等长）
    page_meta: Option<&[(Option<i32>, Option<i32>)]>, // 每 chunk 的页码范围（§10.2.4 引用定位）
) -> Result<usize, AppError> {
    let now = chrono::Utc::now().timestamp();
    let mut tx = db.pool.begin().await?;

    for (i, (content, embedding)) in chunks.iter().enumerate() {
        let chunk_id = uuid::Uuid::new_v4().to_string();
        let context = contexts.and_then(|c| c.get(i));
        let (page_start, page_end) = page_meta
            .and_then(|m| m.get(i))
            .copied()
            .unwrap_or((None, None));
        sqlx::query(
            r#"INSERT INTO rag_chunks (id, document_id, wiki_id, "index", content, embedding, context, page_start, page_end, block_type, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'text', ?10)"#,
        )
        .bind(&chunk_id)
        .bind(doc_id)
        .bind(wiki_id)
        .bind(i as i32)
        .bind(content)
        .bind(embedding.as_deref())
        .bind(context)
        .bind(page_start)
        .bind(page_end)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    // Update chunk_count on the document
    sqlx::query("UPDATE rag_documents SET chunk_count = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(chunks.len() as i32)
        .bind(now)
        .bind(doc_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(chunks.len())
}

/// Get all chunks with their embeddings for a wiki.
pub async fn get_chunks_with_vectors(
    db: &Database,
    wiki_id: &str,
) -> Result<Vec<RagChunkRow>, AppError> {
    let rows = sqlx::query_as::<_, RagChunkRow>(
        r#"SELECT id, document_id, wiki_id, "index" as "index", content, embedding, context, page_start, page_end, section, block_type, created_at FROM rag_chunks WHERE wiki_id = ?1 AND embedding IS NOT NULL"#,
    )
    .bind(wiki_id)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows)
}

/// Delete a document and its chunks (cascade via FK).
pub async fn delete_document(db: &Database, doc_id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM rag_documents WHERE id = ?1")
        .bind(doc_id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

/// List all documents for a wiki.
pub async fn list_documents(db: &Database, wiki_id: &str) -> Result<Vec<RagDocumentRow>, AppError> {
    let rows = sqlx::query_as::<_, RagDocumentRow>(
        "SELECT id, wiki_id, name, mime_type, size, chunk_count, status, error_msg, file_path, fingerprint, created_at, updated_at FROM rag_documents WHERE wiki_id = ?1 ORDER BY created_at DESC",
    )
    .bind(wiki_id)
    .fetch_all(&db.pool)
    .await?;
    Ok(rows)
}

/// 项目级索引（§10.2.1）：按相对路径查找已索引文档
pub async fn find_document_by_path(
    db: &Database,
    wiki_id: &str,
    file_path: &str,
) -> Result<Option<String>, AppError> {
    let id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM rag_documents WHERE wiki_id = ?1 AND file_path = ?2 LIMIT 1",
    )
    .bind(wiki_id)
    .bind(file_path)
    .fetch_optional(&db.pool)
    .await?;
    Ok(id)
}

/// 项目级索引：按相对路径删除文档（文件被删除时清理索引）
pub async fn delete_document_by_path(
    db: &Database,
    wiki_id: &str,
    file_path: &str,
) -> Result<bool, AppError> {
    let res = sqlx::query("DELETE FROM rag_documents WHERE wiki_id = ?1 AND file_path = ?2")
        .bind(wiki_id)
        .bind(file_path)
        .execute(&db.pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// 项目级索引：文件指纹（path+mtime+size）比对
pub async fn fingerprint_of_document(
    db: &Database,
    doc_id: &str,
) -> Result<Option<String>, AppError> {
    let fp: Option<String> =
        sqlx::query_scalar("SELECT fingerprint FROM rag_documents WHERE id = ?1")
            .bind(doc_id)
            .fetch_optional(&db.pool)
            .await?;
    Ok(fp)
}
