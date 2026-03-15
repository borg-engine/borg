use std::collections::HashMap;
use std::path::PathBuf;

use borg_core::types::CustomMcpServer;
use serde_json::{json, Map, Value};

/// Legal provider: (name, env_var, display_label).
pub const LEGAL_PROVIDERS: &[(&str, &str, &str)] = &[
    ("lexisnexis", "LEXISNEXIS_API_KEY", "LexisNexis"),
    ("westlaw", "WESTLAW_API_KEY", "Westlaw"),
    ("clio", "CLIO_API_KEY", "Clio"),
    ("imanage", "IMANAGE_API_KEY", "iManage"),
    ("netdocuments", "NETDOCUMENTS_API_KEY", "NetDocuments"),
    ("congress", "CONGRESS_API_KEY", "Congress.gov"),
    ("openstates", "OPENSTATES_API_KEY", "OpenStates"),
    ("canlii", "CANLII_API_KEY", "CanLII"),
    (
        "regulations_gov",
        "REGULATIONS_GOV_API_KEY",
        "Regulations.gov",
    ),
];

/// Resolves an MCP server path via env var override or CWD-relative fallback.
/// Returns `Some(canonicalized_path)` if the file exists, `None` otherwise.
pub fn resolve_mcp_server_path(env_var: &str, relative_fallback: &str) -> Option<PathBuf> {
    if let Ok(p) = std::env::var(env_var) {
        return PathBuf::from(p).canonicalize().ok();
    }

    let manifest_relative = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_fallback);
    if let Ok(path) = manifest_relative.canonicalize() {
        return Some(path);
    }

    let cwd_relative = std::env::current_dir()
        .unwrap_or_default()
        .join(relative_fallback);
    if let Ok(path) = cwd_relative.canonicalize() {
        return Some(path);
    }

    let clean = relative_fallback.trim_start_matches("../");
    std::env::current_dir()
        .unwrap_or_default()
        .join(clean)
        .canonicalize()
        .ok()
}

/// Returns the env var name for a legal API provider, or `None` if unknown.
pub fn legal_provider_env_name(provider: &str) -> Option<&'static str> {
    LEGAL_PROVIDERS
        .iter()
        .find(|(p, _, _)| *p == provider)
        .map(|(_, env, _)| *env)
}

/// Builds the `mcpServers` JSON map for a pipeline/chat agent invocation.
///
/// Returns the entries to put under `{ "mcpServers": <map> }`.
pub fn build_mcp_servers_json(
    api_url: &str,
    api_token: &str,
    mode: &str,
    project_id: i64,
    workspace_id: i64,
    chat_thread: Option<&str>,
    linked_creds: &[(String, String)],
    custom_servers: &[CustomMcpServer],
) -> Map<String, Value> {
    let mut mcp_servers = Map::new();

    if let Some(borg_server) =
        resolve_mcp_server_path("BORG_MCP_SERVER", "../../../sidecar/borg-mcp/server.js")
    {
        let mut env_vars = Map::new();
        env_vars.insert("API_BASE_URL".into(), json!(api_url));
        env_vars.insert("API_TOKEN".into(), json!(api_token));
        if let Some(thread) = chat_thread {
            env_vars.insert("CHAT_THREAD".into(), json!(thread));
        }
        if project_id > 0 {
            env_vars.insert("PROJECT_ID".into(), json!(project_id.to_string()));
            env_vars.insert("PROJECT_MODE".into(), json!(mode));
        }
        if workspace_id > 0 {
            env_vars.insert("WORKSPACE_ID".into(), json!(workspace_id.to_string()));
        }
        // Pass OCR API key so MCP tools can do OCR
        if let Ok(key) = std::env::var("MISTRAL_API_KEY") {
            if !key.is_empty() {
                env_vars.insert("MISTRAL_API_KEY".into(), json!(key));
            }
        }
        mcp_servers.insert(
            "borg".into(),
            json!({
                "command": "bun",
                "args": ["run", borg_server],
                "env": env_vars,
            }),
        );
    }

    let is_legal = matches!(mode, "lawborg" | "legal");
    if is_legal {
        if let Some(legal_server) = resolve_mcp_server_path(
            "LAWBORG_MCP_SERVER",
            "../../../sidecar/lawborg-mcp/server.js",
        ) {
            let mut env_vars = Map::new();
            for (provider, key) in linked_creds {
                if let Some(env_name) = legal_provider_env_name(provider) {
                    env_vars.insert(env_name.into(), json!(key));
                }
            }
            mcp_servers.insert(
                "legal".into(),
                json!({
                    "command": "bun",
                    "args": ["run", legal_server],
                    "env": env_vars,
                }),
            );
        }
    }

    // Append user-configured custom MCP servers
    for server in custom_servers {
        // Don't let custom servers override built-in ones
        if mcp_servers.contains_key(&server.name) {
            continue;
        }
        let env_map: Map<String, Value> = server
            .env
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        mcp_servers.insert(
            server.name.clone(),
            json!({
                "command": server.command,
                "args": server.args,
                "env": env_map,
            }),
        );
    }

    mcp_servers
}

/// Load custom MCP servers from DB rows (name, command, args, env) into the agent-side struct.
pub fn custom_servers_from_db(
    rows: Vec<(String, String, Vec<String>, HashMap<String, String>)>,
) -> Vec<CustomMcpServer> {
    rows.into_iter()
        .map(|(name, command, args, env)| CustomMcpServer {
            name,
            command,
            args,
            env,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::resolve_mcp_server_path;

    #[test]
    fn resolves_borg_sidecar_relative_to_manifest_dir() {
        let path = resolve_mcp_server_path(
            "BORG_MCP_SERVER_DOES_NOT_EXIST",
            "../../../sidecar/borg-mcp/server.js",
        )
        .expect("expected borg MCP server path to resolve");
        assert!(path.ends_with("sidecar/borg-mcp/server.js"));
    }
}
