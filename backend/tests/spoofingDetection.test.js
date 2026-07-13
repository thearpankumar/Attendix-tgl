const request = require('supertest');
const app = require('../src/server');
const Admin = require('../src/models/Admin');
const Location = require('../src/models/Location');
const Session = require('../src/models/Session');
const ShortLink = require('../src/models/ShortLink');
const { closeRedis } = require('../src/config/redis');

describe('Device Spoofing Detection', () => {
  let admin, session, shortLink;

  beforeAll(async () => {
    admin = await Admin.create({
      username: 'spoofingtest',
      email: 'spoofing@test.com',
      password: 'password123',
    });

    const location = await Location.create({
      name: 'Spoofing Test Location',
      latitude: 40.7128,
      longitude: -74.0060,
      radiusMeters: 100,
      createdBy: admin._id,
    });

    session = await Session.create({
      locationId: location._id,
      tokenHash: 'spoofing-test-token-hash',
      tokenPrefix: 'spf',
      createdBy: admin._id,
      expiresAt: new Date(Date.now() + 3600000),
    });

    shortLink = await ShortLink.create({
      shortCode: 'spooftest123',
      sessionId: session._id,
      createdBy: admin._id,
    });
  });

  afterAll(async () => {
    await closeRedis();
  });

  describe('Sec-CH-UA-Mobile Header Verification', () => {
    const chromeMobileUA = 'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36';
    const chromeDesktopUA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36';

    it('blocks when UA claims mobile but Sec-CH-UA-Mobile says desktop', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', chromeMobileUA)
        .set('sec-ch-ua-mobile', '?0')
        .set('sec-ch-ua', '"Chromium";v="112", "Google Chrome";v="112", "Not:A-Brand";v="99"')
        .set('sec-ch-ua-platform', '"Windows"')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).toBe(403);
      expect(res.body.spoofingDetected).toBe(true);
      expect(res.body.message).toContain('spoofing');
    });

    it('allows when UA and Sec-CH-UA-Mobile both claim mobile', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', chromeMobileUA)
        .set('sec-ch-ua-mobile', '?1')
        .set('sec-ch-ua', '"Chromium";v="112", "Google Chrome";v="112", "Not:A-Brand";v="99"')
        .set('sec-ch-ua-platform', '"Android"')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).not.toBe(403);
    });

    it('allows real desktop browser (not spoofed)', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', chromeDesktopUA)
        .set('sec-ch-ua-mobile', '?0')
        .set('sec-ch-ua', '"Chromium";v="112", "Google Chrome";v="112", "Not:A-Brand";v="99"')
        .set('sec-ch-ua-platform', '"Windows"')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).not.toBe(403);
    });
  });

  describe('Platform Header Verification', () => {
    const iphoneUA = 'Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1';
    const androidUA = 'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36';

    it('blocks when UA claims iPhone but platform says Windows', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', iphoneUA)
        .set('sec-ch-ua-mobile', '?1')
        .set('sec-ch-ua-platform', '"Windows"')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).toBe(403);
      expect(res.body.spoofingDetected).toBe(true);
    });

    it('blocks when UA claims Android but platform says macOS', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', androidUA)
        .set('sec-ch-ua-mobile', '?1')
        .set('sec-ch-ua-platform', '"macOS"')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).toBe(403);
      expect(res.body.spoofingDetected).toBe(true);
    });

    it('allows when platform matches UA (Android)', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', androidUA)
        .set('sec-ch-ua-mobile', '?1')
        .set('sec-ch-ua', '"Chromium";v="112"')
        .set('sec-ch-ua-platform', '"Android"')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).not.toBe(403);
    });
  });

  describe('Bot Detection', () => {
    const botUserAgents = [
      { name: 'curl', ua: 'curl/7.68.0' },
      { name: 'python-requests', ua: 'python-requests/2.25.1' },
      { name: 'wget', ua: 'wget/1.21' },
      { name: 'postman', ua: 'PostmanRuntime/7.28.0' },
      { name: 'insomnia', ua: 'insomnia/2021.7.0' },
      { name: 'generic bot', ua: 'Googlebot/2.1' },
      { name: 'scraper', ua: 'WebScraper/1.0' },
    ];

    botUserAgents.forEach(({ name, ua }) => {
      it(`blocks ${name} user agent`, async () => {
        const res = await request(app)
          .get(`/s/${shortLink.shortCode}/session`)
          .set('User-Agent', ua)
          .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

        expect(res.status).toBe(403);
        expect(res.body.spoofingDetected).toBe(true);
      });
    });
  });

  describe('Edge Cases', () => {
    it('handles missing Sec-CH-UA-Mobile header gracefully (non-Chromium browser)', async () => {
      const safariMobileUA = 'Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1';
      
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', safariMobileUA)
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).not.toBe(403);
    });

    it('handles iPad with MacIntel platform (iPadOS 13+)', async () => {
      const iPadUA = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Safari/605.1.15';
      
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', iPadUA)
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).not.toBe(403);
    });

    it('blocks empty user agent', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', '')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).toBe(403);
    });

    it('handles case-insensitive bot detection', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', 'PYTHON-REQUESTS/2.25.1')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).toBe(403);
    });
  });

  describe('Device Verification Endpoint', () => {
    it('rejects request without metrics', async () => {
      const res = await request(app)
        .post('/api/device/verify')
        .send({});

      expect(res.status).toBe(400);
      expect(res.body.valid).toBe(false);
      expect(res.body.message).toContain('metrics required');
    });

    it('validates clean device metrics', async () => {
      const res = await request(app)
        .post('/api/device/verify')
        .set('User-Agent', 'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36')
        .set('sec-ch-ua-mobile', '?1')
        .set('sec-ch-ua-platform', '"Android"')
        .send({
          metrics: {
            maxTouchPoints: 5,
            hasCoarsePointer: true,
            touchEventSupport: true,
            orientationSupport: true,
            webglRenderer: 'Adreno (TM) 650',
            screenWidth: 1080,
            screenHeight: 2400,
            devicePixelRatio: 2.75,
            hardwareConcurrency: 8,
            deviceMemory: 6,
            isEmulation: false,
            inconsistencies: [],
          }
        });

      expect(res.body.valid).toBe(true);
      expect(res.body.isEmulation).toBe(false);
      expect(res.status).toBe(200);
    });

    it('detects emulation with inconsistencies', async () => {
      const res = await request(app)
        .post('/api/device/verify')
        .set('User-Agent', 'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36')
        .set('sec-ch-ua-mobile', '?1')
        .set('sec-ch-ua-platform', '"Android"')
        .send({
          metrics: {
            maxTouchPoints: 1,
            hasCoarsePointer: true,
            touchEventSupport: true,
            orientationSupport: true,
            webglRenderer: 'NVIDIA GeForce RTX 3080',
            screenWidth: 1920,
            screenHeight: 1080,
            devicePixelRatio: 1,
            hardwareConcurrency: 16,
            deviceMemory: 32,
            isEmulation: true,
            inconsistencies: ['Desktop GPU detected with mobile UA', 'maxTouchPoints exactly 1'],
          }
        });

      expect(res.body.valid).toBe(false);
      expect(res.body.isEmulation).toBe(true);
      expect(res.body.inconsistencies.length).toBeGreaterThan(0);
    });

    it('detects server-side UA/Client-Hint mismatch', async () => {
      const res = await request(app)
        .post('/api/device/verify')
        .set('User-Agent', 'Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1')
        .set('sec-ch-ua-mobile', '?0')
        .set('sec-ch-ua-platform', '"Windows"')
        .send({
          metrics: {
            maxTouchPoints: 5,
            hasCoarsePointer: true,
            isEmulation: false,
            inconsistencies: [],
          }
        });

      expect(res.body.valid).toBe(false);
      expect(res.body.inconsistencies.some(i => i.includes('UA') || i.includes('Mobile'))).toBe(true);
    });
  });

  describe('DEV Bypass Mode', () => {
    const originalEnv = process.env.DEV_BYPASS_ALL;

    beforeAll(() => {
      process.env.DEV_BYPASS_ALL = 'true';
    });

    afterAll(() => {
      if (originalEnv === undefined) {
        delete process.env.DEV_BYPASS_ALL;
      } else {
        process.env.DEV_BYPASS_ALL = originalEnv;
      }
    });

    it('allows all requests when DEV_BYPASS_ALL is true', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', 'curl/7.68.0')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).not.toBe(403);
    });

    it('bypasses spoofing detection in dev mode', async () => {
      const res = await request(app)
        .get(`/s/${shortLink.shortCode}/session`)
        .set('User-Agent', 'Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1')
        .set('sec-ch-ua-mobile', '?0')
        .set('sec-ch-ua-platform', '"Windows"')
        .set('x-test-mobile-check', 'true')
        .set('Accept', 'application/json');

      expect(res.status).not.toBe(403);
    });
  });
});
