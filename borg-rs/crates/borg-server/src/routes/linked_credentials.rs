use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use borg_core::linked_credentials::{
    capture_bundle, restore_bundle, should_revalidate, validate_home, LinkedCredentialValidation,
    PROVIDER_CLAUDE, PROVIDER_OPENAI,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    time::{sleep, Duration},
};

use super::internal;
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LinkedCredentialConnectSession {
    pub id: String,
    pub provider: String,
    pub status: String,
    pub auth_url: String,
    pub device_code: String,
    pub message: String,
    pub error: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing)]
    pub user_id: i64,
}

fn normalize_provider(provider: &str) -> Option<&'static str> {
    match provider.trim().to_ascii_lowercase().as_str() {
        PROVIDER_CLAUDE => Some(PROVIDER_CLAUDE),
        PROVIDER_OPENAI => Some(PROVIDER_OPENAI),
        _ => None,
    }
}

fn auth_session_root(state: &AppState, purpose: &str, session_id: &str) -> PathBuf {
    PathBuf::from(&state.config.data_dir)
        .join("linked-auth")
        .join(purpose)
        .join(session_id)
}

fn trim_token(text: &str) -> &str {
    text.trim_matches(|c: char| {
        c.is_whitespace() || matches!(c, '"' | '\'' | ',' | '.' | ')' | '(' | '[' | ']')
    })
}

fn extract_first_url(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|token| {
        let token = trim_token(token);
        if token.starts_with("https://") || token.starts_with("http://") {
            Some(token.to_string())
        } else {
            None
        }
    })
}

fn extract_device_code(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|token| {
        let token = trim_token(token);
        let valid = token.contains('-')
            && token.len() >= 7
            && token
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-');
        valid.then(|| token.to_string())
    })
}

async fn patch_connect_session(
    state: &AppState,
    session_id: &str,
    mutator: impl FnOnce(&mut LinkedCredentialConnectSession),
) {
    let mut sessions = state.linked_credential_sessions.lock().await;
    if let Some(session) = sessions.get_mut(session_id) {
        mutator(session);
        session.updated_at = Utc::now().to_rfc3339();
    }
}

async fn snapshot_connect_session(
    state: &AppState,
    session_id: &str,
) -> Option<LinkedCredentialConnectSession> {
    let sessions = state.linked_credential_sessions.lock().await;
    sessions.get(session_id).cloned()
}

fn connect_command(provider: &str, temp_home: &str) -> Command {
    let mut cmd = match provider {
        PROVIDER_CLAUDE => {
            let mut cmd = Command::new("claude");
            cmd.args(["auth", "login"]);
            cmd
        },
        PROVIDER_OPENAI => {
            let mut cmd = Command::new("codex");
            cmd.args(["login", "--device-auth"]);
            cmd.env("CODEX_HOME", format!("{temp_home}/.codex"));
            cmd
        },
        _ => unreachable!(),
    };
    cmd.env("HOME", temp_home)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd
}

async fn persist_validated_credential(
    state: &AppState,
    user_id: i64,
    provider: &str,
    temp_home: &str,
    validation: &LinkedCredentialValidation,
) -> anyhow::Result<()> {
    let bundle = capture_bundle(provider, &PathBuf::from(temp_home))?;
    let now = Utc::now().to_rfc3339();
    state.db.upsert_user_linked_credential(
        user_id,
        provider,
        &validation.auth_kind,
        &validation.account_email,
        &validation.account_label,
        "connected",
        &validation.expires_at,
        &now,
        &now,
        "",
        &bundle,
    )?;
    Ok(())
}

async fn revalidate_stored_credential(
    state: &AppState,
    user_id: i64,
    provider: &str,
) -> anyhow::Result<()> {
    let Some(secret) = state.db.get_user_linked_credential(user_id, provider)? else {
        return Ok(());
    };
    let session_id = crate::auth::generate_token();
    let temp_home = auth_session_root(state, "validate", &session_id);
    tokio::fs::create_dir_all(&temp_home).await?;
    restore_bundle(&secret.bundle, &temp_home)?;
    let validation = validate_home(provider, &temp_home).await?;
    let now = Utc::now().to_rfc3339();
    if validation.ok {
        let refreshed_bundle = capture_bundle(provider, &temp_home)?;
        state.db.update_user_linked_credential_state(
            user_id,
            provider,
            &validation.auth_kind,
            &validation.account_email,
            &validation.account_label,
            "connected",
            &validation.expires_at,
            &now,
            "",
            Some(&refreshed_bundle),
        )?;
    } else {
        state.db.update_user_linked_credential_state(
            user_id,
            provider,
            if validation.auth_kind.is_empty() {
                &secret.entry.auth_kind
            } else {
                &validation.auth_kind
            },
            if validation.account_email.is_empty() {
                &secret.entry.account_email
            } else {
                &validation.account_email
            },
            if validation.account_label.is_empty() {
                &secret.entry.account_label
            } else {
                &validation.account_label
            },
            "expired",
            &validation.expires_at,
            &now,
            &validation.last_error,
            None,
        )?;
    }
    let _ = tokio::fs::remove_dir_all(&temp_home).await;
    Ok(())
}

