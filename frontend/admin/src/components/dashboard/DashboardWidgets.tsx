import React from 'react';
import { useNavigate } from 'react-router';
import { GraduationCap, ShieldCheck, Users, AlertTriangle, Info, ArrowRight, ArrowUp, ArrowDown, Minus, Cpu, Server, Container } from 'lucide-react';
import { motion } from 'framer-motion';
import type { Variants } from 'framer-motion';

export interface HealthComponent {
  name: string;
  healthy: boolean;
  score: number;
  weight: number;
  details: {
    modelLoaded?: boolean;
    modelPath?: string | null;
    modelSize?: number;
    express?: boolean;
    redis?: boolean;
    mongodb?: boolean;
    latency?: number;
    containers?: Array<{ name: string; healthy: boolean; status: string }>;
    healthyCount?: number;
    totalCount?: number;
    authEndpoint?: boolean;
    dashboardEndpoint?: boolean;
    adminCount?: number;
    error?: string | null;
  };
}

export interface IntegrityComponents {
  aiModel?: HealthComponent;
  backend?: HealthComponent;
  studentContainers?: HealthComponent;
  adminService?: HealthComponent;
}

export interface MetricPulse {
  value: number;
  target: number;
  delta: number;
  deltaType: 'up' | 'down' | 'right';
  status: 'On Track' | 'At Risk' | 'Critical';
  components?: IntegrityComponents;
}

export interface QuarantinePulse {
  count: number;
  status: 'On Track' | 'At Risk' | 'Critical';
}

export interface DashboardPulseData {
  eligibility: MetricPulse;
  integrity: MetricPulse;
  turnout: MetricPulse;
  quarantine: QuarantinePulse;
}

// ─── Color tokens driven by status ───────────────────────────────────────────

type StatusLevel = 'On Track' | 'At Risk' | 'Critical';

const STATUS_COLORS: Record<StatusLevel, {
  value: string;       // value number color
  badge_bg: string;    // badge background
  badge_text: string;  // badge text color
  badge_border: string;
  dot: string;         // pulsing dot color
  card_class: string;  // glass-card-* CSS class
  icon_bg: string;
  icon_border: string;
  icon_shadow: string;
  icon_color: string;
}> = {
  'On Track': {
    value:        'text-emerald-600 dark:text-[#32d583]',
    badge_bg:     'rgba(18, 183, 106, 0.15)',
    badge_text:   '#10b981',
    badge_border: '1px solid rgba(50, 213, 131, 0.2)',
    dot:          'bg-emerald-500 dark:bg-[#12b76a]',
    card_class:   'glass-card-green',
    icon_bg:      'rgba(18, 183, 106, 0.1)',
    icon_border:  '1px solid rgba(18, 183, 106, 0.3)',
    icon_shadow:  '0 0 12px rgba(18, 183, 106, 0.15)',
    icon_color:   'text-emerald-600 dark:text-[#32d583]',
  },
  'At Risk': {
    value:        'text-amber-500 dark:text-[#fdb022]',
    badge_bg:     'rgba(247, 144, 9, 0.15)',
    badge_text:   '#f59e0b',
    badge_border: '1px solid rgba(253, 176, 34, 0.2)',
    dot:          'bg-amber-500 dark:bg-[#f79009]',
    card_class:   'glass-card-orange',
    icon_bg:      'rgba(247, 144, 9, 0.1)',
    icon_border:  '1px solid rgba(247, 144, 9, 0.3)',
    icon_shadow:  '0 0 12px rgba(247, 144, 9, 0.15)',
    icon_color:   'text-amber-500 dark:text-[#fdb022]',
  },
  'Critical': {
    value:        'text-red-500 dark:text-[#f97066]',
    badge_bg:     'rgba(240, 68, 56, 0.15)',
    badge_text:   '#ef4444',
    badge_border: '1px solid rgba(249, 112, 102, 0.2)',
    dot:          'bg-red-500 dark:bg-[#f04438]',
    card_class:   'glass-card-red',
    icon_bg:      'rgba(240, 68, 56, 0.1)',
    icon_border:  '1px solid rgba(240, 68, 56, 0.3)',
    icon_shadow:  '0 0 12px rgba(240, 68, 56, 0.15)',
    icon_color:   'text-red-500 dark:text-[#f97066]',
  },
};

