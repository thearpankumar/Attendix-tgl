import { isNewerVersion } from './version';

describe('isNewerVersion', () => {
  it('returns true when the latest patch version is higher', () => {
    expect(isNewerVersion('1.1.5', '1.1.4')).toBe(true);
  });

  it('returns true when the latest minor/major version is higher', () => {
    expect(isNewerVersion('1.2.0', '1.1.9')).toBe(true);
    expect(isNewerVersion('2.0.0', '1.9.9')).toBe(true);
  });

  it('returns false when the versions are equal', () => {
    expect(isNewerVersion('1.1.4', '1.1.4')).toBe(false);
  });

  it('returns false when the latest version is older', () => {
    expect(isNewerVersion('1.1.3', '1.1.4')).toBe(false);
  });

  it('treats missing parts as zero', () => {
    expect(isNewerVersion('1.2', '1.1.9')).toBe(true);
    expect(isNewerVersion('1.1', '1.1.0')).toBe(false);
  });
});
