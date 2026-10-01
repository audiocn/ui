import { Analytics } from "@vercel/analytics/next";
import { RootProvider } from "fumadocs-ui/provider/next";
import type { Metadata } from "next";
import { DM_Sans, Geist_Mono, Outfit } from "next/font/google";

import "./globals.css";
import {
  SearchDialog,
  SearchGroupsProvider,
} from "@/components/docs/search-dialog";
import { SiteFooter } from "@/components/docs/site-footer";
import { ThemeProvider } from "@/components/theme-provider";
import { TooltipProvider } from "@/components/ui/tooltip";
import { searchGroups } from "@/lib/search-groups";
import { siteConfig } from "@/lib/site";
import socialImages from "@/lib/social-images.json";
import { getPageMetadata } from "@/lib/social-metadata";
import { source } from "@/lib/source";
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

const homeMetadata = getPageMetadata({ pathname: "/", title: siteConfig.name });
const isSocialCapture =
  process.env.NODE_ENV === "development" &&
  process.env.AUDIOCN_SOCIAL_CAPTURE === "1";

export const metadata: Metadata = {
  ...homeMetadata,
  metadataBase: new URL(siteConfig.url),
  openGraph: {
    ...homeMetadata.openGraph,
    images: [{ ...socialImages["/"], height: 630, width: 1200 }],
  },
  title: {
    default: siteConfig.title,
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
        <SearchGroupsProvider groups={searchGroups(source.getPageTree())}>
          <RootProvider
            search={{ SearchDialog, enabled: !isSocialCapture }}
            theme={{ enabled: false }}
          >
            <TooltipProvider>
              <div className="flex min-h-0 flex-1 flex-col">{children}</div>
              <SiteFooter />
            </TooltipProvider>
          </RootProvider>
        </SearchGroupsProvider>
      </ThemeProvider>
      <Analytics />
    </body>
  </html>
);

export default RootLayout;
