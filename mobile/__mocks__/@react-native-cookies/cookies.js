module.exports = {
  __esModule: true,
  default: {
    clearAll: jest.fn().mockResolvedValue(true),
    flush: jest.fn().mockResolvedValue(undefined),
    get: jest.fn().mockResolvedValue({}),
    set: jest.fn().mockResolvedValue(true),
    clearByName: jest.fn().mockResolvedValue(true),
  },
};
