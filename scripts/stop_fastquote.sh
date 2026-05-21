#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

PID_FILE="${FASTQUOTE_PID_FILE:-${ROOT_DIR}/run/fastquote.pid}"
LOG_DIR="${FASTQUOTE_LOG_DIR:-${ROOT_DIR}/logs}"
LOG_FILE="${FASTQUOTE_LOG_FILE:-${LOG_DIR}/fastquote-$(date +%F).log}"
STOP_TIMEOUT_SECONDS="${FASTQUOTE_STOP_TIMEOUT_SECONDS:-20}"

mkdir -p "${LOG_DIR}" "$(dirname "${PID_FILE}")"

if [[ ! -f "${PID_FILE}" ]]; then
    echo "fastquote is not running: pid file not found"
    exit 0
fi

pid="$(cat "${PID_FILE}" 2>/dev/null || true)"
if [[ -z "${pid}" ]] || ! kill -0 "${pid}" 2>/dev/null; then
    rm -f "${PID_FILE}"
    echo "fastquote is not running: stale pid file removed"
    exit 0
fi

{
    echo
    echo "[$(date '+%F %T')] stopping fastquote pid ${pid}"
} >>"${LOG_FILE}"

kill "${pid}" 2>/dev/null || true

for _ in $(seq 1 "${STOP_TIMEOUT_SECONDS}"); do
    if ! kill -0 "${pid}" 2>/dev/null; then
        rm -f "${PID_FILE}"
        echo "fastquote stopped: pid ${pid}"
        exit 0
    fi
    sleep 1
done

{
    echo "[$(date '+%F %T')] fastquote pid ${pid} did not stop after ${STOP_TIMEOUT_SECONDS}s; sending SIGKILL"
} >>"${LOG_FILE}"

kill -KILL "${pid}" 2>/dev/null || true
rm -f "${PID_FILE}"
echo "fastquote killed: pid ${pid}"
