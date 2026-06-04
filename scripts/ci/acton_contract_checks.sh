#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"

acton_version="${ACTON_VERSION:-1.1.0}"
docker_image="${ACTON_DOCKER_IMAGE:-ghcr.io/ton-blockchain/acton:${acton_version}}"
use_docker="${ACTON_USE_DOCKER:-auto}"
check_output_format="${ACTON_CHECK_OUTPUT_FORMAT:-}"

docker_args=(
  --rm
  -v "${repo_root}:/workspace"
  -w /workspace
  -e HOME=/tmp/acton-home
  -e XDG_CACHE_HOME=/tmp/acton-cache
)

if [[ -n "${CI:-}" ]]; then
  docker_args+=(-e CI="${CI}")
fi

if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
  docker_args+=(-e GITHUB_ACTIONS="${GITHUB_ACTIONS}")
fi

run_acton() {
  if [[ "${use_docker}" == "1" || "${use_docker}" == "true" ]]; then
    docker run "${docker_args[@]}" "${docker_image}" "$@"
    return
  fi

  if [[ "${use_docker}" == "auto" && ! -x "$(command -v acton 2>/dev/null)" ]]; then
    docker run "${docker_args[@]}" "${docker_image}" "$@"
    return
  fi

  acton "$@"
}

if [[ "${use_docker}" == "1" || "${use_docker}" == "true" ]]; then
  command -v docker >/dev/null
elif [[ "${use_docker}" == "auto" ]] && ! command -v acton >/dev/null; then
  command -v docker >/dev/null
fi

check_args=()
if [[ -n "${check_output_format}" ]]; then
  check_args+=(--output-format "${check_output_format}")
fi

cd "${repo_root}"

run_acton --version
run_acton doctor
run_acton build
run_acton test
run_acton check "${check_args[@]}"
run_acton fmt --check
