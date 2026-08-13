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

## Notes

- Project host: `moriss@10.121.180.185`
- Project directory: `/home/moriss/cross-prompt`
- Any privileged command must be appended to `sudo.log` before execution. No `sudo` command has been used.
