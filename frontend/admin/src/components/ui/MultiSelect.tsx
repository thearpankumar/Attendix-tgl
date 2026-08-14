import { useEffect, useRef, useState } from 'react';
import { Check, X } from 'lucide-react';

export interface MultiSelectOption {
  value: string;
  label: string;
}

interface MultiSelectProps {
  id?: string;
  options: MultiSelectOption[];
  selected: string[];
  onChange: (next: string[]) => void;
  placeholder?: string;
  emptyMessage?: string;
}

// Searchable combobox + removable chips. Chosen over a plain checkbox list
// (which was unusable past ~10 options — it just pushed the rest of the
// form off-screen) and over a native <select multiple> (poor mobile
// support, no search). This is the standard "assignee picker" pattern
// (GitHub, Linear, Notion): type to filter, click to toggle, selections
// shown as chips you can remove without reopening the dropdown.
const MultiSelect = ({ id, options, selected, onChange, placeholder, emptyMessage }: MultiSelectProps) => {
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
        setQuery('');
      }
    };
    document.addEventListener('mousedown', onClickOutside);
    return () => document.removeEventListener('mousedown', onClickOutside);
  }, []);

  const selectedOptions = options.filter((o) => selected.includes(o.value));
  const filtered = options.filter((o) => o.label.toLowerCase().includes(query.toLowerCase()));

  const toggle = (value: string) => {
    onChange(selected.includes(value) ? selected.filter((v) => v !== value) : [...selected, value]);
  };

  return (
    <div className="multiselect" ref={containerRef}>
      <div className="multiselect-control" onClick={() => { setOpen(true); inputRef.current?.focus(); }}>
        {selectedOptions.map((o) => (
          <span key={o.value} className="multiselect-chip">
            {o.label}
            <button
              type="button"
              aria-label={`Remove ${o.label}`}
              onClick={(e) => { e.stopPropagation(); toggle(o.value); }}
            >
              <X size={12} />
            </button>
          </span>
        ))}
        <input
          ref={inputRef}
          id={id}
          type="text"
          className="multiselect-input"
          value={query}
          autoComplete="off"
          placeholder={selectedOptions.length === 0 ? placeholder : ''}
          onFocus={() => setOpen(true)}
          onChange={(e) => { setQuery(e.target.value); setOpen(true); }}
          onKeyDown={(e) => {
            if (e.key === 'Backspace' && query === '' && selectedOptions.length > 0) {
              toggle(selectedOptions[selectedOptions.length - 1].value);
            }
          }}
        />
      </div>

      {open && (
        <div className="multiselect-dropdown">
          {filtered.length === 0 ? (
            <div className="multiselect-empty">{emptyMessage ?? 'No matches'}</div>
          ) : (
            filtered.map((o) => {
              const isSelected = selected.includes(o.value);
              return (
                <div
                  key={o.value}
                  className={`multiselect-option${isSelected ? ' selected' : ''}`}
                  onClick={() => toggle(o.value)}
                >
                  <span className="multiselect-check">{isSelected && <Check size={12} />}</span>
                  {o.label}
                </div>
              );
            })
          )}
        </div>
      )}
    </div>
  );
};

export default MultiSelect;
