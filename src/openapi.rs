use axum::Json;
use serde_json::{json, Value};

pub async fn document() -> Json<Value> {
    Json(json!({
      "openapi": "3.1.0",
      "info": { "title": "CrossPrompt API", "version": "0.3.0", "description": "Manage a typed portable asset Vault using a secret link or verified Email OTP session." },
      "servers": [{ "url": "/api/v1" }],
      "components": {
        "securitySchemes": { "vaultSecret": { "type": "http", "scheme": "bearer" } },
        "schemas": {
          "ArtifactTypeKey": { "type": "string", "enum": ["prompt","prompt_template","skill","mcp_server","agent_profile","workflow","context_pack","preferences","tool_api","schema","evaluation_rubric","safety_policy"] },
          "BlockInput": { "type": "object", "required": ["title", "content"], "properties": { "block_type": { "$ref": "#/components/schemas/ArtifactTypeKey", "default": "prompt" }, "title": {"type":"string"}, "content":{"type":"string"}, "position":{"type":["integer","null"]} } },
          "PortableTextInput": { "type": "object", "required": ["block_ids"], "properties": { "block_ids": { "type": "array", "minItems": 1, "uniqueItems": true, "items": { "type": "string" } } } },
          "Callback": { "type":"object", "required":["status","title","message"], "properties": { "status":{"enum":["completed","needs_input","failed"]}, "title":{"type":"string"}, "message":{"type":"string"}, "source":{"type":["string","null"]}, "url":{"type":["string","null"],"format":"uri"} } }
        }
      },
      "security": [{ "vaultSecret": [] }],
      "paths": {
        "/vault": { "get": { "summary": "Get the complete Vault snapshot" }, "patch": { "summary": "Rename the Vault" }, "delete": { "summary": "Soft-delete the Vault" } },
        "/vault/restore": { "post": { "summary": "Restore a user-deleted Vault within seven days" } },
        "/vault/rotate-secret": { "post": { "summary": "Rotate the only Vault secret" } },
        "/email/login/request-code": { "post": { "security": [], "summary": "Request a login code without revealing whether the Email exists" } },
        "/email/login/verify": { "post": { "security": [], "summary": "Verify a six-digit code and create a 30-day HttpOnly Vault session" } },
        "/email/session": { "delete": { "security": [], "summary": "Log out the current Email Vault session" } },
        "/vault/email/request-code": { "post": { "summary": "Send a verification code before binding an Email" } },
        "/vault/email/verify": { "post": { "summary": "Verify and bind one unique Email to the current Vault" } },
        "/vault/email": { "delete": { "summary": "Unbind Email; requires Bearer secret authentication" } },
        "/artifact-types": { "get": { "security": [], "summary": "List supported asset types, default templates, and Agent usage instructions" } },
        "/blocks": { "get": { "summary": "List typed assets" }, "post": { "summary": "Create a typed asset; block_type defaults to prompt" } },
        "/blocks/{id}": { "patch": { "summary": "Update a block with optimistic version checking" }, "delete": { "summary": "Delete a block with optimistic version checking" } },
        "/blocks/reorder": { "post": { "summary": "Replace the complete block ordering" } },
        "/portable-text": { "post": { "summary": "Render selected assets in the requested order as an Agent-ready portable pack", "requestBody": { "content": { "application/json": { "schema": { "$ref": "#/components/schemas/PortableTextInput" } } } } } },
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
