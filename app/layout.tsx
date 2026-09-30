import { RootProvider } from "fumadocs-ui/provider/next";
import type { Metadata } from "next";
import { DM_Sans, Geist_Mono, Outfit } from "next/font/google";

import "./globals.css";
import { SiteFooter } from "@/components/docs/site-footer";
import { ThemeProvider } from "@/components/theme-provider";
import { TooltipProvider } from "@/components/ui/tooltip";
import { siteConfig } from "@/lib/site";
import { getSocialMetadata } from "@/lib/social-metadata";
import { cn } from "@/lib/utils";

const outfitHeading = Outfit({
  subsets: ["latin"],
  variable: "--font-heading",
});

const dmSans = DM_Sans({ subsets: ["latin"], variable: "--font-sans" });

const fontMono = Geist_Mono({
  subsets: ["latin"],
  variable: "--font-mono",
});

export const metadata: Metadata = {
  ...getSocialMetadata(siteConfig.name),
  alternates: { canonical: "/" },
  description: siteConfig.description,
  metadataBase: new URL(siteConfig.url),
  title: {
    default: `${siteConfig.name} — audio components for shadcn/ui`,
    template: `%s — ${siteConfig.name}`,
  },
};

const RootLayout = ({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) => (
  <html
    lang="en"
    suppressHydrationWarning
    className={cn(
      "antialiased",
      fontMono.variable,
      "font-sans",
      dmSans.variable,
      outfitHeading.variable
    )}
  >
    <body className="flex min-h-svh flex-col">
      <ThemeProvider>
        <RootProvider theme={{ enabled: false }}>
          <TooltipProvider>
            <div className="flex min-h-0 flex-1 flex-col">{children}</div>
            <SiteFooter />
          </TooltipProvider>
        </RootProvider>
      </ThemeProvider>
    </body>
  </html>
);

export default RootLayout;
