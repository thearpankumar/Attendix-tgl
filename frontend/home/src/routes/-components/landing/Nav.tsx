import { motion } from "framer-motion";
import { Download } from "lucide-react";
import { useEffect, useState } from "react";

import { MENTOR_APK_URL } from "./constants";

/* ---------- Nav ---------- */
export function Nav() {
  const [scrolled, setScrolled] = useState(false);
  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 20);
    window.addEventListener("scroll", onScroll);
    return () => window.removeEventListener("scroll", onScroll);
  }, []);
  return (
    <motion.header
      initial={{ y: -40, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ duration: 0.8, ease: [0.22, 1, 0.36, 1] }}
      className="fixed top-4 left-1/2 -translate-x-1/2 z-50 w-[min(1120px,calc(100%-2rem))]"
    >
      <div
        className={`glass rounded-full pl-5 pr-5 py-2 flex items-center justify-between transition-all duration-500 ${scrolled ? "backdrop-blur-2xl" : ""}`}
      >
        <a href="#top" className="flex items-center gap-2">
          <div className="relative h-7 w-7 rounded-lg bg-gradient-to-br from-cyan via-electric to-purple grid place-items-center">
            <div className="absolute inset-[3px] rounded-md bg-[#06070A] grid place-items-center">
              <span className="text-[10px] font-bold tracking-tighter text-gradient-accent">
                AX
              </span>
            </div>
          </div>
          <span className="font-display font-semibold tracking-tight">Attendix</span>
          <span className="hidden sm:inline text-xs text-muted-foreground ml-1">
            by TalenciaGlobal
          </span>
        </a>
        <nav className="hidden md:flex items-center gap-7 text-sm text-white/70">
          {["Platform", "Security", "Industries", "Enterprise", "Pricing"].map((l) => (
            <a key={l} href={`#${l.toLowerCase()}`} className="hover:text-white transition-colors">
              {l}
            </a>
          ))}
        </nav>
        <a
          href={MENTOR_APK_URL}
          className="flex items-center gap-1.5 rounded-full bg-white/10 hover:bg-white/20 transition-colors px-4 py-1.5 text-xs font-medium text-white"
        >
          <Download className="h-3.5 w-3.5" />
          <span className="hidden sm:inline">Download Mentor App</span>
          <span className="sm:hidden">App</span>
        </a>
      </div>
    </motion.header>
  );
}
