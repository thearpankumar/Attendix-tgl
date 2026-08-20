import { motion } from "framer-motion";
import { ArrowRight, Download } from "lucide-react";

import { MENTOR_APK_URL } from "./constants";
import { MagneticButton } from "./primitives/MagneticButton";
import { Particles } from "./primitives/Particles";

/* ---------- Section 13: Final CTA ---------- */
export function FinalCTA() {
  return (
    <section id="demo" className="relative py-40 px-6 overflow-hidden">
      <div className="absolute inset-0 bg-mesh opacity-90" />
      <div className="absolute inset-0 grid-lines opacity-40" />
      <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 h-[800px] w-[800px] rounded-full bg-gradient-to-br from-cyan/20 via-electric/15 to-purple/25 blur-3xl animate-pulse-glow" />
      <Particles count={40} />
      <div className="relative max-w-5xl mx-auto text-center">
        <motion.div
          initial={{ opacity: 0, y: 40 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 1, ease: [0.22, 1, 0.36, 1] }}
        >
          <div className="text-xs uppercase tracking-[0.3em] text-white/40">
            The future of attendance
          </div>
          <h2 className="mt-6 font-display text-[clamp(2.5rem,7vw,6rem)] leading-[0.95] tracking-[-0.04em]">
            <span className="text-gradient">Attendance you can trust.</span>
            <br />
            <span className="text-gradient-accent">Every single time.</span>
          </h2>
          <p className="mt-8 max-w-xl mx-auto text-white/60">
            See Attendix live with your data. Enterprise pilots start in as little as one week.
          </p>
          <div className="mt-10 flex flex-wrap items-center justify-center gap-3">
            <MagneticButton href="mailto:hello@talenciaglobal.com" primary>
              Book a demo <ArrowRight className="h-4 w-4" />
            </MagneticButton>
            <MagneticButton href="mailto:sales@talenciaglobal.com">
              Request enterprise pricing
            </MagneticButton>
            <MagneticButton href={MENTOR_APK_URL}>
              <Download className="h-4 w-4" /> Download mentor app (Android)
            </MagneticButton>
          </div>
        </motion.div>
      </div>
    </section>
  );
}
