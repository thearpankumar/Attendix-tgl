const isMobile = require('is-mobile');

function requireMobileDevice(req, res, next) {
  if (process.env.NODE_ENV === 'test' && req.headers['x-test-mobile-check'] !== 'true') {
    return next();
  }
  if (process.env.DEV_BYPASS_ALL === 'true') {
    return next();
  }

  const userAgent = req.headers['user-agent'] || '';
  
  const isChromium = /Chrome|Chromium|Edg|Opera|Brave/i.test(userAgent);
  const uaClaimsMobile = /Mobi|Android|iPhone|iPad|iPod/i.test(userAgent);
  const isObviousBot = /curl|postman|insomnia|python|wget|httpie|bot|spider|scraper/i.test(userAgent);
  
  if (isObviousBot) {
    return res.status(403).json({
      success: false,
      message: 'Access Denied: Automated tools are not allowed.',
      spoofingDetected: true
    });
  }
  
  if (isChromium && req.headers['sec-ch-ua-mobile'] !== undefined) {
    const clientHintMobile = req.headers['sec-ch-ua-mobile'];
    const chSaysMobile = clientHintMobile === '?1';
    
    if (uaClaimsMobile && !chSaysMobile) {
      return res.status(403).json({
        success: false,
        message: 'Device verification failed: User-Agent spoofing detected. Please use a real mobile device.',
        spoofingDetected: true
      });
    }
    
    if (!uaClaimsMobile && chSaysMobile) {
      return res.status(403).json({
        success: false,
        message: 'Device verification failed: Inconsistent device signals detected.',
        spoofingDetected: true
      });
    }
  }
  
  const platform = req.headers['sec-ch-ua-platform'] || '';
  
  // Check platform consistency for Chromium browsers
  if (isChromium && uaClaimsMobile) {
    const validMobilePlatforms = ['"Android"', '"iOS"', '"iPhone"', '"iPad"'];
    const isMobilePlatform = validMobilePlatforms.some(p => platform.includes(p.replace(/"/g, '')));
    const isDesktopPlatform = /Windows|macOS|Linux|Chrome OS/i.test(platform);
    
    if (isDesktopPlatform && !isMobilePlatform && uaClaimsMobile) {
      return res.status(403).json({
        success: false,
        message: 'Device verification failed: Desktop platform with mobile User-Agent.',
        spoofingDetected: true
      });
    }
  }
  
  // Check platform consistency for Safari and other non-Chromium browsers
  if (!isChromium && platform && uaClaimsMobile) {
    const isDesktopPlatform = /Windows|macOS|Linux|Chrome OS/i.test(platform);
    if (isDesktopPlatform) {
      return res.status(403).json({
        success: false,
        message: 'Device verification failed: Desktop platform with mobile User-Agent.',
        spoofingDetected: true
      });
    }
  }
  
  // 1. Is it explicitly a mobile device according to is-mobile?
  const explicitlyMobile = isMobile({ ua: userAgent, tablet: true });
  
  // 2. Could it be a tablet or phone masquerading as a desktop?
  // (e.g., iPadOS 13+ sends Macintosh, Android Desktop Mode sends X11/Linux)
  const mightBeMasquerading = /Macintosh|Windows|Linux|X11|CrOS/i.test(userAgent);

  // We only block if it's not mobile AND doesn't look like a standard OS that could be masquerading.
  // The STRICT hardware check (maxTouchPoints) will happen on the React client side.
  if (!explicitlyMobile && !mightBeMasquerading) {
    if (req.accepts('html') && !req.xhr && !req.path.includes('/api/')) {
      return res.status(403).send(`
        <!DOCTYPE html>
        <html>
        <head>
          <meta name="viewport" content="width=device-width, initial-scale=1.0">
          <title>Mobile Device Required</title>
          <style>
            body { font-family: -apple-system, system-ui, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; background-color: #f3f4f6; color: #1f2937; text-align: center; padding: 20px; }
            .card { background: white; padding: 40px 20px; border-radius: 12px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); max-width: 400px; }
            h2 { color: #ef4444; margin-top: 0; }
            p { font-size: 1.1rem; line-height: 1.5; color: #4b5563; }
          </style>
        </head>
        <body>
          <div class="card">
            <h2>📱 Mobile Device Required</h2>
            <p>For security and location verification, attendance can only be marked using a smartphone or tablet.</p>
            <p><strong>Please scan the QR code or open this link on your mobile device.</strong></p>
          </div>
        </body>
        </html>
      `);
    }

    return res.status(403).json({
      success: false,
      message: 'Access Denied: This action is only allowed on mobile devices.'
    });
  }

  next();
}

module.exports = { requireMobileDevice };
