# CrossPrompt implementation progress

- [x] Scaffold the Rust, Svelte, SQLite, and Docker project
- [x] Implement database migrations and secure application configuration
- [x] Implement Vault, Block, Bundle, Revision, and secret rotation APIs
- [x] Implement notification targets, callback forwarding, rate limits, and SSRF protection
- [x] Implement administrator authentication, dashboard, audit log, and moderation actions
- [x] Implement the public landing page and Vault workspace
- [x] Implement the administrator interface
- [x] Add OpenAPI, health checks, cleanup jobs, deployment documentation, and examples
- [x] Add automated tests for core user, retention, security, and administration flows
- [x] Build and run the full Docker stack on the remote host
- [x] Verify key workflows against the running service

## Typed portable assets

- [x] Add the typed asset catalog and SQLite migration
- [x] Extend the API, revisions, OpenAPI, and portable copy format for typed assets
- [x] Add type templates, guidance, editing, and admin visibility to the web interfaces
- [x] Add automated coverage for typed templates and Agent-ready output
- [x] Rebuild and verify the deployed staging stack

## Email OTP access

- [x] Add verified Vault email bindings, OTP challenges, and email sessions
- [x] Add secure SMTP delivery, expiry, attempt limits, and rate limits
- [x] Allow Vault APIs to authenticate by secret link or Email session
- [x] Add Email login, binding, unbinding, and logout interfaces
- [x] Add automated tests and operational documentation
- [x] Rebuild and verify the deployed staging stack

## Installable portable copy

- [x] Make the Agent Pack explicitly instruct Agents to install or register each asset type
- [x] Add explicit Skill installation guidance and MCP configuration guidance
- [x] Add separate RAW Skill copy actions containing only the Skill Markdown
- [x] Rename packaged copy actions to clarify “paste to Agent for installation”
- [x] Add acceptance coverage and redeploy the updated frontend/backend

## Markdown editor and autosave

- [x] Support Tab and Shift+Tab indentation in Markdown editors
- [x] Make Asset actions directly clickable without draggable-editor focus interference
- [x] Remove manual Block save action and add debounced automatic saving
- [x] Flush pending edits before copying and report autosave/conflict states
- [x] Rebuild, deploy, and verify the updated staging stack

## Relaxed limits and GitHub PR checks

- [x] Raise the per-IP daily Vault creation limit to 100
- [x] Raise the per-Vault Block limit to 1,000
- [x] Raise the per-Vault Bundle limit to 200
- [x] Add a GitHub Action that runs only for opened, synchronized, or reopened PRs
- [x] Push the signed commits to the requested GitHub repository

## Internationalization

- [x] Add a persisted locale store and language switcher for 繁中、English、Español、Français
- [x] Translate Landing, Vault, Email OTP, Notify, and Admin workspace controls
- [x] Add README language index plus English, Spanish, and French versions
- [x] Rebuild and verify the multilingual frontend
- [x] Push the signed i18n commits

## Raw copy and Markdown list indentation

- [x] Add RAW text copy actions for selected Assets, individual Assets, and Bundles
- [x] Keep RAW copies to the original Markdown content without Agent installation instructions
- [x] Make Tab and Shift+Tab indent or unindent Markdown list items like a code editor
- [x] Rebuild and verify the updated frontend

## Immediate language switching

- [x] Refresh Landing, Vault, and Admin UI immediately when locale changes
- [x] Add a clear globe icon and bilingual `Language / 語言` selector label
- [x] Use full native language names in the selector
- [x] Rebuild and verify the language selector behavior

## Notes

- Project host: `moriss@10.121.180.185`
- Project directory: `/home/moriss/cross-prompt`
- Any privileged command must be appended to `sudo.log` before execution. No `sudo` command has been used.

## Google SSO

- [x] Add optional `CROSSPROMPT_GOOGLE_CLIENT_ID` / `SECRET` config
- [x] Leave `.env.example` and `google-oauth.env.example` for the operator to fill
- [x] Implement OAuth start (login GET / bind POST) and callback
- [x] Reuse Email Vault session cookies after Google verification
- [x] Expose `google_login_enabled` on `/api/v1/config`
- [x] Landing Google tab + Vault bind/rebind UI + i18n
- [x] Document redirect URI and security notes in README
- [x] Operator filled Google Cloud OAuth credentials; copied into `.env`
- [x] Redeployed to https://crossprompt.mou.tw with `google_login_enabled: true`
- [x] Google login find-or-create: first sign-in auto-creates Vault (no prior bind required)
- [ ] Live acceptance against a real Google account (retry after auto-create deploy)

## Skill header preview

- [x] Parse SKILL.md YAML frontmatter (`---` / `name` / `description` / block scalars)
- [x] Render it as a Skill 標頭 card instead of a markdown-it setext H2
- [x] Keep Markdown body preview after the header
- [x] Add the standard header to the default Skill template
- [x] Cover parser behavior with `frontend` `node --test`
- [ ] Live Vault UI check after the staging stack is rebuilt

## Landing privacy copy

- [x] Remove the non-E2E / admin-can-view-content warning from the landing page
- [x] Drop unused `privacyWarning` / `privacyText` i18n keys in all locales
- [ ] Browser-check the landing privacy strip after the frontend rebuild (source verified; live stack still building an earlier image)
