<script>
  import DOMPurify from 'dompurify';
  import MarkdownIt from 'markdown-it';

  export let content = '';
  const parser = new MarkdownIt({ html: false, linkify: true, breaks: true });
  $: rendered = DOMPurify.sanitize(parser.render(content || ''), {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['style', 'script', 'iframe', 'object', 'embed', 'form', 'input', 'button'],
    FORBID_ATTR: ['style']
  });
</script>

<div class="markdown">{@html rendered}</div>
