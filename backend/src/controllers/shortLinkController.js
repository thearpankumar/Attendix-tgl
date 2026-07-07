const ShortLink = require('../models/ShortLink');
const Session = require('../models/Session');

async function createShortLink(req, res) {
  try {
    const { shortCode, sessionId, expiresAt } = req.body;
    let finalCode = shortCode;
    if (!finalCode) {
      let attempts = 0;
      while (attempts < 10) {
        finalCode = ShortLink.generateShortCode(6);
        const existing = await ShortLink.findOne({ shortCode: finalCode });
        if (!existing) break;
        attempts++;
      }
    }
    const existingLink = await ShortLink.findOne({ shortCode: finalCode });
    if (existingLink) {
      return res.status(400).json({ message: 'Short code already exists', shortCode: finalCode });
    }
    let sessionObj = null;
    if (sessionId) {
      sessionObj = await Session.findOne({ _id: sessionId, createdBy: req.admin._id });
      if (!sessionObj) {
        return res.status(404).json({ message: 'Session not found or unauthorized' });
      }
      const existingSessionLink = await ShortLink.findOne({ sessionId: sessionId, isActive: true });
      if (existingSessionLink) {
        return res.status(400).json({ 
          message: 'Session already has an active short link', 
          existingLink: existingSessionLink.shortCode 
        });
      }
    }
    const shortLink = new ShortLink({
      shortCode: finalCode,
      sessionId: sessionId || null,
      createdBy: req.admin._id,
      expiresAt: expiresAt || null,
    });
    await shortLink.save();

    if (sessionObj) {
      sessionObj.totpEnabled = true;
      await sessionObj.save();
    }

    res.status(201).json(shortLink);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
}

async function getShortLinks(req, res) {
  try {
    const { page = 1, limit = 20, sessionId, isActive } = req.query;
    const query = {};
    if (sessionId) query.sessionId = sessionId;
    if (isActive !== undefined) query.isActive = isActive === 'true';
    const shortLinks = await ShortLink.find(query)
      .populate('sessionId', 'description isActive expiresAt')
      .populate('createdBy', 'username')
      .sort({ createdAt: -1 })
      .skip((page - 1) * limit)
      .limit(parseInt(limit));
    const total = await ShortLink.countDocuments(query);
    res.json({ shortLinks, total, page: parseInt(page), totalPages: Math.ceil(total / limit) });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
}

async function getShortLinkByCode(req, res) {
  try {
    const { shortCode } = req.params;
    const shortLink = await ShortLink.findOne({ shortCode: shortCode.toLowerCase() })
      .populate('sessionId')
      .populate('createdBy', 'username');
    if (!shortLink) {
      return res.status(404).json({ message: 'Short link not found' });
    }
    if (shortLink.expiresAt && new Date() > shortLink.expiresAt) {
      return res.status(410).json({ message: 'Short link has expired' });
    }
    if (!shortLink.isActive) {
      return res.status(410).json({ message: 'Short link is inactive' });
    }
    if (!shortLink.sessionId) {
      return res.status(400).json({ message: 'Short link not attached to any session' });
    }
    if (!shortLink.sessionId.isActive) {
      return res.status(400).json({ message: 'Associated session is not active' });
    }
    res.json(shortLink);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
}

async function attachShortLinkToSession(req, res) {
  try {
    const { shortCode } = req.params;
    const { sessionId } = req.body;
    const shortLink = await ShortLink.findOne({ shortCode: shortCode.toLowerCase() });
    if (!shortLink) {
      return res.status(404).json({ message: 'Short link not found' });
    }
    if (shortLink.sessionId && shortLink.sessionId.toString() !== sessionId) {
      return res.status(400).json({ 
        message: 'Short link is already attached to another session',
        currentSessionId: shortLink.sessionId 
      });
    }
    const session = await Session.findOne({ _id: sessionId, createdBy: req.admin._id });
    if (!session) {
      return res.status(404).json({ message: 'Session not found or unauthorized' });
    }
    const existingSessionLink = await ShortLink.findOne({ 
      sessionId: sessionId, 
      isActive: true,
      _id: { $ne: shortLink._id }
    });
    if (existingSessionLink) {
      existingSessionLink.sessionId = null;
      existingSessionLink.isActive = false;
      await existingSessionLink.save();
    }
    shortLink.sessionId = sessionId;
    shortLink.isActive = true;
    await shortLink.save();
    session.totpEnabled = true;
    await session.save();
    await shortLink.populate('sessionId');
    res.json(shortLink);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
}

async function detachShortLink(req, res) {
  try {
    const { shortCode } = req.params;
    const shortLink = await ShortLink.findOne({ shortCode: shortCode.toLowerCase() });
    if (!shortLink) {
      return res.status(404).json({ message: 'Short link not found' });
    }
    shortLink.sessionId = null;
    shortLink.isActive = false;
    await shortLink.save();
    res.json({ message: 'Short link detached successfully', shortLink });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
}

async function deleteShortLink(req, res) {
  try {
    const { shortCode } = req.params;
    const shortLink = await ShortLink.findOneAndDelete({ shortCode: shortCode.toLowerCase() });
    if (!shortLink) {
      return res.status(404).json({ message: 'Short link not found' });
    }
    res.json({ message: 'Short link deleted successfully' });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
}

async function getAvailableSessions(req, res) {
  try {
    const sessions = await Session.find({ isActive: true, createdBy: req.admin._id })
      .select('_id description expiresAt createdAt')
      .sort({ createdAt: -1 });
    res.json(sessions);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
}

async function incrementClickCount(req, res) {
  try {
    const { shortCode } = req.params;
    const shortLink = await ShortLink.findOne({ shortCode: shortCode.toLowerCase() });
    if (!shortLink) {
      return res.status(404).json({ message: 'Short link not found' });
    }
    shortLink.clickCount += 1;
    shortLink.lastClickedAt = new Date();
    await shortLink.save();
    res.json({ success: true });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
}

module.exports = {
  createShortLink,
  getShortLinks,
  getShortLinkByCode,
  attachShortLinkToSession,
  detachShortLink,
  deleteShortLink,
  getAvailableSessions,
  incrementClickCount,
};
