#!/usr/bin/env bash
# Magpie Linux/NAS runner.
# Usage: ./run-loop.sh issues|community|nightly
set -euo pipefail

LOOP="${1:-}"
case "$LOOP" in
    issues|community|nightly) ;;
    *)
        echo "usage: $0 issues|community|nightly" >&2
        exit 2
        ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${MAGPIE_REPO_ROOT:-$(cd "$SCRIPT_DIR/../../.." && pwd)}"
MAGPIE_DIR="$REPO_ROOT/automation/magpie"
CONFIG="$MAGPIE_DIR/config.json"
STATE_DIR="$MAGPIE_DIR/state"
LOG_DIR="$MAGPIE_DIR/logs"
mkdir -p "$STATE_DIR" "$LOG_DIR"

STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$LOG_DIR/loop-$LOOP-$STAMP.log"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG"
}

config_value() {
    node -e "process.stdout.write(String(require('$CONFIG').$1))"
}

case "$LOOP" in
    issues) PROMPT="$MAGPIE_DIR/loops/01-issues.md" ;;
    community) PROMPT="$MAGPIE_DIR/loops/02-community.md" ;;
    nightly) PROMPT="$MAGPIE_DIR/loops/03-nightly.md" ;;
esac

cd "$REPO_ROOT"

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
BASE_BRANCH="$(config_value repository.base_branch)"
if [ "$BRANCH" != "$BASE_BRANCH" ]; then
    log "ERROR: automation must run on '$BASE_BRANCH' (current: '$BRANCH')."
    exit 1
fi
if [ -n "$(git status --porcelain)" ]; then
    log "ERROR: working tree is not clean. Refusing to let automation touch user changes."
    exit 1
fi

# One global lock: issues/community/nightly must never edit the same checkout at once.
exec 9>"$STATE_DIR/run.lock"
if ! flock -n 9; then
    log "WARN: another Magpie loop is already running; skipping this run."
    exit 0
fi

# Empty-inbox fast path: do not boot the model when there is nothing to do.
if [ "$LOOP" != "nightly" ]; then
    PRECHECK_JSON="$STATE_DIR/precheck-$LOOP-$STAMP.json"
    PRECHECK_ERR="$STATE_DIR/precheck-$LOOP-$STAMP.err.log"
    if pwsh -NoProfile -File "$MAGPIE_DIR/get-feedback.ps1" -Json > "$PRECHECK_JSON" 2> "$PRECHECK_ERR"; then
        FEEDBACK_COUNT="$(node -e "try{const j=require(process.argv[1]); process.stdout.write(String(j.count ?? 0))}catch(e){process.stdout.write('ERR')}" "$PRECHECK_JSON")"
    else
        FEEDBACK_COUNT="ERR"
    fi

    if [ "$FEEDBACK_COUNT" = "0" ] && [ ! -s "$PRECHECK_ERR" ]; then
        REPORT="$STATE_DIR/report-$LOOP-$STAMP.md"
        {
            echo "# Magpie Loop $LOOP — 空跑"
            echo
            echo "- 时间：$(date '+%Y-%m-%d %H:%M:%S %z')"
            echo "- 结果：没有新的 issue / PR 反馈。"
            echo "- 动作：跳过 opencode，不消耗模型 tokens。"
        } > "$REPORT"
        log "No feedback (count=0); skipped opencode to save tokens."
        exit 0
    fi

    if [ "$FEEDBACK_COUNT" = "ERR" ] || [ -s "$PRECHECK_ERR" ]; then
        log "WARN: feedback precheck failed; continuing with opencode. See $PRECHECK_ERR"
    fi
fi

MODEL="${MAGPIE_MODEL:-$(config_value agent.model)}"
TIMEOUT_MINUTES="${MAGPIE_TIMEOUT_MINUTES:-$(config_value agent.timeout_minutes)}"
PROMPT_FILE="$STATE_DIR/prompt-$LOOP-$STAMP.md"

{
    echo "# Magpie 自动维护上下文"
    echo
    echo "- 时间：$(date '+%Y-%m-%d %H:%M:%S %z')"
    echo "- 循环：$LOOP"
    echo "- 仓库：$REPO_ROOT"
    echo "- 配置文件：$CONFIG"
    echo "- 状态目录：$STATE_DIR"
    echo "- 日志目录：$LOG_DIR"
    echo "- 分支：$BRANCH"
    echo "- 自动合并：$(config_value automation.auto_merge)"
    echo "- 自动发版：$(config_value automation.auto_release)（模式：$(config_value automation.release_mode)）"
    echo
    echo "请严格按下面这份循环提示执行。你处于无人值守模式：不要向用户提问；无法决定的事项写入状态目录下的 questions 文件并跳过。执行前先读 docs/agent/CODE-STANDARDS.md 和 docs/agent/LESSONS.md。"
    echo
    echo "---"
    echo
    cat "$PROMPT"
} > "$PROMPT_FILE"

export MAGPIE_LOOP="$LOOP"
export MAGPIE_STATE_DIR="$STATE_DIR"
export MAGPIE_GATE_QUICK="${MAGPIE_GATE_QUICK:-1}"

log "Starting opencode loop '$LOOP' with model '$MODEL' (timeout ${TIMEOUT_MINUTES}m)."

set +e
timeout "${TIMEOUT_MINUTES}m" opencode run \
    --auto \
    --format default \
    --agent build \
    --model "$MODEL" \
    --title "magpie/$LOOP/$STAMP" \
    --file "$PROMPT_FILE" \
    "执行附件中的 Magpie 自动化维护循环。" >> "$LOG" 2>&1
CODE=$?
set -e

log "Loop '$LOOP' finished with exit code $CODE."
exit "$CODE"
