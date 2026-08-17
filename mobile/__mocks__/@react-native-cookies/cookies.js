module.exports = {
  __esModule: true,
  default: {
    clearAll: jest.fn().mockResolvedValue(true),
    get: jest.fn().mockResolvedValue({}),
    set: jest.fn().mockResolvedValue(true),
    clearByName: jest.fn().mockResolvedValue(true),
  },
};
