<script>
  import { locale, localeOptions, setLocale, t } from '../lib/i18n.js';
</script>

<label class="language-switcher">
  <span class="sr-only">{t('language')}</span>
  <select aria-label={t('language')} value={$locale} on:change={(event) => setLocale(event.currentTarget.value)}>
    {#each localeOptions as option}
      <option value={option.key}>{option.label}</option>
    {/each}
  </select>
</label>

