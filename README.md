# 💎 Premium Quote System (Dual-Stack)

A high-fidelity, cinematic Discord Quote Generator implemented in both **Rust** and **Python**. This system creates professional-grade image cards from Discord messages, featuring 2x SSAA rendering, dynamic font scaling, and full emoji support.

---

## ✨ Features
- **Cinematic Pipeline**: 2x Super-Sampling Anti-Aliasing (SSAA) downsampled with Lanczos3 for extreme clarity.
- **Dynamic Scaling**: Iterative font-scaling algorithm that automatically fits text and emojis to the card.
- **Rich Media Layout**: Support for Unicode (Twemoji) and Custom Server Emojis.
- **Identity Matching**: Support for Discord Member Role Colors and high-resolution avatars.
- **"Mofo" Media Filter**: Strict protection against stickers, images, GIFs, and embeds.
- **Jump to Message**: Integrated navigation buttons on every quote.
- **High Performance**: 
  - **Rust**: Moka asynchronous in-memory caching with parallel fetching.
  - **Python**: Aiohttp parallel fetching with Pillow optimizations.

---

## 🛠️ Prerequisites

### 1. Discord Developer Portal
1. Create a new Application at [Discord Dev Portal](https://discord.com/developers/applications).
2. Go to the **Bot** tab and enable **Message Content Intent**.
3. Reset/Copy your **Bot Token**.
4. Invite the bot using the URL Generator with `bot` and `applications.commands` scopes.

### 2. Assets
Place your branding assets in a `resources/` folder:
- `font.ttf`: Your primary display font (e.g., Montserrat, Inter, or a serif font for "Premium" look).
- `logo.png`: Your community logo (used for the watermark and icon).

---

## ⚙️ Environment Configuration (`.env`)
Create a `.env` file in the root directory:

```env
DISCORD_TOKEN=your_token_here
QUOTE_FOOTER="discord.gg/yourlink | © Your System Name"
RESOURCE_DIR="resources"
```

---

## 🦀 Rust Implementation
The Rust version is built for maximum performance and low-latency response times.

### Installation
1. Install [Rust](https://rustup.rs/).
2. Build the project:
   ```bash
   cargo build --release
   ```
3. Run the bot:
   ```bash
   cargo run --release
   ```

### Dependencies
- `poise` & `serenity`: Discord API framework.
- `image` & `imageproc`: Image processing.
- `ab_glyph`: High-quality font rendering.
- `moka`: High-speed caching.

---

## 🐍 Python Implementation
The Python version is a lightweight, easy-to-deploy counterpart using `discord.py` and `Pillow`.

### Installation
1. Install Python 3.8+.
2. Install dependencies:
   ```bash
   pip install discord.py Pillow aiohttp python-dotenv
   ```
3. Run the bot:
   ```bash
   python python_bot.py
   ```

### Files
- `python_bot.py`: The main Discord bot handler.
- `quote_generator.py`: The standalone image rendering engine.

---

## 🎨 Design Aesthetics
The system uses a 1200x630 (Twitter/Discord OpenGraph size) canvas with a split layout:
- **Left Side**: High-res grayscale avatar with a horizontal smooth vignette fade.
- **Right Side**: Centered quote text, author nickname (in role color), handle, and community watermark.

---

## 📜 Usage
1. Right-click any message in Discord.
2. Select **Apps** -> **Quote Message**.
3. The bot will generate and upload the cinematic card instantly.

---

## 🤝 Community
Designed for high-quality community systems. 
- **Website**: [discord.gg/aqwcruel](https://discord.gg/aqwcruel)
- **Copyright**: © Cruel Quote System
