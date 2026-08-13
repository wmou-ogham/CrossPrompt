<script>
  import { onMount, tick } from 'svelte';
  import Markdown from './Markdown.svelte';
  import { ApiError, api, copyText, downloadJson } from '../lib/api.js';
  import LanguageSwitcher from './LanguageSwitcher.svelte';
  import { locale, t } from '../lib/i18n.js';

  export let secret;
  export let emailSession = false;

  $: activeLocale = $locale;

  let snapshot = null;
  let artifactTypes = [];
  let siteConfig = { email_login_enabled: false };
  let revisions = [];
  let selected = [];
  let activeTab = 'blocks';
  let busy = false;
  let notice = '';
  let error = '';
  let deleted = false;
  let locked = false;
  let newBlock = null;
  let bundleName = '';
  let dragId = '';
  let target = { kind: 'ntfy', url: '', headers: '{}' };
  let emailBinding = { email: '', code: '', step: 'request' };
  let blockSaveStates = {};
  const blockSaveTimers = new Map();
  const blockSavePromises = new Map();

  $: blocks = snapshot?.blocks || [];
  $: bundles = snapshot?.bundles || [];
  $: selectedBlocks = blocks.filter((block) => selected.includes(block.id));
  $: selectedRawSkill = selectedBlocks.length === 1 && selectedBlocks[0].block_type === 'skill' ? selectedBlocks[0] : null;
  $: selectedType = newBlock ? typeFor(newBlock.block_type) : null;

  onMount(load);

  async function load() {
    error = '';
    try {
      const [nextSnapshot, types, nextConfig] = await Promise.all([
        api('/vault', { secret }),
        api('/artifact-types'),
        api('/config')
      ]);
      snapshot = nextSnapshot;
      artifactTypes = types;
      siteConfig = nextConfig;
      if (!emailBinding.email && snapshot.vault.email) emailBinding.email = snapshot.vault.email;
      deleted = false;
      locked = false;
      if (activeTab === 'history') await loadRevisions();
    } catch (requestError) {
      if (requestError instanceof ApiError && requestError.status === 410) deleted = true;
      else if (requestError instanceof ApiError && requestError.status === 423) locked = true;
      else error = requestError.message;
    }
  }

  function typeFor(key) {
    return artifactTypes.find((item) => item.key === (key || 'prompt')) || artifactTypes[0];
  }

  function startTypedBlock(type) {
    newBlock = {
      block_type: type.key,
      title: type.default_title,
      content: type.template
    };
  }

  async function run(action, success = '已儲存') {
    busy = true;
    error = '';
    notice = '';
    try {
      await action();
      notice = success;
      window.setTimeout(() => notice = '', 2200);
    } catch (requestError) {
      error = requestError.message;
    } finally {
      busy = false;
    }
  }

  async function renameVault() {
    await run(async () => {
      const vault = await api('/vault', { method: 'PATCH', body: { name: snapshot.vault.name }, secret });
      snapshot = { ...snapshot, vault };
    }, '名稱已更新');
  }

  async function createBlock() {
    await run(async () => {
      const block = await api('/blocks?source=web', { method: 'POST', body: newBlock, secret });
      snapshot = { ...snapshot, blocks: [...blocks, block] };
      newBlock = null;
    }, '型別化資產已建立');
  }

  function setBlockSaveState(id, state) {
    blockSaveStates = { ...blockSaveStates, [id]: state };
  }

  function clearBlockSaveTimer(id) {
    const timer = blockSaveTimers.get(id);
    if (timer) window.clearTimeout(timer);
    blockSaveTimers.delete(id);
  }

  function scheduleBlockSave(block) {
    if (!block?.id || !snapshot?.blocks?.some((item) => item.id === block.id)) return;
    clearBlockSaveTimer(block.id);
    setBlockSaveState(block.id, 'pending');
    blockSaveTimers.set(block.id, window.setTimeout(() => {
      blockSaveTimers.delete(block.id);
      startBlockSave(block);
    }, 700));
  }

  function startBlockSave(block) {
    const promise = persistBlock(block).finally(() => {
      if (blockSavePromises.get(block.id) === promise) blockSavePromises.delete(block.id);
    });
    blockSavePromises.set(block.id, promise);
    return promise;
  }

  async function persistBlock(block) {
    if (!block?.id || !snapshot?.blocks?.some((item) => item.id === block.id)) return;
    const draft = {
      block_type: block.block_type,
      title: block.title,
      content: block.content,
      position: block.position,
      version: block.version
    };
    setBlockSaveState(block.id, 'saving');
    try {
      const updated = await api(`/blocks/${block.id}?source=web-autosave`, {
        method: 'PATCH',
        body: draft,
        secret
      });
      const current = snapshot.blocks.find((item) => item.id === block.id);
      if (!current) return;
      const changedWhileSaving = current.block_type !== draft.block_type
        || current.title !== draft.title
        || current.content !== draft.content
        || current.position !== draft.position;
      snapshot = {
        ...snapshot,
        blocks: blocks.map((item) => item.id === updated.id
          ? (changedWhileSaving ? { ...item, version: updated.version, updated_at: updated.updated_at } : updated)
          : item)
      };
      setBlockSaveState(block.id, changedWhileSaving ? 'pending' : 'saved');
      if (changedWhileSaving) scheduleBlockSave(snapshot.blocks.find((item) => item.id === block.id));
    } catch (requestError) {
      if (requestError instanceof ApiError && requestError.status === 409) {
        setBlockSaveState(block.id, 'conflict');
        error = `「${block.title}」自動儲存遇到版本衝突，請重新載入後再編輯。`;
      } else {
        setBlockSaveState(block.id, 'error');
        error = requestError.message;
      }
    }
  }

  async function removeBlock(block) {
    if (!confirm(`刪除「${block.title}」？可在版本歷史中還原。`)) return;
    clearBlockSaveTimer(block.id);
    await run(async () => {
      await api(`/blocks/${block.id}?source=web`, { method: 'DELETE', body: { version: block.version }, secret });
      selected = selected.filter((id) => id !== block.id);
      const nextStates = { ...blockSaveStates };
      delete nextStates[block.id];
      blockSaveStates = nextStates;
      await load();
    }, 'Block 已刪除');
  }

  function saveStateLabel(block) {
    return {
      pending: t('autosavePending'),
      saving: t('autosaving'),
      saved: t('autosaved'),
      conflict: t('conflict'),
      error: t('saveFailed')
    }[blockSaveStates[block.id]] || '';
  }

  async function handleMarkdownTab(event, draft) {
    if (event.key !== 'Tab' || !draft) return;
    event.preventDefault();
    const textarea = event.currentTarget;
    const { value, selectionStart: start, selectionEnd: end } = textarea;
    if (start === end) {
      const lineStart = value.lastIndexOf('\n', start - 1) + 1;
      const lineEnd = value.indexOf('\n', start);
      const actualLineEnd = lineEnd === -1 ? value.length : lineEnd;
      const line = value.slice(lineStart, actualLineEnd);
      if (event.shiftKey) {
        const remove = line.startsWith('\t') ? 1 : line.startsWith('  ') ? 2 : 0;
        if (!remove) return;
        draft.content = value.slice(0, lineStart) + value.slice(lineStart + remove);
        await tick();
        textarea.setSelectionRange(Math.max(lineStart, start - remove), Math.max(lineStart, start - remove));
      } else {
        draft.content = value.slice(0, start) + '\t' + value.slice(start);
        await tick();
        textarea.setSelectionRange(start + 1, start + 1);
      }
    } else {
      const lineStart = value.lastIndexOf('\n', start - 1) + 1;
      const nextLine = value.indexOf('\n', end);
      const lineEnd = nextLine === -1 ? value.length : nextLine;
      const selectedLines = value.slice(lineStart, lineEnd).split('\n');
      let removedFirst = 0;
      let removedTotal = 0;
      const transformed = selectedLines.map((line, index) => {
        if (!event.shiftKey) return `\t${line}`;
        const remove = line.startsWith('\t') ? 1 : line.startsWith('  ') ? 2 : 0;
        if (index === 0) removedFirst = remove;
        removedTotal += remove;
        return line.slice(remove);
      }).join('\n');
      draft.content = value.slice(0, lineStart) + transformed + value.slice(lineEnd);
      await tick();
      const nextStart = event.shiftKey ? Math.max(lineStart, start - removedFirst) : start + 1;
      const nextEnd = event.shiftKey ? Math.max(nextStart, end - removedTotal) : end + selectedLines.length;
      textarea.setSelectionRange(nextStart, nextEnd);
    }
    if (draft.id) scheduleBlockSave(draft);
  }

  async function flushBlockSaves(ids = blocks.map((block) => block.id)) {
    const requested = new Set(ids);
    const pending = blocks.filter((block) => requested.has(block.id) && ['pending', 'error'].includes(blockSaveStates[block.id]));
    pending.forEach(clearBlockSaveTimer);
    const inFlight = ids.map((id) => blockSavePromises.get(id)).filter(Boolean);
    const queued = pending.map((block) => startBlockSave(block));
    await Promise.all([...new Set([...inFlight, ...queued])]);
  }

  function toggleSelected(id) {
    const ids = new Set(selected);
    if (ids.has(id)) ids.delete(id); else ids.add(id);
    selected = blocks.filter((block) => ids.has(block.id)).map((block) => block.id);
  }

  async function copySelected(ids = selected) {
    if (!ids.length) return;
    await run(async () => {
      await flushBlockSaves(ids);
      const result = await api('/portable-text', {
        method: 'POST', body: { block_ids: ids }, secret
      });
      await copyText(result.text);
    }, `已複製 ${ids.length} 個可安裝資產`);
  }

  async function copyRawSkill(block) {
    if (!block || block.block_type !== 'skill') return;
    await run(async () => {
      await copyText(block.content);
    }, '已複製 RAW Skill 內容');
  }

  async function dropBefore(targetId) {
    if (!dragId || dragId === targetId) return;
    const ids = blocks.map((block) => block.id);
    const from = ids.indexOf(dragId);
    const to = ids.indexOf(targetId);
    ids.splice(to, 0, ids.splice(from, 1)[0]);
    dragId = '';
    await run(async () => {
      await api('/blocks/reorder', { method: 'POST', body: { block_ids: ids }, secret });
      await load();
    }, '順序已更新');
  }

  function beginDrag(event, id) {
    dragId = id;
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', id);
  }

  async function createBundle() {
    if (!selected.length) return;
    await run(async () => {
      const bundle = await api('/bundles?source=web', {
        method: 'POST', body: { name: bundleName, block_ids: selected }, secret
      });
      snapshot = { ...snapshot, bundles: [...bundles, bundle] };
      bundleName = '';
    }, 'Bundle 已建立');
  }

  async function updateBundle(bundle) {
    await run(async () => {
      const updated = await api(`/bundles/${bundle.id}?source=web`, {
        method: 'PATCH',
        body: { name: bundle.name, block_ids: selected, version: bundle.version },
        secret
      });
      snapshot = { ...snapshot, bundles: bundles.map((item) => item.id === updated.id ? updated : item) };
    }, 'Bundle 已套用目前勾選');
  }

  async function removeBundle(bundle) {
    if (!confirm(`刪除 Bundle「${bundle.name}」？`)) return;
    await run(async () => {
      await api(`/bundles/${bundle.id}?source=web`, { method: 'DELETE', body: { version: bundle.version }, secret });
      snapshot = { ...snapshot, bundles: bundles.filter((item) => item.id !== bundle.id) };
    }, 'Bundle 已刪除');
  }

  async function loadRevisions() {
    revisions = await api('/revisions?limit=100', { secret });
  }

  async function changeTab(tab) {
    activeTab = tab;
    if (tab === 'history') await run(loadRevisions, '版本歷史已更新');
  }

  async function restoreRevision(revision) {
    if (!confirm(`還原 revision #${revision.id} 的變更前狀態？`)) return;
    await run(async () => {
      await api(`/revisions/${revision.id}/restore`, { method: 'POST', body: {}, secret });
      await load();
      await loadRevisions();
    }, '版本已還原');
  }

  async function saveTarget() {
    await run(async () => {
      let headers;
      try { headers = JSON.parse(target.headers || '{}'); } catch { throw new Error('Headers 必須是 JSON object'); }
      await api('/notification-target', {
        method: 'PUT', body: { kind: target.kind, url: target.url, headers }, secret
      });
      await load();
      target = { ...target, url: '' };
    }, '通知目標已加密儲存');
  }

  async function testTarget() {
    await run(() => api('/notification-target/test', { method: 'POST', body: {}, secret }), '測試通知已送出');
  }

  async function removeTarget() {
    if (!confirm('刪除目前的通知目標？')) return;
    await run(async () => {
      await api('/notification-target', { method: 'DELETE', secret });
      await load();
    }, '通知目標已刪除');
  }

  async function copyAiGuide() {
    if (!secret) return;
    const root = `${window.location.origin}/api/v1`;
    const text = `# CrossPrompt AI 操作說明

你可以透過 HTTP API 管理我的永久型別化資產 Vault。除 callback 外，所有請求都使用：
Authorization: Bearer ${secret}
Content-Type: application/json

Base URL: ${root}

讀取完整 Vault：
GET ${root}/vault

先讀取可用型別、預設模板與各型別的 Agent 使用方式。注意：讀到資產後要依型別完成安裝／加入設定，不要只把內容當普通 Prompt 回覆：
GET ${root}/artifact-types

Skill：把內容安裝到目前 Agent 平台的 Skill 目錄或 Skill 設定（通常建立或更新 SKILL.md），再依觸發條件載入。若平台不支援 Skill，先告知使用者，不能假裝已安裝。
MCP Server：把內容加入目前 Agent 平台的 MCP／MCP Server 設定，依 transport、command／URL、arguments 與環境變數配置，完成實際連線與 tools 列舉後才可使用。
Prompt Template：加入 Prompt Template／模板區，先填入所有必要變數再執行。
Agent Profile：加入 system／developer instructions 或角色設定後再開始任務。
Tool / API Contract：加入 tool／connector registry 或 API 設定後才依契約呼叫。
Schema：加入 structured output／response schema 設定並驗證輸出。

新增型別化資產（block_type 必須使用型別目錄中的 key；省略時相容為 prompt）：
POST ${root}/blocks?source=YOUR_AI_NAME
{"block_type":"skill","title":"Skill 標題","content":"Markdown 內容"}

修改資產（version 必須使用讀取到的目前值，否則會回傳 409）：
PATCH ${root}/blocks/{block_id}?source=YOUR_AI_NAME
{"block_type":"skill","title":"新標題","content":"新內容","position":0,"version":1}

刪除 Block：
DELETE ${root}/blocks/{block_id}?source=YOUR_AI_NAME
{"version":1}

完整排序：
POST ${root}/blocks/reorder
{"block_ids":["id-1","id-2"]}

產生可直接交給 Agent、內含型別使用引導的文字包：
POST ${root}/portable-text
{"block_ids":["id-1","id-2"]}

Bundle endpoints：GET/POST ${root}/bundles；PATCH/DELETE ${root}/bundles/{bundle_id}
版本：GET ${root}/revisions；POST ${root}/revisions/{revision_id}/restore

任務完成時通知我：
POST ${root}/callback/${secret}
{"status":"completed","title":"任務完成","message":"分析已完成，可以回來查看結果。","source":"YOUR_AI_NAME","url":"https://example.com/result"}

status 只能是 completed、needs_input 或 failed。完整規格：${root}/openapi.json

安全提醒：把 Bearer secret 視為私人管理連結；不要在聊天、log 或公開內容中轉貼。`;
    await copyText(text);
    notice = 'AI 操作說明已複製';
  }

  async function rotateSecret() {
    if (!confirm('輪替後，舊網頁、API 與 callback 會立即失效。確定繼續？')) return;
    await run(async () => {
      const result = await api('/vault/rotate-secret', { method: 'POST', body: {}, secret });
      window.location.replace(`/#/v/${result.secret}`);
    }, 'Secret 已輪替');
  }

  async function deleteVault() {
    if (!confirm('刪除整個 Vault？七天內可用目前連結復原。')) return;
    await run(async () => {
      await api('/vault', { method: 'DELETE', secret });
      deleted = true;
      snapshot = null;
    }, 'Vault 已軟刪除');
  }

  async function restoreVault() {
    await run(async () => {
      await api('/vault/restore', { method: 'POST', body: {}, secret });
      await load();
    }, 'Vault 已復原');
  }

  async function requestBindCode() {
    await run(async () => {
      await api('/vault/email/request-code', {
        method: 'POST', body: { email: emailBinding.email }, secret
      });
      emailBinding = { ...emailBinding, code: '', step: 'verify' };
    }, 'Email 驗證碼已寄出');
  }

  async function verifyBindCode() {
    const previousEmail = snapshot.vault.email;
    await run(async () => {
      await api('/vault/email/verify', {
        method: 'POST', body: { email: emailBinding.email, code: emailBinding.code }, secret
      });
      if (emailSession && previousEmail && previousEmail !== emailBinding.email.trim().toLowerCase()) {
        window.location.replace('/');
        return;
      }
      emailBinding = { ...emailBinding, code: '', step: 'request' };
      await load();
    }, 'Email 已驗證並綁定');
  }

  async function unbindEmail() {
    if (!secret || !confirm('解除 Email 後，所有 Email 登入工作階段會立即失效。確定繼續？')) return;
    await run(async () => {
      await api('/vault/email', { method: 'DELETE', secret });
      emailBinding = { email: '', code: '', step: 'request' };
      await load();
    }, 'Email 綁定已解除');
  }

  async function logoutEmail() {
    await run(async () => {
      await api('/email/session', { method: 'DELETE' });
      window.location.replace('/');
    }, '已登出');
  }
