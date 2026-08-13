<script>
  import { onMount } from 'svelte';
  import Landing from './components/Landing.svelte';
  import Vault from './components/Vault.svelte';
  import Admin from './components/Admin.svelte';

  let route = { admin: false, secret: '', emailSession: false };

  function readRoute() {
    const match = window.location.hash.match(/^#\/v\/([^/]+)$/);
    route = {
      admin: window.location.pathname === '/admin' || window.location.pathname.startsWith('/admin/'),
      secret: match ? decodeURIComponent(match[1]) : '',
      emailSession: window.location.hash === '#/email-vault'
    };
  }

  onMount(() => {
    readRoute();
    window.addEventListener('hashchange', readRoute);
    return () => window.removeEventListener('hashchange', readRoute);
  });
</script>

{#if route.admin}
  <Admin />
{:else if route.secret}
  <Vault secret={route.secret} />
{:else if route.emailSession}
  <Vault secret="" emailSession={true} />
{:else}
  <Landing />
{/if}
