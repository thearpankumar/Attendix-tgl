import { ConfigContext, ExpoConfig } from 'expo/config';

// app.json holds the static config; this layers the per-environment API base
// URL on top so `eas.json` build profiles (or a local .env) can point the
// app at a dev/staging backend without editing checked-in config.
export default ({ config }: ConfigContext): ExpoConfig => ({
  ...(config as ExpoConfig),
  extra: {
    ...config.extra,
    apiBaseUrl: process.env.EXPO_PUBLIC_API_BASE_URL || (config.extra?.apiBaseUrl as string),
  },
});
