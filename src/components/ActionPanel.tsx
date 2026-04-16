import { useMemo, useState } from 'react';
import type {
  ActionFormField,
  ActionOption,
  ActionSubmission,
  PendingAction,
} from '../types/instance';

interface ActionPanelProps {
  action: PendingAction;
  onSubmit: (submission: ActionSubmission) => void;
}

function optionButtonClass(style: 'primary' | 'secondary' | 'danger') {
  switch (style) {
    case 'primary':
      return 'btn btn--approve';
    case 'danger':
      return 'btn btn--deny';
    case 'secondary':
    default:
      return 'btn btn--abort';
  }
}

function fieldInitialValue(field: ActionFormField) {
  if (field.field_type === 'select') return field.options[0]?.id ?? '';
  if (field.field_type === 'multi_select') return [];
  return '';
}

export const ActionPanel = ({ action, onSubmit }: ActionPanelProps) => {
  const [inputValue, setInputValue] = useState('');
  const [formValues, setFormValues] = useState<Record<string, string | string[]>>(() => {
    if (!action.form) return {};
    return Object.fromEntries(action.form.fields.map((field) => [field.id, fieldInitialValue(field)]));
  });

  const preview = useMemo(() => {
    const details = action.details;
    if (!details) return null;

    if (details.type === 'tool_use') {
      const toolInput = details.tool_input ?? {};
      const command = typeof toolInput.command === 'string'
        ? toolInput.command
        : JSON.stringify(toolInput, null, 2);

      return (
        <div className="action-area__preview">
          <div className="action-area__label">{details.tool_name}</div>
          <div className="action-area__command">{command}</div>
        </div>
      );
    }

    if (details.type === 'command') {
      return (
        <div className="action-area__preview">
          <div className="action-area__label">Command</div>
          <div className="action-area__command">{(details.command ?? []).join(' ')}</div>
          {details.cwd && <div className="action-area__cwd">cwd: {details.cwd}</div>}
          {details.reason && <div className="action-area__meta">{details.reason}</div>}
        </div>
      );
    }

    if (details.type === 'file_diff') {
      return (
        <div className="action-area__preview">
          <div className="action-area__label">File changes</div>
          {(details.files ?? []).map((file) => (
            <div key={file.path} className="action-area__file">
              <span>{file.path}</span>
              <span className="action-area__file-additions">+{file.additions}</span>
              <span className="action-area__file-deletions">-{file.deletions}</span>
            </div>
          ))}
        </div>
      );
    }

    return null;
  }, [action.details]);

  const renderChoice = () => (
    <div className="action-area__buttons">
      {action.options.map((option) => (
        <button
          key={option.id}
          className={optionButtonClass(option.style)}
          onClick={(event) => {
            event.stopPropagation();
            onSubmit({ kind: 'choice', selected_id: option.id });
          }}
          title={option.description ?? undefined}
        >
          {option.label}
        </button>
      ))}
    </div>
  );

  const renderInput = () => {
    if (!action.input) return null;

    return (
      <div className="action-area__input-shell">
        {action.input.multiline ? (
          <textarea
            className="action-area__input action-area__input--multiline"
            value={inputValue}
            placeholder={action.input.placeholder ?? ''}
            onClick={(event) => event.stopPropagation()}
            onChange={(event) => setInputValue(event.target.value)}
          />
        ) : (
          <input
            className="action-area__input"
            value={inputValue}
            placeholder={action.input.placeholder ?? ''}
            onClick={(event) => event.stopPropagation()}
            onChange={(event) => setInputValue(event.target.value)}
          />
        )}
        <button
          className="btn btn--approve"
          onClick={(event) => {
            event.stopPropagation();
            onSubmit({ kind: 'input', value: inputValue });
          }}
        >
          {action.input.submit_label}
        </button>
      </div>
    );
  };

  const renderForm = () => {
    if (!action.form) return null;

    const updateField = (fieldId: string, value: string | string[]) => {
      setFormValues((current) => ({ ...current, [fieldId]: value }));
    };

    const renderChoiceOption = (
      field: ActionFormField,
      option: ActionOption,
      checked: boolean,
    ) => (
      <label key={option.id} className="action-area__choice">
        <input
          type={field.field_type === 'multi_select' ? 'checkbox' : 'radio'}
          name={field.id}
          value={option.id}
          checked={checked}
          onClick={(event) => event.stopPropagation()}
          onChange={(event) => {
            if (field.field_type === 'multi_select') {
              const current = Array.isArray(formValues[field.id])
                ? (formValues[field.id] as string[])
                : [];
              updateField(
                field.id,
                event.target.checked
                  ? [...current, option.id]
                  : current.filter((id: string) => id !== option.id),
              );
            } else {
              updateField(field.id, option.id);
            }
          }}
        />
        <span>
          <span className="action-area__choice-label">{option.label}</span>
          {option.description && (
            <span className="action-area__choice-description">{option.description}</span>
          )}
        </span>
      </label>
    );

    return (
      <div className="action-area__form">
        {action.form.fields.map((field) => (
          <label key={field.id} className="action-area__field">
            <span className="action-area__field-label">{field.label}</span>
            {field.field_type === 'select' || field.field_type === 'multi_select' ? (
              <div className="action-area__choices" data-no-drag="true">
                {field.options.map((option) =>
                  renderChoiceOption(
                    field,
                    option,
                    field.field_type === 'multi_select'
                      ? Array.isArray(formValues[field.id]) &&
                          formValues[field.id].includes(option.id)
                      : formValues[field.id] === option.id,
                  ),
                )}
              </div>
            ) : field.field_type === 'multiline' ? (
              <textarea
                className="action-area__input action-area__input--multiline"
                value={typeof formValues[field.id] === 'string' ? formValues[field.id] : ''}
                placeholder={field.placeholder ?? ''}
                onClick={(event) => event.stopPropagation()}
                onChange={(event) =>
                  setFormValues((current) => ({ ...current, [field.id]: event.target.value }))
                }
              />
            ) : (
              <input
                className="action-area__input"
                value={typeof formValues[field.id] === 'string' ? formValues[field.id] : ''}
                placeholder={field.placeholder ?? ''}
                onClick={(event) => event.stopPropagation()}
                onChange={(event) =>
                  setFormValues((current) => ({ ...current, [field.id]: event.target.value }))
                }
              />
            )}
          </label>
        ))}
        <button
          className="btn btn--approve"
          onClick={(event) => {
            event.stopPropagation();
            onSubmit({ kind: 'form', values: formValues });
          }}
        >
          {action.form.submit_label}
        </button>
      </div>
    );
  };

  return (
    <div className="action-area">
      <div className="action-area__title">{action.title}</div>
      <div className="action-area__message">{action.message}</div>
      {preview}
      {action.kind === 'choice' && renderChoice()}
      {action.kind === 'input' && renderInput()}
      {action.kind === 'form' && renderForm()}
    </div>
  );
};
