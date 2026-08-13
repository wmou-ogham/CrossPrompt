<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';

  let config = { turnstile_required: false, turnstile_site_key: null, email_login_enabled: false };
  let mode = 'create';
  let name = 'My CrossPrompt';
  let vaultAccess = '';
  let email = '';
  let code = '';
  let emailStep = 'request';
  let notice = '';
  let turnstileToken = '';
  let busy = false;
  let error = '';
  let turnstileElement;

  onMount(async () => {
    try {
      config = await api('/config');
      if (config.turnstile_required && config.turnstile_site_key) loadTurnstile();
    } catch (requestError) {
      error = requestError.message;
    }
  });

  function loadTurnstile() {
    const getTurnstile = () => (/** @type {any} */ (window)).turnstile;
    const render = () => getTurnstile()?.render(turnstileElement, {
      sitekey: config.turnstile_site_key,
      callback: (token) => turnstileToken = token,
      'expired-callback': () => turnstileToken = ''
    });
    if (getTurnstile()) return render();
    const script = document.createElement('script');
    script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
    script.async = true;
    script.defer = true;
    script.onload = render;
    document.head.appendChild(script);
  }

  async function createVault() {
    busy = true;
    error = '';
    try {
      const result = await api('/vaults', {
        method: 'POST',
        body: { name, turnstile_token: turnstileToken || null }
      });
      window.location.hash = `/v/${result.secret}`;
    } catch (requestError) {
      error = requestError.message;
    } finally {
      busy = false;
    }
  }

  function openVault() {
    error = '';
    const value = vaultAccess.trim();
    const match = value.match(/#\/v\/([^/?#]+)/);
    const secret = match ? decodeURIComponent(match[1]) : value;
    if (!secret || secret.length < 32) {
      error = '請貼上完整 Vault 管理連結或 secret。';
      return;
    }
    window.location.hash = `/v/${encodeURIComponent(secret)}`;
  }

  async function requestEmailCode() {
    busy = true;
    error = '';
    notice = '';
    try {
      const result = await api('/email/login/request-code', {
        method: 'POST', body: { email }
      });
      notice = result.message;
      emailStep = 'verify';
    } catch (requestError) {
      error = requestError.message;
    } finally {
      busy = false;
    }
  }

  async function verifyEmailCode() {
    busy = true;
    error = '';
    try {
      await api('/email/login/verify', {
        method: 'POST', body: { email, code }
      });
      window.location.hash = '/email-vault';
    } catch (requestError) {
      error = requestError.message;
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>CrossPrompt — 你的 AI 資產 Homepage</title></svelte:head>

<header class="site-header landing-header">
  <a class="brand" href="/" aria-label="CrossPrompt 首頁"><span class="brand-mark">C</span>CrossPrompt</a>
  <a class="text-link" href="/admin">管理員</a>
</header>

<main>
  <section class="hero shell">
    <div class="hero-copy">
      <p class="eyebrow">ONE LINK. EVERY AI.</p>
      <h1>把你習慣的 AI 工具，<br />帶到任何平臺。</h1>
      <p class="hero-lead">不只是 Markdown 剪貼簿。用有結構的 Prompt、Template、Skill、MCP、Workflow 與 Schema 組成私人 Vault，再複製成 Agent 看得懂的單一文字包。</p>
      <div class="promise-row">
        <span>12 Typed Assets</span><span>Agent-ready Bundles</span><span>AI-writable API</span><span>Completion Notify</span>
      </div>
    </div>

    <div class="create-panel access-panel">
      <div class="access-tabs" role="tablist" aria-label="CrossPrompt 存取方式">
        <button type="button" class:active={mode === 'create'} on:click={() => mode = 'create'}>建立</button>
        <button type="button" class:active={mode === 'vault'} on:click={() => mode = 'vault'}>Vault 連結</button>
        <button type="button" class:active={mode === 'email'} on:click={() => mode = 'email'}>Email 驗證碼</button>
      </div>

      {#if mode === 'create'}
        <form class="access-form" on:submit|preventDefault={createVault}>
          <div><span class="step-label">建立你的永久 Vault</span><h2>不用註冊，只要保存連結。</h2></div>
          <label>Vault 名稱<input bind:value={name} maxlength="100" required autocomplete="off" /></label>
          {#if config.turnstile_required}<div bind:this={turnstileElement} class="turnstile"></div>{/if}
          {#if error}<p class="error-banner" role="alert">{error}</p>{/if}
          <button class="primary large" disabled={busy || (config.turnstile_required && !turnstileToken)}>{busy ? '建立中…' : '建立私人 Vault →'}</button>
          <p class="fine-print">建立後會產生唯一高熵祕密連結。你可以進入 Vault 後再綁定已驗證的 Email。</p>
        </form>
      {:else if mode === 'vault'}
        <form class="access-form" on:submit|preventDefault={openVault}>
          <div><span class="step-label">方式一 · VAULT</span><h2>使用目前的管理連結。</h2></div>
          <label>Vault 管理連結或 secret<input bind:value={vaultAccess} autocomplete="off" placeholder="https://…/#/v/…" required /></label>
          {#if error}<p class="error-banner" role="alert">{error}</p>{/if}
          <button class="primary large">開啟 Vault →</button>
          <p class="fine-print">Secret 只留在 URL fragment，不會送進一般 HTTP access log。請把它當成密碼保管。</p>
        </form>
      {:else}
        <form class="access-form" on:submit|preventDefault={emailStep === 'request' ? requestEmailCode : verifyEmailCode}>
          <div><span class="step-label">方式二 · EMAIL OTP</span><h2>{emailStep === 'request' ? '寄一組登入驗證碼。' : '輸入六位數驗證碼。'}</h2></div>
          <label>Email<input type="email" bind:value={email} maxlength="254" autocomplete="email" required disabled={emailStep === 'verify'} /></label>
          {#if emailStep === 'verify'}
            <label>六位數驗證碼<input class="otp-input" inputmode="numeric" pattern="[0-9]{6}" maxlength="6" bind:value={code} autocomplete="one-time-code" required /></label>
          {/if}
          {#if notice}<p class="access-notice" role="status">{notice}</p>{/if}
          {#if error}<p class="error-banner" role="alert">{error}</p>{/if}
          <button class="primary large" disabled={busy || !config.email_login_enabled}>{busy ? '處理中…' : emailStep === 'request' ? '寄送驗證碼 →' : '驗證並登入 →'}</button>
          {#if emailStep === 'verify'}<button type="button" class="panel-link" on:click={() => { emailStep = 'request'; code = ''; notice = ''; }}>更換 Email 或重新寄送</button>{/if}
          <p class="fine-print">{config.email_login_enabled ? '驗證碼 10 分鐘內有效；成功後此瀏覽器保持登入 30 天。系統不會透露未綁定的 Email。' : '站台管理員尚未設定 SMTP，因此 Email 登入目前不可用。'}</p>
        </form>
      {/if}
    </div>
  </section>

  <section class="workflow shell">
    <div class="section-intro"><p class="eyebrow">HOW IT TRAVELS</p><h2>有型別的文字，讓 Agent 知道該怎麼用。</h2></div>
    <div class="steps-grid">
      <article><span>01</span><h3>選型別，取得骨架</h3><p>建立 Prompt Template、Skill、MCP Server、Agent Profile、Workflow、Schema 等 12 種資產時，自動帶入可編輯模板。</p></article>
      <article><span>02</span><h3>組成 Agent Pack</h3><p>依任務勾選資產，一鍵產生總體說明、各型別使用方式與內容，貼到任何能接收文字的 AI。</p></article>
      <article><span>03</span><h3>交給 AI 維護</h3><p>複製內建 API 說明，AI 就能新增、修改、排序及刪除；做完還能通知你。</p></article>
    </div>
  </section>

  <section class="privacy-strip">
    <div class="shell privacy-grid">
      <div><p class="eyebrow">CLEAR BY DESIGN</p><h2>永久保存，但不是祕密保管箱。</h2></div>
      <div class="privacy-copy">
        <p>有內容或通知設定的 Vault 不會因為閒置而過期。只有建立 30 天仍完全空白的 Vault，或刪除後超過 7 天的 Vault，才會清除。</p>
        <p><strong>CrossPrompt 並非端對端加密。</strong>服務管理員可基於維運與濫用處理查看內容。請勿存放密碼、API Key、助記詞或其他機密。</p>
      </div>
    </div>
  </section>
</main>

<footer class="site-footer shell"><span>CrossPrompt</span><span>Portable AI assets, without platform lock-in.</span></footer>
