// The Meeting Display design system as React components. No native control is
// shown as the browser draws it: every interactive element is styled from the
// tokens (tokens.css) by the classes in components.css.
import { useId, useState, type KeyboardEvent, type ReactNode, type WheelEvent } from "react";
import "./tokens.css";
import "./components.css";

// ---------- icons
type IconName = "settings" | "back" | "chevron" | "eye" | "resume" | "skip" | "prev" | "next" | "auto" | "light" | "dark";
const ICON_PATHS: Record<IconName, ReactNode> = {
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M10.325 4.317c.426 -1.756 2.924 -1.756 3.35 0a1.724 1.724 0 0 0 2.573 1.066c1.543 -.94 3.31 .826 2.37 2.37a1.724 1.724 0 0 0 1.065 2.572c1.756 .426 1.756 2.924 0 3.35a1.724 1.724 0 0 0 -1.066 2.573c.94 1.543 -.826 3.31 -2.37 2.37a1.724 1.724 0 0 0 -2.572 1.065c-.426 1.756 -2.924 1.756 -3.35 0a1.724 1.724 0 0 0 -2.573 -1.066c-1.543 .94 -3.31 -.826 -2.37 -2.37a1.724 1.724 0 0 0 -1.065 -2.572c-1.756 -.426 -1.756 -2.924 0 -3.35a1.724 1.724 0 0 0 1.066 -2.573c-.94 -1.543 .826 -3.31 2.37 -2.37c1 .608 2.296 .07 2.572 -1.065z" />
    </>
  ),
  back: <path d="M19 12H5M11 6l-6 6 6 6" />,
  chevron: <path d="M9 6l6 6-6 6" />,
  skip: (
    <>
      <path d="M4 5v14l12 -7z" />
      <path d="M20 5v14" />
    </>
  ),
  prev: <path d="M15 6l-6 6 6 6" />,
  next: <path d="M9 6l6 6-6 6" />,
  resume: (
    <>
      <path d="M9 14l-4 -4l4 -4" />
      <path d="M5 10h11a4 4 0 1 1 0 8h-1" />
    </>
  ),
  auto: (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 3v18M12 9h8.5M12 15h8.5" />
    </>
  ),
  light: (
    <>
      <circle cx="12" cy="12" r="4" />
      <path d="M3 12h1M12 3v1M20 12h1M12 20v1M5.6 5.6l.7 .7M18.4 5.6l-.7 .7M17.7 17.7l.7 .7M6.3 17.7l-.7 .7" />
    </>
  ),
  dark: <path d="M12 3c.132 0 .263 0 .393 0a7.5 7.5 0 0 0 7.92 12.446a9 9 0 1 1 -8.313 -12.454z" />,
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
export function Button({
  primary,
  small,
  onClick,
  children,
}: {
  primary?: boolean;
  small?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`md-button${primary ? " is-primary" : ""}${small ? " is-small" : ""}`}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

/** A round secondary button that carries an icon only, for the action next to the primary one. */
export function IconPill({ icon, label, onClick }: { icon: IconName; label: string; onClick: () => void }) {
  return (
    <button type="button" className="md-button is-icon" aria-label={label} data-tip={label} onClick={onClick}>
      <Icon name={icon} />
    </button>
  );
}

export function IconButton({ icon, label, onClick }: { icon: IconName; label: string; onClick: () => void }) {
  return (
    <button type="button" className="md-icon-button" aria-label={label} data-tip={label} onClick={onClick}>
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

/** Offers an update at the top of the agenda; nothing is installed without the click. */
export function UpdateBanner({ text, busy, onUpdate }: { text: string; busy?: boolean; onUpdate: () => void }) {
  return (
    <div className="md-update">
      <span className="md-update-text">{text}</span>
      {busy ? (
        <span className="md-update-busy">Mise à jour…</span>
      ) : (
        <Button small onClick={onUpdate}>
          Mettre à jour
        </Button>
      )}
    </div>
  );
}

/** The installed versions, two quiet lines closing the settings. */
export function VersionInfo({ app, update, barApp }: { app: string; update?: string | null; barApp?: string | null }) {
  return (
    <p className="md-version">
      Meeting Display {app}
      {update ? ` · ${update}` : ""}
      {barApp !== undefined && (
        <>
          <br />
          App de la barre {barApp ?? "non installée"}
        </>
      )}
    </p>
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

export type RowState = "upcoming" | "tentative" | "done" | "skipped";
const ROW_SUFFIX: Record<RowState, string> = {
  upcoming: "",
  tentative: " · provisoire",
  done: " · réalisée",
  skipped: " · ignorée",
};

/** An agenda line. Done and skipped meetings show a resume icon when the line is hovered. */
export function MeetingRow({
  time,
  subject,
  state = "upcoming",
  detail,
  onResume,
}: {
  time: string;
  subject: string;
  state?: RowState;
  /** A second line under the subject: planned and actual length of a meeting that was held. */
  detail?: ReactNode;
  onResume?: () => void;
}) {
  const setAside = state === "done" || state === "skipped";
  return (
    <div className={`md-row${state === "upcoming" ? "" : ` is-${state}`}${detail ? " has-detail" : ""}`}>
      <span className="md-row-bar" />
      <span className="md-row-time">{time}</span>
      <span className="md-row-main">
        <span className="md-row-subject">
          {subject}
          {ROW_SUFFIX[state]}
        </span>
        {detail && <span className="md-row-detail">{detail}</span>}
      </span>
      {setAside && onResume ? (
        <button type="button" className="md-row-action" aria-label="Reprendre cette réunion" data-tip="Reprendre" onClick={onResume}>
          <Icon name="resume" />
        </button>
      ) : (
        <span />
      )}
    </div>
  );
}

/** The panel header: the day on screen, arrows to change it, and whatever sits on the right. */
export function DayNav({
  label,
  today,
  onPrev,
  onNext,
  children,
}: {
  label: string;
  today: boolean;
  onPrev?: () => void;
  onNext?: () => void;
  children?: ReactNode;
}) {
  return (
    <header className="md-daynav">
      <button type="button" className="md-nav-button" aria-label="Jour précédent" data-tip="Jour précédent" disabled={!onPrev} onClick={onPrev}>
        <Icon name="prev" />
      </button>
      <span className={`md-caps${today ? " is-date" : ""}`}>{label}</span>
      <button type="button" className="md-nav-button" aria-label="Jour suivant" data-tip="Jour suivant" disabled={!onNext} onClick={onNext}>
        <Icon name="next" />
      </button>
      <span className="md-daynav-side">{children}</span>
    </header>
  );
}

export type Theme = "auto" | "light" | "dark";
const THEME_ORDER: Theme[] = ["auto", "light", "dark"];
const THEME_NAME: Record<Theme, string> = { auto: "automatique", light: "clair", dark: "sombre" };

/** Cycles automatic, light, dark. The icon shows the theme in use. */
export function ThemeButton({ theme, onChange }: { theme: Theme; onChange: (next: Theme) => void }) {
  const next = THEME_ORDER[(THEME_ORDER.indexOf(theme) + 1) % THEME_ORDER.length];
  return <IconButton icon={theme} label={`Thème : ${THEME_NAME[theme]}`} onClick={() => onChange(next)} />;
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
