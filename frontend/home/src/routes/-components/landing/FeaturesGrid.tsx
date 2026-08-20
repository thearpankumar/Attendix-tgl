import { motion } from "framer-motion";
import {
  Activity,
  AlertTriangle,
  ArrowRight,
  Fingerprint,
  KeyRound,
  LineChart,
  MapPin,
  ScanFace,
  ShieldCheck,
} from "lucide-react";

import { Section } from "./primitives/Section";
import { TiltCard } from "./primitives/TiltCard";

/* ---------- Section 5: Features ---------- */
export function FeaturesGrid() {
  const feats = [
    {
      icon: ScanFace,
      title: "Face Recognition",
      desc: "3D depth-mapped identity match with anti-spoof.",
      size: "lg:col-span-2 lg:row-span-2",
    },
    { icon: Fingerprint, title: "Biometrics", desc: "Native TouchID, FaceID and Windows Hello." },
    { icon: MapPin, title: "Geo-location", desc: "Geofence + IP + WiFi triangulation." },
    { icon: KeyRound, title: "Passkeys", desc: "FIDO2 signed check-ins. Phish-proof." },
    {
      icon: LineChart,
      title: "Workforce Analytics",
      desc: "Live dashboards, cohort trends, exports.",
      size: "lg:col-span-2",
    },
    { icon: Activity, title: "Live Monitoring", desc: "Real-time attendance & anomaly stream." },
    { icon: AlertTriangle, title: "Fraud Alerts", desc: "23-signal scoring, tuned per site." },
    { icon: ShieldCheck, title: "Admin Console", desc: "Roles, audit trail, SSO, SCIM." },
  ];
  return (
    <Section
      id="features"
      eyebrow="Platform"
      title={
        <>
          Everything the <span className="text-gradient-accent">modern workforce</span> needs.
        </>
      }
    >
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 auto-rows-[220px]">
        {feats.map((f, i) => (
          <TiltCard key={f.title} className={f.size}>
            <motion.div
              initial={{ opacity: 0, y: 20 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ delay: i * 0.05, duration: 0.6 }}
              className="relative h-full w-full glass rounded-2xl p-6 overflow-hidden group"
            >
              <div className="absolute -inset-px rounded-2xl opacity-0 group-hover:opacity-100 transition duration-500 bg-gradient-to-br from-cyan/10 via-transparent to-purple/10 pointer-events-none" />
              <div className="relative flex items-start justify-between">
                <div className="h-11 w-11 rounded-xl glass-strong grid place-items-center">
                  <f.icon className="h-5 w-5 text-cyan" />
                </div>
                <ArrowRight className="h-4 w-4 text-white/30 group-hover:text-white group-hover:translate-x-1 transition" />
              </div>
              <div className="absolute bottom-6 left-6 right-6">
                <div className="font-display text-xl">{f.title}</div>
                <div className="mt-1.5 text-sm text-white/55">{f.desc}</div>
              </div>
            </motion.div>
          </TiltCard>
        ))}
      </div>
    </Section>
  );
}
