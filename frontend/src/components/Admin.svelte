<script>
  import { onMount } from 'svelte';
  import { ApiError, api } from '../lib/api.js';
  import LanguageSwitcher from './LanguageSwitcher.svelte';
  import { locale, t } from '../lib/i18n.js';

  $: activeLocale = $locale;

  let authenticated = false;
  let login = { username: '', password: '' };
  let csrf = sessionStorage.getItem('crossprompt_csrf') || '';
  let overview = null;
  let vaults = [];
  let detail = null;
  let audit = [];
  let filters = { q: '', status: '', sort: 'updated_desc', page: 1 };
  let view = 'vaults';
  let busy = false;
  let error = '';
  let notice = '';

  onMount(() => {
    const handleLocaleChange = (event) => activeLocale = event.detail;
    window.addEventListener('crossprompt:locale', handleLocaleChange);
    return () => window.removeEventListener('crossprompt:locale', handleLocaleChange);
  });

  onMount(async () => {
    if (!csrf) return;
    try {
      await api('/admin/session');
      authenticated = true;
      await refresh();
    } catch {
      sessionStorage.removeItem('crossprompt_csrf');
      csrf = '';
    }
  });

  async function signIn() {
    await run(async () => {
      const result = await api('/admin/session', { method: 'POST', body: login });
      csrf = result.csrf_token;
      sessionStorage.setItem('crossprompt_csrf', csrf);
      authenticated = true;
      login.password = '';
      await refresh();
    }, '已登入管理後台');
  }

  async function signOut() {
    await run(async () => {
      await api('/admin/session', { method: 'DELETE', csrf });
      sessionStorage.removeItem('crossprompt_csrf');
      csrf = '';
      authenticated = false;
      overview = null;
    }, '已登出');
  }

  async function run(action, success = '') {
    busy = true;
    error = '';
    notice = '';
    try {
      await action();
      notice = success;
      if (success) window.setTimeout(() => notice = '', 2200);
    } catch (requestError) {
      if (requestError instanceof ApiError && requestError.status === 401) {
        authenticated = false;
        csrf = '';
        sessionStorage.removeItem('crossprompt_csrf');
      }
      error = requestError.message;
    } finally {
      busy = false;
    }
  }

  async function refresh() {
    const [summary] = await Promise.all([api('/admin/overview'), loadVaults()]);
    overview = summary;
  }

  async function loadVaults() {
    const params = new URLSearchParams({ page: String(filters.page), sort: filters.sort });
    if (filters.q.trim()) params.set('q', filters.q.trim());
    if (filters.status) params.set('status', filters.status);
    const result = await api(`/admin/vaults?${params}`);
    vaults = result.items;
  }

  async function applyFilters() {
    filters.page = 1;
    await run(loadVaults);
  }

  async function openVault(id) {
    await run(async () => {
      detail = await api(`/admin/vaults/${id}`);
    });
  }

  async function showAudit() {
    view = 'audit';
    await run(async () => {
      const result = await api('/admin/audit-log');
      audit = result.items;
    });
  }

  async function action(id, name) {
    let body = { reason: null };
    let method = 'POST';
    if (name === 'suspend' || name === 'delete' || name === 'resume' || name === 'restore') {
      const reason = prompt(name === 'suspend' || name === 'delete' ? '請填寫內部原因（可留空）' : '復原／恢復原因（可留空）');
      if (reason === null) return;
      body = { reason };
    }
    if (name === 'permanent') {
      const confirmation = prompt(`這會立即且不可復原地清除所有資料。請輸入完整 Vault ID：\n${id}`);
      if (confirmation === null) return;
      const reason = prompt('永久刪除原因（可留空）');
      if (reason === null) return;
      body = { confirmation, reason };
      method = 'DELETE';
    }
    await run(async () => {
      await api(`/admin/vaults/${id}/${name}`, { method, body, csrf });
      detail = null;
      await refresh();
    }, `管理動作 ${name} 已完成`);
  }

  function bytes(value) {
    if (value < 1024) return `${value} B`;
    if (value < 1048576) return `${(value / 1024).toFixed(1)} KiB`;
    return `${(value / 1048576).toFixed(1)} MiB`;
  }
</script>

<svelte:head><title>Admin — CrossPrompt</title></svelte:head>

