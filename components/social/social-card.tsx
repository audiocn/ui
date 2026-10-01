"use client";

import { useEffect, useRef } from "react";

import { BrandLogo } from "@/components/brand-logo";
import { SocialPreview } from "@/components/social/social-previews";
import type { SocialCardDefinition } from "@/lib/social-catalog";

import styles from "./social-card.module.css";

export const SocialCard = ({ card }: { card: SocialCardDefinition }) => {
  const cardRef = useRef<HTMLElement>(null);
  useEffect(() => {
    if (cardRef.current) {
      cardRef.current.dataset.socialReady = "true";
    }
  }, []);

  return (
    <article
      className={styles.card}
      data-social-card={card.id}
      data-social-kind={card.pathname.includes("/hooks/") ? "hook" : "page"}
      data-social-long-title={card.title.length > 24}
      data-social-ready="false"
      data-theme="stone"
      ref={cardRef}
    >
      <header className={styles.header}>
        <div className={styles.brand}>
          <BrandLogo size={36} />
          <span>audiocn</span>
        </div>
        <span className={styles.headerLabel}>REACT + SHADCN/UI</span>
      </header>
      <div className={styles.body}>
        <div className={styles.copy}>
          <p className={styles.category}>{card.category}</p>
          <h1 className={styles.title}>{card.title}</h1>
          <p className={styles.caption}>{card.caption}</p>
        </div>
        <figure className={styles.stage}>
          <div className={styles.preview} data-social-preview={card.preview}>
            <SocialPreview name={card.preview} />
          </div>
          <figcaption className="sr-only">{card.alt}</figcaption>
        </figure>
      </div>
      <footer className={styles.footer}>
        <span>COPY. PASTE. OWN.</span>
        <span>audiocn.dev</span>
      </footer>
    </article>
  );
};
