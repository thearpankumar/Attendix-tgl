/**
 * The one sanctioned place `console` is called from application code, so
 * `no-console` only needs a single exemption per app instead of one at every
 * call site. Use for best-effort operations where failure is swallowed and
 * only needs to be visible for debugging (e.g. a logout request that fails
 * but the user is signed out locally anyway).
 */
export function logWarning(message: string, detail?: unknown): void {
  // eslint-disable-next-line no-console
  console.warn(message, detail);
}
