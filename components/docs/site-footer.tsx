import Link from "next/link";

const FOOTER_LINK_CLASS =
  "text-muted-foreground underline-offset-4 hover:text-foreground hover:underline";

export const SiteFooter = () => (
  <footer>
    <div className="mx-auto flex w-full max-w-6xl flex-col items-center justify-between gap-3 px-4 py-6 text-center sm:flex-row sm:px-6 sm:text-left">
      <p className="text-muted-foreground text-sm">
        Built by{" "}
        <a
          className="underline underline-offset-4"
          href="https://x.com/fortysevenfx"
          rel="noopener noreferrer"
          target="_blank"
        >
          fortysevenfx
        </a>{" "}
        and{" "}
        <a
          className="underline underline-offset-4"
          href="https://x.com/orcdev"
          rel="noopener noreferrer"
          target="_blank"
        >
          orcdev
        </a>{" "}
        with <span aria-hidden="true">🪓🪓</span>
      </p>
      <nav aria-label="Secondary" className="flex items-center gap-4 text-sm">
        <Link className={FOOTER_LINK_CLASS} href="/contributors">
          Contributors
        </Link>
      </nav>
    </div>
  </footer>
);
