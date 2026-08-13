use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Vault {
    pub id: String,
    #[serde(skip_serializing)]
    #[allow(dead_code)]
    pub secret_hash: Vec<u8>,
    pub name: String,
    pub status: String,
    pub ever_used: bool,
    pub suspended_reason: Option<String>,
    pub deleted_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub email: Option<String>,
    pub email_verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Block {
    pub id: String,
    pub vault_id: String,
    #[serde(default = "default_block_type")]
    pub block_type: String,
    pub title: String,
    pub content: String,
    pub position: i64,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

fn default_block_type() -> String { "prompt".into() }

#[cfg(test)]
mod tests {
    use super::Block;

    #[test]
    fn legacy_revision_blocks_default_to_prompt_type() {
        let block: Block = serde_json::from_str(
            r#"{
                "id":"block-1",
                "vault_id":"vault-1",
                "title":"Legacy prompt",
                "content":"Be concise.",
                "position":0,
                "version":1,
                "created_at":"2026-01-01T00:00:00Z",
                "updated_at":"2026-01-01T00:00:00Z"
            }"#,
        )
        .unwrap();

        assert_eq!(block.block_type, "prompt");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BundleRow {
    pub id: String,
    pub vault_id: String,
    pub name: String,
    pub block_ids: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub id: String,
    pub vault_id: String,
    pub name: String,
    pub block_ids: Vec<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<BundleRow> for Bundle {
    type Error = serde_json::Error;

    fn try_from(row: BundleRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            vault_id: row.vault_id,
            name: row.name,
            block_ids: serde_json::from_str(&row.block_ids)?,
            version: row.version,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Revision {
    pub id: i64,
    pub vault_id: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub action: String,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub url: String,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationTargetView {
    pub kind: String,
    pub masked_url: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultSnapshot {
    pub vault: Vault,
    pub blocks: Vec<Block>,
    pub bundles: Vec<Bundle>,
    pub notification_target: Option<NotificationTargetView>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CallbackPayload {
    pub status: String,
    pub title: String,
    pub message: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}
