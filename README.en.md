# CrossPrompt

Languages: [繁體中文](README.md) · [English](README.en.md) · [Español](README.es.md) · [Français](README.fr.md)

CrossPrompt is a private AI-asset homepage with no general user accounts. A high-entropy secret link gives you a permanent Vault for typed Markdown assets, Bundles, and one-click Portable Agent Packs. An AI can maintain the Vault through the HTTP API and call a completion callback that forwards to Pushcut, ntfy, or a generic JSON webhook.

Vault access supports both the original management link and a six-digit one-time code sent to a verified email. Email binding is optional and never replaces or reveals the Vault secret.

## Typed portable assets

Creating an asset loads an editable title and Markdown starter template for its type. The database preserves `block_type` across edits and revision restores.

| Type | Purpose |
| --- | --- |
| `prompt` | Directly executable free-form prompt |
| `prompt_template` | Prompt with `{{variable}}` fields and required-value guidance |
| `skill` | Trigger, workflow, tools, quality checks, and exception handling |
| `mcp_server` | MCP transport, connection, capabilities, environment, and safety boundaries |
| `agent_profile` | Agent role, goals, behavior, and response style |
| `workflow` | Multi-step flow, branches, completion conditions, and notifications |
| `context_pack` | Facts, terms, assumptions, sources, and freshness |
| `preferences` | Long-term communication, tool, and work preferences |
| `tool_api` | Tool/API inputs, outputs, side effects, and error contract |
| `schema` | Fields and limits for structured output such as JSON |
| `evaluation_rubric` | Evidence-based criteria, weights, and passing threshold |
| `safety_policy` | Allowed, confirmation-required, and prohibited actions |

`POST /api/v1/portable-text` returns one ordered text pack. Its header tells the receiving Agent not to execute every item immediately; each asset includes its type, “how to use” guidance, and raw content. MCP and API entries describe configuration only and do not imply a live connection.

## Architecture

- Rust, Axum, Tokio, SQLx, and SQLite backend.
- Svelte/Vite single-page frontend served by the same Rust binary.
- One application container; persistent data lives in the `/data` volume.
- TLS is terminated by an existing Caddy, Nginx, or other reverse proxy.

Management links use `/#/v/{secret}`. The URL fragment is not sent in ordinary requests, logs, or referrers; only a SHA-256 digest is stored in SQLite. Callback URLs contain a path secret, so configure the reverse proxy not to log complete URIs.

## Run

```sh
cp .env.example .env
openssl rand -hex 32       # CROSSPROMPT_SESSION_SECRET
openssl rand -base64 32    # CROSSPROMPT_MASTER_KEY (decode to 32 bytes)
openssl rand -hex 24       # CROSSPROMPT_IP_HASH_SALT
docker build --target password-tool -t crossprompt-password-tool .
printf '%s' 'your-long-admin-password' | docker run --rm -i crossprompt-password-tool
docker compose up -d --build
curl --fail http://127.0.0.1:8080/readyz
```

Production rejects missing administrator credentials, session secret, master key, or IP-hash salt. It also requires an HTTPS public URL, Secure cookies, and paired Turnstile keys. Use `development` or `staging` for local environments.

## Email OTP

Configure SMTP STARTTLS to enable email login and binding:

```dotenv
CROSSPROMPT_SMTP_HOST=smtp.example.com
CROSSPROMPT_SMTP_PORT=587
CROSSPROMPT_SMTP_USERNAME=your-smtp-user
CROSSPROMPT_SMTP_PASSWORD=your-smtp-password
CROSSPROMPT_SMTP_FROM=CrossPrompt <no-reply@example.com>
```

Codes are six digits, valid for ten minutes, limited to five attempts, and stored only as keyed digests. Email sessions are 30-day HttpOnly/SameSite cookies (Secure in production). Rotating a Vault secret invalidates all email sessions.

## Data and limits

- Up to 100 Vaults per IP per day.
- Up to 1,000 Blocks and 200 Bundles per Vault; total content remains 1 MiB and each Block 64 KiB.
- Vaults with Blocks, Bundles, notification settings, or real use never expire for inactivity.
- A completely empty, never-used Vault is removed after 30 days; soft-deleted data is removed after seven days.
- The most recent 100 revisions are retained.

CrossPrompt is **not end-to-end encrypted**. Administrators may inspect content for operations and abuse handling. Never store passwords, API keys, private keys, recovery phrases, or other secrets.

## API and administration

OpenAPI is available at `/api/v1/openapi.json`. User API calls use `Authorization: Bearer {vault-secret}`; Block and Bundle updates/deletes require the current `version` and return `409 Conflict` on mismatch. Vault API requests are limited to 120/minute; callbacks to 10/minute and 100/day.

The `/admin` console has one administrator identity, 12-hour sessions, CSRF-protected mutations, login throttling, and immutable audit records. It can inspect capacity, Vault content, activity, and masked notification targets; suspend, resume, soft-delete, restore within seven days, or permanently delete after entering the full Vault ID. Administrators cannot retrieve secrets or edit user content.

## Development

```sh
docker run --rm -v "$PWD":/app -w /app rust:1.86-bookworm cargo test --bins
docker run --rm -v "$PWD/frontend":/app -w /app node:22-bookworm npm ci
docker run --rm -v "$PWD/frontend":/app -w /app node:22-bookworm npm run check
docker run --rm -v "$PWD/frontend":/app -w /app node:22-bookworm npm run build
```

See [progress.md](progress.md) for the implementation checklist and [sudo.log](sudo.log) for recorded sudo usage.