async fn run_connect_session(
    state: Arc<AppState>,
    session_id: String,
    user_id: i64,
    provider: String,
    temp_home: String,
) {
    let spawn = connect_command(&provider, &temp_home).spawn();
    let mut child = match spawn {
        Ok(child) => child,
        Err(err) => {
            patch_connect_session(&state, &session_id, |session| {
                session.status = "failed".to_string();
                session.error = format!("failed to start {provider} login: {err}");
            })
            .await;
            return;
        },
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            patch_connect_session(&state, &session_id, |session| {
                session.status = "failed".to_string();
                session.error = format!("{provider} login missing stdout");
            })
            .await;
            return;
        },
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            patch_connect_session(&state, &session_id, |session| {
                session.status = "failed".to_string();
                session.error = format!("{provider} login missing stderr");
            })
            .await;
            return;
        },
    };

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();
    let mut stdout_done = false;
    let mut stderr_done = false;
    let mut transcript = Vec::new();

    while !stdout_done || !stderr_done {
        tokio::select! {
            line = stdout_reader.next_line(), if !stdout_done => {
                match line {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if !line.is_empty() {
                            transcript.push(line.clone());
                            patch_connect_session(&state, &session_id, |session| {
                                if session.auth_url.is_empty() {
                                    if let Some(url) = extract_first_url(&line) {
                                        session.auth_url = url;
                                    }
                                }
                                if session.device_code.is_empty() {
                                    if let Some(code) = extract_device_code(&line) {
                                        session.device_code = code;
                                    }
                                }
                                session.message = line.clone();
                            }).await;
                        }
                    }
                    Ok(None) => stdout_done = true,
                    Err(err) => {
                        transcript.push(format!("stdout read error: {err}"));
                        stdout_done = true;
                    }
                }
            }
            line = stderr_reader.next_line(), if !stderr_done => {
                match line {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if !line.is_empty() {
                            transcript.push(line.clone());
                            patch_connect_session(&state, &session_id, |session| {
                                if session.auth_url.is_empty() {
                                    if let Some(url) = extract_first_url(&line) {
                                        session.auth_url = url;
                                    }
                                }
                                if session.device_code.is_empty() {
                                    if let Some(code) = extract_device_code(&line) {
                                        session.device_code = code;
                                    }
                                }
                                if session.message.is_empty() {
                                    session.message = line.clone();
                                }
                            }).await;
                        }
                    }
                    Ok(None) => stderr_done = true,
                    Err(err) => {
                        transcript.push(format!("stderr read error: {err}"));
                        stderr_done = true;
                    }
                }
            }
        }
    }

    let wait_result = child.wait().await;
    let transcript_text = transcript.join("\n");
    match wait_result {
        Ok(status) if status.success() => {
            match validate_home(&provider, &PathBuf::from(&temp_home)).await {
                Ok(validation) if validation.ok => {
                    match persist_validated_credential(
                        &state,
                        user_id,
                        &provider,
                        &temp_home,
                        &validation,
                    )
                    .await
                    {
                        Ok(()) => {
                            patch_connect_session(&state, &session_id, |session| {
                                session.status = "connected".to_string();
                                session.message =
                                    "Credential linked and ready for agent runs".to_string();
                                session.error.clear();
                            })
                            .await;
                        },
                        Err(err) => {
                            patch_connect_session(&state, &session_id, |session| {
                                session.status = "failed".to_string();
                                session.error = format!("failed to save linked credential: {err}");
                            })
                            .await;
                        },
                    }
                },
                Ok(validation) => {
                    patch_connect_session(&state, &session_id, |session| {
                        session.status = "failed".to_string();
                        session.error = if validation.last_error.is_empty() {
                            "login completed but no valid credential was stored".to_string()
                        } else {
                            validation.last_error
                        };
                    })
                    .await;
                },
                Err(err) => {
                    patch_connect_session(&state, &session_id, |session| {
                        session.status = "failed".to_string();
                        session.error = format!("credential validation failed: {err}");
                    })
                    .await;
                },
            }
        },
        Ok(status) => {
            patch_connect_session(&state, &session_id, |session| {
                session.status = "failed".to_string();
                session.error = if transcript_text.is_empty() {
                    format!(
                        "{provider} login exited with {}",
                        status.code().unwrap_or(-1)
                    )
                } else {
                    transcript_text.clone()
                };
            })
            .await;
        },
        Err(err) => {
            patch_connect_session(&state, &session_id, |session| {
                session.status = "failed".to_string();
                session.error = format!("failed waiting for {provider} login: {err}");
            })
            .await;
        },
    }

    let _ = tokio::fs::remove_dir_all(&temp_home).await;
}

