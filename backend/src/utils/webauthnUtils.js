const {
  generateRegistrationOptions,
  verifyRegistrationResponse,
  generateAuthenticationOptions,
  verifyAuthenticationResponse,
} = require('@simplewebauthn/server');
const { isoUint8Array } = require('@simplewebauthn/server/helpers');
const crypto = require('crypto');
const config = require('../config');

function timingSafeEqual(a, b) {
  if (typeof a !== 'string' || typeof b !== 'string') return false;
  if (a.length !== b.length) return false;
  return crypto.timingSafeEqual(Buffer.from(a, 'utf8'), Buffer.from(b, 'utf8'));
}

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
      userVerification: 'required',
      residentKey: 'required',
      requireResidentKey: true,
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
    userVerification: 'required',
  });
  
  return options;
}

async function createAuthenticationOptionsWithoutCredentials() {
  const options = await generateAuthenticationOptions({
    rpID,
    allowCredentials: [],
    userVerification: 'required',
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
  
  const UV_FLAG = 0x04;
  const UP_FLAG = 0x01;
  const BE_FLAG = 0x08;
  
  if (flags & UV_FLAG && flags & UP_FLAG) {
    return 'biometric_verified';
  }
  
  if (flags & UP_FLAG && !(flags & UV_FLAG)) {
    return 'presence_only';
  }
  
  if (flags & BE_FLAG) {
    return 'backup_eligible';
  }
  
  return 'unknown';
}

function getAuthenticatorAttachment(userAgent) {
  if (!userAgent) return 'platform';
  
  const ua = userAgent.toLowerCase();
  
  if (ua.includes('iphone') || ua.includes('ipad') || 
      ua.includes('android') || ua.includes('mobile')) {
    return 'platform';
  }
  
  return 'cross-platform';
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
  createAuthenticationOptionsWithoutCredentials,
  verifyAuthentication,
  getVerificationMethod,
  getAuthenticatorAttachment,
  timingSafeEqual,
};
