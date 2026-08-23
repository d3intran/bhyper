# 🚇 BHyper Web & Telegram Mini App 云端部署与 Cloudflare 零信任配置指南

本文档提供在云端 VPS（Ubuntu 24.04 LTS / Azure Tokyo）上一站式配置 **BHyper Web 控制台**、**Cloudflare Tunnel (`cloudflared`)**、**Cloudflare Zero Trust (Access)** 及 **Telegram Mini App (TMA)** 的全流程实操指南。

---

## 📑 目录
1. [架构概述](#1-架构概述)
2. [云端 VPS 安装与配置 cloudflared](#2-云端-vps-安装与配置-cloudflared)
   - 方式 A：30 秒快速临时测试（Quick Tunnel，免域名）
   - 方式 B：生产级固定域名隧道（Named Tunnel，推荐）
3. [配置 Cloudflare Zero Trust (Access) 边缘身份防线](#3-配置-cloudflare-zero-trust-access-边缘身份防线)
4. [配置 Telegram Mini App (BotFather)](#4-配置-telegram-mini-app-botfather)
5. [配置 Systemd 24/7 后台无人值守守护进程](#5-配置-systemd-247-后台无人值守守护进程)
6. [常见问题排查 (FAQ)](#6-常见问题排查-faq)

---

## 1. 架构概述

```
[手机/电脑 Telegram / 浏览器]
           │ (HTTPS / WSS)
           ▼
[🛡️ Cloudflare Zero Trust (邮箱验证/Google OAuth/TG HMAC 鉴权)]
           │ (QUIC 加密通道)
           ▼
[🚇 cloudflared (VPS 本地出站隧道)]
           │ (HTTP: http://127.0.0.1:8080)
           ▼
[⚡ BHyper Axum Web Server (嵌入式单文件终端 + 实时 WebSocket + 内存热调参)]
```

- **公网零暴露**：VPS 无需开放任何入站端口（80/443 全关），不暴露真实 IP。
- **自动 HTTPS**：Cloudflare 自动提供边缘 TLS 证书，满足 Telegram WebApp 强制 HTTPS 要求。
- **双端自适应**：同一套前端自动适配 PC 宽屏量化工作站与手机 Telegram 触屏操作。

---

## 2. 云端 VPS 安装与配置 cloudflared

SSH 连接到你的 VPS（参考 `vps.local.md`）：
```bash
ssh -i ~/.ssh/id_rsa azureuser@20.78.160.171
```

### 步骤 1：安装 `cloudflared`
```bash
# 导入 Cloudflare 官方 GPG 密钥
sudo mkdir -p --mode=0755 /usr/share/keyrings
curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg | sudo tee /usr/share/keyrings/cloudflare-main.gpg >/dev/null

# 添加 apt 软件源
echo 'deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared any main' | sudo tee /etc/apt/sources.list.d/cloudflared.list

# 更新并安装
sudo apt-get update && sudo apt-get install -y cloudflared

# 验证安装
cloudflared --version
```

---

### 方式 A：30 秒极速临时测试（Quick Tunnel - 免域名）

如果你想立即在手机或浏览器体验，无需绑定域名：

1. 在 VPS 上启动 BHyper Web 服务：
   ```bash
   cd ~/bhyper_deploy
   ./bhyper web --port 8080
   ```
2. 在另一个终端窗口启动 Quick Tunnel：
   ```bash
   cloudflared tunnel --url http://127.0.0.1:8080
   ```
3. 控制台会输出一个临时公网 HTTPS 地址（例如 `https://random-words.trycloudflare.com`），直接在手机浏览器或 Telegram 打开即可！

---

### 方式 B：生产级固定域名隧道（Named Tunnel - 推荐）

1. **登录授权 Cloudflare 账号**：
   ```bash
   cloudflared tunnel login
   ```
   *终端会输出一个 URL，在浏览器中打开并选择你的域名授权，授权后会在 `~/.cloudflared/cert.pem` 生成凭证。*

2. **创建命名隧道**：
   ```bash
   cloudflared tunnel create bhyper-tunnel
   ```
   *记下输出的 Tunnel-ID（如 `3a8f9c12-xxxx-xxxx-xxxx-xxxxxxxxxxxx`）。*

3. **配置本地映射文件 (`~/.cloudflared/config.yml`)**：
   ```bash
   mkdir -p ~/.cloudflared
   cat << 'EOF' > ~/.cloudflared/config.yml
   tunnel: bhyper-tunnel
   credentials-file: /home/azureuser/.cloudflared/YOUR_TUNNEL_ID.json

   ingress:
     - hostname: app.yourdomain.com
       service: http://127.0.0.1:8080
     - service: http_status:404
   EOF
   ```
   *(将 `YOUR_TUNNEL_ID` 和 `app.yourdomain.com` 替换为你的真实 ID 和域名)*

4. **将域名解析路由到隧道**：
   ```bash
   cloudflared tunnel route dns bhyper-tunnel app.yourdomain.com
   ```

5. **安装为系统服务自启**：
   ```bash
   sudo cloudflared service install
   sudo systemctl enable --now cloudflared
   ```

---

## 3. 配置 Cloudflare Zero Trust (Access) 边缘身份防线

为了防止公网任何人未授权访问你的 Web 监控界面，建议在 Cloudflare 仪表盘配置 Zero Trust 访问策略：

1. 登录 [Cloudflare Dashboard](https://dash.cloudflare.com/) -> 进入 **Zero Trust** 控制台。
2. 导航到 **Access** -> **Applications** -> 点击 **Add an Application**。
3. 选择 **Self-hosted**：
   - **Application name**: `BHyper Quant Terminal`
   - **Application domain**: `app.yourdomain.com`
4. 配置 **Policy (策略)**：
   - **Policy name**: `Allow Admin Only`
   - **Action**: `Allow`
   - **Include Rule**: 
     - Selector: `Emails`
     - Value: 填入你个人的邮箱（如 `your_email@gmail.com`）
5. 点击 **Save**。
6. 现在，任何人访问 `https://app.yourdomain.com` 时，必须输入邮箱收到的 6 位验证码（或 Google OAuth 登录）才能进入，未经授权的流量根本碰不到你的 VPS！

---

## 4. 配置 Telegram Mini App (BotFather)

通过 Telegram Mini App，你可以直接在 Telegram 聊天窗口中点一个按钮秒开控制台，自动带 HMAC 签名免密登录：

1. 打开 Telegram，搜索并进入 `@BotFather`。
2. 发送指令 `/newapp`。
3. 选择你现有的 BHyper 预警 Bot。
4. 输入 Mini App 标题（例如 `BHyper Quant Matrix`）。
5. 输入 Mini App 简介（例如 `Real-time Funding Rate & Basis Arbitrage Console`）。
6. 上传图标（建议 640x360 缩略图，可截图仪表盘）。
7. **Web App URL**：输入你的 Cloudflare 域名（例如 `https://app.yourdomain.com`）。
8. 输入 Short Name（例如 `app`）。
9. 配置 Bot 左下角快捷菜单按钮：
   - 发送 `/setmenubutton` -> 选择你的 Bot。
   - 选择 **Configure menu button** -> 输入按钮文案（例如 `⚡ 打开量化控制台`）。
   - 输入 Web App URL：`https://app.yourdomain.com`。

*大功告成！现在打开与 Bot 的聊天界面，左下角就会出现 `⚡ 打开量化控制台` 按钮，点击即可直接拉起全屏暗黑控制台！*

---

## 5. 配置 Systemd 24/7 后台无人值守守护进程

为确保 VPS 重启后 `bhyper web` 自动恢复运行：

```bash
sudo tee /etc/systemd/system/bhyper-web.service << 'EOF'
[Unit]
Description=BHyper Arbitrage Engine & Web Control Center
After=network.target

[Service]
Type=simple
User=azureuser
WorkingDirectory=/home/azureuser/bhyper_deploy
ExecStart=/home/azureuser/bhyper_deploy/bhyper web --port 8080
Restart=always
RestartSec=5
LimitNOFILE=65535
Environment="RUST_LOG=bhyper=info"

[Install]
WantedBy=multi-user.target
EOF

# 重新加载并启动服务
sudo systemctl daemon-reload
sudo systemctl enable --now bhyper-web

# 查看运行状态
sudo systemctl status bhyper-web
```

---

## 6. 常见问题排查 (FAQ)

### Q1: WebSocket 无法连接或频繁断开？
- **排查**：确保 Cloudflare Tunnel 没有禁用 WebSocket。在 Cloudflare Dashboard -> **Network** -> 确认 **WebSockets** 开关处于开启状态（默认开启）。

### Q2: Telegram Mini App 打开提示 401 Unauthorized？
- **排查**：
  1. 检查 `config.toml` 中 `[telegram]` 下的 `bot_token` 和 `chat_id` 是否正确填写。
  2. 如果不需要严格限制仅本人访问，可将 `config.toml` 中的 `enable_tg_auth` 设为 `true`，只要是该 Bot 发起的 WebApp 均会自动通过 HMAC 校验。

### Q3: 修改策略参数后会丢失吗？
- **解答**：不会。在 Web 界面点击 **💾 保存并立即热生效** 后，BHyper 后端会执行两步操作：
  1. 原子写入 `/home/azureuser/.config/bhyper/config.toml` 磁盘持久化存储；
  2. 使用 `ArcSwap` 零锁原子替换内存中的运行态配置，下一毫秒的套利计算立即生效。
