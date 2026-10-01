import type { Metadata } from "next";
import { notFound } from "next/navigation";

import { SocialCard } from "@/components/social/social-card";
import { getSocialCards } from "@/lib/social-catalog";
import { source } from "@/lib/source";

export const metadata: Metadata = {
  robots: { follow: false, index: false },
  title: "Social image capture",
};

const SocialPreviewPage = async ({
  params,
}: {
  params: Promise<{ id: string }>;
}) => {
  // Capture is a development tool; this route always returns 404 in production.
  if (
    process.env.NODE_ENV !== "development" ||
    process.env.AUDIOCN_SOCIAL_CAPTURE !== "1"
  ) {
    notFound();
  }
  const { id } = await params;
  const card = getSocialCards(source.getPages()).find((item) => item.id === id);
  if (!card) {
    notFound();
  }
  return <SocialCard card={card} />;
};

export default SocialPreviewPage;
