# CrossPrompt

CrossPrompt 是一個無一般使用者帳號的私人 Prompt Homepage。使用者取得一條高熵祕密連結後，可永久保存 Markdown Blocks、建立 Bundles、一鍵合併複製，或讓 AI 透過 HTTP API 完整維護內容。AI 完成工作後也能呼叫 callback，轉送到 Pushcut、ntfy 或通用 JSON webhook。

## 架構

- Rust、Axum、Tokio、SQLx 與 SQLite 後端。
- Svelte 與 Vite 單頁前端；production build 由同一個 Rust binary 提供。
- 單一 application container，持久資料只放在 `/data` volume。
- TLS 交給既有 Caddy、Nginx 或其他反向代理。

管理連結採用 `/#/v/{secret}`。URL fragment 不會出現在一般 HTTP request、access log 或 referrer；資料庫只保存 secret 的 SHA-256 digest。Callback 依規格使用 path secret，因此應確保反向代理 access log 不記錄完整 URI；CrossPrompt 本身的 structured trace 明確不記錄 URI。

## 啟動

1. 複製環境設定：

   ```sh
   cp .env.example .env
   ```

2. 產生三組隨機值：

   ```sh
   openssl rand -hex 32
   openssl rand -base64 32
   openssl rand -hex 24
   ```

   分別填入 `CROSSPROMPT_SESSION_SECRET`、`CROSSPROMPT_MASTER_KEY`（必須解碼為 32 bytes）與 `CROSSPROMPT_IP_HASH_SALT`。

3. 產生 Argon2id 管理員密碼 hash。密碼透過 stdin 傳入，不會成為 container 參數：

   ```sh
   docker build --target password-tool -t crossprompt-password-tool .
   printf '%s' 'your-long-admin-password' | docker run --rm -i crossprompt-password-tool
   ```

   將輸出完整填入 `CROSSPROMPT_ADMIN_PASSWORD_HASH`，並用單引號包住整段 hash，例如 `CROSSPROMPT_ADMIN_PASSWORD_HASH='$argon2id$…'`，避免 Compose interpolation。

4. 設定正式網域、Turnstile keys，然後啟動：

   ```sh
   docker compose up -d --build
   docker compose ps
   curl --fail http://127.0.0.1:8080/readyz
   ```

Production 模式缺少管理員帳號、Argon2id hash、Session secret、master key 或 IP hash salt 時會拒絕啟動。管理 Cookie 在 production 預設為 `Secure`、`HttpOnly`、`SameSite=Strict`，Session 有效 12 小時。

## 使用者資料規則

- 有 Block、Bundle、通知設定，或曾真正使用過的 Vault 永不因閒置而自動過期。
- 建立後 30 天仍從未建立 Block、沒有 Bundle、沒有通知設定，且未停用／刪除的空白 Vault 會永久清除。
- 使用者或管理員 soft delete 後保留七天，再永久清除。
- 使用者只能復原自己刪除的 Vault；管理員刪除必須由管理員復原。
- 最近保留 100 筆 Revision。

CrossPrompt **不是端對端加密服務**。管理員基於維運與濫用處理可以查看內容。不要存放密碼、API Key、私鑰、助記詞或其他機密。

## API

OpenAPI 文件位於 `/api/v1/openapi.json`。除了建立 Vault 與 callback，使用者 API 都需要：

```http
Authorization: Bearer {vault-secret}
Content-Type: application/json
```

常用範例：

```sh
curl -H "Authorization: Bearer $VAULT_SECRET" http://localhost:8080/api/v1/vault

curl -X POST -H "Authorization: Bearer $VAULT_SECRET" -H 'Content-Type: application/json' \
  'http://localhost:8080/api/v1/blocks?source=Claude' \
  --data '{"title":"System Prompt","content":"Answer in Traditional Chinese."}'
```

Block 與 Bundle 更新／刪除必須帶目前 `version`；不同步會回傳 `409 Conflict`。一般 Vault API 每分鐘 120 次；callback 每分鐘 10 次、每日 100 次。

Callback payload：

```json
{
  "status": "completed",
  "title": "任務完成",
  "message": "分析已完成，可以回來查看結果。",
  "source": "Claude",
  "url": "https://example.com/result"
}
```

`status` 只接受 `completed`、`needs_input`、`failed`。

## Webhook 安全

通知 URL 與自訂 headers 以 AES-256-GCM 和 server master key 加密保存。通用 webhook 只允許 HTTPS 443、禁止 redirects，DNS 解析結果在請求期間固定，並拒絕 loopback、private、link-local、metadata、documentation 與 reserved networks。連線 timeout 為五秒。Delivery log 只保存 target 類型、HTTP status、成功與否及時間，不保存通知全文。

## 管理後台

`/admin` 只有單一管理員身份。登入同一 IP 每 15 分鐘最多五次失敗；所有 mutation 都需要 CSRF token。管理員可以：

- 檢視容量、建立量、delivery 成敗與最大 Vault。
- 依 ID／名稱、狀態、建立／修改時間及容量查詢，每頁 50 筆。
- 查看完整 Block、Bundle、Revision 與遮罩後的通知 target。
- Suspend、Resume、soft delete、七日內 Restore，或輸入完整 Vault ID 後 permanent delete。

管理員不能取得 secret，也不能修改使用者內容或通知設定。查看內容及所有管理動作都會留下 UI 無法修改的 audit log；IP 只保存 salted hash。

## 備份與還原

最簡單且一致的備份方式是短暫停止服務後封存 named volume：

```sh
docker compose stop crossprompt
docker run --rm -v cross-prompt_crossprompt_data:/data -v "$PWD":/backup alpine \
  tar -czf /backup/crossprompt-data.tgz -C /data .
docker compose start crossprompt
```

還原前先停止服務，並先另存目前 volume。還原完成後執行 `docker compose up -d`，再檢查 `/readyz`、Vault 與管理後台。

## 開發與驗證

```sh
docker run --rm -v "$PWD":/app -w /app rust:1.86-bookworm cargo test --bins
docker run --rm -v "$PWD/frontend":/app -w /app node:22-bookworm npm ci
docker run --rm -v "$PWD/frontend":/app -w /app node:22-bookworm npm run check
docker run --rm -v "$PWD/frontend":/app -w /app node:22-bookworm npm run build
```

健康檢查：`GET /healthz`；包含 SQLite readiness 的檢查：`GET /readyz`。

## 維運檔案

- [progress.md](progress.md)：Markdown checklist 形式的實作進度。
- [sudo.log](sudo.log)：遠端主機上的 sudo 使用紀錄。目前專案建立與部署不需要 sudo。
