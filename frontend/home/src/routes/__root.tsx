import { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext } from "@tanstack/react-router";

import appCss from "../styles.css?url";
import { ErrorComponent } from "./-components/root/ErrorComponent";
import { NotFoundComponent } from "./-components/root/NotFoundComponent";
import { RootComponent } from "./-components/root/RootComponent";
import { RootShell } from "./-components/root/RootShell";

export const Route = createRootRouteWithContext<{ queryClient: QueryClient }>()({
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { title: "Attendix — Attendance. Verified. Not Assumed." },
      {
        name: "description",
        content:
          "AI-powered attendance intelligence by TalenciaGlobal. Face recognition, biometrics, passkeys and geo-location — one verified identity, every time.",
      },
      { name: "author", content: "TalenciaGlobal" },
      { name: "theme-color", content: "#06070A" },
      { property: "og:title", content: "Attendix — Attendance. Verified. Not Assumed." },
      {
        property: "og:description",
        content: "The future of workforce attendance. AI-verified identity, location and device.",
      },
      { property: "og:type", content: "website" },
      { name: "twitter:card", content: "summary_large_image" },
      { name: "twitter:site", content: "@TalenciaGlobal" },
    ],
    links: [
      { rel: "stylesheet", href: appCss },
      { rel: "icon", href: "/favicon.ico", type: "image/x-icon" },
      { rel: "preconnect", href: "https://fonts.googleapis.com" },
      { rel: "preconnect", href: "https://fonts.gstatic.com", crossOrigin: "anonymous" },
      {
        rel: "stylesheet",
        href: "https://fonts.googleapis.com/css2?family=Inter+Tight:wght@300;400;500;600;700;800&display=swap",
      },
    ],
  }),
  shellComponent: RootShell,
  component: RootComponent,
  notFoundComponent: NotFoundComponent,
  errorComponent: ErrorComponent,
});
