import { useEffect, useRef, useState, useLayoutEffect, useCallback } from "react";
import { useTranslation } from "../../contexts/LanguageContext";
import type { TourStep } from "./useTour";

interface TourTooltipProps {
  step: TourStep;
  stepIndex: number;
  totalSteps: number;
  onNext: () => void;
  onPrev: () => void;
  onSkip: () => void;
}

export function TourTooltip({ step, stepIndex, totalSteps, onNext, onPrev, onSkip }: TourTooltipProps) {
  const { t } = useTranslation();
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ top: 0, left: 0 });
  const [highlightRect, setHighlightRect] = useState<DOMRect | null>(null);

  const reposition = useCallback(() => {
    const target = document.getElementById(step.targetId);
    if (!target) return;

    const rect = target.getBoundingClientRect();
    setHighlightRect(rect);

    const tooltip = tooltipRef.current;
    if (!tooltip) return;
    const tRect = tooltip.getBoundingClientRect();
    const gap = 12;

    let top = 0, left = 0;
    switch (step.placement) {
      case "bottom":
        top = rect.bottom + gap;
        left = rect.left + rect.width / 2 - tRect.width / 2;
        break;
      case "top":
        top = rect.top - tRect.height - gap;
        left = rect.left + rect.width / 2 - tRect.width / 2;
        break;
      case "right":
        top = rect.top + rect.height / 2 - tRect.height / 2;
        left = rect.right + gap;
        break;
      case "left":
        top = rect.top + rect.height / 2 - tRect.height / 2;
        left = rect.left - tRect.width - gap;
        break;
    }

    left = Math.max(8, Math.min(left, window.innerWidth - tRect.width - 8));
    top = Math.max(8, Math.min(top, window.innerHeight - tRect.height - 8));
    setPos({ top, left });
  }, [step]);

  useLayoutEffect(() => {
    // Scroll target into view first, then position after scroll settles
    const target = document.getElementById(step.targetId);
    target?.scrollIntoView({ block: "nearest" });
    // Double rAF to let layout settle after scroll
    requestAnimationFrame(() => requestAnimationFrame(reposition));
  }, [step, reposition]);

  // Reposition on resize/scroll
  useEffect(() => {
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  }, [reposition]);

  useEffect(() => {
    const btn = tooltipRef.current?.querySelector<HTMLButtonElement>("button");
    btn?.focus();
  }, [step]);

  return (
    <>
      {highlightRect && (
        <div
          className="tour-highlight"
          aria-hidden="true"
          style={{
            position: "fixed",
            top: highlightRect.top - 4,
            left: highlightRect.left - 4,
            width: highlightRect.width + 8,
            height: highlightRect.height + 8,
            zIndex: 1510,
            borderRadius: "var(--r-md, 8px)",
            boxShadow: "0 0 0 4px var(--accent), 0 0 0 9999px rgba(0,0,0,0.55)",
            pointerEvents: "none",
          }}
        />
      )}

      <div
        ref={tooltipRef}
        className="tour-tooltip"
        role="dialog"
        aria-labelledby="tour-tooltip-title"
        aria-live="polite"
        style={{ position: "fixed", top: pos.top, left: pos.left, zIndex: 1600 }}
      >
        <div className="tour-tooltip-header">
          <span id="tour-tooltip-title" className="tour-tooltip-title">{t(step.titleKey)}</span>
          <button className="btn btn-icon btn-ghost btn-sm" onClick={onSkip} aria-label={t("tour.skipTour")}>&times;</button>
        </div>
        <p className="tour-tooltip-body">{t(step.bodyKey)}</p>
        <div className="tour-tooltip-footer">
          <span className="tour-step-counter" aria-label={t("tour.stepOf", { current: stepIndex + 1, total: totalSteps })}>
            {stepIndex + 1} / {totalSteps}
          </span>
          <div style={{ flex: 1 }} />
          {stepIndex > 0 && (
            <button className="btn btn-sm" onClick={onPrev}>{t("tour.prev")}</button>
          )}
          <button className="btn btn-sm btn-accent" onClick={onNext}>
            {stepIndex === totalSteps - 1 ? t("tour.finish") : t("tour.next")}
          </button>
        </div>
      </div>
    </>
  );
}