pub(crate) fn spawn_linked_credential_maintenance(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let entries = match state.db.list_all_linked_credentials() {
                Ok(entries) => entries,
                Err(err) => {
                    tracing::warn!("linked credential sweep failed to list credentials: {err}");
                    continue;
                },
            };
            for entry in entries {
                if entry.status == "connected"
                    && should_revalidate(&entry.last_validated_at, &entry.expires_at)
                {
                    if let Err(err) =
                        revalidate_stored_credential(&state, entry.user_id, &entry.provider).await
                    {
                        tracing::warn!(
                            user_id = entry.user_id,
                            provider = entry.provider.as_str(),
                            "linked credential revalidation failed: {err}"
                        );
                    }
                }
            }
            let cutoff = Utc::now() - ChronoDuration::hours(1);
            let mut sessions = state.linked_credential_sessions.lock().await;
            sessions.retain(|_, session| {
                chrono::DateTime::parse_from_rfc3339(&session.updated_at)
                    .map(|ts| ts.with_timezone(&Utc) >= cutoff)
                    .unwrap_or(false)
            });
        }
    });
}

pub(crate) async fn list_user_linked_credentials(
    State(state): State<Arc<AppState>>,
    axum::Extension(user): axum::Extension<crate::auth::AuthUser>,
) -> Result<Json<Value>, StatusCode> {
    let credentials = state
        .db
        .list_user_linked_credentials(user.id)
        .map_err(internal)?;
    Ok(Json(json!({ "credentials": credentials })))
}

pub(crate) async fn start_linked_credential_connect(
    State(state): State<Arc<AppState>>,
    axum::Extension(user): axum::Extension<crate::auth::AuthUser>,
    Path(provider): Path<String>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    let provider = normalize_provider(&provider).ok_or(StatusCode::NOT_FOUND)?;
    let session_id = crate::auth::generate_token();
    let temp_home = auth_session_root(state.as_ref(), "connect", &session_id);
    tokio::fs::create_dir_all(&temp_home)
        .await
        .map_err(internal)?;
    let now = Utc::now().to_rfc3339();
    {
        let mut sessions = state.linked_credential_sessions.lock().await;
        sessions.insert(
            session_id.clone(),
            LinkedCredentialConnectSession {
                id: session_id.clone(),
                provider: provider.to_string(),
                status: "pending".to_string(),
                auth_url: String::new(),
                device_code: String::new(),
                message: "Waiting for provider login instructions".to_string(),
                error: String::new(),
                created_at: now.clone(),
                updated_at: now,
                user_id: user.id,
            },
        );
    }

    tokio::spawn(run_connect_session(
        Arc::clone(&state),
        session_id.clone(),
        user.id,
        provider.to_string(),
        temp_home.to_string_lossy().to_string(),
    ));

    for _ in 0..30 {
        if let Some(session) = snapshot_connect_session(&state, &session_id).await {
            if !session.auth_url.is_empty() || session.status != "pending" {
                return Ok((StatusCode::ACCEPTED, Json(json!(session))));
            }
        }
        sleep(Duration::from_millis(100)).await;
    }

    let session = snapshot_connect_session(&state, &session_id)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::ACCEPTED, Json(json!(session))))
}

pub(crate) async fn get_linked_credential_connect_session(
    State(state): State<Arc<AppState>>,
    axum::Extension(user): axum::Extension<crate::auth::AuthUser>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let session = snapshot_connect_session(&state, &id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    if session.user_id != user.id {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(json!(session)))
}

pub(crate) async fn delete_user_linked_credential(
    State(state): State<Arc<AppState>>,
    axum::Extension(user): axum::Extension<crate::auth::AuthUser>,
    Path(provider): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let provider = normalize_provider(&provider).ok_or(StatusCode::NOT_FOUND)?;
    state
        .db
        .delete_user_linked_credential(user.id, provider)
        .map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::{extract_device_code, extract_first_url};

    #[test]
    fn extracts_provider_urls() {
        assert_eq!(
            extract_first_url("visit: https://claude.ai/oauth/authorize?x=1"),
            Some("https://claude.ai/oauth/authorize?x=1".to_string())
        );
    }

    #[test]
    fn extracts_device_code_tokens() {
        assert_eq!(
            extract_device_code("Enter this one-time code O078-ZFUYD"),
            Some("O078-ZFUYD".to_string())
        );
    }
}
