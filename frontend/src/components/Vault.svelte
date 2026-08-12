<script>
  import { onMount } from 'svelte';
  import Markdown from './Markdown.svelte';
  import { ApiError, api, copyText, downloadJson } from '../lib/api.js';

  export let secret;

  let snapshot = null;
  let artifactTypes = [];
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

  $: blocks = snapshot?.blocks || [];
  $: bundles = snapshot?.bundles || [];
  $: selectedType = newBlock ? typeFor(newBlock.block_type) : null;

  onMount(load);

  async function load() {
    error = '';
    try {
      const [nextSnapshot, types] = await Promise.all([
        api('/vault', { secret }),
        api('/artifact-types')
      ]);
      snapshot = nextSnapshot;
      artifactTypes = types;
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

  async function saveBlock(block) {
    await run(async () => {
      const updated = await api(`/blocks/${block.id}?source=web`, {
        method: 'PATCH',
        body: { block_type: block.block_type, title: block.title, content: block.content, position: block.position, version: block.version },
        secret
      });
      snapshot = { ...snapshot, blocks: blocks.map((item) => item.id === updated.id ? updated : item) };
    }, 'Block 已儲存');
  }

  async function removeBlock(block) {
    if (!confirm(`刪除「${block.title}」？可在版本歷史中還原。`)) return;
    await run(async () => {
      await api(`/blocks/${block.id}?source=web`, { method: 'DELETE', body: { version: block.version }, secret });
      selected = selected.filter((id) => id !== block.id);
      await load();
    }, 'Block 已刪除');
  }

  function toggleSelected(id) {
    const ids = new Set(selected);
    if (ids.has(id)) ids.delete(id); else ids.add(id);
    selected = blocks.filter((block) => ids.has(block.id)).map((block) => block.id);
  }

  async function copySelected(ids = selected) {
    if (!ids.length) return;
    await run(async () => {
      const result = await api('/portable-text', {
        method: 'POST', body: { block_ids: ids }, secret
      });
      await copyText(result.text);
    }, `已複製 ${ids.length} 個 Agent-ready 資產`);
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
    const root = `${window.location.origin}/api/v1`;
    const text = `# CrossPrompt AI 操作說明

你可以透過 HTTP API 管理我的永久型別化資產 Vault。除 callback 外，所有請求都使用：
Authorization: Bearer ${secret}
Content-Type: application/json

Base URL: ${root}

讀取完整 Vault：
GET ${root}/vault

先讀取可用型別、預設模板與各型別的 Agent 使用方式：
GET ${root}/artifact-types

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
</script>

<svelte:head><title>{snapshot?.vault?.name || 'Vault'} — CrossPrompt</title></svelte:head>

<header class="site-header workspace-header">
  <a class="brand" href="/"><span class="brand-mark">C</span>CrossPrompt</a>
  <div class="header-actions">
    <button class="quiet" on:click={copyAiGuide}>複製 AI 使用說明</button>
    {#if snapshot}<button class="quiet" on:click={() => downloadJson(`crossprompt-${snapshot.vault.id}.json`, snapshot)}>匯出 JSON</button>{/if}
  </div>
</header>

{#if error}<div class="floating-message error-banner" role="alert">{error}</div>{/if}
{#if notice}<div class="floating-message success-banner" role="status">{notice}</div>{/if}

{#if deleted}
  <main class="state-page shell">
    <p class="eyebrow">SOFT DELETED</p><h1>這個 Vault 已刪除。</h1>
    <p>若是你自行刪除，可在七天內使用原連結復原。管理員刪除的 Vault 無法由使用者復原。</p>
    <div class="button-row"><button class="primary" on:click={restoreVault} disabled={busy}>復原 Vault</button><a class="button quiet" href="/">回首頁</a></div>
  </main>
{:else if locked}
  <main class="state-page shell">
    <p class="eyebrow">423 LOCKED</p><h1>Vault 已被管理員停用。</h1><p>內容仍完整保留，但網頁、API 與 callback 暫時無法使用。</p>
  </main>
{:else if !snapshot}
  <main class="state-page shell"><span class="spinner"></span><p>正在開啟 Vault…</p></main>
{:else}
  <main class="workspace shell-wide">
    <aside class="workspace-sidebar">
      <div class="vault-identity">
        <label for="vault-name">Vault</label>
        <div class="inline-edit"><input id="vault-name" bind:value={snapshot.vault.name} maxlength="100" /><button on:click={renameVault} disabled={busy}>儲存</button></div>
        <p>{blocks.length}/100 Assets · {bundles.length}/20 Bundles</p>
      </div>
      <nav class="tabs" aria-label="Vault 功能">
        <button class:active={activeTab === 'blocks'} on:click={() => changeTab('blocks')}><span>01</span>Assets</button>
        <button class:active={activeTab === 'bundles'} on:click={() => changeTab('bundles')}><span>02</span>Bundles</button>
        <button class:active={activeTab === 'history'} on:click={() => changeTab('history')}><span>03</span>版本歷史</button>
        <button class:active={activeTab === 'notify'} on:click={() => changeTab('notify')}><span>04</span>Notify</button>
        <button class:active={activeTab === 'settings'} on:click={() => changeTab('settings')}><span>05</span>設定</button>
      </nav>
      <div class="selection-box">
        <span>已選 {selected.length} 個</span>
        <button class="primary compact" disabled={!selected.length || busy} on:click={() => copySelected()}>複製 Agent Pack</button>
      </div>
    </aside>

    <section class="workspace-main">
      {#if activeTab === 'blocks'}
        <div class="page-heading"><div><p class="eyebrow">TYPED PORTABLE ASSETS</p><h1>選一種資產開始</h1></div><p>每種型別都有預設模板與 Agent 使用規則；複製時會自動包成可直接理解的 Portable Agent Pack。</p></div>
        <section class="type-catalog" aria-label="資產型別">
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
              <button type="button" class="quiet compact" on:click={() => newBlock = null}>取消</button>
            </div>
            <p class="agent-guidance"><strong>複製給 Agent 時：</strong>{selectedType.agent_instructions}</p>
            <input bind:value={newBlock.title} placeholder="資產標題" maxlength="100" required />
            <textarea bind:value={newBlock.content} placeholder="輸入 Markdown 內容…" maxlength="65536" required></textarea>
            <div class="form-footer"><span>{newBlock.content.length.toLocaleString()} / 65,536 bytes</span><button class="primary" disabled={busy}>建立 {selectedType.short_label}</button></div>
          </form>
        {/if}

        <div class="block-list">
          {#each blocks as block (block.id)}
            <article class="block-editor" draggable="true" on:dragstart={() => dragId = block.id} on:dragover|preventDefault on:drop={() => dropBefore(block.id)}>
              <div class="block-toolbar">
                <span class="drag-handle" title="拖曳排序">⠿</span>
                <label class="check"><input type="checkbox" checked={selected.includes(block.id)} on:change={() => toggleSelected(block.id)} /><span></span></label>
                <select class="type-select" bind:value={block.block_type} aria-label={`${block.title} 型別`}>
                  {#each artifactTypes as type}<option value={type.key}>{type.short_label}</option>{/each}
                </select>
                <input class="block-title" bind:value={block.title} maxlength="100" aria-label="Block 標題" />
                <span class="version">v{block.version}</span>
                <button class="quiet compact" on:click={() => copySelected([block.id])}>複製</button>
                <button class="quiet compact" on:click={() => saveBlock(block)} disabled={busy}>儲存</button>
                <button class="danger-link compact" on:click={() => removeBlock(block)}>刪除</button>
              </div>
              {#if typeFor(block.block_type)}
                <details class="type-guidance"><summary>{typeFor(block.block_type).label} · Agent 如何使用</summary><p>{typeFor(block.block_type).agent_instructions}</p></details>
              {/if}
              <div class="editor-grid">
                <textarea bind:value={block.content} maxlength="65536" aria-label={`${block.title} Markdown`}></textarea>
                <div class="preview"><span class="preview-label">安全預覽</span><Markdown content={block.content} /></div>
              </div>
            </article>
          {:else}
            <div class="empty-state"><h3>還沒有可攜資產</h3><p>從上方選擇 Prompt、Skill、MCP、Schema 或其他型別，系統會先替你建立骨架。</p></div>
          {/each}
        </div>
      {:else if activeTab === 'bundles'}
        <div class="page-heading"><div><p class="eyebrow">SAVED COMBINATIONS</p><h1>Bundles</h1></div><p>Bundle 保存有順序的資產組合；複製時會帶上總體引導與每個型別的使用方式。</p></div>
        <form class="bundle-create" on:submit|preventDefault={createBundle}>
          <input bind:value={bundleName} placeholder="Bundle 名稱" maxlength="100" required />
          <span>{selected.length} 個已勾選資產</span>
          <button class="primary" disabled={!selected.length || busy}>儲存 Bundle</button>
        </form>
        <div class="bundle-grid">
          {#each bundles as bundle (bundle.id)}
            <article class="bundle-card">
              <span class="eyebrow">BUNDLE · v{bundle.version}</span>
              <input bind:value={bundle.name} maxlength="100" aria-label="Bundle 名稱" />
              <p>{bundle.block_ids.length} 個資產</p>
              <ol>{#each bundle.block_ids as id}<li><span class="inline-type">{typeFor(blocks.find((block) => block.id === id)?.block_type)?.short_label || 'Unknown'}</span>{blocks.find((block) => block.id === id)?.title || '已移除的資產'}</li>{/each}</ol>
              <div class="button-row">
                <button class="primary compact" on:click={() => copySelected(bundle.block_ids)}>複製</button>
                <button class="quiet compact" on:click={() => selected = [...bundle.block_ids]}>載入勾選</button>
                <button class="quiet compact" on:click={() => updateBundle(bundle)} disabled={!selected.length}>以目前勾選更新</button>
                <button class="danger-link compact" on:click={() => removeBundle(bundle)}>刪除</button>
              </div>
            </article>
          {:else}<div class="empty-state"><h3>還沒有 Bundle</h3><p>回到 Blocks 勾選內容，再把組合儲存起來。</p></div>{/each}
        </div>
      {:else if activeTab === 'history'}
        <div class="page-heading"><div><p class="eyebrow">LAST 100 CHANGES</p><h1>版本歷史</h1></div><button class="quiet" on:click={loadRevisions}>重新整理</button></div>
        <div class="history-list">
          {#each revisions as revision}
            <article><div><span class="status-pill">{revision.action}</span><strong>{revision.resource_type}</strong><code>{revision.resource_id || 'vault'}</code></div><div><span>{revision.source} · {new Date(revision.created_at).toLocaleString()}</span>{#if revision.resource_type !== 'vault'}<button class="quiet compact" on:click={() => restoreRevision(revision)}>還原變更前</button>{/if}</div></article>
          {:else}<div class="empty-state"><h3>尚無版本紀錄</h3></div>{/each}
        </div>
      {:else if activeTab === 'notify'}
        <div class="page-heading"><div><p class="eyebrow">TASK COMPLETION</p><h1>Notify callback</h1></div><p>AI 完成任務後呼叫 callback，由伺服器轉送到你的裝置。</p></div>
        {#if snapshot.notification_target}<div class="current-target"><span>目前目標</span><strong>{snapshot.notification_target.kind}</strong><code>{snapshot.notification_target.masked_url}</code><button class="quiet compact" on:click={testTarget}>送出測試</button><button class="danger-link compact" on:click={removeTarget}>刪除</button></div>{/if}
        <form class="settings-form" on:submit|preventDefault={saveTarget}>
          <label>服務類型<select bind:value={target.kind}><option value="ntfy">ntfy</option><option value="pushcut">Pushcut</option><option value="generic_json">Generic JSON webhook</option></select></label>
          <label>HTTPS URL<input type="url" bind:value={target.url} placeholder="https://…" required /></label>
          <label>額外 Headers（JSON object）<textarea class="code-field" bind:value={target.headers} spellcheck="false"></textarea></label>
          <p class="fine-print">只允許 HTTPS 443；redirect、私人網路、loopback、link-local、metadata 與 reserved IP 都會被拒絕。URL 與 credential 會以 server master key 加密。</p>
          <button class="primary" disabled={busy}>儲存通知目標</button>
        </form>
      {:else if activeTab === 'settings'}
        <div class="page-heading"><div><p class="eyebrow">VAULT CONTROL</p><h1>設定與可攜性</h1></div></div>
        <div class="settings-grid">
          <article><h3>AI 操作說明</h3><p>包含 Base URL、Bearer secret、CRUD、版本與 callback 範例。請只貼給你信任的 AI 工作階段。</p><button class="primary" on:click={copyAiGuide}>複製完整說明</button></article>
          <article><h3>完整匯出</h3><p>下載目前 Vault snapshot，包含 Blocks、Bundles 與遮罩後的通知 metadata；不含原始通知 credential。</p><button class="quiet" on:click={() => downloadJson(`crossprompt-${snapshot.vault.id}.json`, snapshot)}>下載 JSON</button></article>
          <article class="warning-card"><h3>輪替 Secret</h3><p>立即使舊管理連結、Bearer API 與 callback URL 全部失效。內容不受影響。</p><button class="danger" on:click={rotateSecret}>輪替 Secret</button></article>
          <article class="warning-card"><h3>刪除 Vault</h3><p>軟刪除後保留七天；這段期間可使用目前連結復原，之後永久清除。</p><button class="danger" on:click={deleteVault}>刪除 Vault</button></article>
        </div>
      {/if}
    </section>
  </main>
{/if}
