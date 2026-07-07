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
  totpSecret: {
    type: String,
    default: function() {
      return crypto.randomBytes(32).toString('hex');
    },
  },
  totpEnabled: {
    type: Boolean,
    default: false,
  },
  totpWindowSeconds: {
    type: Number,
    default: 15,
    min: 5,
    max: 60,
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

sessionSchema.methods.generateTOTP = function() {
  const counter = Math.floor(Date.now() / (this.totpWindowSeconds * 1000));
  const data = `${this._id}:${counter}:${this.totpSecret || ''}`;
  return crypto.createHmac('sha256', this.totpSecret || 'default-secret')
    .update(data)
    .digest('hex')
    .slice(0, 6)
    .toUpperCase();
};

sessionSchema.methods.validateTOTP = function(totpCode, toleranceWindows = 1) {
  const currentWindow = Math.floor(Date.now() / (this.totpWindowSeconds * 1000));
  for (let i = -toleranceWindows; i <= toleranceWindows; i++) {
    const counter = currentWindow + i;
    const data = `${this._id}:${counter}:${this.totpSecret || ''}`;
    const expectedCode = crypto.createHmac('sha256', this.totpSecret || 'default-secret')
      .update(data)
      .digest('hex')
      .slice(0, 6)
      .toUpperCase();
    if (expectedCode === totpCode.toUpperCase()) {
      return { valid: true, window: counter };
    }
  }
  return { valid: false, window: null };
};

module.exports = mongoose.model('Session', sessionSchema);
