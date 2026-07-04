const crypto = require('crypto');

const usedCodes = new Map();
const CODE_TTL_MS = 60000;

let cleanupInterval = null;

function startCleanupInterval() {
  if (cleanupInterval) return;
  cleanupInterval = setInterval(() => {
    const now = Date.now();
    for (const [key, timestamp] of usedCodes.entries()) {
      if (now - timestamp > CODE_TTL_MS) {
        usedCodes.delete(key);
      }
    }
  }, 30000);
}

function stopCleanupInterval() {
  if (cleanupInterval) {
    clearInterval(cleanupInterval);
    cleanupInterval = null;
  }
}

if (process.env.NODE_ENV !== 'test') {
  startCleanupInterval();
}

function generateTOTPSecret() {
  return crypto.randomBytes(32).toString('hex');
}

function generateTOTPCode(secret, sessionId, windowSeconds = 5) {
  const counter = Math.floor(Date.now() / (windowSeconds * 1000));
  const data = `${sessionId}:${counter}:${secret}`;
  return crypto.createHmac('sha256', secret)
    .update(data)
    .digest('hex')
    .slice(0, 6)
    .padStart(6, '0')
    .toUpperCase();
}

function validateTOTPCode(providedCode, secret, sessionId, windowSeconds = 5, toleranceWindows = 1) {
  if (!providedCode || providedCode.length !== 6) {
    return { valid: false, reason: 'Invalid code format' };
  }
  const currentWindow = Math.floor(Date.now() / (windowSeconds * 1000));
  for (let i = -toleranceWindows; i <= toleranceWindows; i++) {
    const counter = currentWindow + i;
    const data = `${sessionId}:${counter}:${secret}`;
    const expectedCode = crypto.createHmac('sha256', secret)
      .update(data)
      .digest('hex')
      .slice(0, 6)
      .padStart(6, '0')
      .toUpperCase();
    if (expectedCode === providedCode.toUpperCase()) {
      return { valid: true, window: counter, windowIndex: i };
    }
  }
  return { valid: false, reason: 'Code expired or invalid' };
}

function markCodeAsUsed(code, sessionId, rollNumber) {
  const key = `${sessionId}:${code}:${rollNumber}`;
  usedCodes.set(key, Date.now());
}

function isCodeUsed(code, sessionId, rollNumber) {
  const key = `${sessionId}:${code}:${rollNumber}`;
  return usedCodes.has(key);
}

function generateTOTPWithTimestamp(secret, sessionId, windowSeconds = 5) {
  const code = generateTOTPCode(secret, sessionId, windowSeconds);
  const expiresAt = new Date(Math.ceil(Date.now() / (windowSeconds * 1000)) * (windowSeconds * 1000) + (windowSeconds * 1000));
  return { code, expiresAt, windowSeconds };
}

// QR Anti-Sharing: 4-second rotating token
// Token format: "${slot}.${hmac_first16}" where slot = floor(ms / 4000)
const QR_WINDOW_MS = 4000;

function generateQRToken(shortCode, secret) {
  const slot = Math.floor(Date.now() / QR_WINDOW_MS);
  const sig = crypto.createHmac('sha256', secret)
    .update(`${shortCode}:${slot}`)
    .digest('hex')
    .slice(0, 16);
  return `${slot}.${sig}`;
}

// Accept current slot and previous slot (±4s window for scan/network lag)
function validateQRToken(shortCode, secret, qrToken) {
  if (!qrToken) return { valid: false, reason: 'No QR token' };
  const parts = qrToken.split('.');
  if (parts.length !== 2) return { valid: false, reason: 'Malformed token' };
  const [slotStr, sig] = parts;
  const tokenSlot = parseInt(slotStr, 10);
  if (isNaN(tokenSlot)) return { valid: false, reason: 'Invalid slot' };
  const currentSlot = Math.floor(Date.now() / QR_WINDOW_MS);
  // Allow current slot and one previous slot (total ~8 second validity window)
  for (const slot of [currentSlot, currentSlot - 1]) {
    if (slot === tokenSlot) {
      const expectedSig = crypto.createHmac('sha256', secret)
        .update(`${shortCode}:${slot}`)
        .digest('hex')
        .slice(0, 16);
      if (expectedSig === sig) return { valid: true };
    }
  }
  return { valid: false, reason: 'QR code expired' };
}

module.exports = {
  generateTOTPSecret,
  generateTOTPCode,
  validateTOTPCode,
  generateTOTPWithTimestamp,
  generateQRToken,
  validateQRToken,
  markCodeAsUsed,
  isCodeUsed,
  startCleanupInterval,
  stopCleanupInterval,
};
