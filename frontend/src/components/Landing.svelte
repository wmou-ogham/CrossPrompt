<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';

  let config = { turnstile_required: false, turnstile_site_key: null };
  let name = 'My CrossPrompt';
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

    <form class="create-panel" on:submit|preventDefault={createVault}>
      <div>
        <span class="step-label">建立你的永久 Vault</span>
        <h2>不用註冊，只要保存連結。</h2>
      </div>
      <label>Vault 名稱
        <input bind:value={name} maxlength="100" required autocomplete="off" />
      </label>
      {#if config.turnstile_required}<div bind:this={turnstileElement} class="turnstile"></div>{/if}
      {#if error}<p class="error-banner" role="alert">{error}</p>{/if}
      <button class="primary large" disabled={busy || (config.turnstile_required && !turnstileToken)}>
        {busy ? '建立中…' : '建立私人 Vault →'}
      </button>
      <p class="fine-print">建立後會產生唯一高熵祕密連結。遺失後無法復原，請立即收藏。</p>
    </form>
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
