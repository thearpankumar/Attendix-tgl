const { verifyRegistrationResponse } = require('@simplewebauthn/server');
const crypto = require('crypto');

async function test() {
  try {
    const fakeResponse = {
      id: 'fake-id',
      rawId: 'fake-raw-id',
      type: 'public-key',
      response: {
        attestationObject: 'fake-attestation',
        clientDataJSON: Buffer.from(JSON.stringify({ type: 'webauthn.create', challenge: 'fake-challenge', origin: 'http://localhost' })).toString('base64url')
      }
    };
    
    await verifyRegistrationResponse({
      response: fakeResponse,
      expectedChallenge: 'fake-challenge',
      expectedOrigin: 'http://localhost',
      expectedRPID: 'localhost'
    });
  } catch (err) {
    console.log('Caught error:', err.message);
  }
}
test();
