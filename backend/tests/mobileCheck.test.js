const request = require('supertest');
const app = require('../src/server');
const Admin = require('../src/models/Admin');
const Location = require('../src/models/Location');
const Session = require('../src/models/Session');
const ShortLink = require('../src/models/ShortLink');
const { closeRedis } = require('../src/config/redis');

describe('Mobile Device Middleware', () => {
  let admin, session, shortLink;

  beforeAll(async () => {
    admin = await Admin.create({
      username: 'mobilechecktest',
      email: 'mobilecheck@test.com',
      password: 'password123',
    });

    const location = await Location.create({
      name: 'Mobile Check Location',
      latitude: 40.7128,
      longitude: -74.0060,
      radiusMeters: 100,
      createdBy: admin._id,
    });

    session = await Session.create({
      locationId: location._id,
      tokenHash: 'mobile-check-token-hash',
      tokenPrefix: 'mchk',
      createdBy: admin._id,
      expiresAt: new Date(Date.now() + 3600000),
    });

    shortLink = await ShortLink.create({
      shortCode: 'mobiletest123',
      sessionId: session._id,
      createdBy: admin._id,
    });
  });

  afterAll(async () => {
    await closeRedis();
  });

  const endpoints = [
    { method: 'get', path: (code) => `/s/${code}` },
    { method: 'get', path: (code) => `/s/${code}/session` },
    { method: 'post', path: (code) => `/s/${code}/submit` },
  ];

  describe('Device Type Blocking', () => {
    const testCases = [
      { name: 'iPhone', ua: 'Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1', allowed: true },
      { name: 'iPad', ua: 'Mozilla/5.0 (iPad; CPU OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1', allowed: true },
      { name: 'Android Phone', ua: 'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36', allowed: true },
      { name: 'Android Tablet', ua: 'Mozilla/5.0 (Linux; Android 13; SM-X700) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36', allowed: true },
      // Under the hybrid plan, standard desktop OSs are passed to the frontend for strict hardware checks
      { name: 'Windows Desktop', ua: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36', allowed: true },
      { name: 'Mac Desktop', ua: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36', allowed: true },
      // Bots and empty agents are strictly blocked at the edge
      { name: 'Curl Bot', ua: 'curl/7.68.0', allowed: false },
      { name: 'Python Requests', ua: 'python-requests/2.25.1', allowed: false },
      { name: 'Empty User Agent', ua: '', allowed: false },
      { name: 'No User Agent', ua: null, allowed: false },
    ];

    testCases.forEach((tc) => {
      describe(`User Agent: ${tc.name}`, () => {
        endpoints.forEach((ep) => {
          it(`${tc.allowed ? 'Allows' : 'Blocks'} ${ep.method.toUpperCase()} ${ep.path(':code')}`, async () => {
            const req = request(app)[ep.method](ep.path(shortLink.shortCode));
            req.set('x-test-mobile-check', 'true');
            if (tc.ua !== null) {
              req.set('User-Agent', tc.ua);
            }
            if (ep.method === 'post' || ep.path('').endsWith('/session')) {
              req.set('Accept', 'application/json');
            }
            if (ep.method === 'post') {
              req.send({}); // Send empty body for submit
            }

            const res = await req;

            if (tc.allowed) {
              // If allowed, we expect whatever error comes from the actual endpoint, but NOT 403 Access Denied
              expect(res.status).not.toBe(403);
              // Either 200, 302, 400 (validation error), etc.
            } else {
              expect(res.status).toBe(403);
            if (ep.method === 'get' && ep.path(shortLink.shortCode) === `/s/${shortLink.shortCode}`) {
                // Short link HTML response - bots get JSON instead of HTML
                expect(res.text).toMatch(/Mobile Device Required|Automated tools are not allowed|spoofingDetected/);
              } else {
                // JSON response
                // Bot detection has a different message
                expect(res.body.message).toMatch(/only allowed on mobile devices|Automated tools are not allowed/i);
              }
            }
          });
        });
      });
    });
  });
});
