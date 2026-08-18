type GreetingPeriod = 'morning' | 'afternoon' | 'night';

const greetingWord = () => {
  const hour = new Date().getHours();
  if (hour < 12) return 'Good morning';
  if (hour < 17) return 'Good afternoon';
  return 'Good evening';
};

// Same hour boundaries as greetingWord — "evening" shares the "night" visual
// treatment since both cover the same after-17:00 stretch of the day.
const greetingPeriod = (): GreetingPeriod => {
  const hour = new Date().getHours();
  if (hour < 12) return 'morning';
  if (hour < 17) return 'afternoon';
  return 'night';
};

// Gradient stands in for a time-of-day sky photo — no image to fetch/host,
// crisp at any size, and each pair starts with a mid/dark tone at the
// top-left (where the greeting text sits) so white text stays legible
// without relying solely on the text shadow below.
const PALETTES: Record<GreetingPeriod, [string, string]> = {
  morning: ['#e2711d', '#4568dc'],
  afternoon: ['#1e6f8e', '#4fb3d9'],
  night: ['#0f0c29', '#33296b'],
};

const SkyArt = ({ period }: { period: GreetingPeriod }) => {
  if (period === 'night') {
    const stars: [number, number][] = [
      [28, 18], [52, 44], [88, 14], [118, 52], [18, 66], [66, 78], [140, 30],
    ];
    return (
      <svg viewBox="0 0 200 100" preserveAspectRatio="xMaxYMid slice" className="greeting-banner-art">
        <circle cx={158} cy={28} r={16} fill="#fdf3d0" opacity={0.95} />
        <circle cx={151} cy={22} r={13} fill={PALETTES.night[1]} />
        {stars.map(([cx, cy], i) => (
          <circle key={i} cx={cx} cy={cy} r={1.4} fill="#ffffff" opacity={0.75} />
        ))}
      </svg>
    );
  }

  // Morning: low sun, still rising. Afternoon: high sun, full glow.
  const sunY = period === 'morning' ? 72 : 26;
  return (
    <svg viewBox="0 0 200 100" preserveAspectRatio="xMaxYMid slice" className="greeting-banner-art">
      <circle cx={162} cy={sunY} r={26} fill="#ffffff" opacity={0.18} />
      <circle cx={162} cy={sunY} r={15} fill="#fff7e0" opacity={0.65} />
    </svg>
  );
};

/// Dashboard's greeting card — time-of-day gradient + sky art behind a
/// simple "Good morning/afternoon/evening" (no name: the Topbar already
/// shows who's logged in, right below the Attendix logo).
const GreetingBanner = () => {
  const period = greetingPeriod();
  const [from, to] = PALETTES[period];

  return (
    <div className="greeting-banner" style={{ background: `linear-gradient(135deg, ${from}, ${to})` }}>
      <SkyArt period={period} />
      <h1 className="greeting-banner-title">{greetingWord()} 👋</h1>
      <p className="greeting-banner-sub">Here&apos;s what&apos;s happening today</p>
    </div>
  );
};

export default GreetingBanner;
