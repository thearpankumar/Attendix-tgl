import { greetingWord } from './greeting';

describe('greetingWord', () => {
  afterEach(() => {
    jest.useRealTimers();
  });

  const atHour = (hour: number) => {
    jest.useFakeTimers().setSystemTime(new Date(2026, 0, 1, hour, 0, 0));
  };

  it('says good morning before noon', () => {
    atHour(9);
    expect(greetingWord()).toBe('Good morning');
  });

  it('says good morning right up to 11:59', () => {
    atHour(11);
    expect(greetingWord()).toBe('Good morning');
  });

  it('says good afternoon at noon', () => {
    atHour(12);
    expect(greetingWord()).toBe('Good afternoon');
  });

  it('says good afternoon right up to 16:59', () => {
    atHour(16);
    expect(greetingWord()).toBe('Good afternoon');
  });

  it('says good evening at 17:00 and later', () => {
    atHour(17);
    expect(greetingWord()).toBe('Good evening');
  });

  it('says good evening late at night', () => {
    atHour(23);
    expect(greetingWord()).toBe('Good evening');
  });
});