</script>

<svelte:head><title>{snapshot?.vault?.name || 'Vault'} — CrossPrompt</title></svelte:head>

<header class="site-header workspace-header">
  <a class="brand" href="/"><span class="brand-mark">C</span>CrossPrompt</a>
  <div class="header-actions">
    <LanguageSwitcher />
    {#if secret}<button class="quiet" on:click={copyAiGuide}>{t('copyGuide')}</button>{/if}
    {#if snapshot}<button class="quiet" on:click={() => downloadJson(`crossprompt-${snapshot.vault.id}.json`, snapshot)}>{t('exportJson')}</button>{/if}
    {#if emailSession}<button class="quiet" on:click={logoutEmail}>{t('logoutEmail')}</button>{/if}
  </div>
</header>

{#if error}<div class="floating-message error-banner" role="alert">{error}</div>{/if}
{#if notice}<div class="floating-message success-banner" role="status">{notice}</div>{/if}

{#if deleted}
  <main class="state-page shell">
    <p class="eyebrow">SOFT DELETED</p><h1>{t('softDeleted')}</h1>
    <p>{t('restoreVault')} 7 天內可用原連結復原；管理員刪除的 Vault 無法由使用者復原。</p>
    <div class="button-row">{#if secret}<button class="primary" on:click={restoreVault} disabled={busy}>{t('restoreVault')}</button>{/if}<a class="button quiet" href="/">{t('backHome')}</a></div>
  </main>
{:else if locked}
  <main class="state-page shell">
    <p class="eyebrow">423 LOCKED</p><h1>{t('lockedTitle')}</h1><p>{t('lockedText')}</p>
  </main>
{:else if !snapshot}
  <main class="state-page shell"><span class="spinner"></span><p>{t('openingVault')}</p></main>
{:else}
  <main class="workspace shell-wide" data-locale={activeLocale}>
    <aside class="workspace-sidebar">
      <div class="vault-identity">
        <label for="vault-name">Vault</label>
        <div class="inline-edit"><input id="vault-name" bind:value={snapshot.vault.name} maxlength="100" /><button on:click={renameVault} disabled={busy}>{t('save')}</button></div>
        <p>{blocks.length}/1000 Assets · {bundles.length}/200 Bundles</p>
      </div>
      <nav class="tabs" aria-label={t('vaultFeatures')}>
        <button class:active={activeTab === 'blocks'} on:click={() => changeTab('blocks')}><span>01</span>{t('assets')}</button>
        <button class:active={activeTab === 'bundles'} on:click={() => changeTab('bundles')}><span>02</span>{t('bundles')}</button>
        <button class:active={activeTab === 'history'} on:click={() => changeTab('history')}><span>03</span>{t('history')}</button>
        <button class:active={activeTab === 'notify'} on:click={() => changeTab('notify')}><span>04</span>Notify</button>
        <button class:active={activeTab === 'settings'} on:click={() => changeTab('settings')}><span>05</span>{t('settings')}</button>
      </nav>
      <div class="selection-box">
        <span>{t('selected', { count: selected.length })}</span>
        <div class="selection-actions">
          <button class="primary compact" disabled={!selected.length || busy} on:click={() => copySelected()}>{t('copyInstall')}</button>
          {#if selectedRawSkill}<button class="quiet compact" disabled={busy} on:click={() => copyRawSkill(selectedRawSkill)}>{t('rawSkill')}</button>{/if}
        </div>
      </div>
    </aside>

    <section class="workspace-main">
      {#if activeTab === 'blocks'}
        <div class="page-heading"><div><p class="eyebrow">TYPED PORTABLE ASSETS</p><h1>{t('typedAssetsHeading')}</h1></div><p>{t('typedAssetsText')}</p></div>
        <section class="type-catalog" aria-label={t('assetTypes')}>
          {#each artifactTypes as type}
            <button type="button" class="type-card" class:active={newBlock?.block_type === type.key} on:click={() => startTypedBlock(type)}>
              <span>{type.short_label}</span>
              <strong>{type.label}</strong>
              <small>{type.description}</small>
            </button>
          {/each}
        </section>

        {#if newBlock && selectedType}
          <form class="new-block typed-compose" on:submit|preventDefault={createBlock}>
            <div class="compose-heading">
              <div><span class="type-pill">{selectedType.short_label}</span><strong>{selectedType.label}</strong></div>
              <button type="button" class="quiet compact" on:click={() => newBlock = null}>{t('cancel')}</button>
            </div>
            <p class="agent-guidance"><strong>{t('copyInstruction')}</strong>{selectedType.agent_instructions}</p>
            <input bind:value={newBlock.title} placeholder={t('assetTitle')} maxlength="100" required />
            <textarea bind:value={newBlock.content} on:keydown={(event) => handleMarkdownTab(event, newBlock)} placeholder={t('markdownPlaceholder')} maxlength="65536" required></textarea>
            <div class="form-footer"><span>{newBlock.content.length.toLocaleString()} / 65,536 bytes</span><button class="primary" disabled={busy}>{t('create')} {selectedType.short_label}</button></div>
          </form>
        {/if}

        <div class="block-list">
          {#each blocks as block (block.id)}
            <article class="block-editor" on:dragover|preventDefault on:drop={() => dropBefore(block.id)}>
              <div class="block-toolbar">
                <span class="drag-handle" role="button" tabindex="0" aria-label={t('dragSort')} draggable="true" on:dragstart={(event) => beginDrag(event, block.id)} title={t('dragSort')}>⠿</span>
                <label class="check"><input type="checkbox" checked={selected.includes(block.id)} on:change={() => toggleSelected(block.id)} /><span></span></label>
                <select class="type-select" bind:value={block.block_type} on:change={() => scheduleBlockSave(block)} aria-label={`${block.title} 型別`}>
                  {#each artifactTypes as type}<option value={type.key}>{type.short_label}</option>{/each}
                </select>
                <input class="block-title" bind:value={block.title} on:input={() => scheduleBlockSave(block)} maxlength="100" aria-label={t('blockTitle')} />
                <span class="version">v{block.version}</span>
                {#if saveStateLabel(block)}<span class="autosave-status {blockSaveStates[block.id]}">{saveStateLabel(block)}</span>{/if}
                <button type="button" class="quiet compact" on:click|stopPropagation={() => copySelected([block.id])}>{t('copyInstall')}</button>
                {#if block.block_type === 'skill'}<button type="button" class="quiet compact raw-copy" on:click|stopPropagation={() => copyRawSkill(block)}>{t('rawSkill')}</button>{/if}
                <button type="button" class="danger-link compact" on:click|stopPropagation={() => removeBlock(block)}>{t('delete')}</button>
              </div>
              {#if typeFor(block.block_type)}
                <details class="type-guidance"><summary>{typeFor(block.block_type).label} · Agent 如何使用</summary><p>{typeFor(block.block_type).agent_instructions}</p></details>
              {/if}
              <div class="editor-grid">
                <textarea bind:value={block.content} on:input={() => scheduleBlockSave(block)} on:keydown={(event) => handleMarkdownTab(event, block)} maxlength="65536" aria-label={`${block.title} Markdown`}></textarea>
                <div class="preview"><span class="preview-label">{t('safePreview')}</span><Markdown content={block.content} /></div>
              </div>
            </article>
          {:else}
            <div class="empty-state"><h3>{t('emptyAssets')}</h3><p>{t('emptyAssetsText')}</p></div>
          {/each}
        </div>
      {:else if activeTab === 'bundles'}
        <div class="page-heading"><div><p class="eyebrow">{t('savedCombinations')}</p><h1>{t('bundles')}</h1></div><p>{t('bundlesText')}</p></div>
        <form class="bundle-create" on:submit|preventDefault={createBundle}>
          <input bind:value={bundleName} placeholder={t('bundleName')} maxlength="100" required />
          <span>{selected.length} {t('selectedAssets')}</span>
          <button class="primary" disabled={!selected.length || busy}>{t('saveBundle')}</button>
        </form>
        <div class="bundle-grid">
          {#each bundles as bundle (bundle.id)}
            <article class="bundle-card">
              <span class="eyebrow">BUNDLE · v{bundle.version}</span>
              <input bind:value={bundle.name} maxlength="100" aria-label="Bundle 名稱" />
              <p>{bundle.block_ids.length} {t('assetCount')}</p>
              <ol>{#each bundle.block_ids as id}<li><span class="inline-type">{typeFor(blocks.find((block) => block.id === id)?.block_type)?.short_label || 'Unknown'}</span>{blocks.find((block) => block.id === id)?.title || '已移除的資產'}</li>{/each}</ol>
              <div class="button-row">
                <button class="primary compact" on:click={() => copySelected(bundle.block_ids)}>{t('copyInstall')}</button>
                <button class="quiet compact" on:click={() => selected = [...bundle.block_ids]}>{t('loadSelection')}</button>
                <button class="quiet compact" on:click={() => updateBundle(bundle)} disabled={!selected.length}>{t('updateSelection')}</button>
                <button class="danger-link compact" on:click={() => removeBundle(bundle)}>{t('delete')}</button>
              </div>
            </article>
          {:else}<div class="empty-state"><h3>{t('emptyBundles')}</h3><p>{t('emptyBundlesText')}</p></div>{/each}
        </div>
      {:else if activeTab === 'history'}
        <div class="page-heading"><div><p class="eyebrow">{t('lastChanges')}</p><h1>{t('history')}</h1></div><button class="quiet" on:click={loadRevisions}>{t('refresh')}</button></div>
        <div class="history-list">
          {#each revisions as revision}
            <article><div><span class="status-pill">{revision.action}</span><strong>{revision.resource_type}</strong><code>{revision.resource_id || 'vault'}</code></div><div><span>{revision.source} · {new Date(revision.created_at).toLocaleString()}</span>{#if revision.resource_type !== 'vault'}<button class="quiet compact" on:click={() => restoreRevision(revision)}>還原變更前</button>{/if}</div></article>
          {:else}<div class="empty-state"><h3>{t('noHistory')}</h3></div>{/each}
        </div>
      {:else if activeTab === 'notify'}
        <div class="page-heading"><div><p class="eyebrow">{t('taskCompletion')}</p><h1>{t('notifyTitle')}</h1></div><p>{t('notifyText')}</p></div>
        {#if snapshot.notification_target}<div class="current-target"><span>{t('currentTarget')}</span><strong>{snapshot.notification_target.kind}</strong><code>{snapshot.notification_target.masked_url}</code><button class="quiet compact" on:click={testTarget}>{t('sendTest')}</button><button class="danger-link compact" on:click={removeTarget}>{t('delete')}</button></div>{/if}
        <form class="settings-form" on:submit|preventDefault={saveTarget}>
          <label>{t('serviceType')}<select bind:value={target.kind}><option value="ntfy">ntfy</option><option value="pushcut">Pushcut</option><option value="generic_json">Generic JSON webhook</option></select></label>
          <label>HTTPS URL<input type="url" bind:value={target.url} placeholder="https://…" required /></label>
          <label>{t('headers')}<textarea class="code-field" bind:value={target.headers} spellcheck="false"></textarea></label>
          <p class="fine-print">只允許 HTTPS 443；redirect、私人網路、loopback、link-local、metadata 與 reserved IP 都會被拒絕。URL 與 credential 會以 server master key 加密。</p>
          <button class="primary" disabled={busy}>{t('saveTarget')}</button>
        </form>
      {:else if activeTab === 'settings'}
        <div class="page-heading"><div><p class="eyebrow">{t('vaultControl')}</p><h1>{t('portability')}</h1></div></div>
        <div class="settings-grid">
          <article class="email-access-card">
            <span class="eyebrow">{t('emailAccess')}</span><h3>{t('bindEmail')}</h3>
            {#if snapshot.vault.email}<p>目前已綁定 <strong>{snapshot.vault.email}</strong>。可用一次性驗證碼登入這個 Vault。</p>{:else}<p>驗證信箱所有權後，即可用 Email 收取一次性登入碼。</p>{/if}
            {#if siteConfig.email_login_enabled}
              <label>Email<input type="email" bind:value={emailBinding.email} maxlength="254" required disabled={emailBinding.step === 'verify'} /></label>
              {#if emailBinding.step === 'verify'}
                <label>六位數驗證碼<input class="otp-input" inputmode="numeric" pattern="[0-9]{6}" maxlength="6" bind:value={emailBinding.code} autocomplete="one-time-code" /></label>
                <div class="button-row"><button class="primary" disabled={busy || emailBinding.code.length !== 6} on:click={verifyBindCode}>{t('verifyBind')}</button><button class="quiet" on:click={() => emailBinding = { ...emailBinding, step: 'request', code: '' }}>{t('reenter')}</button></div>
              {:else}
                <div class="button-row"><button class="primary" disabled={busy || !emailBinding.email} on:click={requestBindCode}>{snapshot.vault.email ? t('changeEmail') : t('sendBindCode')}</button>{#if snapshot.vault.email && secret}<button class="danger-link" on:click={unbindEmail}>{t('unbind')}</button>{/if}</div>
              {/if}
            {:else}<p class="fine-print">管理員尚未設定 SMTP，Email 綁定與登入目前不可用。</p>{/if}
          </article>
          {#if secret}<article><h3>{t('aiGuide')}</h3><p>包含 Base URL、Bearer secret、CRUD、版本與 callback 範例。請只貼給你信任的 AI 工作階段。</p><button class="primary" on:click={copyAiGuide}>{t('copyFullGuide')}</button></article>{/if}
          <article><h3>{t('fullExport')}</h3><p>下載目前 Vault snapshot，包含 Blocks、Bundles 與遮罩後的通知 metadata；不含原始通知 credential。</p><button class="quiet" on:click={() => downloadJson(`crossprompt-${snapshot.vault.id}.json`, snapshot)}>{t('downloadJson')}</button></article>
          <article class="warning-card"><h3>{t('rotate')}</h3><p>立即使舊管理連結、Bearer API 與 callback URL 全部失效。內容不受影響。</p><button class="danger" on:click={rotateSecret}>{t('rotate')}</button></article>
          {#if secret}<article class="warning-card"><h3>{t('deleteVault')}</h3><p>軟刪除後保留七天；這段期間可使用目前連結復原，之後永久清除。</p><button class="danger" on:click={deleteVault}>{t('deleteVault')}</button></article>{/if}
        </div>
      {/if}
    </section>
  </main>
{/if}
