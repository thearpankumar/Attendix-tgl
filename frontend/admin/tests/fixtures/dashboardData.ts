import type { DashboardPulseData, IntegrityComponents } from '../../src/components/dashboard/DashboardWidgets';
import type { ChartFunnel, ChartIntegrityBreakdown, WeeklyTrendPoint } from '../../src/components/dashboard/DashboardCharts';
import type { WorklistsData } from '../../src/components/dashboard/DashboardTables';

export interface SystemHealthData {
  score: number;
  status: string;
  healthStatus: string;
  components: IntegrityComponents;
  summary: {
    healthyComponents: number;
    totalComponents: number;
    maxScore: number;
    weightedScore: number;
  };
  lastChecked: string;
}

export interface DashboardData {
  pulse: DashboardPulseData;
  charts: {
    funnel: ChartFunnel;
    integrityBreakdown: ChartIntegrityBreakdown;
    systemHealth?: SystemHealthData;
    weeklyTrends: WeeklyTrendPoint[];
  };
  worklists: WorklistsData;
  lastUpdated: string;
}

export interface FilterOptions {
  batches: { value: string; label: string }[];
  centers: { value: string; label: string }[];
  timeframes: string[];
  riskLevels: string[];
}

export const mockDashboardData: DashboardData = {
  pulse: {
    eligibility: { value: 85, target: 90, delta: 5, deltaType: 'up', status: 'On Track' },
    integrity: { 
      value: 100, 
      target: 100, 
      delta: 0, 
      deltaType: 'right', 
      status: 'On Track',
      components: {
        aiModel: { name: 'AI Model', healthy: true, score: 25, weight: 25, details: { modelLoaded: true, modelSize: 500000, latency: 50 } },
        backend: { name: 'Backend Service', healthy: true, score: 25, weight: 25, details: { express: true, redis: true, mongodb: true, latency: 10 } },
        studentContainers: { name: 'Student Containers', healthy: true, score: 25, weight: 25, details: { healthyCount: 1, totalCount: 1, containers: [{ name: 'student-frontend', healthy: true, status: 'running' }] } },
        adminService: { name: 'Admin Service', healthy: true, score: 25, weight: 25, details: { authEndpoint: true, dashboardEndpoint: true, adminCount: 1 } }
      }
    },
    turnout: { value: 78, target: 85, delta: -3, deltaType: 'down', status: 'At Risk' },
    quarantine: { count: 12, status: 'At Risk' }
  },
  charts: {
    funnel: {
      total: 1000,
      onTrack: { count: 700, percentage: 70 },
      atRisk: { count: 200, percentage: 20 },
      disqualified: { count: 100, percentage: 10 }
    },
    integrityBreakdown: {
      totalCheckins: 5000,
      flaggedAnomalies: 150,
      score: 97,
      flags: {
        gpsViolations: { count: 100, percentage: 2 },
        deviceAnomalies: { count: 50, percentage: 1 }
      }
    },
    systemHealth: {
      score: 100,
      status: 'On Track',
      healthStatus: 'healthy',
      components: {
        aiModel: { name: 'AI Model', healthy: true, score: 25, weight: 25, details: {} },
        backend: { name: 'Backend Service', healthy: true, score: 25, weight: 25, details: {} },
        studentContainers: { name: 'Student Containers', healthy: true, score: 25, weight: 25, details: {} },
        adminService: { name: 'Admin Service', healthy: true, score: 25, weight: 25, details: {} }
      },
      summary: { healthyComponents: 4, totalComponents: 4, maxScore: 100, weightedScore: 100 },
      lastChecked: '2026-07-16T10:30:00Z'
    },
    weeklyTrends: [
      { date: 'Jul 10', day: 'Thu 10', rate: 85 },
      { date: 'Jul 11', day: 'Fri 11', rate: 82 },
      { date: 'Jul 12', day: 'Sat 12', rate: 78 },
      { date: 'Jul 13', day: 'Sun 13', rate: 75 },
      { date: 'Jul 14', day: 'Mon 14', rate: 80 },
      { date: 'Jul 15', day: 'Tue 15', rate: 88 },
      { date: 'Jul 16', day: 'Wed 16', rate: 90 }
    ]
  },
  worklists: {
    rescueList: [
      { rollNo: 'STU001', name: 'John Doe', batch: 'Batch A', attendance: 68, trend: 'down' },
      { rollNo: 'STU002', name: 'Jane Smith', batch: 'Batch B', attendance: 72, trend: 'right' },
      { rollNo: 'STU003', name: 'Bob Wilson', batch: 'Batch A', attendance: 80, trend: 'up' }
    ],
    rescueCount: 3,
    highBatches: [
      { name: 'Batch E', center: 'Center Alpha', trainer: 'Trainer X', attendance: 96 },
      { name: 'Batch F', center: 'Center Beta', trainer: 'Trainer Y', attendance: 92 }
    ],
    highBatchesCount: 2,
    lowBatches: [
      { name: 'Batch C', center: 'Center Alpha', trainer: 'Trainer X', attendance: 72 },
      { name: 'Batch D', center: 'Center Beta', trainer: 'Trainer Y', attendance: 65 }
    ],
    lowBatchesCount: 2
  },
  lastUpdated: '2026-07-16T10:30:00Z'
};

