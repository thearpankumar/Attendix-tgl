import { initials } from './initials';

describe('initials', () => {
  it('takes the first letter of the first two words', () => {
    expect(initials('Jane Doe')).toBe('JD');
  });

  it('uppercases lowercase names', () => {
    expect(initials('jane doe')).toBe('JD');
  });

  it('handles a single-word name', () => {
    expect(initials('Cher')).toBe('C');
  });

  it('ignores extra words beyond the first two', () => {
    expect(initials('Mary Jane Watson')).toBe('MJ');
  });

  it('collapses repeated whitespace instead of producing empty parts', () => {
    expect(initials('Jane   Doe')).toBe('JD');
  });

  it('falls back to "?" for an empty or whitespace-only name', () => {
    expect(initials('')).toBe('?');
    expect(initials('   ')).toBe('?');
  });
});
