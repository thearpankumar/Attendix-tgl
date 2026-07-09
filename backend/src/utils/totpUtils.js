const crypto = require('crypto');


// QR Anti-Sharing: 5-second rotating token
// Token format: "${slot}.${hmac_first16}" where slot = floor(ms / 5000)
const QR_WINDOW_MS = 5000;

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
  generateQRToken,
  validateQRToken,
};