export const mockFilterOptions: FilterOptions = {
  batches: [
    { value: 'all', label: 'All Batches' },
    { value: 'batch1', label: 'React Native Advanced' },
    { value: 'batch2', label: 'Java Backend Web' },
    { value: 'batch3', label: 'UI/UX Design Bootcamp' }
  ],
  centers: [
    { value: 'all', label: 'All Centers' },
    { value: 'center1', label: 'Center Alpha - Tech Hub' },
    { value: 'center2', label: 'Center Beta - Innovation Lab' }
  ],
  timeframes: [
    'This Week (Jul 10 - Jul 16)',
    'Today (Jul 16)',
    'Yesterday (Jul 15)',
    'This Month (July)'
  ],
  riskLevels: ['All Levels', 'High Risk', 'Medium Risk', 'Low Risk']
};

export const mockEmptyDashboardData: DashboardData = {
  pulse: {
    eligibility: { value: 0, target: 90, delta: 0, deltaType: 'right', status: 'At Risk' },
    integrity: { 
      value: 100, 
      target: 100, 
      delta: 0, 
      deltaType: 'right', 
      status: 'On Track',
      components: {
        aiModel: { name: 'AI Model', healthy: true, score: 25, weight: 25, details: { modelLoaded: true } },
        backend: { name: 'Backend Service', healthy: true, score: 25, weight: 25, details: { express: true, redis: true, mongodb: true } },
        studentContainers: { name: 'Student Containers', healthy: true, score: 25, weight: 25, details: { healthyCount: 1, totalCount: 1 } },
        adminService: { name: 'Admin Service', healthy: true, score: 25, weight: 25, details: { authEndpoint: true, dashboardEndpoint: true } }
      }
    },
    turnout: { value: 0, target: 85, delta: 0, deltaType: 'right', status: 'At Risk' },
    quarantine: { count: 0, status: 'On Track' }
  },
  charts: {
    funnel: { total: 0, onTrack: { count: 0, percentage: 0 }, atRisk: { count: 0, percentage: 0 }, disqualified: { count: 0, percentage: 0 } },
    integrityBreakdown: { totalCheckins: 0, flaggedAnomalies: 0, score: 100, flags: { gpsViolations: { count: 0, percentage: 0 }, deviceAnomalies: { count: 0, percentage: 0 } } },
    systemHealth: {
      score: 100,
      status: 'On Track',
      healthStatus: 'healthy',
      components: {},
      summary: { healthyComponents: 4, totalComponents: 4, maxScore: 100, weightedScore: 100 },
      lastChecked: '2026-07-16T10:30:00Z'
    },
    weeklyTrends: []
  },
  worklists: {
    rescueList: [],
    rescueCount: 0,
    highBatches: [],
    highBatchesCount: 0,
    lowBatches: [],
    lowBatchesCount: 0
  },
  lastUpdated: '2026-07-16T10:30:00Z'
};

export const mockXSSDashboardData: DashboardData = {
  pulse: {
    eligibility: { value: 85, target: 90, delta: 5, deltaType: 'up', status: 'On Track' },
    integrity: { value: 92, target: 95, delta: 2, deltaType: 'up', status: 'On Track' },
    turnout: { value: 78, target: 85, delta: -3, deltaType: 'down', status: 'At Risk' },
    quarantine: { count: 1, status: 'At Risk' }
  },
  charts: {
    funnel: { total: 1, onTrack: { count: 1, percentage: 100 }, atRisk: { count: 0, percentage: 0 }, disqualified: { count: 0, percentage: 0 } },
    integrityBreakdown: { totalCheckins: 1, flaggedAnomalies: 0, score: 100, flags: { gpsViolations: { count: 0, percentage: 0 }, deviceAnomalies: { count: 0, percentage: 0 } } },
    weeklyTrends: [{ date: 'Jul 16', day: 'Wed 16', rate: 85 }]
  },
  worklists: {
    rescueList: [
      { rollNo: 'STU001', name: '<script>alert("xss")</script>', batch: '<img onerror="alert(1)" src=x>', attendance: 68, trend: 'down' }
    ],
    rescueCount: 1,
    highBatches: [
      { name: '<img src=x onerror="alert(1)">', center: 'Test Center', trainer: 'Trainer', attendance: 95 }
    ],
    highBatchesCount: 1,
    lowBatches: [
      { name: '<script>document.cookie</script>', center: 'Test Center', trainer: 'Trainer', attendance: 72 }
    ],
    lowBatchesCount: 1
  },
  lastUpdated: '2026-07-16T10:30:00Z'
};

export const mockBoundaryDashboardData: DashboardData = {
  pulse: {
    eligibility: { value: 84, target: 90, delta: 0, deltaType: 'right', status: 'At Risk' },
    integrity: { value: 85, target: 95, delta: 1, deltaType: 'up', status: 'On Track' },
    turnout: { value: 70, target: 85, delta: 0, deltaType: 'right', status: 'Critical' },
    quarantine: { count: 0, status: 'On Track' }
  },
  charts: {
    funnel: { total: 100, onTrack: { count: 50, percentage: 50 }, atRisk: { count: 30, percentage: 30 }, disqualified: { count: 20, percentage: 20 } },
    integrityBreakdown: { totalCheckins: 1000, flaggedAnomalies: 0, score: 100, flags: { gpsViolations: { count: 0, percentage: 0 }, deviceAnomalies: { count: 0, percentage: 0 } } },
    weeklyTrends: [{ date: 'Jul 16', day: 'Wed 16', rate: 69 }]
  },
  worklists: {
    rescueList: [
      { rollNo: 'STU001', name: 'Boundary Student', batch: 'Test Batch', attendance: 74, trend: 'right' }
    ],
    rescueCount: 1,
    highBatches: [],
    highBatchesCount: 0,
    lowBatches: [],
    lowBatchesCount: 0
  },
  lastUpdated: '2026-07-16T10:30:00Z'
};
