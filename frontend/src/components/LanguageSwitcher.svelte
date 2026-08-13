<script>
  import { locale, localeOptions, setLocale } from '../lib/i18n.js';
</script>

<label class="language-switcher">
  <span class="language-icon" aria-hidden="true">🌐</span>
  <span class="language-label">Language / 語言</span>
  <select aria-label="Language / 語言" value={$locale} on:change={(event) => setLocale(event.currentTarget.value)}>
    {#each localeOptions as option}
      <option value={option.key}>{option.label}</option>
    {/each}
  </select>
</label>
