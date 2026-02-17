import { useCallback, useReducer } from "react";

export interface TourStep {
  targetId: string;
  titleKey: string;
  bodyKey: string;
  placement: "top" | "bottom" | "left" | "right";
}

const TOUR_STEPS: TourStep[] = [
  { targetId: "sidebar-scanners", titleKey: "tour.scanners.title", bodyKey: "tour.scanners.body", placement: "right" },
  { targetId: "btn-scan", titleKey: "tour.scan.title", bodyKey: "tour.scan.body", placement: "bottom" },
  { targetId: "btn-save-pdf", titleKey: "tour.savePdf.title", bodyKey: "tour.savePdf.body", placement: "bottom" },
  { targetId: "btn-ocr", titleKey: "tour.ocr.title", bodyKey: "tour.ocr.body", placement: "bottom" },
  { targetId: "btn-analyze", titleKey: "tour.analyze.title", bodyKey: "tour.analyze.body", placement: "bottom" },
  { targetId: "panel-tabs", titleKey: "tour.panels.title", bodyKey: "tour.panels.body", placement: "left" },
  { targetId: "btn-settings", titleKey: "tour.settings.title", bodyKey: "tour.settings.body", placement: "bottom" },
];

type TourState = { isActive: boolean; stepIndex: number };
type TourAction = { type: "start" } | { type: "next" } | { type: "prev" } | { type: "skip" };

function tourReducer(state: TourState, action: TourAction): TourState {
  switch (action.type) {
    case "start":
      return { isActive: true, stepIndex: 0 };
    case "next":
      if (state.stepIndex >= TOUR_STEPS.length - 1) return { isActive: false, stepIndex: 0 };
      return { ...state, stepIndex: state.stepIndex + 1 };
    case "prev":
      return { ...state, stepIndex: Math.max(0, state.stepIndex - 1) };
    case "skip":
      return { isActive: false, stepIndex: 0 };
  }
}

export function useTour() {
  const [state, dispatch] = useReducer(tourReducer, { isActive: false, stepIndex: 0 });

  const start = useCallback(() => dispatch({ type: "start" }), []);
  const next = useCallback(() => dispatch({ type: "next" }), []);
  const prev = useCallback(() => dispatch({ type: "prev" }), []);
  const skip = useCallback(() => dispatch({ type: "skip" }), []);

  return {
    isActive: state.isActive,
    currentStep: TOUR_STEPS[state.stepIndex],
    stepIndex: state.stepIndex,
    totalSteps: TOUR_STEPS.length,
    start,
    next,
    prev,
    skip,
  };
}
