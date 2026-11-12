# HGhub 个人服务器网页版部署

这是独立的 `web/personal-server` 分支，保留原客户端。服务器运行后，通过浏览器访问，不需要安装客户端、IPA 签名或刷新签名。它不是第三方影视服务的替代后端：上游接口、账号规则、地域限制、片源失效仍可能影响使用。仅供个人、非商业使用，请遵守仓库许可证及内容权利要求，不要公开分享访问密码或做公共影视站。

## 当前版本范围

- 私有单用户、上游游客浏览版。入口密码只保护你自己的网页，不是作者网站的账号。上游登录、注册、会员、云收藏等账号操作没有移植，请继续使用原客户端。
- 上游线路由服务器管理员通过 `HGHUB_WEB_UPSTREAM` 设置为可信 HTTPS origin，默认原站；浏览器不能更改全局线路或关闭网络安全保护。
- 会话有效期 12 小时，服务器重启或改密码后需重新输入入口密码。播放中到期需刷新并登录。
- 图片上限 8 MiB，播放列表 2 MiB，单个媒体对象/Range 响应上限 512 MiB；超大非分片视频、HLS 变量列表暂不支持。服务器出口会承担媒体流量，请注意带宽和流量账单。
- 已通过 21 项后端测试、网页/原生前端构建、Chromium 登录/退出/会话过期与 iPhone UA 分支测试，以及真实 Docker 镜像构建和容器健康/鉴权检查：[验证记录](https://github.com/nbbk/HGhub/actions/runs/43179928513)。这不是 iPhone 真机或真实片源播放验证；本次环境的上游 DNS 解析失败，仍需在你的服务器出口确认片源可用。

## 选择最简单的方式

| 场景 | 模式 | 做法 |
|---|---|---|
| 新服务器，有域名，80/443 没被占用 | caddy | 一条脚本命令构建并启动，Caddy 自动申请 HTTPS |
| 已有宝塔/Nginx | proxy | 脚本启动应用，宝塔添加 HTTPS 反向代理 |
| 没有域名，仅电脑测试 | local | 回环 HTTP + SSH 隧道，不对公网开放 |

建议起步 2 核、2 GB 内存并预留数 GB 磁盘；源码首次构建可能需要更多内存和时间。Docker 镜像、npm 和 Rust 依赖需要网络访问。请通过 Docker 官方文档或宝塔官方界面安装 Docker Engine 和 Compose v2，不执行来源不明的 `curl | sh`。脚本不会自动安装 Docker、修改防火墙、增加用户组或部署到其他机器。

```bash
docker version
docker compose version
git clone --single-branch --branch web/personal-server https://github.com/nbbk/HGhub.git HGhub-web
cd HGhub-web
```

已有同名目录不要覆盖，换个目录名即可。以下命令均在仓库目录执行。

## 一、独立服务器一键启动（推荐）

1. 把你自己的域名 A 记录指向服务器公网 IPv4；只有服务器 IPv6 正常时才添加 AAAA。
2. 自行在云安全组/服务器防火墙放行 80、443。不要开放 8080。确保没有其他站点占用 80/443。
3. 把下面示例域名换成自己的：

```bash
bash scripts/web-install.sh --mode caddy --origin https://video.example.com
```

也可直接运行 `bash scripts/web-install.sh` 按提示选择模式。加 `--check` 只做前置检查，不创建配置或启动容器。

首次会构建前端和 Rust 服务，再启动 Caddy。DNS 和端口正确后访问你自己的 HTTPS 地址。访问密码自动随机生成，保存在 `.hghub-web/access-password`，请用可信编辑器查看并保存到自己的密码管理器。**脚本不把密码打印到终端，也不要把密码发到聊天、工单或截图。** 这是私有站点的访问密码，不是上游账号密码。

配置目录权限为 700；密码文件为 444，以便非 root 容器读取绑定挂载，但其他普通系统用户不能穿过外层目录。服务器 root 和 Docker 管理员仍可读取秘密。`.hghub-web/` 被 Git 和 Docker 构建上下文排除，不要手动上传它。

首次构建失败不要删除配置或反复运行初始化脚本，排除网络/磁盘问题后重试：

```bash
docker compose -f compose.web.yml --profile https up -d --build
```

## 二、宝塔部署

适合宝塔已经管理网站和 HTTPS 的服务器，**不要同时启动 Caddy 占用 80/443**。

1. 在宝塔终端确认 Docker/Compose v2 可用，下载本分支代码。
2. 初始化应用：

```bash
bash scripts/web-install.sh --mode proxy --origin https://video.example.com
```

3. 宝塔「网站」添加你自己的域名站点，不需要 PHP 或 Node 项目。
4. 在该站点「SSL」申请或安装有效证书，启用 HTTPS；可开启强制 HTTPS。
5. 「反向代理」新增代理，目标 URL 为 `http://127.0.0.1:8080`，发送域名使用当前域名，代理目录为 `/`。关闭缓存。
6. 外部访问地址必须与初始化时 `HGHUB_WEB_ORIGIN` 完全一致。不要部署在 `/hghub/` 子路径。

若手动编辑 Nginx，在已有 HTTPS `server` 块使用以下 location（不要和面板生成的同名 location 重复）：

```nginx
location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header Range $http_range;
    proxy_set_header If-Range $http_if_range;
    proxy_buffering off;
    proxy_cache off;
    proxy_read_timeout 120s;
    client_max_body_size 1m;
}
```

修改后先检查 Nginx 配置再重载。证书签发所需 DNS、80/443 放行由你管理。应用默认仅发布在服务器 `127.0.0.1:8080`，不要改成 `0.0.0.0` 来解决代理问题。如果宝塔/Nginx 自身也在容器中，它的 `127.0.0.1` 不是宿主机，应由管理员配置专用容器网络，不要直接把后端端口暴露公网。

## 三、没有域名先测试

```bash
bash scripts/web-install.sh --mode local
```

应用只监听服务器回环发布端口。在自己的电脑建立隧道：

```bash
ssh -L 8080:127.0.0.1:8080 你的用户名@服务器IP
```

保持 SSH 打开，在这台电脑访问 `http://localhost:8080`。iPhone 的 localhost 指向 iPhone 本身，不能用这个地址访问电脑；日常手机使用请选择域名 + HTTPS。

## 四、更新、查看状态和停止

```bash
docker compose -f compose.web.yml ps
docker compose -f compose.web.yml logs --tail=100 app
git pull --ff-only
docker compose -f compose.web.yml build app
docker compose -f compose.web.yml up -d app
```

Caddy 模式查看完整状态用 `docker compose -f compose.web.yml --profile https ps`。不要公开日志里的访问链接、令牌、账号信息。更新保留 `.hghub-web/`，不要再次运行初始化。拉取失败先备份并处理本地修改，不要用强制重置覆盖自己的配置。

修改域名：用编辑器修改 `.hghub-web/config.env` 内 origin/domain，完成 DNS/证书配置后重建容器；Caddy 模式加 `--profile https`。更改访问密码：

```bash
chmod 600 .hghub-web/access-password
# 用可信编辑器写入新的至少 24 位随机密码，不要把明文写进 shell 命令或日志
chmod 444 .hghub-web/access-password
docker compose -f compose.web.yml up -d --force-recreate app
```

重启后需要重新登录。停止但保留本地配置及 Caddy 证书卷：

```bash
docker compose -f compose.web.yml --profile https down
```

不要随意添加 `-v`，以免删除证书数据。备份配置时应加密，并限制备份访问权限。

## 五、Cloudflare 不需要

本方案不需要 Cloudflare 账号或 Tunnel。若域名 DNS 托管在 Cloudflare，建议使用 **DNS only / 灰云** 直接指向服务器，不推荐用免费橙云 CDN 或 Tunnel 作为视频流量代理。视频传输涉及平台政策及流量限制，参见 [Cloudflare 视频交付政策](https://developers.cloudflare.com/fundamentals/reference/policies-compliances/delivering-videos-with-cloudflare/)。服务器公网流量费用和带宽仍由你承担。

## 常见问题与限制

- **502**：检查 app 容器健康及反向代理目标是否为宿主机 127.0.0.1:8080。
- **HTTPS 申请失败**：检查 DNS、错误 AAAA、80/443 占用及安全组；不要关闭浏览器证书检查。
- **登录后仍跳回入口**：确认访问域名和 `HGHUB_WEB_ORIGIN` 一致，公网必须 HTTPS，浏览器允许本网站 Cookie。
- **能打开但不能播放**：可能是第三方接口/片源、地域、格式、浏览器或服务器出口限制，不等于必须安装客户端。不能保证所有源都能播放。
- **iPhone**：使用 Safari，播放可能需要手动点播放；自动播放、后台播放、全屏表现受 iOS 限制。手机真机播放需另行验证，不能把构建成功当作播放验证通过。
- **原作者后续改接口/账号规则**：服务器版仍依赖上游，可能需要更新；不能保证永久可用。
- 本方案仅面向个人单用户，不是多用户托管平台，不要移除访问保护做开放代理。

代码和脚本提供部署能力，不代表已在你的服务器执行部署。Docker 构建与容器启动已通过 CI；你的域名证书签发、服务器出口和真实片源播放仍须在部署环境验证。
