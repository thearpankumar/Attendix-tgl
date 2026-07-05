const mongoose = require('mongoose');
const crypto = require('crypto');

const shortLinkSchema = new mongoose.Schema({
  shortCode: {
    type: String,
    required: true,
    unique: true,
    trim: true,
    lowercase: true,
    match: /^[a-z0-9-]{3,20}$/,
  },
  sessionId: {
    type: mongoose.Schema.Types.ObjectId,
    ref: 'Session',
    default: null,
  },
  createdBy: {
    type: mongoose.Schema.Types.ObjectId,
    ref: 'Admin',
    required: true,
  },
  isActive: {
    type: Boolean,
    default: true,
  },
  expiresAt: {
    type: Date,
    default: null,
  },
  clickCount: {
    type: Number,
    default: 0,
  },
  lastClickedAt: {
    type: Date,
    default: null,
  },
  createdAt: {
    type: Date,
    default: Date.now,
  },
});

// Removed duplicate index - unique: true in schema already creates the index
shortLinkSchema.index({ sessionId: 1 });
shortLinkSchema.index({ createdBy: 1 });

shortLinkSchema.statics.generateShortCode = function(length = 6) {
  const chars = 'abcdefghijkmnpqrstuvwxyz23456789';
  const bytes = crypto.randomBytes(length);
  let code = '';
  for (let i = 0; i < length; i++) {
    code += chars[bytes[i] % chars.length];
  }
  return code;
};

module.exports = mongoose.model('ShortLink', shortLinkSchema);
