#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

PATH="${HOME}/.cargo/bin:/usr/local/bin:/usr/bin:/bin:${PATH:-}"

BIN_PATH="${FASTQUOTE_BIN:-${ROOT_DIR}/target/release/fastquote}"
PID_FILE="${FASTQUOTE_PID_FILE:-${ROOT_DIR}/run/fastquote.pid}"
LOG_DIR="${FASTQUOTE_LOG_DIR:-${ROOT_DIR}/logs}"
LOG_FILE="${FASTQUOTE_LOG_FILE:-${LOG_DIR}/fastquote-$(date +%F).log}"
START_CMD="${FASTQUOTE_START_CMD:-}"

mkdir -p "${LOG_DIR}" "$(dirname "${PID_FILE}")" "${ROOT_DIR}/data"

if [[ -f "${PID_FILE}" ]]; then
    old_pid="$(cat "${PID_FILE}" 2>/dev/null || true)"
    if [[ -n "${old_pid}" ]] && kill -0 "${old_pid}" 2>/dev/null; then
        echo "fastquote already running: pid ${old_pid}"
        exit 0
    fi
    rm -f "${PID_FILE}"
fi

cd "${ROOT_DIR}"

if [[ -n "${START_CMD}" ]]; then
    command=(bash -lc "${START_CMD}")
else
    if [[ ! -x "${BIN_PATH}" ]]; then
        cargo build --release -p fastquote-bin >>"${LOG_FILE}" 2>&1
    fi
    command=(env RUST_LOG="${RUST_LOG:-info}" "${BIN_PATH}")
fi

{
    echo
    echo "[$(date '+%F %T')] starting fastquote"
    echo "root=${ROOT_DIR}"
    echo "log=${LOG_FILE}"
} >>"${LOG_FILE}"

nohup "${command[@]}" >>"${LOG_FILE}" 2>&1 &
pid="$!"
echo "${pid}" >"${PID_FILE}"

echo "fastquote started: pid ${pid}, log ${LOG_FILE}"
