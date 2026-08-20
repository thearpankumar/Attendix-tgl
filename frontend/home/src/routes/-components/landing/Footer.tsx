/* ---------- Footer ---------- */
export function Footer() {
  return (
    <footer className="relative border-t border-white/10 bg-[#04050890]">
      <div className="max-w-7xl mx-auto px-6 py-10 flex flex-col sm:flex-row items-center justify-between gap-4 text-center sm:text-left">
        <div className="flex items-center gap-2">
          <div className="relative h-7 w-7 rounded-lg bg-gradient-to-br from-cyan via-electric to-purple grid place-items-center">
            <div className="absolute inset-[3px] rounded-md bg-[#06070A] grid place-items-center">
              <span className="text-[10px] font-bold text-gradient-accent">AX</span>
            </div>
          </div>
          <div className="leading-tight">
            <div className="font-display font-semibold">Attendix</div>
            <div className="text-xs text-white/40">by TalenciaGlobal</div>
          </div>
        </div>
        <a
          href="mailto:hello@talenciaglobal.com"
          className="text-sm text-white/60 hover:text-white transition"
        >
          hello@talenciaglobal.com
        </a>
        <div className="text-xs text-white/40">
          © {new Date().getFullYear()} TalenciaGlobal. All rights reserved.
        </div>
      </div>
    </footer>
  );
}
