import { getSocialCards } from "@/lib/social-catalog";
import { source } from "@/lib/source";

export const GET = () => {
  if (
    process.env.NODE_ENV !== "development" ||
    process.env.AUDIOCN_SOCIAL_CAPTURE !== "1"
  ) {
    return new Response(null, { status: 404 });
  }
  return Response.json(getSocialCards(source.getPages()), {
    headers: { "X-Robots-Tag": "noindex, nofollow" },
  });
};
