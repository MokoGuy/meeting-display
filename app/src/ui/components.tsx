// The Meeting Display design system as React components. No native control is
// shown as the browser draws it: every interactive element is styled from the
// tokens (tokens.css) by the classes in components.css.
import { useId, useState, type KeyboardEvent, type ReactNode, type WheelEvent } from "react";
import "./tokens.css";
import "./components.css";

// ---------- icons (the only three, plus the reveal eye)
type IconName = "settings" | "back" | "chevron" | "eye";
const ICON_PATHS: Record<IconName, ReactNode> = {
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8" />
    </>
  ),
  back: <path d="M19 12H5M11 6l-6 6 6 6" />,
  chevron: <path d="M9 6l6 6-6 6" />,
  eye: (
    <>
      <path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" />
      <circle cx="12" cy="12" r="2.5" />
    </>
  ),
};

export function Icon({ name }: { name: IconName }) {
  return (
    <svg className="md-icon" viewBox="0 0 24 24" aria-hidden="true">
      {ICON_PATHS[name]}
    </svg>
  );
}

// ---------- actions
export function Button({ primary, onClick, children }: { primary?: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" className={`md-button${primary ? " is-primary" : ""}`} onClick={onClick}>
      {children}
    </button>
  );
}

export function IconButton({ icon, label, onClick }: { icon: IconName; label: string; onClick: () => void }) {
  return (
    <button type="button" className="md-icon-button" aria-label={label} title={label} onClick={onClick}>
      <Icon name={icon} />
    </button>
  );
}

// ---------- settings controls
export function Switch({
  checked,
  onChange,
  label,
  hint,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  hint?: string;
}) {
  const id = useId();
  return (
    <div className="md-switch-row" onClick={() => onChange(!checked)}>
      <span className="md-switch-label" id={id}>
        {label}
        {hint && <span className="md-switch-hint">{hint}</span>}
      </span>
      <button
        type="button"
        className="md-switch"
        role="switch"
        aria-checked={checked}
        aria-labelledby={id}
        onClick={(event) => {
          event.stopPropagation();
          onChange(!checked);
        }}
      />
    </div>
  );
}

export function TextField({
  label,
  value,
  onChange,
  secret,
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  secret?: boolean;
}) {
  const id = useId();
  const [revealed, setRevealed] = useState(false);
  return (
    <div className="md-field">
      <label className="md-field-label" htmlFor={id}>
        {label}
      </label>
      <span className="md-field-box">
        <input
          id={id}
          className="md-field-input"
          type={secret && !revealed ? "password" : "text"}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          spellCheck={false}
          autoComplete="off"
        />
        {secret && (
          <IconButton icon="eye" label={revealed ? "Masquer" : "Afficher"} onClick={() => setRevealed(!revealed)} />
        )}
      </span>
    </div>
  );
}

/** One segment of a TimeField: hours or minutes, as a spinbutton. */
function TimeSegment({
  label,
  value,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number | null;
  max: number;
  step: number;
  onChange: (next: number | null) => void;
}) {
  const wrap = (n: number) => ((n % (max + 1)) + (max + 1)) % (max + 1);
  const bump = (direction: number) => onChange(value === null ? 0 : wrap(value + direction * step));
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "ArrowUp") {
      bump(1);
      event.preventDefault();
    } else if (event.key === "ArrowDown") {
      bump(-1);
      event.preventDefault();
    } else if (event.key === "Backspace" || event.key === "Delete") {
      onChange(null);
    } else if (/^[0-9]$/.test(event.key)) {
      const digit = Number(event.key);
      const typed = (value === null ? 0 : value % 10) * 10 + digit;
      onChange(typed > max ? digit : typed);
    }
  };
  const onWheel = (event: WheelEvent<HTMLButtonElement>) => bump(event.deltaY < 0 ? 1 : -1);
  return (
    <button
      type="button"
      className={`md-time-segment${value === null ? " is-empty" : ""}`}
      role="spinbutton"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={value ?? undefined}
      onKeyDown={onKeyDown}
      onWheel={onWheel}
    >
      {value === null ? "--" : String(value).padStart(2, "0")}
    </button>
  );
}

/** Time of day as "HH:MM", or "" when unset. Clearing either segment clears both. */
export function TimeField({ label, value, onChange }: { label: string; value: string; onChange: (next: string) => void }) {
  const match = /^(\d{1,2}):(\d{2})/.exec(value);
  const hours = match ? Number(match[1]) : null;
  const minutes = match ? Number(match[2]) : null;
  const emit = (h: number | null, m: number | null) =>
    onChange(h === null || m === null ? "" : `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`);
  return (
    <div className="md-field">
      <span className="md-field-label">{label}</span>
      <span className="md-time" role="group" aria-label={label}>
        <TimeSegment label="Heures" value={hours} max={23} step={1} onChange={(h) => emit(h, h === null ? null : (minutes ?? 0))} />
        <span className="md-time-sep">:</span>
        <TimeSegment label="Minutes" value={minutes} max={59} step={5} onChange={(m) => emit(m === null ? null : (hours ?? 0), m)} />
      </span>
    </div>
  );
}

export function Disclosure({ label, children }: { label: string; children: ReactNode }) {
  const id = useId();
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button type="button" className="md-disclosure-trigger" aria-expanded={open} aria-controls={id} onClick={() => setOpen(!open)}>
        <Icon name="chevron" />
        {label}
      </button>
      <div className="md-disclosure-body" id={id} hidden={!open}>
        {children}
      </div>
    </div>
  );
}

// ---------- agenda
export function StatusDot({ online, children }: { online?: boolean; children: ReactNode }) {
  return (
    <span className={`md-status${online ? " is-online" : ""}`}>
      {online !== undefined && <span className="md-status-dot" />}
      {children}
    </span>
  );
}

export function MeetingRow({ time, subject, tentative }: { time: string; subject: string; tentative?: boolean }) {
  return (
    <div className={`md-row${tentative ? " is-tentative" : ""}`}>
      <span className="md-row-bar" />
      <span className="md-row-time">{time}</span>
      <span className="md-row-subject">
        {subject}
        {tentative ? " · provisoire" : ""}
      </span>
    </div>
  );
}

export function NowRow({
  subject,
  time,
  state,
  live,
  children,
}: {
  subject: string;
  time: string;
  state: string;
  live?: boolean;
  children: ReactNode;
}) {
  return (
    <div className={`md-now${live ? " is-live" : ""}`}>
      <span className="md-now-bar" />
      <div>
        <div className="md-now-subject">{subject}</div>
        <div className="md-now-time">{time}</div>
        <div className="md-now-line">
          <span className="md-now-state">{state}</span>
          {children}
        </div>
      </div>
    </div>
  );
}
