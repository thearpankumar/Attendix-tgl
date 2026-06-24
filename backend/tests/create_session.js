const mongoose = require('mongoose');
const Admin = require('../src/models/Admin');
const Location = require('../src/models/Location');
const Session = require('../src/models/Session');

async function createTestSession() {
  await mongoose.connect('mongodb://localhost:27017/attendance-geotag');
  console.log('Connected to DB');

  let admin = await Admin.findOne();
  if (!admin) {
    admin = await Admin.create({
      username: 'testadmin',
      email: 'admin@test.com',
      password: 'password123'
    });
  }

  let location = await Location.findOne();
  if (!location) {
    location = await Location.create({
      name: 'Classroom A',
      latitude: 12.9715987,
      longitude: 77.5945627,
      radiusMeters: 100,
      createdBy: admin._id
    });
  }

  const token = Session.generateToken();
  const session = await Session.create({
    locationId: location._id,
    tokenHash: Session.hashToken(token),
    tokenPrefix: token.substring(0, 4),
    createdBy: admin._id,
    expiresAt: new Date(Date.now() + 120 * 60 * 1000) // 2 hours
  });

  console.log('Token created successfully!');
  console.log(`Token link: http://localhost/attend/${token}`);
  await mongoose.disconnect();
}

createTestSession();
