const mongoose = require('mongoose');
const crypto = require('crypto');

const sessionSchema = new mongoose.Schema({
  locationId: {
    type: mongoose.Schema.Types.ObjectId,
    ref: 'Location',
    required: true,
  },
  tokenHash: {
    type: String,
    required: true,
    unique: true,
  },
  tokenPrefix: {
    type: String,
    required: true,
  },
  description: {
    type: String,
    trim: true,
    maxlength: 200,
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
    required: true,
  },
  rotationCount: {
    type: Number,
    default: 0,
  },
  totpSecret: { // Legacy name, now strictly used as the HMAC secret for QR Anti-Sharing
    type: String,
    default: function() {
      return crypto.randomBytes(32).toString('hex');
    },
  },
  createdAt: {
    type: Date,
    default: Date.now,
  },
});

sessionSchema.statics.generateToken = function () {
  return crypto.randomBytes(16).toString('hex');
};

sessionSchema.statics.hashToken = function (token) {
  return crypto.createHash('sha256').update(token).digest('hex');
};

module.exports = mongoose.model('Session', sessionSchema);
