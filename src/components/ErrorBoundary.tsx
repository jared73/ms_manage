/**
 * 全局错误边界:捕获渲染期抛错,显示降级 UI 而不是白屏。
 *
 * 设计:
 * - UI 上只显示简要错误信息(防止把文件路径、源码片段、token 等敏感 stack
 *   信息直接渲染到屏幕上)
 * - 完整 stack 仍然写进后端日志 + 控制台,事后排查不受影响
 * - 文案走 i18n
 * - ErrorFallback 拆到 ./ErrorFallback.tsx,本文件只剩 ErrorBoundary 类,
 *   满足 react-refresh 的 fast-refresh 约束
 */
import { Component, type ErrorInfo, type ReactNode } from "react";
import { logEvent } from "@/lib/ipc";
import { describeError } from "@/lib/errors";
import { ErrorFallback } from "./ErrorFallback";

interface Props {
  children: ReactNode;
  /** 出错时显示哪个分段名,默认 "应用" */
  label?: string;
}

interface State {
  err: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  override state: State = { err: null };

  static getDerivedStateFromError(err: Error): State {
    return { err };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    // 写到后端日志(失败时静默)—— 这一份是完整的 stack,排查用
    // 把 error.stack 也写进去(componentStack 是 React 组件树,error.stack 是真实异常栈)
    const fullDetail = [
      info.componentStack ?? "",
      "---- error.stack ----",
      error.stack ?? "(no stack)",
    ]
      .join("\n")
      .slice(0, 4000);
    // logEvent 是 async fire-and-forget:必须 .catch,否则 unhandled rejection
    // 会被 Promise 链吞掉且在 dev 模式下报红
    logEvent(
      "error",
      "frontend",
      `渲染崩溃: ${describeError(error)}`,
      fullDetail,
    ).catch(() => {
      // 后端写日志失败,完全静默
    });
    // 控制台留一份原始 stack,方便开发
    console.error("[ErrorBoundary]", error, info);
  }

  private handleReload = () => {
    // 重置错误态再刷新 —— 让用户能继续往下走而不必整个 app 重启
    this.setState({ err: null });
  };

  private handleHardReload = () => {
    window.location.reload();
  };

  override render(): ReactNode {
    const { err } = this.state;
    if (!err) return this.props.children;
    return <ErrorFallback label={this.props.label} message={describeError(err)} onRetry={this.handleReload} onHardReload={this.handleHardReload} />;
  }
}