// Delta indicator helper
const DeltaIndicator: React.FC<{ delta: number; deltaType: 'up' | 'down' | 'right'; label: string }> = ({
  delta, deltaType, label
}) => {
  if (deltaType === 'up') {
    return (
      <span className="text-emerald-600 dark:text-[#32d583]" style={{ display: 'flex', alignItems: 'center', gap: '2px', fontSize: '12px', fontWeight: 600 }}>
        <ArrowUp size={12} />{delta}% {label}
      </span>
    );
  }
  if (deltaType === 'down') {
    return (
      <span className="text-red-500 dark:text-[#f87171]" style={{ display: 'flex', alignItems: 'center', gap: '2px', fontSize: '12px', fontWeight: 600 }}>
        <ArrowDown size={12} />{delta}% {label}
      </span>
    );
  }
  return (
    <span className="text-slate-700 dark:text-[#8b90b8]" style={{ display: 'flex', alignItems: 'center', gap: '2px', fontSize: '12px', fontWeight: 600 }}>
      <Minus size={12} />{delta}% {label}
    </span>
  );
};

// ─── Value → color level (matches bar gradient thresholds: 0–70 red, 70–85 amber, 85+ green)
const valueToColorLevel = (value: number): StatusLevel => {
  if (value >= 85) return 'On Track';
  if (value >= 70) return 'At Risk';
  return 'Critical';
};

// ─── Animation variants ───────────────────────────────────────────────────────

const containerVariants: Variants = {
  hidden: { opacity: 0 },
  show: { opacity: 1, transition: { staggerChildren: 0.1 } },
};

const cardVariants: Variants = {
  hidden: { opacity: 0, y: 20 },
  show: { opacity: 1, y: 0, transition: { type: 'spring', stiffness: 100, damping: 15 } },
};

// ─── Props ────────────────────────────────────────────────────────────────────

interface DashboardWidgetsProps {
  pulse: DashboardPulseData | null;
  loading: boolean;
}

// ─── Loading skeleton ─────────────────────────────────────────────────────────