{#key activeLocale}
<header class="site-header admin-header">
  <a class="brand" href="/"><span class="brand-mark">C</span>CrossPrompt <small>ADMIN</small></a>
  <div class="header-actions"><LanguageSwitcher />{#if authenticated}<button class:active={view === 'vaults'} class="quiet" on:click={() => view = 'vaults'}>Vaults</button><button class:active={view === 'audit'} class="quiet" on:click={showAudit}>{t('auditLog')}</button><button class="quiet" on:click={signOut}>{t('signOut')}</button>{/if}</div>
</header>

{#if error}<div class="floating-message error-banner" role="alert">{error}</div>{/if}
{#if notice}<div class="floating-message success-banner">{notice}</div>{/if}

{#if !authenticated}
  <main class="login-page shell" data-locale={activeLocale}>
    <form class="login-panel" on:submit|preventDefault={signIn}>
      <p class="eyebrow">RESTRICTED OPERATIONS</p><h1>{t('adminLogin')}</h1>
      <p>{t('adminLoginText')}</p>
      <label>{t('username')}<input bind:value={login.username} autocomplete="username" required /></label>
      <label>{t('password')}<input type="password" bind:value={login.password} autocomplete="current-password" required /></label>
      <button class="primary large" disabled={busy}>{busy ? t('verifying') : t('login')}</button>
    </form>
  </main>
{:else}
  <main class="admin-main shell-wide" data-locale={activeLocale}>
    {#if overview}
      <section class="metric-grid">
        <article><span>Vaults</span><strong>{overview.vaults.total}</strong><small>{overview.vaults.active} active · {overview.vaults.suspended} suspended</small></article>
        <article><span>Content</span><strong>{overview.objects.blocks + overview.objects.bundles}</strong><small>{overview.objects.blocks} blocks · {overview.objects.bundles} bundles</small></article>
        <article><span>Revisions</span><strong>{overview.objects.revisions}</strong><small>{bytes(overview.storage.revision_bytes)} logical</small></article>
        <article><span>SQLite</span><strong>{bytes(overview.storage.database_file_bytes)}</strong><small>DB + WAL + SHM</small></article>
        <article><span>Created / 30d</span><strong>{overview.vaults.created_30d}</strong><small>{overview.vaults.created_24h} in last 24h</small></article>
        <article><span>Webhook / 30d</span><strong>{overview.webhooks_30d.success}</strong><small>{overview.webhooks_30d.failed} failed</small></article>
      </section>
    {/if}

    {#if view === 'vaults'}
      <section class="admin-section">
        <div class="page-heading"><div><p class="eyebrow">{t('operations')}</p><h1>{t('vaultManagement')}</h1></div><p>{t('adminReadOnly')}</p></div>
        <form class="filters" on:submit|preventDefault={applyFilters}>
          <input bind:value={filters.q} placeholder={t('searchVault')} />
          <select bind:value={filters.status}><option value="">{t('allStatuses')}</option><option value="active">Active</option><option value="suspended">Suspended</option><option value="deleted">Deleted</option><option value="empty">Never used</option></select>
          <select bind:value={filters.sort}><option value="updated_desc">{t('recentlyUpdated')}</option><option value="updated_asc">{t('oldestUpdated')}</option><option value="created_asc">{t('oldestCreated')}</option><option value="size_desc">{t('largest')}</option></select>
          <button class="primary">{t('query')}</button>
        </form>
        <div class="table-wrap"><table><thead><tr><th>Vault</th><th>狀態</th><th>Objects</th><th>容量</th><th>最後修改</th><th></th></tr></thead><tbody>
          {#each vaults as vault}
            <tr><td><strong>{vault.name}</strong><code>{vault.id}</code></td><td><span class="status-pill {vault.status}">{vault.status}</span>{#if !vault.ever_used}<small>never used</small>{/if}</td><td>{vault.block_count} / {vault.bundle_count} / {vault.revision_count}</td><td>{bytes(vault.content_bytes)}</td><td>{new Date(vault.updated_at).toLocaleString()}</td><td><button class="quiet compact" on:click={() => openVault(vault.id)}>{t('view')}</button></td></tr>
          {:else}<tr><td colspan="6">{t('noVaults')}</td></tr>{/each}
        </tbody></table></div>
        <div class="pager"><button class="quiet" disabled={filters.page <= 1} on:click={async () => { filters.page -= 1; await loadVaults(); }}>{t('previous')}</button><span>{t('page', { page: filters.page })}</span><button class="quiet" disabled={vaults.length < 50} on:click={async () => { filters.page += 1; await loadVaults(); }}>{t('next')}</button></div>
      </section>
    {:else}
      <section class="admin-section">
        <div class="page-heading"><div><p class="eyebrow">{t('immutable')}</p><h1>{t('auditTitle')}</h1></div><button class="quiet" on:click={showAudit}>{t('refresh')}</button></div>
        <div class="table-wrap"><table><thead><tr><th>時間</th><th>操作</th><th>Vault ID</th><th>原因</th><th>管理員 IP hash</th></tr></thead><tbody>
          {#each audit as item}<tr><td>{new Date(item.created_at).toLocaleString()}</td><td><span class="status-pill">{item.action}</span></td><td><code>{item.vault_id || '—'}</code></td><td>{item.reason || '—'}</td><td><code>{item.ip_hash.slice(0, 16)}…</code></td></tr>{/each}
        </tbody></table></div>
      </section>
    {/if}
  </main>
{/if}

{#if detail}
  <div class="modal-backdrop" role="presentation" on:click={(event) => event.currentTarget === event.target && (detail = null)}>
    <div class="admin-detail" role="dialog" aria-modal="true" aria-label={t('detail')} tabindex="-1">
      <div class="detail-header"><div><span class="status-pill {detail.vault.status}">{detail.vault.status}</span><h2>{detail.vault.name}</h2><code>{detail.vault.id}</code></div><button class="quiet" on:click={() => detail = null}>{t('close')}</button></div>
      <dl class="detail-meta"><div><dt>{t('created')}</dt><dd>{new Date(detail.vault.created_at).toLocaleString()}</dd></div><div><dt>{t('modified')}</dt><dd>{new Date(detail.vault.updated_at).toLocaleString()}</dd></div><div><dt>Email</dt><dd>{detail.vault.email || t('unbound')}</dd></div><div><dt>Notify</dt><dd>{detail.notification_target?.masked_url || t('unset')}</dd></div><div><dt>{t('objects')}</dt><dd>{detail.blocks.length} / {detail.bundles.length} / {detail.revisions.length}</dd></div></dl>
      <div class="admin-actions">
        {#if detail.vault.status === 'active'}<button class="warning" on:click={() => action(detail.vault.id, 'suspend')}>{t('suspend')}</button>{/if}
        {#if detail.vault.status === 'suspended'}<button class="primary" on:click={() => action(detail.vault.id, 'resume')}>{t('resume')}</button>{/if}
        {#if detail.vault.status !== 'deleted'}<button class="danger" on:click={() => action(detail.vault.id, 'delete')}>{t('softDelete')}</button>{/if}
        {#if detail.vault.status === 'deleted'}<button class="primary" on:click={() => action(detail.vault.id, 'restore')}>{t('restore')}</button>{/if}
        <button class="danger" on:click={() => action(detail.vault.id, 'permanent')}>{t('permanentDelete')}</button>
      </div>
      <h3>{t('typedAssets')}</h3>
      {#each detail.blocks as block}<article class="content-inspection"><div><strong><span class="inline-type">{block.block_type || 'prompt'}</span>{block.title}</strong><span>v{block.version} · position {block.position}</span></div><pre>{block.content}</pre></article>{:else}<p class="muted">{t('noAssets')}</p>{/each}
      <h3>Bundles</h3>
      {#each detail.bundles as bundle}<article class="content-inspection"><strong>{bundle.name}</strong><pre>{JSON.stringify(bundle.block_ids, null, 2)}</pre></article>{:else}<p class="muted">{t('noBundles')}</p>{/each}
      <h3>{t('recentSources')}</h3>
      <div class="history-list compact-list">{#each detail.revisions as revision}<article><div><strong>{revision.action} {revision.resource_type}</strong><code>{revision.resource_id || 'vault'}</code></div><span>{revision.source} · {new Date(revision.created_at).toLocaleString()}</span></article>{/each}</div>
    </div>
  </div>
{/if}
{/key}
