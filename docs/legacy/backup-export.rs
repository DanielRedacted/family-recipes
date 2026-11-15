use std::io::{self, Cursor, Read, Write};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::{AppError, AppState, Result};

// ---------------------------------------------------------------------------
// Import history
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ImportHistoryRow {
    pub id: i64,
    pub source_type: String,
    pub source: String,
    pub state: String,
    pub recipe_id: Option<i64>,
    pub recipe_name: Option<String>,
    pub method: Option<String>,
    pub error_msg: Option<String>,
    pub imported_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveImportHistoryBody {
    pub source_type: String,
    pub source: String,
    pub state: String,
    pub recipe_id: Option<i64>,
    pub recipe_name: Option<String>,
    pub method: Option<String>,
    pub error_msg: Option<String>,
}

pub async fn list_import_history(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ImportHistoryRow>>> {
    let rows = sqlx::query_as::<_, ImportHistoryRow>(
        "SELECT id, source_type, source, state, recipe_id, recipe_name, method, error_msg, imported_at \
         FROM import_history ORDER BY imported_at DESC LIMIT 500",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

pub async fn save_import_history(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SaveImportHistoryBody>,
) -> Result<Json<serde_json::Value>> {
    if body.source_type != "url" && body.source_type != "file" {
        return Err(AppError::Validation("source_type must be 'url' or 'file'".into()));
    }
    if body.state != "done" && body.state != "error" {
        return Err(AppError::Validation("state must be 'done' or 'error'".into()));
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO import_history (source_type, source, state, recipe_id, recipe_name, method, error_msg) \
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&body.source_type)
    .bind(&body.source)
    .bind(&body.state)
    .bind(body.recipe_id)
    .bind(&body.recipe_name)
    .bind(&body.method)
    .bind(&body.error_msg)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(json!({ "id": id })))
}

pub async fn delete_import_history(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<axum::http::StatusCode> {
    sqlx::query("DELETE FROM import_history WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn zip_err(e: impl std::fmt::Display) -> AppError {
    AppError::Io(io::Error::new(io::ErrorKind::Other, e.to_string()))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// If `name` already exists in `used`, appends `_2`, `_3`, … until unique.
fn dedup_name(used: &mut std::collections::HashSet<String>, prefix: &str, name: &str) -> String {
    let candidate = format!("{prefix}{name}");
    if used.insert(candidate.clone()) {
        return candidate;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) => (&name[..i], &name[i..]),
        None => (name, ""),
    };
    let mut n = 2u32;
    loop {
        let candidate = format!("{prefix}{stem}_{n}{ext}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

/// Build an in-memory zip containing:
///  - `recipes.db` and optionally `config.toml` (full backup)
///  - `recipes/{id}_{name}.json` — structured JSON for every recipe
///  - `originals/{filename}` — original uploaded source files
///  - `originals/photos/{name}.ext` — uploaded recipe photos
///  - Any extra root-level text files (e.g. `imported-urls.txt`)
///
/// Runs synchronously so no zip types cross await points.
fn build_export_zip(
    db_bytes: &[u8],
    config_bytes: Option<&[u8]>,
    recipe_jsons: &[(String, Vec<u8>)],
    originals: &[(String, Vec<u8>)],
    text_files: &[(String, Vec<u8>)],
) -> Result<Vec<u8>> {
    let mut zip_buf: Vec<u8> = Vec::with_capacity(db_bytes.len() + 512);
    let cursor = Cursor::new(&mut zip_buf);
    let mut zip = ZipWriter::new(cursor);
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("recipes.db", opts).map_err(zip_err)?;
    zip.write_all(db_bytes)?;

    if let Some(cfg) = config_bytes {
        zip.start_file("config.toml", opts).map_err(zip_err)?;
        zip.write_all(cfg)?;
    }

    for (filename, json_bytes) in recipe_jsons {
        zip.start_file(format!("recipes/{filename}"), opts).map_err(zip_err)?;
        zip.write_all(json_bytes)?;
    }

    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (filename, file_bytes) in originals {
        let entry = dedup_name(&mut used, "originals/", filename);
        zip.start_file(&entry, opts).map_err(zip_err)?;
        zip.write_all(file_bytes)?;
    }

    for (filename, content) in text_files {
        zip.start_file(filename, opts).map_err(zip_err)?;
        zip.write_all(content)?;
    }

    zip.finish().map_err(zip_err)?;
    Ok(zip_buf)
}

/// Extract `recipes.db` from a zip archive.  Runs synchronously for the same
/// reason as `build_export_zip`.
fn extract_db_from_zip(zip_bytes: &[u8]) -> Result<Vec<u8>> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| AppError::Validation(format!("invalid zip: {e}")))?;
    let mut entry = archive.by_name("recipes.db").map_err(|_| {
        AppError::Validation("zip does not contain recipes.db".to_string())
    })?;
    let mut db_bytes = Vec::new();
    entry.read_to_end(&mut db_bytes)?;
    Ok(db_bytes)
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

pub async fn export(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse> {
    // Flush WAL so the on-disk db file is fully current.
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&state.db)
        .await?;

    let db_path = &state.config.database.path;
    let db_bytes = tokio::fs::read(db_path).await?;
    let cfg_bytes = tokio::fs::read("config.toml").await.ok();

    // Build structured JSON for every recipe.
    let recipe_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM recipes ORDER BY id")
        .fetch_all(&state.db)
        .await?;

    let mut recipe_jsons: Vec<(String, Vec<u8>)> = Vec::with_capacity(recipe_ids.len());
    for id in recipe_ids {
        if let Some(full) = crate::db::queries::get_recipe_full(&state.db, id).await? {
            let safe = sanitize_filename(&full.recipe.name);
            let filename = format!("{id}_{safe}.json");
            let json_bytes = serde_json::to_vec_pretty(&full)
                .map_err(|e| AppError::Io(io::Error::new(io::ErrorKind::Other, e.to_string())))?;
            recipe_jsons.push((filename, json_bytes));
        }
    }

    // Collect all file-backed sources and read from disk.
    let file_sources: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT value, file_path FROM sources \
         WHERE file_path IS NOT NULL AND file_path != '' ORDER BY id",
    )
    .fetch_all(&state.db)
    .await?;

    let upload_dir = std::path::Path::new(&state.config.files.upload_dir);
    let mut originals: Vec<(String, Vec<u8>)> = Vec::new();
    for (original_name, stored_opt) in file_sources {
        let stored = match stored_opt {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };
        // Prevent path traversal: use only the filename component.
        let safe_stored = std::path::Path::new(&stored)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&stored)
            .to_string();
        match tokio::fs::read(upload_dir.join(&safe_stored)).await {
            Ok(bytes) => {
                let display = std::path::Path::new(&original_name)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&original_name)
                    .to_string();
                originals.push((display, bytes));
            }
            Err(_) => {} // File missing on disk — skip silently.
        }
    }

    // Collect uploaded photos, named after their recipe.
    let photo_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT rp.file_path, rp.original_name, r.name \
         FROM recipe_photos rp \
         JOIN recipes r ON r.id = rp.recipe_id \
         ORDER BY rp.recipe_id, rp.sort_order, rp.id",
    )
    .fetch_all(&state.db)
    .await?;

    for (file_path, original_name, recipe_name) in photo_rows {
        let safe_stored = std::path::Path::new(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&file_path)
            .to_string();
        match tokio::fs::read(upload_dir.join(&safe_stored)).await {
            Ok(bytes) => {
                let ext = std::path::Path::new(&original_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| format!(".{}", e.to_lowercase()))
                    .unwrap_or_default();
                let safe_name = sanitize_filename(&recipe_name);
                // "photos/" prefix → lands at originals/photos/<recipe>.ext in the zip
                originals.push((format!("photos/{safe_name}{ext}"), bytes));
            }
            Err(_) => {} // File missing on disk — skip silently.
        }
    }

    // Build the imported-urls.txt document.
    // Combines successfully imported URLs (from sources) and failed attempts
    // (from import_history errors) so the history is complete.
    let imported_urls: Vec<(String,)> = sqlx::query_as(
        "SELECT value FROM sources WHERE source_type = 'url' ORDER BY imported_at",
    )
    .fetch_all(&state.db)
    .await?;

    let txt = imported_urls
        .iter()
        .map(|(url,)| url.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let text_files = vec![("imported-urls.txt".to_string(), txt.into_bytes())];

    // Build the zip archive synchronously so no ZipWriter types cross the
    // await boundary above (which would make the future non-'static).
    let zip_buf = build_export_zip(&db_bytes, cfg_bytes.as_deref(), &recipe_jsons, &originals, &text_files)?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"recipes-export.zip\"",
        )
        .body(Body::from(zip_buf))
        .unwrap();

    Ok(response)
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

pub async fn import(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>> {
    // Read the uploaded zip file (accept any field name).
    let mut zip_bytes: Option<bytes::Bytes> = None;
    while let Some(field) = multipart.next_field().await? {
        zip_bytes = Some(field.bytes().await?);
        break;
    }
    let zip_bytes =
        zip_bytes.ok_or_else(|| AppError::Validation("no file uploaded".to_string()))?;

    // Extract recipes.db from the archive synchronously so no ZipFile<'_>
    // types with non-'static lifetimes are held across any await point.
    let db_bytes = extract_db_from_zip(&zip_bytes)?;

    // Write the backup db to a temp file next to the live db.
    let db_path = &state.config.database.path;
    let temp_path = format!("{db_path}.import_tmp");
    tokio::fs::write(&temp_path, &db_bytes).await?;

    // Restore, then always delete the temp file regardless of outcome.
    let result = do_restore(&state.db, &temp_path).await;
    let _ = tokio::fs::remove_file(&temp_path).await;
    result?;

    Ok(Json(json!({ "ok": true, "message": "Database restored successfully." })))
}

// ---------------------------------------------------------------------------
// Erase
// ---------------------------------------------------------------------------

/// Deletes all recipe data (recipes, ingredients, utensils, tags, sources).
/// AI settings are preserved.
pub async fn erase(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>> {
    let mut conn = state.db.acquire().await?;

    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;

    sqlx::query("BEGIN EXCLUSIVE").execute(&mut *conn).await?;

    let result = async {
        for stmt in &[
            "DELETE FROM import_history",
            "DELETE FROM recipe_ingredients",
            "DELETE FROM recipe_utensils",
            "DELETE FROM recipe_tags",
            "DELETE FROM sources",
            "DELETE FROM recipes",
            "DELETE FROM ingredients",
            "DELETE FROM utensils",
            "DELETE FROM tags",
        ] {
            sqlx::query(stmt).execute(&mut *conn).await?;
        }
        // Reset autoincrement counters so IDs start from 1 again.
        for table in &[
            "recipes",
            "ingredients",
            "utensils",
            "tags",
            "sources",
            "recipe_ingredients",
            "import_history",
        ] {
            sqlx::query(&format!(
                "UPDATE sqlite_sequence SET seq = 0 WHERE name = '{table}'"
            ))
            .execute(&mut *conn)
            .await?;
        }
        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok::<_, AppError>(())
    }
    .await;

    if result.is_err() {
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
    }

    let _ = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await;

    result?;

    tracing::info!("All recipe data erased by user request");
    Ok(Json(json!({ "ok": true, "message": "All recipe data erased." })))
}

// ---------------------------------------------------------------------------
// Restore implementation
// ---------------------------------------------------------------------------

/// Restores all application data from `temp_path` (a SQLite file) into the
/// live pool.  Uses a single held connection so that ATTACH and PRAGMA state
/// are consistent throughout.
async fn do_restore(pool: &sqlx::SqlitePool, temp_path: &str) -> Result<()> {
    let mut conn = pool.acquire().await?;

    // These pragmas must be set outside a transaction.
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;

    // ATTACH does not support bound parameters in SQLite — the filename must
    // be a string literal.  Single-quote any apostrophes in the path.
    let attach_sql = format!(
        "ATTACH DATABASE '{}' AS src",
        temp_path.replace('\'', "''")
    );
    sqlx::query(&attach_sql).execute(&mut *conn).await?;

    // Run the full restore inside an exclusive transaction so no concurrent
    // reads see a partially-restored database.
    let restore_result = async {
        sqlx::query("BEGIN EXCLUSIVE").execute(&mut *conn).await?;

        // Delete in dependency order so FK constraints (even with FK checks
        // off) don't cause confusion.  The fts_recipes_delete trigger fires
        // per-row as recipes are removed, clearing the FTS index automatically.
        for stmt in &[
            "DELETE FROM import_history",
            "DELETE FROM recipe_ingredients",
            "DELETE FROM recipe_utensils",
            "DELETE FROM recipe_tags",
            "DELETE FROM sources",
            "DELETE FROM recipes",
            "DELETE FROM ingredients",
            "DELETE FROM utensils",
            "DELETE FROM tags",
            "DELETE FROM settings",
        ] {
            sqlx::query(stmt).execute(&mut *conn).await?;
        }

        // Restore in dependency order.  Inserting recipes fires the
        // fts_recipes_insert trigger per row, which creates FTS entries with
        // correct name/description/cuisine/category and empty ingredient/tag text.
        // The ingredient and tag text columns are fixed up after COMMIT.
        for stmt in &[
            "INSERT INTO ingredients        SELECT * FROM src.ingredients",
            "INSERT INTO utensils           SELECT * FROM src.utensils",
            "INSERT INTO tags               SELECT * FROM src.tags",
            "INSERT INTO recipes            SELECT * FROM src.recipes",
            "INSERT INTO recipe_ingredients SELECT * FROM src.recipe_ingredients",
            "INSERT INTO recipe_utensils    SELECT * FROM src.recipe_utensils",
            "INSERT INTO recipe_tags        SELECT * FROM src.recipe_tags",
            "INSERT INTO sources            SELECT * FROM src.sources",
            "INSERT OR REPLACE INTO settings SELECT * FROM src.settings",
            "INSERT INTO import_history     SELECT * FROM src.import_history",
        ] {
            sqlx::query(stmt).execute(&mut *conn).await?;
        }

        // Keep AUTOINCREMENT counters consistent with the restored IDs so that
        // new inserts after restore don't collide.
        for table in &[
            "recipes",
            "ingredients",
            "utensils",
            "tags",
            "sources",
            "recipe_ingredients",
            "import_history",
        ] {
            sqlx::query(&format!(
                "UPDATE sqlite_sequence \
                 SET seq = (SELECT COALESCE(MAX(id), 0) FROM {table}) \
                 WHERE name = '{table}'"
            ))
            .execute(&mut *conn)
            .await?;
        }

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok::<_, AppError>(())
    }
    .await;

    if restore_result.is_err() {
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
    }

    // Always re-enable FK enforcement and detach the backup, regardless of
    // whether the restore succeeded.
    let _ = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await;
    let _ = sqlx::query("DETACH DATABASE src")
        .execute(&mut *conn)
        .await;

    restore_result?;

    // Fix up the FTS ingredients_text and tags_text columns that the trigger
    // left empty.  Do this after the transaction so we can query join tables.
    let recipe_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM recipes")
        .fetch_all(&mut *conn)
        .await?;

    for id in recipe_ids {
        let ing_text: String = sqlx::query_scalar(
            "SELECT COALESCE(GROUP_CONCAT(i.name, ' '), '') \
             FROM recipe_ingredients ri \
             JOIN ingredients i ON i.id = ri.ingredient_id \
             WHERE ri.recipe_id = ?",
        )
        .bind(id)
        .fetch_one(&mut *conn)
        .await?;

        let tag_text: String = sqlx::query_scalar(
            "SELECT COALESCE(GROUP_CONCAT(t.name, ' '), '') \
             FROM recipe_tags rt \
             JOIN tags t ON t.id = rt.tag_id \
             WHERE rt.recipe_id = ?",
        )
        .bind(id)
        .fetch_one(&mut *conn)
        .await?;

        sqlx::query(
            "UPDATE fts_recipes \
             SET ingredients_text = ?, tags_text = ? \
             WHERE recipe_id = ?",
        )
        .bind(&ing_text)
        .bind(&tag_text)
        .bind(id)
        .execute(&mut *conn)
        .await?;
    }

    Ok(())
}
