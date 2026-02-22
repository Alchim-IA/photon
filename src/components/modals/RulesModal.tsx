import { useState, useCallback } from "react";
import { useTranslation } from "../../contexts/LanguageContext";
import { useFocusTrap } from "../../hooks/useFocusTrap";
import Icons from "../Icons";
import type {
  AutomationRule,
  RuleCondition,
  RuleAction,
  ScanProfile,
  TagDefinition,
} from "../../types";

// ─── Rule Editor sub-component ──────────────────────────────────
function RuleEditor({
  rule,
  onSave,
  onCancel,
}: {
  rule: AutomationRule;
  onSave: (rule: AutomationRule) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<AutomationRule>(rule);
  const conditionFields = [
    "DocumentType",
    "Tag",
    "TextContains",
    "AmountAbove",
    "AmountBelow",
    "HasField",
  ];
  const conditionOperators = [
    "Equals",
    "NotEquals",
    "Contains",
    "Regex",
    "GreaterThan",
    "LessThan",
  ];
  const actionTypes = ["Rename", "MoveToFolder", "AddTag", "ApplyProfile"];

  const addCondition = () =>
    setDraft((d) => ({
      ...d,
      conditions: [
        ...d.conditions,
        { field: "DocumentType", operator: "Equals", value: "" },
      ],
    }));
  const removeCondition = (i: number) =>
    setDraft((d) => ({
      ...d,
      conditions: d.conditions.filter((_, idx) => idx !== i),
    }));
  const updateCondition = (
    i: number,
    key: keyof RuleCondition,
    value: string
  ) =>
    setDraft((d) => ({
      ...d,
      conditions: d.conditions.map((c, idx) =>
        idx === i ? { ...c, [key]: value } : c
      ),
    }));
  const addAction = () =>
    setDraft((d) => ({
      ...d,
      actions: [...d.actions, { action_type: "AddTag", value: "" }],
    }));
  const removeAction = (i: number) =>
    setDraft((d) => ({
      ...d,
      actions: d.actions.filter((_, idx) => idx !== i),
    }));
  const updateAction = (i: number, key: keyof RuleAction, value: string) =>
    setDraft((d) => ({
      ...d,
      actions: d.actions.map((a, idx) =>
        idx === i ? { ...a, [key]: value } : a
      ),
    }));

  return (
    <div className="rule-editor">
      <div className="rule-editor-section">
        <div className="rule-editor-label">{t("rules.ruleName")}</div>
        <input
          className="glass-input"
          value={draft.name}
          onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
        />
      </div>
      <div className="rule-editor-section">
        <div className="rule-editor-label">{t("rules.logic")}</div>
        <div className="chip-group">
          <button
            className={`chip ${draft.condition_logic === "And" ? "active" : ""}`}
            onClick={() =>
              setDraft((d) => ({ ...d, condition_logic: "And" }))
            }
          >
            {t("rules.logicAnd")}
          </button>
          <button
            className={`chip ${draft.condition_logic === "Or" ? "active" : ""}`}
            onClick={() =>
              setDraft((d) => ({ ...d, condition_logic: "Or" }))
            }
          >
            {t("rules.logicOr")}
          </button>
        </div>
      </div>
      <div className="rule-editor-section">
        <div className="rule-editor-label">{t("rules.conditions")}</div>
        <div className="rule-rows">
          {draft.conditions.map((c, i) => (
            <div key={i} className="rule-row">
              <select
                className="glass-select"
                value={c.field}
                onChange={(e) => updateCondition(i, "field", e.target.value)}
              >
                {conditionFields.map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
              <select
                className="glass-select"
                value={c.operator}
                onChange={(e) =>
                  updateCondition(i, "operator", e.target.value)
                }
              >
                {conditionOperators.map((o) => (
                  <option key={o} value={o}>
                    {o}
                  </option>
                ))}
              </select>
              <input
                className="glass-input"
                value={c.value}
                onChange={(e) =>
                  updateCondition(i, "value", e.target.value)
                }
                placeholder={t("rules.valuePlaceholder")}
              />
              <button
                className="btn btn-icon btn-sm btn-ghost"
                onClick={() => removeCondition(i)}
                aria-label="Remove"
              >
                {Icons.close}
              </button>
            </div>
          ))}
        </div>
        <button className="btn btn-sm" onClick={addCondition}>
          {Icons.plus} {t("rules.addCondition")}
        </button>
      </div>
      <div className="rule-editor-section">
        <div className="rule-editor-label">{t("rules.actions")}</div>
        <div className="rule-rows">
          {draft.actions.map((a, i) => (
            <div key={i} className="rule-row">
              <select
                className="glass-select"
                value={a.action_type}
                onChange={(e) =>
                  updateAction(i, "action_type", e.target.value)
                }
              >
                {actionTypes.map((at) => (
                  <option key={at} value={at}>
                    {at}
                  </option>
                ))}
              </select>
              <input
                className="glass-input"
                value={a.value}
                onChange={(e) =>
                  updateAction(i, "value", e.target.value)
                }
                placeholder={t("rules.valuePlaceholder")}
              />
              <button
                className="btn btn-icon btn-sm btn-ghost"
                onClick={() => removeAction(i)}
                aria-label="Remove"
              >
                {Icons.close}
              </button>
            </div>
          ))}
        </div>
        <button className="btn btn-sm" onClick={addAction}>
          {Icons.plus} {t("rules.addAction")}
        </button>
      </div>
      <div className="rule-editor-actions">
        <button className="btn btn-sm" onClick={onCancel}>
          {t("edit.cancel")}
        </button>
        <button
          className="btn btn-sm btn-accent"
          onClick={() => onSave(draft)}
        >
          {t("rules.save")}
        </button>
      </div>
    </div>
  );
}

// ─── Rules Modal ─────────────────────────────────────────────────
interface RulesModalProps {
  show: boolean;
  onClose: () => void;
  rules: AutomationRule[];
  editingRule: AutomationRule | null;
  onEditRule: (rule: AutomationRule | null) => void;
  onSaveRule: (rule: AutomationRule) => void;
  onDeleteRule: (ruleId: string) => void;
  onNewRule: () => void;
  onCancelEdit: () => void;
  scanProfiles: ScanProfile[];
  tagDefinitions: TagDefinition[];
}

export function RulesModal({
  show,
  onClose,
  rules,
  editingRule,
  onEditRule,
  onSaveRule,
  onDeleteRule,
  onNewRule,
  onCancelEdit,
}: RulesModalProps) {
  const { t } = useTranslation();
  const closeHandler = useCallback(() => {
    onCancelEdit();
    onClose();
  }, [onClose, onCancelEdit]);
  const modalRef = useFocusTrap(show, closeHandler);

  if (!show) return null;

  return (
    <div
      className="settings-overlay"
      onClick={(e) => e.target === e.currentTarget && closeHandler()}
    >
      <div
        ref={modalRef}
        className="settings-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="rules-title"
      >
        <div className="settings-header">
          <span id="rules-title" className="settings-title">
            {t("rules.title")}
          </span>
          <button
            className="btn btn-icon btn-ghost"
            onClick={closeHandler}
            aria-label={t("a11y.close")}
          >
            {Icons.close}
          </button>
        </div>
        <div className="settings-body">
          {editingRule ? (
            <RuleEditor
              key={editingRule.id}
              rule={editingRule}
              onSave={onSaveRule}
              onCancel={onCancelEdit}
            />
          ) : (
            <>
              <button
                className="btn btn-sm btn-accent rules-new-btn"
                onClick={onNewRule}
              >
                {Icons.plus} {t("rules.newRule")}
              </button>
              {rules.length === 0 ? (
                <div className="rules-empty">{t("rules.noRules")}</div>
              ) : (
                rules.map((rule) => (
                  <div key={rule.id} className="rule-card">
                    <div className="rule-card-header">
                      <input
                        type="checkbox"
                        className="toggle"
                        checked={rule.enabled}
                        onChange={(e) =>
                          onSaveRule({
                            ...rule,
                            enabled: e.target.checked,
                          })
                        }
                      />
                      <span className="rule-card-name">{rule.name}</span>
                      <div className="rule-card-spacer" />
                      <button
                        className="btn btn-icon btn-sm btn-ghost"
                        onClick={() => onEditRule(rule)}
                        aria-label={t("rules.edit")}
                      >
                        {Icons.rename}
                      </button>
                      <button
                        className="btn btn-icon btn-sm btn-ghost"
                        onClick={() => onDeleteRule(rule.id)}
                        aria-label={t("rules.delete")}
                      >
                        {Icons.delete}
                      </button>
                    </div>
                    <div className="rule-card-detail">
                      <span className="rule-logic">
                        {rule.condition_logic === "And"
                          ? t("rules.logicAnd")
                          : t("rules.logicOr")}
                      </span>
                      {rule.conditions.map((c, i) => (
                        <span key={i} className="rule-cond-chip">
                          {c.field} {c.operator} "{c.value}"
                        </span>
                      ))}
                      <span className="rule-arrow">&rarr;</span>
                      {rule.actions.map((a, i) => (
                        <span key={i} className="rule-action-chip">
                          {a.action_type}: {a.value}
                        </span>
                      ))}
                    </div>
                  </div>
                ))
              )}
            </>
          )}
        </div>
        <div className="settings-footer">
          <button className="btn" onClick={closeHandler}>
            {t("rules.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
