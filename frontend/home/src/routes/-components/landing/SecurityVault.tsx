import { motion } from "framer-motion";
import { Cpu, Eye, KeyRound, Lock, ShieldCheck } from "lucide-react";

import { Section } from "./primitives/Section";

/* ---------- Section 10: Security vault ---------- */
export function SecurityVault() {
  return (
    <Section
      id="security"
      eyebrow="Security"
      title={
        <>
          Enter the <span className="text-gradient-accent">vault.</span>
        </>
      }
      subtitle="Zero-trust by design. Every verification is signed, timestamped and stored in a tamper-evident ledger."
    >
      <div className="relative grid lg:grid-cols-2 gap-8 items-center">
        <div className="relative aspect-square max-w-[520px] mx-auto w-full">
          <motion.div
            animate={{ rotate: 360 }}
            transition={{ duration: 40, repeat: Infinity, ease: "linear" }}
            className="absolute inset-0 rounded-full border border-white/[0.08]"
          />
          <motion.div
            animate={{ rotate: -360 }}
            transition={{ duration: 50, repeat: Infinity, ease: "linear" }}
            className="absolute inset-8 rounded-full border border-dashed border-white/[0.06]"
          />
          <motion.div
            animate={{ scale: [1, 1.04, 1] }}
            transition={{ duration: 4, repeat: Infinity }}
            className="absolute inset-16 rounded-full glass-strong grid place-items-center overflow-hidden"
          >
            <div className="absolute inset-0 bg-gradient-to-br from-electric/25 via-purple/10 to-cyan/25 blur-2xl" />
            <ShieldCheck className="relative h-24 w-24 text-white/90" strokeWidth={1.2} />
          </motion.div>
          {["SOC 2", "GDPR", "ISO 27001", "HIPAA", "FIDO2", "Zero Trust"].map((b, i) => {
            const a = (i / 6) * Math.PI * 2;
            const x = 50 + Math.cos(a) * 46;
            const y = 50 + Math.sin(a) * 46;
            return (
              <div
                key={b}
                className="absolute -translate-x-1/2 -translate-y-1/2"
                style={{ left: `${x}%`, top: `${y}%` }}
              >
                <div className="glass rounded-full px-3 py-1 text-[11px] tracking-wider">{b}</div>
              </div>
            );
          })}
        </div>
        <div className="space-y-3">
          {[
            {
              icon: Lock,
              t: "Encrypted end-to-end",
              d: "AES-256 at rest · TLS 1.3 in flight · HSM-backed keys.",
            },
            {
              icon: KeyRound,
              t: "Passkeys & FIDO2",
              d: "Phish-resistant auth, hardware-bound credentials.",
            },
            {
              icon: Eye,
              t: "Liveness & anti-spoof",
              d: "Depth, motion and reflection checks reject video replay.",
            },
            {
              icon: Cpu,
              t: "AI fraud engine",
              d: "23-signal scoring, tuned per site, learns weekly.",
            },
            {
              icon: ShieldCheck,
              t: "Tamper-evident ledger",
              d: "Every event is hash-chained and exportable.",
            },
          ].map((x) => (
            <div key={x.t} className="glass rounded-xl p-4 flex gap-4">
              <div className="h-10 w-10 rounded-lg glass-strong grid place-items-center shrink-0">
                <x.icon className="h-4 w-4 text-cyan" />
              </div>
              <div>
                <div className="font-medium">{x.t}</div>
                <div className="text-sm text-white/55 mt-0.5">{x.d}</div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </Section>
  );
}
