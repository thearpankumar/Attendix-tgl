import axios from 'axios';

// Deliberately a separate, bare axios call rather than the shared `api`
// instance in `client.ts` — that instance is wired to this app's own
// backend (auth header injection, cookie clearing, 401 handling) which
// doesn't apply to api.github.com.
const GITHUB_REPO = 'thearpankumar/Attendix-tgl';
const RELEASE_TAG_PATTERN = /^mobile-v(\d+\.\d+\.\d+)/;
const APK_ASSET_NAME = 'attendix-mentor-android.apk';

interface GitHubReleaseAsset {
  name: string;
  browser_download_url: string;
}

interface GitHubReleaseResponse {
  tag_name: string;
  body: string | null;
  assets: GitHubReleaseAsset[];
}

export interface MobileRelease {
  version: string;
  downloadUrl: string;
  notes: string;
}

// Exported separately from the network call so the parsing logic (the part
// actually worth testing) doesn't need a mocked HTTP layer.
export function parseMobileRelease(release: GitHubReleaseResponse): MobileRelease | null {
  const match = RELEASE_TAG_PATTERN.exec(release.tag_name);
  const asset = release.assets.find((a) => a.name === APK_ASSET_NAME);
  if (!match || !asset) return null;

  return {
    version: match[1],
    downloadUrl: asset.browser_download_url,
    notes: release.body || '',
  };
}

// mobile-release.yml is the only workflow in this repo that publishes
// GitHub Releases, so /releases/latest is safe here — no risk of it
// returning some other service's release instead.
//
// Unauthenticated GitHub API calls are capped at 60 requests/hour per
// source IP. Fine at this app's mentor-count scale, but worth remembering
// if this ever gets called more often than "once per app open".
export async function fetchLatestMobileRelease(): Promise<MobileRelease | null> {
  const res = await axios.get<GitHubReleaseResponse>(
    `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`
  );
  return parseMobileRelease(res.data);
}
