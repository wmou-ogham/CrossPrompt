# CrossPrompt

Idiomas: [繁體中文](README.md) · [English](README.en.md) · [Español](README.es.md) · [Français](README.fr.md)

CrossPrompt es una página privada de activos de IA sin cuentas de usuario generales. Un enlace secreto de alta entropía proporciona un Vault permanente para activos Markdown tipados, Bundles y Portable Agent Packs. La IA puede mantener el Vault mediante la API HTTP y llamar a un callback de finalización que reenvía avisos a Pushcut, ntfy o un webhook JSON genérico.

El Vault admite el enlace de administración y un código de un solo uso de seis dígitos enviado a un email verificado. Vincular un email es opcional y nunca sustituye ni revela el secreto del Vault.

## Activos portátiles tipados

Cada tipo carga una plantilla Markdown editable. Se incluyen `prompt`, `prompt_template`, `skill`, `mcp_server`, `agent_profile`, `workflow`, `context_pack`, `preferences`, `tool_api`, `schema`, `evaluation_rubric` y `safety_policy`. Cada activo se copia con instrucciones para que el Agent sepa si debe instalar un Skill, añadir un MCP, aplicar una plantilla o validar un Schema. Las entradas MCP y API describen configuración; no fingen una conexión activa.

`POST /api/v1/portable-text` genera un paquete de texto ordenado. Su cabecera indica al Agent que no debe ejecutar todos los elementos de inmediato y cada activo conserva su tipo, instrucciones y contenido original.

## Arquitectura y ejecución

- Backend Rust, Axum, Tokio, SQLx y SQLite.
- Frontend Svelte/Vite servido por el mismo binario Rust.
- Un contenedor de aplicación; los datos persistentes están en `/data`.
- TLS mediante Caddy, Nginx u otro proxy inverso.

```sh
cp .env.example .env
openssl rand -hex 32       # CROSSPROMPT_SESSION_SECRET
openssl rand -base64 32    # CROSSPROMPT_MASTER_KEY, 32 bytes
openssl rand -hex 24       # CROSSPROMPT_IP_HASH_SALT
docker build --target password-tool -t crossprompt-password-tool .
printf '%s' 'contraseña-larga' | docker run --rm -i crossprompt-password-tool
docker compose up -d --build
curl --fail http://127.0.0.1:8080/readyz
```

En producción son obligatorios las credenciales del administrador, las claves de sesión y cifrado, el salt de IP, HTTPS, cookies Secure y las claves Turnstile. Para desarrollo usa `development` o `staging`.

## Email OTP

Configura SMTP STARTTLS con `CROSSPROMPT_SMTP_HOST`, `CROSSPROMPT_SMTP_PORT`, `CROSSPROMPT_SMTP_USERNAME`, `CROSSPROMPT_SMTP_PASSWORD` y `CROSSPROMPT_SMTP_FROM`. Los códigos tienen seis dígitos, duran diez minutos, permiten cinco intentos y solo se guardan como digest. La sesión del navegador dura 30 días.

## Datos, límites y seguridad

- Hasta 100 Vaults por IP y día.
- Hasta 1.000 Blocks y 200 Bundles por Vault; 1 MiB total y 64 KiB por Block.
- Un Vault con contenido, Bundles, notificaciones o uso real no caduca por inactividad.
- Un Vault vacío y nunca usado se elimina tras 30 días; un soft delete se elimina tras siete días.
- Se conservan las 100 revisiones más recientes.

CrossPrompt **no cifra de extremo a extremo**. El administrador puede consultar el contenido para operar el servicio y gestionar abusos. No guardes contraseñas, claves API, claves privadas, frases de recuperación ni otros secretos.

## API y administración

La especificación OpenAPI está en `/api/v1/openapi.json`. La API usa `Authorization: Bearer {vault-secret}`; las modificaciones de Blocks y Bundles requieren `version` y devuelven `409 Conflict` si está desactualizada. La API normal permite 120 solicitudes por minuto; callbacks, 10 por minuto y 100 por día.

`/admin` ofrece una identidad administrativa, sesiones de 12 horas, CSRF, limitación de intentos y un registro de auditoría inmutable. Permite consultar capacidad, contenido y destinos enmascarados, además de suspender, reanudar, borrar suavemente, restaurar durante siete días o borrar permanentemente con confirmación del ID completo. Nunca puede recuperar secretos ni editar contenido de usuarios.

Consulta [progress.md](progress.md) para el checklist y [sudo.log](sudo.log) para el registro de sudo.
