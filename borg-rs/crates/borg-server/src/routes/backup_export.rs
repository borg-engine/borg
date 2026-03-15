use std::sync::Arc;

use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use zip::write::SimpleFileOptions;

use super::{internal, require_project_access};
use crate::AppState;

#[derive(Deserialize)]
pub(crate) struct BackupExportBody {
    pub provider: Option<String>,
    #[serde(default = "default_true")]
    pub include_files: bool,
    #[serde(default = "default_true")]
    pub include_tasks: bool,
    #[serde(default = "default_true")]
    pub include_knowledge: bool,
}

fn default_true() -> bool {
    true
}

/// POST /api/projects/:id/backup
///
/// Creates a ZIP archive of project data and either returns it as a download
/// or uploads it to S3.
pub(crate) async fn create_project_backup(
    State(state): State<Arc<AppState>>,
    axum::Extension(workspace): axum::Extension<crate::auth::WorkspaceContext>,
    Path(project_id): Path<i64>,
    Json(body): Json<BackupExportBody>,
) -> Result<axum::response::Response, StatusCode> {
    let project = require_project_access(&state, &workspace, project_id)?;
    let provider = body.provider.as_deref().unwrap_or("download");

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let safe_name = project
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let filename = format!("{safe_name}-backup-{timestamp}.zip");

    let zip_bytes =
        build_project_zip(&state, project_id, &body).await.map_err(internal)?;

    match provider {
        "download" => {
            let response = axum::response::Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/zip")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{filename}\""),
                )
                .header(header::CONTENT_LENGTH, zip_bytes.len())
                .body(Body::from(zip_bytes))
                .map_err(|e| {
                    tracing::error!("failed to build response: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            Ok(response)
        },
        "s3" => {
            let s3_provider = {
                let export_providers = state.backup_export_providers.lock().map_err(|_| {
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
                export_providers
                    .iter()
                    .find(|p| p.name() == "s3")
                    .cloned()
                    .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
            };

            let result = s3_provider
                .export(Bytes::from(zip_bytes), &filename)
                .await
                .map_err(internal)?;

            Ok(Json(json!({
                "provider": result.provider,
                "location": result.location,
                "size_bytes": result.size_bytes,
                "timestamp": result.timestamp.to_rfc3339(),
            }))
            .into_response())
        },
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

/// GET /api/projects/:id/backup/status
pub(crate) async fn get_project_backup_status(
    State(state): State<Arc<AppState>>,
    axum::Extension(workspace): axum::Extension<crate::auth::WorkspaceContext>,
    Path(project_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    let _project = require_project_access(&state, &workspace, project_id)?;

    let available: Vec<String> = {
        let export_providers = state.backup_export_providers.lock().map_err(|_| {
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        export_providers
            .iter()
            .filter(|p| p.is_available())
            .map(|p| p.name().to_string())
            .collect()
    };

    let has_files = state
        .db
        .list_project_files(project_id)
        .map(|f| !f.is_empty())
        .unwrap_or(false);
    let has_tasks = state
        .db
        .list_project_tasks(project_id)
        .map(|t| !t.is_empty())
        .unwrap_or(false);

    Ok(Json(json!({
        "available_providers": available,
        "has_files": has_files,
        "has_tasks": has_tasks,
        "download_available": true,
    })))
}

async fn build_project_zip(
    state: &AppState,
    project_id: i64,
    body: &BackupExportBody,
) -> anyhow::Result<Vec<u8>> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Project metadata
    if let Some(project) = state.db.get_project(project_id)? {
        let meta = serde_json::to_vec_pretty(&json!({
            "id": project.id,
            "name": project.name,
            "mode": project.mode,
            "status": project.status,
            "created_at": project.created_at,
        }))?;
        zip.start_file("project.json", options)?;
        std::io::Write::write_all(&mut zip, &meta)?;
    }

    // Project files
    if body.include_files {
        let files = state.db.list_project_files(project_id).unwrap_or_default();
        for file in &files {
            let file_data = match state.file_storage.read_all(&file.stored_path).await {
                Ok(data) => data,
                Err(_) => continue,
            };
            let entry_name = format!("files/{}", file.file_name);
            zip.start_file(&entry_name, options)?;
            std::io::Write::write_all(&mut zip, &file_data)?;
        }

        let files_meta = serde_json::to_vec_pretty(
            &files
                .iter()
                .map(|f| {
                    json!({
                        "id": f.id,
                        "file_name": f.file_name,
                        "mime_type": f.mime_type,
                        "size_bytes": f.size_bytes,
                        "privileged": f.privileged,
                    })
                })
                .collect::<Vec<_>>(),
        )?;
        zip.start_file("files/_manifest.json", options)?;
        std::io::Write::write_all(&mut zip, &files_meta)?;
    }

    // Tasks and outputs
    if body.include_tasks {
        let tasks = state.db.list_project_tasks(project_id).unwrap_or_default();
        for task in &tasks {
            let outputs = state.db.get_task_outputs(task.id).unwrap_or_default();
            let messages = state.db.get_task_messages(task.id).unwrap_or_default();

            let task_data = serde_json::to_vec_pretty(&json!({
                "task": task,
                "outputs": outputs,
                "messages": messages,
            }))?;
            let entry_name = format!("tasks/task-{}.json", task.id);
            zip.start_file(&entry_name, options)?;
            std::io::Write::write_all(&mut zip, &task_data)?;
        }
    }

    // Knowledge files
    if body.include_knowledge {
        let knowledge = state
            .db
            .list_knowledge_files()?
            .into_iter()
            .filter(|k| k.project_id == Some(project_id))
            .collect::<Vec<_>>();

        if !knowledge.is_empty() {
            let knowledge_meta = serde_json::to_vec_pretty(
                &knowledge
                    .iter()
                    .map(|k| {
                        json!({
                            "id": k.id,
                            "file_name": k.file_name,
                            "description": k.description,
                            "category": k.category,
                            "size_bytes": k.size_bytes,
                        })
                    })
                    .collect::<Vec<_>>(),
            )?;
            zip.start_file("knowledge/_manifest.json", options)?;
            std::io::Write::write_all(&mut zip, &knowledge_meta)?;

            for kf in &knowledge {
                let kf_path = format!(
                    "{}/knowledge/workspaces/{}/{}",
                    state.config.data_dir, kf.workspace_id, kf.file_name
                );
                if let Ok(data) = tokio::fs::read(&kf_path).await {
                    let entry_name = format!("knowledge/{}", kf.file_name);
                    zip.start_file(&entry_name, options)?;
                    std::io::Write::write_all(&mut zip, &data)?;
                }
            }
        }
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

/// Execute a BackupExport cron job. Called by the server-layer cron executor.
pub(crate) async fn execute_backup_export_cron(
    state: &AppState,
    job: &borg_core::cron::CronJob,
) -> anyhow::Result<String> {
    let config = &job.config;
    let project_id = config["project_id"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("backup_export cron job missing project_id"))?;
    let provider = config["provider"].as_str().unwrap_or("s3");

    let body = BackupExportBody {
        provider: Some(provider.to_string()),
        include_files: config["include_files"].as_bool().unwrap_or(true),
        include_tasks: config["include_tasks"].as_bool().unwrap_or(true),
        include_knowledge: config["include_knowledge"].as_bool().unwrap_or(true),
    };

    let zip_bytes = build_project_zip(state, project_id, &body).await?;

    let project_name = state
        .db
        .get_project(project_id)?
        .map(|p| p.name)
        .unwrap_or_else(|| format!("project-{project_id}"));
    let safe_name = project_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let filename = format!("{safe_name}-backup-{timestamp}.zip");

    let target_provider = {
        let export_providers = state
            .backup_export_providers
            .lock()
            .map_err(|_| anyhow::anyhow!("export providers lock poisoned"))?;
        export_providers
            .iter()
            .find(|p| p.name() == provider)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!("backup export provider '{provider}' not available")
            })?
    };

    let result = target_provider
        .export(Bytes::from(zip_bytes), &filename)
        .await?;

    Ok(format!(
        "exported to {}: {} ({} bytes)",
        result.provider, result.location, result.size_bytes
    ))
}
