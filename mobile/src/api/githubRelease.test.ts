import { parseMobileRelease } from './githubRelease';

describe('parseMobileRelease', () => {
  it('extracts the semver from a mobile-v tag and finds the APK asset', () => {
    const result = parseMobileRelease({
      tag_name: 'mobile-v1.1.4-53bdc2c',
      body: 'Release notes here',
      assets: [{ name: 'attendix-mentor-android.apk', browser_download_url: 'https://example.com/app.apk' }],
    });

    expect(result).toEqual({
      version: '1.1.4',
      downloadUrl: 'https://example.com/app.apk',
      notes: 'Release notes here',
    });
  });

  it('returns null when the tag does not match the mobile-v pattern', () => {
    const result = parseMobileRelease({
      tag_name: 'backend-v1.5.3-abcdef1',
      body: '',
      assets: [{ name: 'attendix-mentor-android.apk', browser_download_url: 'https://example.com/app.apk' }],
    });

    expect(result).toBeNull();
  });

  it('returns null when the APK asset is missing', () => {
    const result = parseMobileRelease({
      tag_name: 'mobile-v1.1.4-53bdc2c',
      body: '',
      assets: [],
    });

    expect(result).toBeNull();
  });

  it('falls back to an empty string when the release body is null', () => {
    const result = parseMobileRelease({
      tag_name: 'mobile-v1.1.4-53bdc2c',
      body: null,
      assets: [{ name: 'attendix-mentor-android.apk', browser_download_url: 'https://example.com/app.apk' }],
    });

    expect(result?.notes).toBe('');
  });
});
