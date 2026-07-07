const {
  generateRegistrationOptions,
  verifyRegistrationResponse,
  generateAuthenticationOptions,
  verifyAuthenticationResponse,
} = require('@simplewebauthn/server');
const { isoUint8Array } = require('@simplewebauthn/server/helpers');
const crypto = require('crypto');
const config = require('../config');

const rpName = config.webauthn.rpName;
const rpID = config.webauthn.rpID;
const origin = config.webauthn.origin;

function generateChallenge() {
  return crypto.randomBytes(32).toString('base64url');
}

function bufferToBase64URL(buffer) {
  return Buffer.from(buffer).toString('base64url');
}

function base64URLToBuffer(base64url) {
  return Buffer.from(base64url, 'base64url');
}

async function createRegistrationOptions(userId, userName, displayName, excludeCredentials = []) {
  const options = await generateRegistrationOptions({
    rpName,
    rpID,
    userName,
    userDisplayName: displayName || userName,
    userID: isoUint8Array.fromUTF8String(userId),
    attestationType: 'none',
    excludeCredentials: excludeCredentials.map(cred => ({
      id: cred.credentialId,
      type: 'public-key',
      transports: cred.transports || [],
    })),
    authenticatorSelection: {
      userVerification: 'preferred',
      residentKey: 'preferred',
    },
    supportedAlgorithmIDs: [-7, -257],
  });
  
  return options;
}

async function verifyRegistration(registrationResponse, expectedChallenge) {
  try {
    const verification = await verifyRegistrationResponse({
      response: registrationResponse,
      expectedChallenge,
      expectedOrigin: origin,
      expectedRPID: rpID,
    });
    
    return verification;
  } catch (error) {
    throw new Error(`Registration verification failed: ${error.message}`, { cause: error });
  }
}

async function createAuthenticationOptions(allowCredentials = []) {
  const options = await generateAuthenticationOptions({
    rpID,
    allowCredentials: allowCredentials.map(cred => ({
      id: cred.credentialId,
      type: 'public-key',
      transports: cred.transports || [],
    })),
    userVerification: 'preferred',
  });
  
  return options;
}

async function verifyAuthentication(authenticationResponse, expectedChallenge, credential) {
  try {
    const verification = await verifyAuthenticationResponse({
      response: authenticationResponse,
      expectedChallenge,
      expectedOrigin: origin,
      expectedRPID: rpID,
      credential: {
        id: credential.credentialId,
        publicKey: new Uint8Array(credential.publicKey.buffer || credential.publicKey),
        counter: credential.counter,
        transports: credential.transports || [],
      },
    });
    
    return verification;
  } catch (error) {
    throw new Error(`Authentication verification failed: ${error.message}`, { cause: error });
  }
}

function getVerificationMethod(authenticatorData) {
  if (!authenticatorData) return 'unknown';
  
  const flags = authenticatorData.flags || 0;
  
  if (flags & 0x04) {
    return 'face_id';
  }
  
  if (flags & 0x01) {
    return 'fingerprint';
  }
  
  return 'passkey_fallback';
}

function getAuthenticatorAttachment(userAgent) {
  if (!userAgent) return 'platform';
  
  const ua = userAgent.toLowerCase();
  
  if (ua.includes('iphone') || ua.includes('ipad') || 
      ua.includes('android') || ua.includes('mobile')) {
    return 'platform';
  }
  
  return 'platform';
}

module.exports = {
  rpName,
  rpID,
  origin,
  generateChallenge,
  bufferToBase64URL,
  base64URLToBuffer,
  createRegistrationOptions,
  verifyRegistration,
  createAuthenticationOptions,
  verifyAuthentication,
  getVerificationMethod,
  getAuthenticatorAttachment,
};
