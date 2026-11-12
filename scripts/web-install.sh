#!/usr/bin/env bash
set -euo pipefail
umask 077
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"
mode= origin= check=0
usage() { echo '用法: bash scripts/web-install.sh [--mode caddy|proxy|local] [--origin https://域名] [--check]'; }
fail() { echo "错误：$*" >&2; exit 1; }
while (($#)); do
  case "$1" in
    --mode|--origin) (($# >= 2)) || fail '参数缺少值'; if [[ $1 == --mode ]]; then mode=$2; else origin=$2; fi; shift 2;;
    --check) check=1; shift;;
    -h|--help) usage; exit 0;;
    *) usage; fail "未知参数：$1";;
  esac
done
for tool in docker openssl; do command -v "$tool" >/dev/null || fail "请先安装 $tool"; done
docker compose version >/dev/null || fail '需要 Docker Compose v2'
docker info >/dev/null 2>&1 || fail 'Docker 未运行或当前用户无权限'
if [[ -z $mode ]]; then
  [[ -t 0 ]] || fail '非交互运行请指定 --mode'
  read -r -p '部署模式 caddy（自动 HTTPS）/proxy（宝塔）/local（本机）[proxy]: ' mode
  mode=${mode:-proxy}
fi
case "$mode" in
  local) origin=${origin:-http://localhost:8080}; [[ $origin == http://localhost:8080 || $origin == http://127.0.0.1:8080 ]] || fail 'local 仅允许 localhost 或 127.0.0.1:8080'; domain=localhost;;
  caddy|proxy)
    if [[ -z $origin && -t 0 ]]; then read -r -p '完整访问地址（https://域名，不带末尾斜杠）: ' origin; fi
    [[ $origin =~ ^https://[A-Za-z0-9]([A-Za-z0-9.-]*[A-Za-z0-9])?$ ]] || fail '请填写 https://公开域名，不带端口、路径、凭据或斜杠'
    domain=${origin#https://}
    [[ $domain == *.* && $domain != *..* && $domain != *.local && $domain != *.localhost && ! $domain =~ ^[0-9.]+$ ]] || fail '需要公开 DNS 域名';;
  *) fail '模式必须是 caddy、proxy 或 local';;
esac
if ((check)); then echo '前置检查通过；未创建文件或启动服务。'; exit 0; fi
[[ ! -L .hghub-web ]] || fail '配置目录不得是符号链接'
[[ ! -e .hghub-web/access-password && ! -L .hghub-web/access-password && ! -e .hghub-web/config.env && ! -L .hghub-web/config.env ]] || fail '配置已存在，不覆盖密码；请按教程更新或重试启动'
mkdir -p .hghub-web
chmod 700 .hghub-web
openssl rand -hex 24 > .hghub-web/access-password
chmod 444 .hghub-web/access-password
printf 'HGHUB_WEB_ORIGIN=%s\nHGHUB_WEB_DOMAIN=%s\n' "$origin" "$domain" > .hghub-web/config.env
printf '%s\n' "$mode" > .hghub-web/mode
args=(-f compose.web.yml)
[[ $mode != caddy ]] || args+=(--profile https)
docker compose "${args[@]}" up -d --build
echo "服务启动命令已完成。访问：$origin"
echo '访问密码保存在 .hghub-web/access-password，请用可信编辑器查看；不要发到聊天、截图或日志中。'
echo '首次构建失败可保留配置，按教程重新执行 compose 启动命令。'
