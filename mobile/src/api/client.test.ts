import { apiErrorMessage } from './client';

describe('apiErrorMessage', () => {
  it('prefers the server-provided message field', () => {
    const error = { response: { data: { message: 'Wrong password', error: 'bad_request' } } };
    expect(apiErrorMessage(error, 'fallback')).toBe('Wrong password');
  });

  it('falls back to the error field when message is absent', () => {
    const error = { response: { data: { error: 'Too many requests' } } };
    expect(apiErrorMessage(error, 'fallback')).toBe('Too many requests');
  });

  it('falls back to the provided default when the response has no body', () => {
    const error = { response: {} };
    expect(apiErrorMessage(error, 'fallback')).toBe('fallback');
  });

  it('falls back to the provided default for a network error with no response', () => {
    const error = { message: 'Network Error' };
    expect(apiErrorMessage(error, 'fallback')).toBe('fallback');
  });

  it('falls back to the provided default for a non-error value', () => {
    expect(apiErrorMessage(undefined, 'fallback')).toBe('fallback');
    expect(apiErrorMessage(null, 'fallback')).toBe('fallback');
  });
});
