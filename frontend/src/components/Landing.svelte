<script>
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';
  import LanguageSwitcher from './LanguageSwitcher.svelte';
  import { locale, t } from '../lib/i18n.js';

  $: activeLocale = $locale;

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
      error = t('invalidSecret');
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

<svelte:head><title>CrossPrompt — {t('typedHeading')}</title></svelte:head>

<header class="site-header landing-header">
  <a class="brand" href="/" aria-label="CrossPrompt 首頁"><span class="brand-mark">C</span>CrossPrompt</a>
  <div class="header-actions"><LanguageSwitcher /><a class="text-link" href="/admin">{t('admin')}</a></div>
</header>

<main data-locale={activeLocale}>
  <section class="hero shell">
    <div class="hero-copy">
      <p class="eyebrow">ONE LINK. EVERY AI.</p>
      <h1>{@html t('heroTitle')}</h1>
      <p class="hero-lead">{t('heroLead')}</p>
      <div class="promise-row">
        <span>{t('typedAssets')}</span><span>{t('agentBundles')}</span><span>{t('aiApi')}</span><span>{t('notify')}</span>
      </div>
    </div>

    <div class="create-panel access-panel">
      <div class="access-tabs" role="tablist" aria-label={t('home')}>
        <button type="button" class:active={mode === 'create'} on:click={() => mode = 'create'}>{t('create')}</button>
        <button type="button" class:active={mode === 'vault'} on:click={() => mode = 'vault'}>{t('vaultLink')}</button>
        <button type="button" class:active={mode === 'email'} on:click={() => mode = 'email'}>{t('emailOtp')}</button>
      </div>

      {#if mode === 'create'}
        <form class="access-form" on:submit|preventDefault={createVault}>
          <div><span class="step-label">{t('permanentVault')}</span><h2>{t('noRegistration')}</h2></div>
          <label>{t('vaultName')}<input bind:value={name} maxlength="100" required autocomplete="off" /></label>
          {#if config.turnstile_required}<div bind:this={turnstileElement} class="turnstile"></div>{/if}
          {#if error}<p class="error-banner" role="alert">{error}</p>{/if}
          <button class="primary large" disabled={busy || (config.turnstile_required && !turnstileToken)}>{busy ? t('creating') : t('createVault')}</button>
          <p class="fine-print">{t('vaultCreatedHint')}</p>
        </form>
      {:else if mode === 'vault'}
        <form class="access-form" on:submit|preventDefault={openVault}>
          <div><span class="step-label">{t('vaultMethod')}</span><h2>{t('useLink')}</h2></div>
          <label>{t('vaultAccess')}<input bind:value={vaultAccess} autocomplete="off" placeholder={t('vaultPlaceholder')} required /></label>
          {#if error}<p class="error-banner" role="alert">{error}</p>{/if}
          <button class="primary large">{t('openVault')}</button>
          <p class="fine-print">{t('vaultSecurity')}</p>
        </form>
      {:else}
        <form class="access-form" on:submit|preventDefault={emailStep === 'request' ? requestEmailCode : verifyEmailCode}>
          <div><span class="step-label">{t('emailMethod')}</span><h2>{emailStep === 'request' ? t('sendCode') : t('enterCode')}</h2></div>
          <label>{t('email')}<input type="email" bind:value={email} maxlength="254" autocomplete="email" required disabled={emailStep === 'verify'} /></label>
          {#if emailStep === 'verify'}
            <label>{t('otp')}<input class="otp-input" inputmode="numeric" pattern="[0-9]{6}" maxlength="6" bind:value={code} autocomplete="one-time-code" required /></label>
          {/if}
          {#if notice}<p class="access-notice" role="status">{notice}</p>{/if}
          {#if error}<p class="error-banner" role="alert">{error}</p>{/if}
          <button class="primary large" disabled={busy || !config.email_login_enabled}>{busy ? t('processing') : emailStep === 'request' ? t('sendCodeButton') : t('verifyLogin')}</button>
          {#if emailStep === 'verify'}<button type="button" class="panel-link" on:click={() => { emailStep = 'request'; code = ''; notice = ''; }}>{t('changeEmail')}</button>{/if}
          <p class="fine-print">{config.email_login_enabled ? t('emailEnabled') : t('emailDisabled')}</p>
        </form>
      {/if}
    </div>
  </section>

  <section class="workflow shell">
    <div class="section-intro"><p class="eyebrow">{t('eyebrowTravel')}</p><h2>{t('typedHeading')}</h2></div>
    <div class="steps-grid">
      <article><span>01</span><h3>{t('step1')}</h3><p>{t('step1Text')}</p></article>
      <article><span>02</span><h3>{t('step2')}</h3><p>{t('step2Text')}</p></article>
      <article><span>03</span><h3>{t('step3')}</h3><p>{t('step3Text')}</p></article>
    </div>
  </section>

  <section class="privacy-strip">
    <div class="shell privacy-grid">
      <div><p class="eyebrow">{t('clearByDesign')}</p><h2>{t('permanentNotVault')}</h2></div>
      <div class="privacy-copy">
        <p>{t('permanentText')}</p>
        <p><strong>{t('privacyWarning')}</strong> {t('privacyText')}</p>
      </div>
    </div>
  </section>
</main>

<footer class="site-footer shell"><span>CrossPrompt</span><span>{t('footer')}</span></footer>
