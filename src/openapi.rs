use axum::Json;
use serde_json::{json, Value};

pub async fn document() -> Json<Value> {
    Json(json!({
      "openapi": "3.1.0",
      "info": { "title": "CrossPrompt API", "version": "0.1.0", "description": "Manage an anonymous Markdown Vault and send completion callbacks." },
      "servers": [{ "url": "/api/v1" }],
      "components": {
        "securitySchemes": { "vaultSecret": { "type": "http", "scheme": "bearer" } },
        "schemas": {
          "BlockInput": { "type": "object", "required": ["title", "content"], "properties": { "title": {"type":"string"}, "content":{"type":"string"}, "position":{"type":["integer","null"]} } },
          "Callback": { "type":"object", "required":["status","title","message"], "properties": { "status":{"enum":["completed","needs_input","failed"]}, "title":{"type":"string"}, "message":{"type":"string"}, "source":{"type":["string","null"]}, "url":{"type":["string","null"],"format":"uri"} } }
        }
      },
      "security": [{ "vaultSecret": [] }],
      "paths": {
        "/vault": { "get": { "summary": "Get the complete Vault snapshot" }, "patch": { "summary": "Rename the Vault" }, "delete": { "summary": "Soft-delete the Vault" } },
        "/vault/restore": { "post": { "summary": "Restore a user-deleted Vault within seven days" } },
        "/vault/rotate-secret": { "post": { "summary": "Rotate the only Vault secret" } },
        "/blocks": { "get": { "summary": "List blocks" }, "post": { "summary": "Create a block" } },
        "/blocks/{id}": { "patch": { "summary": "Update a block with optimistic version checking" }, "delete": { "summary": "Delete a block with optimistic version checking" } },
        "/blocks/reorder": { "post": { "summary": "Replace the complete block ordering" } },
        "/bundles": { "get": { "summary": "List bundles" }, "post": { "summary": "Create a saved block combination" } },
        "/bundles/{id}": { "patch": { "summary": "Update a bundle" }, "delete": { "summary": "Delete a bundle" } },
        "/revisions": { "get": { "summary": "List recent revisions" } },
        "/revisions/{id}/restore": { "post": { "summary": "Restore a block or bundle to its pre-change state" } },
        "/notification-target": { "get": { "summary": "Get the masked target" }, "put": { "summary": "Set Pushcut, ntfy, or generic_json target" }, "delete": { "summary": "Delete target" } },
        "/notification-target/test": { "post": { "summary": "Send a test notification" } },
        "/callback/{secret}": { "post": { "security": [], "summary": "Send a completion notification", "requestBody": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Callback" } } } } } }
      }
    }))
}

