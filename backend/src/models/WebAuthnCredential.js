const mongoose = require('mongoose');

const webAuthnCredentialSchema = new mongoose.Schema({
  studentId: {
    type: String,
    required: true,
    trim: true,
    uppercase: true,
  },
  credentialId: {
    type: String,
    required: true,
    unique: true,
    index: true,
  },
  publicKey: {
    type: Buffer,
    required: true,
  },
  counter: {
    type: Number,
    required: true,
    default: 0,
  },
  deviceLabel: {
    type: String,
    default: 'Unknown Device',
  },
  deviceType: {
    type: String,
    enum: ['single_device', 'multi_device', 'singleDevice', 'multiDevice'],
    default: 'multiDevice',
  },
  transports: [{
    type: String,
    enum: ['internal', 'hybrid', 'ble', 'nfc', 'usb', 'cable', 'smart-card'],
  }],
  enrolledAt: {
    type: Date,
    default: Date.now,
  },
  enrolledIpAddress: String,
  enrolledUserAgent: String,
  createdByAdminId: {
    type: mongoose.Schema.Types.ObjectId,
    ref: 'Admin',
    default: null,
  },
  signCount: {
    type: Number,
    default: 0,
  },
  lastUsedAt: Date,
  lastSessionId: {
    type: mongoose.Schema.Types.ObjectId,
    ref: 'Session',
  },
  isSuspended: {
    type: Boolean,
    default: false,
  },
  suspendedReason: String,
  suspendedAt: Date,
  suspendedBy: {
    type: mongoose.Schema.Types.ObjectId,
    ref: 'Admin',
  },
  aaguid: {
    type: String,
    default: null,
  },
});

webAuthnCredentialSchema.index({ studentId: 1 }, { unique: true });
webAuthnCredentialSchema.index({ aaguid: 1 });
webAuthnCredentialSchema.index({ enrolledAt: -1 });
webAuthnCredentialSchema.index({ isSuspended: 1 });

module.exports = mongoose.model('WebAuthnCredential', webAuthnCredentialSchema);
