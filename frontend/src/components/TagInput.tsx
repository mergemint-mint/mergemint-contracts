import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api } from '../lib/api';

const DEFAULT_MAX_TAGS = 10;
const MAX_SUGGESTIONS = 6;

/**
 * Normalize a tag so near-duplicates like `rust` and `Rust` collapse to one.
 * Lowercases, trims, and collapses internal whitespace to single dashes.
 */
export function normalizeTag(raw: string): string {
  return raw
    .trim()
    .toLowerCase()
    .replace(/\s+/g, '-')
    .replace(/[^a-z0-9-_]/g, '')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '');
}

export interface TagInputProps {
  value: string[];
  onChange: (tags: string[]) => void;
  /** Contract-enforced maximum number of tags. */
  maxTags?: number;
  placeholder?: string;
  disabled?: boolean;
  id?: string;
  label?: string;
}

export default function TagInput({
  value,
  onChange,
  maxTags = DEFAULT_MAX_TAGS,
  placeholder = 'Add a tag…',
  disabled = false,
  id = 'tag-input',
  label = 'Tags',
}: TagInputProps) {
  const [input, setInput] = useState('');
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [activeIndex, setActiveIndex] = useState(-1);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestId = useRef(0);
  const containerRef = useRef<HTMLDivElement>(null);

  const atLimit = value.length >= maxTags;

  const normalizedValue = useMemo(
    () => new Set(value.map((t) => normalizeTag(t))),
    [value],
  );

  const addTag = useCallback(
    (raw: string) => {
      const tag = normalizeTag(raw);
      if (!tag) return;
      if (normalizedValue.has(tag)) {
        setError(`"${tag}" is already added.`);
        return;
      }
      if (value.length >= maxTags) {
        setError(`You can add up to ${maxTags} tags.`);
        return;
      }
      setError(null);
      onChange([...value, tag]);
      setInput('');
      setSuggestions([]);
      setActiveIndex(-1);
      setOpen(false);
    },
    [maxTags, normalizedValue, onChange, value],
  );

  const removeTag = useCallback(
    (tag: string) => {
      setError(null);
      onChange(value.filter((t) => t !== tag));
    },
    [onChange, value],
  );

  // Fetch existing tags from the backend as the user types.
  useEffect(() => {
    const query = normalizeTag(input);
    if (!query || disabled || atLimit) {
      setSuggestions([]);
      setActiveIndex(-1);
      return;
    }

    const current = ++requestId.current;
    const timer = setTimeout(async () => {
      try {
        const results = await api.searchTags(query);
        if (current !== requestId.current) return;
        const filtered = (results ?? [])
          .map((t) => normalizeTag(t))
          .filter((t) => t && !normalizedValue.has(t) && t.includes(query));
        const unique = Array.from(new Set(filtered)).slice(0, MAX_SUGGESTIONS);
        setSuggestions(unique);
        setActiveIndex(unique.length > 0 ? 0 : -1);
        setOpen(unique.length > 0);
      } catch {
        if (current !== requestId.current) return;
        setSuggestions([]);
        setActiveIndex(-1);
      }
    }, 200);

    return () => clearTimeout(timer);
  }, [input, disabled, atLimit, normalizedValue]);

  // Close the suggestion list on outside click.
  useEffect(() => {
    const onDocClick = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', onDocClick);
    return () => document.removeEventListener('mousedown', onDocClick);
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown' && suggestions.length > 0) {
      e.preventDefault();
      setOpen(true);
      setActiveIndex((i) => (i + 1) % suggestions.length);
      return;
    }
    if (e.key === 'ArrowUp' && suggestions.length > 0) {
      e.preventDefault();
      setOpen(true);
      setActiveIndex((i) => (i - 1 + suggestions.length) % suggestions.length);
      return;
    }
    if (e.key === 'Enter' || e.key === ',') {
      e.preventDefault();
      if (open && activeIndex >= 0 && suggestions[activeIndex]) {
        addTag(suggestions[activeIndex]);
      } else {
        addTag(input);
      }
      return;
    }
    if (e.key === 'Escape') {
      setOpen(false);
      setActiveIndex(-1);
      return;
    }
    if (e.key === 'Backspace' && input === '' && value.length > 0) {
      removeTag(value[value.length - 1]);
    }
  };

  return (
    <div className="tag-input" ref={containerRef}>
      <label className="tag-input__label" htmlFor={id}>
        {label}
        <span className="tag-input__count">
          {value.length}/{maxTags}
        </span>
      </label>

      <div
        className={`tag-input__field${disabled ? ' tag-input__field--disabled' : ''}`}
        onClick={() => document.getElementById(id)?.focus()}
      >
        {value.map((tag) => (
          <span key={tag} className="tag-input__chip">
            {tag}
            <button
              type="button"
              className="tag-input__chip-remove"
              aria-label={`Remove ${tag}`}
              disabled={disabled}
              onClick={(e) => {
                e.stopPropagation();
                removeTag(tag);
              }}
            >
              ×
            </button>
          </span>
        ))}

        <input
          id={id}
          type="text"
          className="tag-input__control"
          value={input}
          placeholder={atLimit ? `Maximum ${maxTags} tags reached` : placeholder}
          disabled={disabled || atLimit}
          autoComplete="off"
          role="combobox"
          aria-expanded={open}
          aria-controls={`${id}-suggestions`}
          aria-autocomplete="list"
          aria-activedescendant={
            open && activeIndex >= 0 ? `${id}-option-${activeIndex}` : undefined
          }
          onChange={(e) => {
            setInput(e.target.value);
            setError(null);
          }}
          onKeyDown={handleKeyDown}
          onFocus={() => suggestions.length > 0 && setOpen(true)}
        />
      </div>

      {open && suggestions.length > 0 && (
        <ul
          id={`${id}-suggestions`}
          className="tag-input__suggestions"
          role="listbox"
        >
          {suggestions.map((tag, index) => (
            <li
              key={tag}
              id={`${id}-option-${index}`}
              role="option"
              aria-selected={index === activeIndex}
              className={`tag-input__suggestion${
                index === activeIndex ? ' tag-input__suggestion--active' : ''
              }`}
              onMouseDown={(e) => {
                e.preventDefault();
                addTag(tag);
              }}
              onMouseEnter={() => setActiveIndex(index)}
            >
              {tag}
            </li>
          ))}
        </ul>
      )}

      {error && (
        <p className="tag-input__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
