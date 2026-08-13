# CrossPrompt

Langues : [繁體中文](README.md) · [English](README.en.md) · [Español](README.es.md) · [Français](README.fr.md)

CrossPrompt est une page privée d’assets IA sans comptes utilisateurs généraux. Un lien secret à haute entropie donne accès à un Vault permanent contenant des assets Markdown typés, des Bundles et des Portable Agent Packs. Une IA peut maintenir le Vault via l’API HTTP et appeler un callback de fin qui transmet une notification à Pushcut, ntfy ou à un webhook JSON générique.

Le Vault accepte le lien de gestion et un code à usage unique de six chiffres envoyé à un e-mail vérifié. La liaison d’un e-mail est facultative et ne remplace ni ne révèle le secret du Vault.

## Assets portables typés

Chaque type possède un modèle Markdown modifiable. Les types inclus sont `prompt`, `prompt_template`, `skill`, `mcp_server`, `agent_profile`, `workflow`, `context_pack`, `preferences`, `tool_api`, `schema`, `evaluation_rubric` et `safety_policy`. Lors de la copie, chaque asset explique à l’Agent s’il doit installer un Skill, ajouter un MCP, appliquer un template ou valider un Schema. Les éléments MCP et API décrivent une configuration et ne simulent pas une connexion.

`POST /api/v1/portable-text` produit un pack texte ordonné. Son en-tête indique à l’Agent de ne pas tout exécuter immédiatement ; chaque asset conserve son type, son mode d’emploi et son contenu brut.

## Architecture et démarrage

- Backend Rust, Axum, Tokio, SQLx et SQLite.
- Frontend Svelte/Vite servi par le même binaire Rust.
- Un conteneur applicatif ; les données persistantes sont dans `/data`.
- TLS fourni par Caddy, Nginx ou un autre reverse proxy.

```sh
cp .env.example .env
openssl rand -hex 32       # CROSSPROMPT_SESSION_SECRET
openssl rand -base64 32    # CROSSPROMPT_MASTER_KEY, 32 octets
openssl rand -hex 24       # CROSSPROMPT_IP_HASH_SALT
docker build --target password-tool -t crossprompt-password-tool .
printf '%s' 'mot-de-passe-long' | docker run --rm -i crossprompt-password-tool
docker compose up -d --build
curl --fail http://127.0.0.1:8080/readyz
```

En production, les identifiants administrateur, les secrets de session et de chiffrement, le salt IP, HTTPS, les cookies Secure et les clés Turnstile sont obligatoires. Utilisez `development` ou `staging` en local.

## Email OTP

Configurez SMTP STARTTLS avec `CROSSPROMPT_SMTP_HOST`, `CROSSPROMPT_SMTP_PORT`, `CROSSPROMPT_SMTP_USERNAME`, `CROSSPROMPT_SMTP_PASSWORD` et `CROSSPROMPT_SMTP_FROM`. Les codes comportent six chiffres, sont valables dix minutes, autorisent cinq essais et sont conservés uniquement sous forme de digest. La session navigateur dure 30 jours.

## Données, limites et sécurité

- Jusqu’à 100 Vaults par IP et par jour.
- Jusqu’à 1 000 Blocks et 200 Bundles par Vault ; 1 MiB au total et 64 KiB par Block.
- Un Vault contenant du contenu, des Bundles, des notifications ou une utilisation réelle n’expire pas par inactivité.
- Un Vault vide et jamais utilisé est supprimé après 30 jours ; un soft delete après sept jours.
- Les 100 dernières révisions sont conservées.

CrossPrompt **n’est pas chiffré de bout en bout**. L’administrateur peut consulter le contenu pour l’exploitation et la lutte contre les abus. Ne stockez pas de mots de passe, clés API, clés privées, phrases de récupération ou autres secrets.

## API et administration

La documentation OpenAPI se trouve à `/api/v1/openapi.json`. L’API utilise `Authorization: Bearer {vault-secret}` ; les mises à jour et suppressions de Blocks/Bundles exigent la `version` courante et renvoient `409 Conflict` en cas de conflit. L’API normale est limitée à 120 requêtes/minute ; les callbacks à 10/minute et 100/jour.

`/admin` fournit une identité administrateur, des sessions de 12 heures, une protection CSRF, une limitation des tentatives et un journal d’audit immuable. Il permet de consulter capacité, contenu et cibles masquées, puis de suspendre, reprendre, supprimer temporairement, restaurer pendant sept jours ou supprimer définitivement après confirmation de l’ID complet. Les secrets ne peuvent jamais être récupérés et le contenu utilisateur n’est pas modifiable.

Voir [progress.md](progress.md) pour la checklist et [sudo.log](sudo.log) pour le journal sudo.
