#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

START_SCRIPT="${ROOT_DIR}/scripts/start_fastquote.sh"
STOP_SCRIPT="${ROOT_DIR}/scripts/stop_fastquote.sh"
CRON_LOG="${ROOT_DIR}/logs/fastquote-cron.log"

mkdir -p "${ROOT_DIR}/logs"

escape_cron_path() {
    printf '%s' "$1" | sed 's/ /\\ /g'
}

root_escaped="$(escape_cron_path "${ROOT_DIR}")"
start_escaped="$(escape_cron_path "${START_SCRIPT}")"
stop_escaped="$(escape_cron_path "${STOP_SCRIPT}")"
cron_log_escaped="$(escape_cron_path "${CRON_LOG}")"

begin_marker="# fastquote schedule begin"
end_marker="# fastquote schedule end"

tmp_file="$(mktemp)"
trap 'rm -f "${tmp_file}"' EXIT

crontab -l 2>/dev/null | sed "/${begin_marker}/,/${end_marker}/d" >"${tmp_file}" || true

cat >>"${tmp_file}" <<EOF
${begin_marker}
30 9 * * 1-5 cd ${root_escaped} && ${start_escaped} >> ${cron_log_escaped} 2>&1
0 15 * * 1-5 cd ${root_escaped} && ${stop_escaped} >> ${cron_log_escaped} 2>&1
${end_marker}
EOF

crontab "${tmp_file}"

echo "installed fastquote cron schedule:"
echo "  start: weekdays 09:30"
echo "  stop:  weekdays 15:00"
echo "  log:   ${CRON_LOG}"
