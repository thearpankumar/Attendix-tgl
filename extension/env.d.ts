/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Backend/site origin this extension talks to — see lib/config.ts. */
  readonly WXT_API_BASE_URL?: string;
}
