/**
 * Triggers a browser download of a blob response (e.g. an xlsx export) via a
 * throwaway object URL + anchor click. Shared by every export button that
 * calls `responseType: 'blob'` — previously duplicated inline per page.
 */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

/**
 * Extracts the filename from a `Content-Disposition: attachment;
 * filename="..."` response header, falling back to `fallback` if the header
 * is absent or unparseable.
 */
export function filenameFromContentDisposition(header: string | undefined, fallback: string): string {
  if (!header) return fallback;
  const match = header.match(/filename="?([^"]+)"?/);
  return match?.[1] || fallback;
}
