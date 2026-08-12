export class ApiError extends Error {
  /** @param {number} status @param {string} code @param {string} message */
  constructor(status, code, message) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

/** @typedef {{ method?: string, body?: unknown, secret?: string, csrf?: string }} ApiOptions */

/** @param {string} path @param {ApiOptions} [options] */
export async function api(path, options = {}) {
  const { method = 'GET', body, secret, csrf } = options;
  const headers = { Accept: 'application/json' };
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  if (secret) headers.Authorization = `Bearer ${secret}`;
  if (csrf) headers['X-CSRF-Token'] = csrf;
  const response = await fetch(`/api/v1${path}`, {
    method,
    headers,
    credentials: 'same-origin',
    body: body === undefined ? undefined : JSON.stringify(body)
  });
  if (!response.ok) {
    let payload = {};
    try { payload = await response.json(); } catch { /* empty error body */ }
    throw new ApiError(
      response.status,
      payload?.error?.code || 'request_failed',
      payload?.error?.message || `Request failed (${response.status})`
    );
  }
  if (response.status === 204) return null;
  return response.json();
}

/** @param {string} value */
export async function copyText(value) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }
  const field = document.createElement('textarea');
  field.value = value;
  field.style.position = 'fixed';
  field.style.opacity = '0';
  document.body.appendChild(field);
  field.select();
  document.execCommand('copy');
  field.remove();
}

/** @param {Array<{id: string, title: string, content: string}>} blocks @param {string[]} selectedIds */
export function mergeBlocks(blocks, selectedIds) {
  return blocks
    .filter((block) => selectedIds.includes(block.id))
    .map((block) => `## ${block.title}\n\n${block.content}`)
    .join('\n\n---\n\n');
}

/** @param {string} filename @param {unknown} value */
export function downloadJson(filename, value) {
  const url = URL.createObjectURL(new Blob([JSON.stringify(value, null, 2)], { type: 'application/json' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}
