import { writable, get } from 'svelte/store';

export const localeOptions = [
  { key: 'zh-TW', label: '繁中' },
  { key: 'en', label: 'English' },
  { key: 'es', label: 'Español' },
  { key: 'fr', label: 'Français' }
];

const messages = {
  'zh-TW': {
    language: '語言', home: '首頁', admin: '管理員', create: '建立', vaultLink: 'Vault 連結', emailOtp: 'Email 驗證碼',
    heroTitle: '把你習慣的 AI 工具，<br />帶到任何平臺。',
    heroLead: '不只是 Markdown 剪貼簿。用有結構的 Prompt、Template、Skill、MCP、Workflow 與 Schema 組成私人 Vault，再複製成 Agent 看得懂的單一文字包。',
    typedAssets: '12 Typed Assets', agentBundles: 'Agent-ready Bundles', aiApi: 'AI-writable API', notify: 'Completion Notify',
    permanentVault: '建立你的永久 Vault', noRegistration: '不用註冊，只要保存連結。', vaultName: 'Vault 名稱', createVault: '建立私人 Vault →', creating: '建立中…',
    vaultCreatedHint: '建立後會產生唯一高熵祕密連結。你可以進入 Vault 後再綁定已驗證的 Email。',
    vaultMethod: '方式一 · VAULT', useLink: '使用目前的管理連結。', vaultAccess: 'Vault 管理連結或 secret', vaultPlaceholder: 'https://…/#/v/…', openVault: '開啟 Vault →',
    vaultSecurity: 'Secret 只留在 URL fragment，不會送進一般 HTTP access log。請把它當成密碼保管。',
    emailMethod: '方式二 · EMAIL OTP', sendCode: '寄一組登入驗證碼。', enterCode: '輸入六位數驗證碼。', email: 'Email', otp: '六位數驗證碼', processing: '處理中…', sendCodeButton: '寄送驗證碼 →', verifyLogin: '驗證並登入 →', changeEmail: '更換 Email 或重新寄送',
    emailEnabled: '驗證碼 10 分鐘內有效；成功後此瀏覽器保持登入 30 天。系統不會透露未綁定的 Email。', emailDisabled: '站台管理員尚未設定 SMTP，因此 Email 登入目前不可用。',
    eyebrowTravel: 'HOW IT TRAVELS', typedHeading: '有型別的文字，讓 Agent 知道該怎麼用。',
    step1: '選型別，取得骨架', step1Text: '建立 Prompt Template、Skill、MCP Server、Agent Profile、Workflow、Schema 等 12 種資產時，自動帶入可編輯模板。',
    step2: '組成 Agent Pack', step2Text: '依任務勾選資產，一鍵產生總體說明、各型別使用方式與內容，貼到任何能接收文字的 AI。',
    step3: '交給 AI 維護', step3Text: '複製內建 API 說明，AI 就能新增、修改、排序及刪除；做完還能通知你。',
    clearByDesign: 'CLEAR BY DESIGN', permanentNotVault: '永久保存，但不是祕密保管箱。', permanentText: '有內容或通知設定的 Vault 不會因為閒置而過期。只有建立 30 天仍完全空白的 Vault，或刪除後超過 7 天的 Vault，才會清除。',
    privacyWarning: 'CrossPrompt 並非端對端加密。', privacyText: '服務管理員可基於維運與濫用處理查看內容。請勿存放密碼、API Key、助記詞或其他機密。',
    footer: 'Portable AI assets, without platform lock-in.', invalidSecret: '請貼上完整 Vault 管理連結或 secret。'
  },
  en: {
    language: 'Language', home: 'Home', admin: 'Admin', create: 'Create', vaultLink: 'Vault link', emailOtp: 'Email code',
    heroTitle: 'Take your AI toolkit,<br />to every platform.',
    heroLead: 'More than a Markdown clipboard. Build a private Vault from typed Prompts, Templates, Skills, MCP, Workflows, and Schemas, then copy one Agent-ready text pack.',
    typedAssets: '12 Typed Assets', agentBundles: 'Agent-ready Bundles', aiApi: 'AI-writable API', notify: 'Completion Notify',
    permanentVault: 'Create your permanent Vault', noRegistration: 'No account. Just keep the link.', vaultName: 'Vault name', createVault: 'Create private Vault →', creating: 'Creating…',
    vaultCreatedHint: 'A high-entropy secret link is generated. You can bind a verified email from inside the Vault later.',
    vaultMethod: 'METHOD ONE · VAULT', useLink: 'Use your existing management link.', vaultAccess: 'Vault link or secret', vaultPlaceholder: 'https://…/#/v/…', openVault: 'Open Vault →',
    vaultSecurity: 'The secret stays in the URL fragment and is not sent to normal HTTP access logs. Treat it like a password.',
    emailMethod: 'METHOD TWO · EMAIL OTP', sendCode: 'Send me a sign-in code.', enterCode: 'Enter the six-digit code.', email: 'Email', otp: 'Six-digit code', processing: 'Working…', sendCodeButton: 'Send code →', verifyLogin: 'Verify and sign in →', changeEmail: 'Change email or resend',
    emailEnabled: 'Codes are valid for 10 minutes; this browser stays signed in for 30 days. We never reveal whether an email is unbound.', emailDisabled: 'Email login is unavailable because SMTP has not been configured by the site administrator.',
    eyebrowTravel: 'HOW IT TRAVELS', typedHeading: 'Typed text that tells an Agent what to do.',
    step1: 'Choose a type, get a skeleton', step1Text: 'Create any of 12 assets—Prompt Template, Skill, MCP Server, Agent Profile, Workflow, Schema, and more—with an editable starter template.',
    step2: 'Build an Agent Pack', step2Text: 'Select assets for a task and produce one guide with type-specific instructions and content, ready for any text-capable AI.',
    step3: 'Let AI maintain it', step3Text: 'Copy the built-in API guide so an AI can create, edit, reorder, and delete assets—and notify you when done.',
    clearByDesign: 'CLEAR BY DESIGN', permanentNotVault: 'Permanent storage, not a secret vault.', permanentText: 'Vaults with content or notification settings never expire for inactivity. Only a completely empty Vault after 30 days, or a deleted Vault after seven days, is removed.',
    privacyWarning: 'CrossPrompt is not end-to-end encrypted.', privacyText: 'Service administrators may view content for operations and abuse handling. Do not store passwords, API keys, recovery phrases, or other secrets.',
    footer: 'Portable AI assets, without platform lock-in.', invalidSecret: 'Paste a complete Vault management link or secret.'
  },
  es: {
    language: 'Idioma', home: 'Inicio', admin: 'Administración', create: 'Crear', vaultLink: 'Enlace del Vault', emailOtp: 'Código de email',
    heroTitle: 'Lleva tus herramientas de IA,<br />a cualquier plataforma.',
    heroLead: 'Más que un portapapeles Markdown. Crea un Vault privado con Prompts, Plantillas, Skills, MCP, Workflows y Schemas tipados, y copia un único paquete listo para tu Agent.',
    typedAssets: '12 activos tipados', agentBundles: 'Paquetes para Agent', aiApi: 'API editable por IA', notify: 'Notificación al completar',
    permanentVault: 'Crea tu Vault permanente', noRegistration: 'Sin cuenta. Solo guarda el enlace.', vaultName: 'Nombre del Vault', createVault: 'Crear Vault privado →', creating: 'Creando…',
    vaultCreatedHint: 'Se genera un enlace secreto de alta entropía. Después puedes vincular un email verificado desde el Vault.',
    vaultMethod: 'MÉTODO UNO · VAULT', useLink: 'Usa tu enlace de administración.', vaultAccess: 'Enlace o secreto del Vault', vaultPlaceholder: 'https://…/#/v/…', openVault: 'Abrir Vault →',
    vaultSecurity: 'El secreto queda en el fragmento de URL y no aparece en los registros HTTP normales. Trátalo como una contraseña.',
    emailMethod: 'MÉTODO DOS · EMAIL OTP', sendCode: 'Envíame un código de acceso.', enterCode: 'Introduce el código de seis dígitos.', email: 'Email', otp: 'Código de seis dígitos', processing: 'Procesando…', sendCodeButton: 'Enviar código →', verifyLogin: 'Verificar e iniciar sesión →', changeEmail: 'Cambiar email o reenviar',
    emailEnabled: 'El código caduca en 10 minutos; este navegador conserva la sesión 30 días. Nunca revelamos si un email no está vinculado.', emailDisabled: 'El acceso por email no está disponible porque el administrador aún no configuró SMTP.',
    eyebrowTravel: 'CÓMO VIAJA', typedHeading: 'Texto tipado para que el Agent sepa cómo usarlo.',
    step1: 'Elige un tipo, obtén una base', step1Text: 'Crea 12 tipos de activos—Prompt Template, Skill, MCP Server, Agent Profile, Workflow, Schema y más—con plantillas editables.',
    step2: 'Crea un Agent Pack', step2Text: 'Selecciona activos para una tarea y genera una guía con instrucciones por tipo y contenido, lista para cualquier IA que acepte texto.',
    step3: 'Deja que la IA lo mantenga', step3Text: 'Copia la guía de API para que una IA pueda crear, editar, ordenar y borrar activos, y avisarte al terminar.',
    clearByDesign: 'DISEÑO TRANSPARENTE', permanentNotVault: 'Almacenamiento permanente, no una caja fuerte.', permanentText: 'Los Vaults con contenido o notificaciones nunca caducan por inactividad. Solo se elimina un Vault totalmente vacío tras 30 días, o uno borrado tras siete días.',
    privacyWarning: 'CrossPrompt no tiene cifrado de extremo a extremo.', privacyText: 'Los administradores pueden ver el contenido para operar el servicio y gestionar abusos. No guardes contraseñas, claves API, frases de recuperación ni otros secretos.',
    footer: 'Activos de IA portátiles, sin bloqueo de plataforma.', invalidSecret: 'Pega un enlace completo de administración o un secreto del Vault.'
  },
  fr: {
    language: 'Langue', home: 'Accueil', admin: 'Administration', create: 'Créer', vaultLink: 'Lien du Vault', emailOtp: 'Code e-mail',
    heroTitle: 'Emportez vos outils IA,<br />sur toutes les plateformes.',
    heroLead: 'Bien plus qu’un presse-papiers Markdown. Composez un Vault privé avec des Prompts, Templates, Skills, MCP, Workflows et Schemas typés, puis copiez un pack texte prêt pour votre Agent.',
    typedAssets: '12 assets typés', agentBundles: 'Packs pour Agent', aiApi: 'API modifiable par IA', notify: 'Notification de fin',
    permanentVault: 'Créer votre Vault permanent', noRegistration: 'Aucun compte. Conservez simplement le lien.', vaultName: 'Nom du Vault', createVault: 'Créer un Vault privé →', creating: 'Création…',
    vaultCreatedHint: 'Un lien secret à haute entropie est généré. Vous pourrez ensuite lier un e-mail vérifié depuis le Vault.',
    vaultMethod: 'MÉTHODE 1 · VAULT', useLink: 'Utilisez votre lien de gestion.', vaultAccess: 'Lien ou secret du Vault', vaultPlaceholder: 'https://…/#/v/…', openVault: 'Ouvrir le Vault →',
    vaultSecurity: 'Le secret reste dans le fragment de l’URL et n’apparaît pas dans les journaux HTTP classiques. Traitez-le comme un mot de passe.',
    emailMethod: 'MÉTHODE 2 · EMAIL OTP', sendCode: 'Envoyer un code de connexion.', enterCode: 'Saisissez le code à six chiffres.', email: 'E-mail', otp: 'Code à six chiffres', processing: 'Traitement…', sendCodeButton: 'Envoyer le code →', verifyLogin: 'Vérifier et se connecter →', changeEmail: 'Changer d’e-mail ou renvoyer',
    emailEnabled: 'Le code est valable 10 minutes ; ce navigateur reste connecté 30 jours. Nous ne révélons jamais si un e-mail n’est pas lié.', emailDisabled: 'La connexion par e-mail est indisponible car le SMTP n’a pas été configuré par l’administrateur.',
    eyebrowTravel: 'COMMENT ÇA VOYAGE', typedHeading: 'Du texte typé pour guider votre Agent.',
    step1: 'Choisir un type, obtenir une base', step1Text: 'Créez 12 types d’assets—Prompt Template, Skill, MCP Server, Agent Profile, Workflow, Schema et plus—avec un modèle modifiable.',
    step2: 'Composer un Agent Pack', step2Text: 'Sélectionnez les assets d’une tâche et générez un guide unique avec les instructions propres à chaque type et leur contenu.',
    step3: 'Laisser l’IA le maintenir', step3Text: 'Copiez le guide API intégré pour qu’une IA crée, modifie, réordonne et supprime les assets, puis vous prévienne.',
    clearByDesign: 'CONÇU EN TOUTE CLARTÉ', permanentNotVault: 'Stockage permanent, pas un coffre-fort.', permanentText: 'Les Vaults contenant du contenu ou des notifications n’expirent jamais par inactivité. Seul un Vault entièrement vide après 30 jours, ou supprimé après sept jours, est effacé.',
    privacyWarning: 'CrossPrompt n’est pas chiffré de bout en bout.', privacyText: 'Les administrateurs peuvent consulter le contenu pour l’exploitation et la lutte contre les abus. Ne stockez pas de mots de passe, clés API, phrases de récupération ou autres secrets.',
    footer: 'Des assets IA portables, sans verrouillage de plateforme.', invalidSecret: 'Collez un lien de gestion complet ou le secret du Vault.'
  }
};

function detectLocale() {
  if (typeof localStorage !== 'undefined') {
    const stored = localStorage.getItem('crossprompt_locale');
    if (stored && messages[stored]) return stored;
  }
  const language = typeof navigator !== 'undefined' ? navigator.language.toLowerCase() : '';
  if (language.startsWith('zh')) return 'zh-TW';
  if (language.startsWith('es')) return 'es';
  if (language.startsWith('fr')) return 'fr';
  return 'en';
}

export const locale = writable(detectLocale());

export function setLocale(next) {
  if (!messages[next]) return;
  locale.set(next);
  if (typeof localStorage !== 'undefined') localStorage.setItem('crossprompt_locale', next);
  if (typeof document !== 'undefined') document.documentElement.lang = next;
}

export function t(key, vars = {}) {
  const lang = get(locale);
  let value = messages[lang]?.[key] || messages.en[key] || key;
  return Object.entries(vars).reduce((result, [name, replacement]) => result.replaceAll(`{{${name}}}`, String(replacement)), value);
}

