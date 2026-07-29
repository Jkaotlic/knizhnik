import React from "react";

// Без этого любая ошибка при отрисовке даёт молчаливую пустоту: React сносит
// поддерево, а человек видит «пустую полку» и думает, что данные потерялись.
// Лучше показать, что именно сломалось.

interface State {
  error: Error | null;
}

export class ErrorBoundary extends React.Component<{ children: React.ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    // Уходит в консоль вебвью — видно при отладке
    console.error("Ошибка отрисовки:", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div style={{ padding: 24 }}>
        <div className="eyebrow">Сбой интерфейса</div>
        <h2 className="page-title" style={{ marginBottom: 12 }}>Экран не отрисовался</h2>
        <p className="error-note" style={{ whiteSpace: "pre-wrap" }}>
          {this.state.error.message || String(this.state.error)}
        </p>
        <p className="small muted" style={{ marginTop: 10 }}>
          Данные в базе целы — сломалось только отображение.
        </p>
        <div className="btn-row" style={{ marginTop: 14 }}>
          <button className="btn btn--primary" onClick={() => this.setState({ error: null })}>
            Попробовать снова
          </button>
        </div>
      </div>
    );
  }
}
