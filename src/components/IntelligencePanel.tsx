import { useTranslation } from "../contexts/LanguageContext";
import Icons from "./Icons";
import type {
  TagDefinition,
  AnalysisResultDto,
  SemanticResult,
  DetectedTable,
  SensitiveInfo,
  RuleAction,
} from "../types";

interface IntelligencePanelProps {
  documentTags: string[];
  tagDefinitions: TagDefinition[];
  onAddTag: (tag: string) => void;
  onRemoveTag: (tag: string) => void;
  analysisResult: AnalysisResultDto | null;
  isAnalyzing: boolean;
  onAnalyze: () => void;
  hasDocument: boolean;
  selectedDocId: string | null;
  onApplySuggestion: () => void;
  onApplyRuleActions: (ruleName: string, actions: RuleAction[]) => void;
  aiSummary: string;
  aiTranslation: string;
  aiTargetLang: string;
  onAiTargetLangChange: (lang: string) => void;
  isAiLoading: boolean;
  onAiOcr: () => void;
  onAiSummarize: () => void;
  onAiTranslate: () => void;
  onDetectSensitive: () => void;
  onDetectTables: () => void;
  semanticQuery: string;
  semanticResults: SemanticResult[];
  onSemanticQueryChange: (query: string) => void;
  onSemanticSearch: () => void;
  detectedTables: DetectedTable[];
  sensitiveItems: SensitiveInfo[];
  hasGroqKey: boolean;
  onShowRedaction: () => void;
  onExportTableCsv: (table: DetectedTable) => void;
}

export function IntelligencePanel({
  documentTags,
  tagDefinitions,
  onAddTag,
  onRemoveTag,
  analysisResult,
  isAnalyzing,
  onAnalyze,
  hasDocument,
  onApplySuggestion,
  onApplyRuleActions,
}: IntelligencePanelProps) {
  const { t } = useTranslation();

  return (
    <>
      <div className="config-header">{t("intelligence.header")}</div>

      {hasDocument && (
        <div className="config-section">
          <div className="config-label">
            {Icons.tag} {t("intelligence.tags")}
          </div>
          <div className="tags-container">
            {documentTags.map((tag) => {
              const def = tagDefinitions.find((d) => d.name === tag);
              return (
                <span
                  key={tag}
                  className="tag-chip"
                  style={{
                    background: def?.color || "var(--accent-color)",
                  }}
                >
                  {tag}
                  <button
                    className="tag-remove"
                    onClick={() => onRemoveTag(tag)}
                    aria-label={`Remove ${tag}`}
                  >
                    &times;
                  </button>
                </span>
              );
            })}
            <select
              className="glass-select tag-add-select"
              value=""
              onChange={(e) => {
                if (e.target.value) onAddTag(e.target.value);
                e.target.value = "";
              }}
            >
              <option value="">{t("intelligence.addTag")}</option>
              {tagDefinitions
                .filter((d) => !documentTags.includes(d.name))
                .map((d) => (
                  <option key={d.name} value={d.name}>
                    {d.name}
                  </option>
                ))}
            </select>
          </div>
        </div>
      )}

      <div className="config-section">
        <div className="config-label">
          {Icons.brain} {t("intelligence.header")}
        </div>
        <button
          className="btn btn-sm btn-accent intelligence-analyze-btn"
          onClick={onAnalyze}
          disabled={!hasDocument || isAnalyzing}
        >
          {Icons.sparkle}{" "}
          {isAnalyzing
            ? t("intelligence.analyzing")
            : t("intelligence.analyze")}
        </button>
      </div>

      {analysisResult && (
        <>
          <div className="config-section">
            <div className="config-label">
              {t("intelligence.classification")}
            </div>
            <div className="intelligence-result">
              <div className="intelligence-type">
                {t(
                  `docTypes.${analysisResult.classification.doc_type}`
                ) || analysisResult.classification.doc_type}
              </div>
              <div className="intelligence-confidence">
                {t("intelligence.confidence", {
                  percent: Math.round(
                    analysisResult.classification.confidence * 100
                  ),
                })}
              </div>
              <div className="intelligence-scores">
                {analysisResult.classification.scores
                  .slice(0, 5)
                  .map(([name, score]) => (
                    <div key={name} className="score-bar">
                      <span className="score-label">
                        {t(`docTypes.${name}`) || name}
                      </span>
                      <div className="score-track">
                        <div
                          className="score-fill"
                          style={{
                            width: `${Math.min(score * 3, 100)}%`,
                          }}
                        />
                      </div>
                      <span className="score-value">{score.toFixed(1)}</span>
                    </div>
                  ))}
              </div>
            </div>
          </div>

          {Object.keys(analysisResult.extracted_data.fields).length > 0 && (
            <div className="config-section">
              <div className="config-label">
                {t("intelligence.extractedData")}
              </div>
              <div className="extracted-fields">
                {Object.entries(analysisResult.extracted_data.fields).map(
                  ([key, values]) => (
                    <div key={key} className="extracted-field">
                      <span className="field-name">{key}</span>
                      <span className="field-values">
                        {values.join(", ")}
                      </span>
                    </div>
                  )
                )}
              </div>
            </div>
          )}

          <div className="config-section">
            <div className="config-label">
              {Icons.sparkle} {t("intelligence.suggestion")}
            </div>
            <div className="suggestion-card">
              <div className="suggestion-row">
                <span className="suggestion-label">
                  {t("intelligence.suggestedName")}
                </span>
                <span className="suggestion-value">
                  {analysisResult.suggestion.suggested_name}
                </span>
              </div>
              <div className="suggestion-row">
                <span className="suggestion-label">
                  {t("intelligence.suggestedFolder")}
                </span>
                <span className="suggestion-value">
                  {analysisResult.suggestion.suggested_folder}
                </span>
              </div>
              <div className="suggestion-row">
                <span className="suggestion-label">
                  {t("intelligence.suggestedTags")}
                </span>
                <span className="suggestion-value">
                  {analysisResult.suggestion.suggested_tags.join(", ")}
                </span>
              </div>
              <button
                className="btn btn-sm btn-accent suggestion-apply-btn"
                onClick={onApplySuggestion}
              >
                {Icons.check} {t("intelligence.applySuggestions")}
              </button>
            </div>
          </div>

          {analysisResult.rule_results.length > 0 && (
            <div className="config-section">
              <div className="config-label">
                {Icons.rules} {t("intelligence.matchingRules")}
              </div>
              {analysisResult.rule_results.map((rr, i) => (
                <div key={i} className="rule-result">
                  <div className="rule-result-name">{rr.rule_name}</div>
                  <div className="rule-result-actions">
                    {rr.actions.map((a, j) => (
                      <span key={j} className="rule-action-chip">
                        {a.action_type}: {a.value}
                      </span>
                    ))}
                  </div>
                  <button
                    className="btn btn-sm rule-result-apply-btn"
                    onClick={() =>
                      onApplyRuleActions(rr.rule_name, rr.actions)
                    }
                  >
                    {t("actions.apply")}
                  </button>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </>
  );
}
