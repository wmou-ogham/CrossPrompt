<script>
  import DOMPurify from 'dompurify';
  import MarkdownIt from 'markdown-it';
  import { splitFrontmatter } from '../lib/frontmatter.js';
  import { locale, t } from '../lib/i18n.js';

  export let content = '';
  const parser = new MarkdownIt({ html: false, linkify: true, breaks: true });

  $: parsed = splitFrontmatter(content || '');
  $: fields = parsed.fields || [];
  $: values = Object.fromEntries(fields.map((field) => [field.key, field.value]));
  $: extras = fields.filter((field) => field.key !== 'name' && field.key !== 'description');
  $: hasHeader = parsed.raw !== null && (fields.length > 0 || Boolean(parsed.raw.trim()));
  $: rendered = DOMPurify.sanitize(parser.render(parsed.body || ''), {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['style', 'script', 'iframe', 'object', 'embed', 'form', 'input', 'button'],
    FORBID_ATTR: ['style']
  });
</script>

<div class="markdown-preview">
  {#if hasHeader}
    <aside class="skill-meta" aria-label={t('skillHeader')} data-locale={$locale}>
      <span class="skill-meta-label">{t('skillHeader')}</span>
      {#if values.name}
        <p class="skill-meta-name">
          <span class="skill-meta-key">name</span>
          <code>{values.name}</code>
        </p>
      {/if}
      {#if values.description}
        <p class="skill-meta-description">
          <span class="skill-meta-key">description</span>
          {values.description}
        </p>
      {/if}
      {#if extras.length}
        <dl class="skill-meta-fields">
          {#each extras as field, index (`${index}-${field.key}`)}
            <div>
              <dt>{field.key}</dt>
              <dd>{field.value}</dd>
            </div>
          {/each}
        </dl>
      {/if}
      {#if !fields.length && parsed.raw}
        <pre class="skill-meta-raw">{parsed.raw.trim()}</pre>
      {/if}
    </aside>
  {/if}
  {#if parsed.body.trim()}
    <div class="markdown">{@html rendered}</div>
  {/if}
</div>