const DashboardWidgets: React.FC<DashboardWidgetsProps> = ({ pulse, loading }) => {
  const navigate = useNavigate();

  if (loading) {
    return (
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-6 w-full">
        {[...Array(4)].map((_, i) => (
          <div
            key={i}
            className="glass-card animate-pulse"
            style={{ padding: '24px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between', height: '190px' }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%' }}>
              <div className="bg-slate-200 dark:bg-slate-800/50 rounded-full flex-shrink-0" style={{ width: '56px', height: '56px' }} />
              <div style={{ flex: 1, minWidth: 0 }} className="space-y-2">
                <div className="bg-slate-200 dark:bg-slate-800/50 rounded" style={{ height: '14px', width: '70%' }} />
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', marginTop: '4px' }}>
                  <div className="bg-slate-200 dark:bg-slate-800/50 rounded" style={{ height: '28px', width: '35%' }} />
                  <div className="bg-slate-200 dark:bg-slate-800/50 rounded-full" style={{ height: '18px', width: '25%' }} />
                </div>
              </div>
            </div>
            {i < 3 ? (
              <div style={{ marginTop: '16px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <div className="bg-slate-200 dark:bg-slate-800/50 rounded-full" style={{ height: '6px', width: '100%' }} />
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <div className="bg-slate-200 dark:bg-slate-800/50 rounded" style={{ height: '12px', width: '25%' }} />
                  <div className="bg-slate-200 dark:bg-slate-800/50 rounded" style={{ height: '12px', width: '25%' }} />
                </div>
              </div>
            ) : (
              <div style={{ marginTop: '12px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                <div className="bg-slate-200 dark:bg-slate-800/50 rounded" style={{ height: '12px', width: '50%' }} />
                <div className="bg-slate-200 dark:bg-slate-800/50 rounded-lg" style={{ height: '32px', width: '100%' }} />
              </div>
            )}
          </div>
        ))}
      </div>
    );
  }

  if (!pulse) return null;

  // Card visual theme driven by value position on the bar, not the coarser API status text
  const eligC  = STATUS_COLORS[valueToColorLevel(pulse.eligibility.value)];
  const intC   = STATUS_COLORS[valueToColorLevel(pulse.integrity.value)];
  const turnC  = STATUS_COLORS[valueToColorLevel(pulse.turnout.value)];
  // Quarantine is a count, not a %, so keep using the API status
  const quarC  = STATUS_COLORS[pulse.quarantine.status];

  return (
    <motion.div
      variants={containerVariants}
      initial="hidden"
      animate="show"
      className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-6 w-full"
    >
      {/* 1. Placement Eligibility Index */}
      <motion.div
        variants={cardVariants}
        className={`glass-card ${eligC.card_class}`}
        style={{ padding: '24px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between', height: '190px' }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%' }}>
          <div style={{
            width: '56px', height: '56px', borderRadius: '9999px',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            backgroundColor: eligC.icon_bg, border: eligC.icon_border,
            boxShadow: eligC.icon_shadow, flexShrink: 0,
          }}>
            <GraduationCap className={eligC.icon_color} size={26} />
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              <span className="text-slate-800 dark:text-[#8b90b8] text-[13px] font-semibold truncate">Placement Eligibility Index</span>
              <span className="widget-tooltip-wrapper">
                <Info size={13} className="text-slate-400 dark:text-[#5c6080] hover:text-slate-600 dark:hover:text-[#8b90b8] cursor-help" />
                <span className="widget-tooltip">Percentage of students qualifying for placement drives.</span>
              </span>
            </div>
            <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', marginTop: '4px', width: '100%' }}>
              <span className={`${eligC.value} text-3xl font-extrabold tracking-tight`}>{pulse.eligibility.value}%</span>
              <span style={{ padding: '2px 8px', borderRadius: '9999px', fontSize: '10px', fontWeight: 'bold', backgroundColor: eligC.badge_bg, color: eligC.badge_text, border: eligC.badge_border, display: 'flex', alignItems: 'center', gap: '4px', flexShrink: 0 }}>
                <span className={`w-1.5 h-1.5 rounded-full ${eligC.dot} status-dot-blink`} />
                {pulse.eligibility.status}
              </span>
            </div>
          </div>
        </div>
        <div style={{ marginTop: '16px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <div className="widget-progress-track">
            <div className="widget-progress-thumb" style={{ left: `${Math.min(Math.max(pulse.eligibility.value, 0), 100)}%` }} />
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span className="text-slate-700 dark:text-[#8b90b8] text-xs font-semibold">Target: &ge; {pulse.eligibility.target}%</span>
            <DeltaIndicator delta={pulse.eligibility.delta} deltaType={pulse.eligibility.deltaType} label="vs last week" />
          </div>
        </div>
      </motion.div>

      {/* 2. System Integrity Score */}
      <motion.div
        variants={cardVariants}
        className={`glass-card ${intC.card_class}`}
        style={{ padding: '24px', display: 'flex', flexDirection: 'column', height: '190px', position: 'relative' }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%' }}>
          <div style={{
            width: '56px', height: '56px', borderRadius: '9999px',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            backgroundColor: intC.icon_bg, border: intC.icon_border,
            boxShadow: intC.icon_shadow, flexShrink: 0,
          }}>
            <ShieldCheck className={intC.icon_color} size={26} />
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              <span className="text-slate-800 dark:text-[#8b90b8] text-[13px] font-semibold truncate">System Integrity Score</span>
              <span className="widget-tooltip-wrapper">
                <Info size={13} className="text-slate-400 dark:text-[#5c6080] hover:text-slate-600 dark:hover:text-[#8b90b8] cursor-help" />
                <span className="widget-tooltip">Measures session security and data legitimacy.</span>
              </span>
            </div>
          </div>
        </div>

        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', justifyContent: 'center', paddingBottom: '8px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
            <span className={`${intC.value} text-[56px] font-black tracking-tighter leading-none drop-shadow-sm`}>{pulse.integrity.value}%</span>
            <span style={{ padding: '4px 12px', borderRadius: '9999px', fontSize: '11px', fontWeight: 'bold', backgroundColor: intC.badge_bg, color: intC.badge_text, border: intC.badge_border, display: 'flex', alignItems: 'center', gap: '6px', flexShrink: 0, transform: 'translateY(4px)' }}>
              <span className={`w-2 h-2 rounded-full ${intC.dot} status-dot-blink`} />
              {pulse.integrity.value >= 85 ? 'Healthy' : pulse.integrity.value >= 70 ? 'Degraded' : 'Critical'}
            </span>
          </div>
        </div>

        {/* Triangle Icon Cluster in the bottom-right */}
        <div className="absolute bottom-6 right-6 w-[42px] h-[42px]">
          {/* Top Center Icon (AI Model) */}
          <div className="absolute top-0 left-1/2 -translate-x-1/2 w-[22px] h-[22px] rounded-md bg-white/60 dark:bg-slate-800/80 border border-slate-200/50 dark:border-white/10 shadow-sm flex items-center justify-center backdrop-blur-md" title={`AI Model: ${pulse.integrity.components?.aiModel?.score || 0}/25`}>
            <Cpu size={12} className={pulse.integrity.components?.aiModel?.healthy !== false ? 'text-emerald-600 dark:text-[#32d583]' : 'text-red-600 dark:text-[#f97066]'} />
          </div>
          {/* Bottom Left Icon (Backend) */}
          <div className="absolute bottom-0 left-0 w-[22px] h-[22px] rounded-md bg-white/60 dark:bg-slate-800/80 border border-slate-200/50 dark:border-white/10 shadow-sm flex items-center justify-center backdrop-blur-md" title={`Backend: ${pulse.integrity.components?.backend?.score || 0}/25`}>
            <Server size={12} className={pulse.integrity.components?.backend?.healthy !== false ? 'text-emerald-600 dark:text-[#32d583]' : 'text-red-600 dark:text-[#f97066]'} />
          </div>
          {/* Bottom Right Icon (Containers) */}
          <div className="absolute bottom-0 right-0 w-[22px] h-[22px] rounded-md bg-white/60 dark:bg-slate-800/80 border border-slate-200/50 dark:border-white/10 shadow-sm flex items-center justify-center backdrop-blur-md" title={`Containers: ${pulse.integrity.components?.studentContainers?.score || 0}/25`}>
            <Container size={12} className={pulse.integrity.components?.studentContainers?.healthy !== false ? 'text-emerald-600 dark:text-[#32d583]' : 'text-red-600 dark:text-[#f97066]'} />
          </div>
        </div>
      </motion.div>

      {/* 3. Active Turnout Rate */}
      <motion.div
        variants={cardVariants}
        className={`glass-card ${turnC.card_class}`}
        style={{ padding: '24px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between', height: '190px' }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%' }}>
          <div style={{
            width: '56px', height: '56px', borderRadius: '9999px',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            backgroundColor: turnC.icon_bg, border: turnC.icon_border,
            boxShadow: turnC.icon_shadow, flexShrink: 0,
          }}>
            <Users className={turnC.icon_color} size={26} />
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              <span className="text-slate-800 dark:text-[#8b90b8] text-[13px] font-semibold truncate">Active Turnout Rate (Today)</span>
              <span className="widget-tooltip-wrapper">
                <Info size={13} className="text-slate-400 dark:text-[#5c6080] hover:text-slate-600 dark:hover:text-[#8b90b8] cursor-help" />
                <span className="widget-tooltip">Percentage of students present today across batches.</span>
              </span>
            </div>
            <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', marginTop: '4px', width: '100%' }}>
              <span className={`${turnC.value} text-3xl font-extrabold tracking-tight`}>{pulse.turnout.value}%</span>
              <span style={{ padding: '2px 8px', borderRadius: '9999px', fontSize: '10px', fontWeight: 'bold', backgroundColor: turnC.badge_bg, color: turnC.badge_text, border: turnC.badge_border, display: 'flex', alignItems: 'center', gap: '4px', flexShrink: 0 }}>
                <span className={`w-1.5 h-1.5 rounded-full ${turnC.dot} status-dot-blink`} />
                {pulse.turnout.status}
              </span>
            </div>
          </div>
        </div>
        <div style={{ marginTop: '16px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <div className="widget-progress-track">
            <div className="widget-progress-thumb" style={{ left: `${Math.min(Math.max(pulse.turnout.value, 0), 100)}%` }} />
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span className="text-slate-700 dark:text-[#8b90b8] text-xs font-semibold">Target: &ge; {pulse.turnout.target}%</span>
            <DeltaIndicator delta={pulse.turnout.delta} deltaType={pulse.turnout.deltaType} label="vs yesterday" />
          </div>
        </div>
      </motion.div>

      {/* 4. Security Quarantine Count */}
      <motion.div
        variants={cardVariants}
        className={`glass-card ${quarC.card_class}`}
        style={{ padding: '24px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between', height: '190px' }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '16px', width: '100%' }}>
          <div style={{
            width: '56px', height: '56px', borderRadius: '9999px',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            backgroundColor: quarC.icon_bg, border: quarC.icon_border,
            boxShadow: quarC.icon_shadow, flexShrink: 0,
          }}>
            <AlertTriangle className={quarC.icon_color} size={26} />
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              <span className="text-slate-800 dark:text-[#8b90b8] text-[13px] font-semibold truncate">Security Quarantine Count</span>
              <span className="widget-tooltip-wrapper">
                <Info size={13} className="text-slate-400 dark:text-[#5c6080] hover:text-slate-600 dark:hover:text-[#8b90b8] cursor-help" />
                <span className="widget-tooltip">Records flagged for geo-violations or multi-student devices.</span>
              </span>
            </div>
            <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', marginTop: '4px', width: '100%' }}>
              <span className={`${quarC.value} text-3xl font-extrabold tracking-tight`}>{pulse.quarantine.count}</span>
              <span style={{ padding: '2px 8px', borderRadius: '9999px', fontSize: '10px', fontWeight: 'bold', backgroundColor: quarC.badge_bg, color: quarC.badge_text, border: quarC.badge_border, display: 'flex', alignItems: 'center', gap: '4px', flexShrink: 0 }}>
                <span className={`w-1.5 h-1.5 rounded-full ${quarC.dot} status-dot-blink`} />
                {pulse.quarantine.status}
              </span>
            </div>
          </div>
        </div>
        <div style={{ marginTop: '12px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
          <p className="text-slate-700 dark:text-[#8b90b8] text-[11px] font-semibold">Unreviewed critical flags</p>
          <button
            onClick={() => navigate('/flagged')}
            className="w-full py-1.5 bg-transparent hover:bg-red-50 dark:hover:bg-[#f04438]/10 border border-red-500 text-red-600 dark:text-[#f97066] hover:text-red-700 dark:hover:text-white rounded-lg text-xs font-bold transition-all duration-300 flex items-center justify-center gap-1.5 cursor-pointer"
          >
            Review Now
            <ArrowRight size={14} />
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
};

export default DashboardWidgets;
