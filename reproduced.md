# Reproduce Google SSO setup

## 1. Fill credentials

Copy placeholders and edit:

```sh
cp google-oauth.env.example google-oauth.env   # optional local worksheet; do not commit secrets
# Or edit .env directly using keys from .env.example / google-oauth.env.example
```

Required in `.env` (alongside existing CrossPrompt vars):

```dotenv
CROSSPROMPT_PUBLIC_BASE_URL=https://your-host.example   # must match Google Console origin
CROSSPROMPT_GOOGLE_CLIENT_ID=....apps.googleusercontent.com
CROSSPROMPT_GOOGLE_CLIENT_SECRET=...
```

Google Cloud Console (OAuth client, Web application):

- Authorized JavaScript origins: same as `CROSSPROMPT_PUBLIC_BASE_URL` (no trailing slash)
- Authorized redirect URI: `{CROSSPROMPT_PUBLIC_BASE_URL}/api/v1/auth/google/callback`

Local example:

- Origin: `http://localhost:8080`
- Redirect: `http://localhost:8080/api/v1/auth/google/callback`

## 2. Build and run

```sh
docker compose up -d --build
curl --fail http://127.0.0.1:8080/readyz
curl -s http://127.0.0.1:8080/api/v1/config | jq '.google_login_enabled'
# expect: true after credentials are set
```

Without Docker (dev):

```sh
cd frontend && npm ci && npm run build && cd ..
CROSSPROMPT_FRONTEND_DIR=frontend/dist cargo run
```

## 3. Manual check

1. Open the site → **Google** tab → Continue with Google.
2. Or open a Vault → Settings → Bind / change email with Google.
3. After success you should land on `#/email-vault` with a working session.

## 4. Automated tests

```sh
cargo test
```

# Reproduce Skill header preview

The Vault Markdown preview used to treat a SKILL.md YAML header as Markdown. The closing `---` became a setext heading underline, so `name` / `description` rendered as a giant H2.

## Automated checks

From `frontend/`:

```sh
npm ci
npm test      # YAML frontmatter split + markdown-it regression
npm run check # Svelte diagnostics
```

The backend Skill template now starts with a standard header. After Rust is available:

```sh
cargo test catalog_has_unique_complete_types --locked
```

Docker image build also runs `npm test` during the frontend stage:

```sh
docker compose build
```

## Manual check

1. Open a Vault → Assets → create or edit a `skill` block.
2. Paste a standard SKILL.md, for example:

```md
---
name: porting-session-hygiene
description: Use for long porting/bringup sessions on a remote machine.
---

# Porting / build session hygiene

Keep the tree clean.
```

3. The **安全預覽** pane should show a **Skill 標頭** card (`name` + `description`) and then the H1 body. It must not render `name:` / `description:` as a huge bold heading.
4. New Skill assets from the type catalog should already include the YAML header skeleton.

