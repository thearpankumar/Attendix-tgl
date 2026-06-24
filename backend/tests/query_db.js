const mongoose = require('mongoose');
const Attendance = require('../src/models/Attendance');
const Session = require('../src/models/Session');
require('../src/models/Location');

async function checkDatabase() {
  await mongoose.connect('mongodb://localhost:27017/attendance-geotag');
  console.log('Connected to DB');

  const sessions = await Session.find().populate('locationId');
  console.log('Sessions total:', sessions.length);
  for (const s of sessions) {
    console.log(`- Session ID: ${s._id}, Code: ${s.sessionCode}, Location: ${s.locationId ? s.locationId.name : 'none'}, Expiration: ${s.expiresAt}`);
  }

  const attendances = await Attendance.find().sort({ capturedAt: -1 });
  console.log('Attendances total:', attendances.length);
  for (const att of attendances) {
    console.log(`- Attendance: ${att.studentName} (${att.rollNumber}), Verified: ${att.verified}, Photo URL: ${att.photoUrl}, Captured: ${att.capturedAt}`);
  }

  await mongoose.disconnect();
}

checkDatabase();
