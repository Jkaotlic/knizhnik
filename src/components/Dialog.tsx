import { createContext, useCallback, useContext, useMemo, useRef, useState } from "react";

// Встроенные диалоги вместо window.prompt/confirm/alert:
// в macOS WKWebView нативный prompt() не работает (возвращает null).

type PromptOpts = { defaultValue?: string; placeholder?: string };
type ConfirmOpts = { danger?: boolean; okLabel?: string };

interface DialogApi {
  prompt: (title: string, opts?: PromptOpts) => Promise<string | null>;
  confirm: (title: string, opts?: ConfirmOpts) => Promise<boolean>;
  alert: (title: string) => Promise<void>;
}

const Ctx = createContext<DialogApi | null>(null);

export function useDialog(): DialogApi {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useDialog вне DialogProvider");
  return ctx;
}

type State =
  | { kind: "prompt"; title: string; placeholder?: string; resolve: (v: string | null) => void }
  | { kind: "confirm"; title: string; danger?: boolean; okLabel?: string; resolve: (v: boolean) => void }
  | { kind: "alert"; title: string; resolve: () => void }
  | null;

export function DialogProvider({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<State>(null);
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const api = useMemo<DialogApi>(
    () => ({
      prompt: (title, opts) =>
        new Promise((resolve) => {
          setValue(opts?.defaultValue ?? "");
          setState({ kind: "prompt", title, placeholder: opts?.placeholder, resolve });
        }),
      confirm: (title, opts) =>
        new Promise((resolve) => setState({ kind: "confirm", title, danger: opts?.danger, okLabel: opts?.okLabel, resolve })),
      alert: (title) => new Promise((resolve) => setState({ kind: "alert", title, resolve })),
    }),
    []
  );

  const done = useCallback(
    (result: string | boolean | null | void) => {
      if (!state) return;
      (state.resolve as (v: unknown) => void)(result);
      setState(null);
    },
    [state]
  );

  const cancel = () => done(state?.kind === "prompt" ? null : state?.kind === "confirm" ? false : undefined);
  const ok = () => done(state?.kind === "prompt" ? value : state?.kind === "confirm" ? true : undefined);

  return (
    <Ctx.Provider value={api}>
      {children}
      {state && (
        <div className="modal-overlay" onMouseDown={(e) => e.target === e.currentTarget && cancel()}>
          <div className="modal" role="dialog" aria-modal="true">
            <div className="modal__title">{state.title}</div>

            {state.kind === "prompt" && (
              <input
                ref={inputRef}
                className="input"
                style={{ width: "100%" }}
                autoFocus
                value={value}
                placeholder={state.placeholder}
                onChange={(e) => setValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") ok();
                  if (e.key === "Escape") cancel();
                }}
              />
            )}

            <div className="modal__actions">
              {state.kind !== "alert" && (
                <button className="btn btn--ghost" onClick={cancel}>Отмена</button>
              )}
              <button
                className={`btn ${state.kind === "confirm" && state.danger ? "btn--brass" : "btn--primary"}`}
                onClick={ok}
                autoFocus={state.kind !== "prompt"}
              >
                {state.kind === "confirm" ? state.okLabel ?? "Да" : "ОК"}
              </button>
            </div>
          </div>
        </div>
      )}
    </Ctx.Provider>
  );
}
