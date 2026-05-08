import asyncio
import re
import io
import os
from pathlib import Path
from typing import List, Tuple, Union, Dict

import aiohttp
from PIL import Image, ImageDraw, ImageFont, ImageOps
from dotenv import load_dotenv

# Load .env file if it exists
load_dotenv()

# --- Constants ---
BG_COLOR = (0, 0, 0, 255)
TEXT_COLOR = (255, 255, 255, 255)
DIM_TEXT_COLOR = (180, 180, 180, 255)
FOOTER_COLOR = (100, 100, 100, 255)

TARGET_WIDTH = 1200
TARGET_HEIGHT = 630
SCALE = 2
CANVAS_WIDTH = TARGET_WIDTH * SCALE
CANVAS_HEIGHT = TARGET_HEIGHT * SCALE

LEFT_IMAGE_WIDTH = 550 * SCALE
RIGHT_SECTION_START = 550 * SCALE
RIGHT_SECTION_WIDTH = CANVAS_WIDTH - RIGHT_SECTION_START
PADDING = 80 * SCALE
WATERMARK_SIZE = 60 * SCALE
ICON_SIZE = 28 * SCALE

# --- Regex ---
CUSTOM_EMOJI_RE = re.compile(r"<(a?):(\w+):(\d+)>")
# Basic Unicode emoji range (can be expanded)
UNICODE_EMOJI_RE = re.compile(r"[\U0001F300-\U0001F9FF\u2600-\u26FF\u2700-\u27BF]")
COMBINED_RE = re.compile(f"{CUSTOM_EMOJI_RE.pattern}|{UNICODE_EMOJI_RE.pattern}")

class RichSegment:
    def __init__(self, type: str, content: Union[str, Image.Image]):
        self.type = type # "text" or "emoji"
        self.content = content

async def fetch_image(session: aiohttp.ClientSession, url: str) -> Image.Image:
    async with session.get(url) as resp:
        if resp.status == 200:
            data = await resp.read()
            return Image.open(io.BytesIO(data)).convert("RGBA")
    return None

def wrap_rich_text(segments: List[RichSegment], font: ImageFont.FreeTypeFont, max_width: int, scale: int) -> List[List[RichSegment]]:
    lines = []
    current_line = []
    current_width = 0
    
    # Emoji size matches font height approximately
    emoji_size = font.size
    spacing = 4 * scale

    for seg in segments:
        if seg.type == "text":
            words = seg.content.split(" ")
            for i, word in enumerate(words):
                # Add space back if not the first word of the segment
                w_text = (" " if i > 0 else "") + word
                if not w_text: continue
                
                # Get width using font.getlength
                w_width = font.getlength(w_text)
                
                if current_width + w_width > max_width:
                    if current_line:
                        lines.append(current_line)
                    current_line = [RichSegment("text", word)] # New line starts without leading space
                    current_width = font.getlength(word)
                else:
                    if current_line and current_line[-1].type == "text":
                        current_line[-1].content += w_text
                    else:
                        current_line.append(RichSegment("text", w_text))
                    current_width += w_width
        else:
            # Emoji
            if current_width + emoji_size + spacing > max_width:
                if current_line:
                    lines.append(current_line)
                current_line = [seg]
                current_width = emoji_size + spacing
            else:
                current_line.append(seg)
                current_width += emoji_size + spacing
                
    if current_line:
        lines.append(current_line)
    return lines

async def generate_quote_image(
    avatar_url: str,
    nickname: str,
    username: str,
    content: str,
    role_color: Tuple[int, int, int] = (255, 255, 255)
) -> bytes:
    # 0. Parsing & Planning
    fetch_tasks = []
    fetch_urls = set()
    fetch_plan = []
    
    last_idx = 0
    for match in COMBINED_RE.finditer(content):
        # Before
        before = content[last_idx:match.start()]
        if before: fetch_plan.append({"type": "text", "content": before})
        
        # Match
        m_str = match.group(0)
        custom_match = CUSTOM_EMOJI_RE.match(m_str)
        if custom_match:
            emoji_id = custom_match.group(3)
            url = f"https://cdn.discordapp.com/emojis/{emoji_id}.png?size=128"
        else:
            # Unicode
            hex_str = "-".join([f"{ord(c):x}" for c in m_str if c != '\ufe0f'])
            url = f"https://cdn.jsdelivr.net/gh/twitter/twemoji@latest/assets/72x72/{hex_str}.png"
            
        fetch_urls.add(url)
        fetch_plan.append({"type": "emoji_url", "url": url, "orig": m_str})
        last_idx = match.end()
        
    after = content[last_idx:]
    if after: fetch_plan.append({"type": "text", "content": after})
    if not fetch_plan and content: fetch_plan.append({"type": "text", "content": content})

    # 1. Parallel Fetching
    async with aiohttp.ClientSession() as session:
        # Fetch Emojis
        emoji_results = await asyncio.gather(*[fetch_image(session, u) for u in fetch_urls])
        emoji_map = dict(zip(fetch_urls, emoji_results))
        
        # Fetch Avatar
        avatar_img = await fetch_image(session, avatar_url)

    # Convert plan to segments
    segments = []
    for p in fetch_plan:
        if p["type"] == "text":
            segments.append(RichSegment("text", p["content"]))
        else:
            img = emoji_map.get(p["url"])
            if img:
                segments.append(RichSegment("emoji", img))
            else:
                segments.append(RichSegment("text", p["orig"]))

    # 2. Resources (Using ENV with fallback)
    res_dir_env = os.getenv("RESOURCE_DIR")
    if res_dir_env and Path(res_dir_env).exists():
        res_dir = Path(res_dir_env)
    elif Path("crates/yx-bot/resources").exists():
        res_dir = Path("crates/yx-bot/resources")
    else:
        res_dir = Path("resources")
        
    font_path = str(res_dir / "font.ttf")
    logo_path = str(res_dir / "logo.png")
    
    # 3. Process Avatar
    if not avatar_img:
        avatar_img = Image.new("RGBA", (LEFT_IMAGE_WIDTH, CANVAS_HEIGHT), (30, 30, 30, 255))
    else:
        avatar_img = ImageOps.fit(avatar_img, (LEFT_IMAGE_WIDTH, CANVAS_HEIGHT), method=Image.Resampling.LANCZOS)
        avatar_img = avatar_img.convert("L").convert("RGBA") # Grayscale
        
    # Vignette
    avatar_with_alpha = avatar_img.copy()
    alpha = Image.new("L", (LEFT_IMAGE_WIDTH, CANVAS_HEIGHT), 255)
    fade_start = int(LEFT_IMAGE_WIDTH * 0.6)
    for x in range(fade_start, LEFT_IMAGE_WIDTH):
        progress = (x - fade_start) / (LEFT_IMAGE_WIDTH - fade_start)
        val = int(255 * (1.0 - progress))
        for y in range(CANVAS_HEIGHT):
            alpha.putpixel((x, y), val)
    avatar_with_alpha.putalpha(alpha)

    # 4. Drawing Base
    canvas = Image.new("RGBA", (CANVAS_WIDTH, CANVAS_HEIGHT), BG_COLOR)
    canvas.alpha_composite(avatar_with_alpha)
    draw = ImageDraw.Draw(canvas)

    # 5. Iterative Font Scaling
    content_size = 70 * SCALE
    max_text_h = CANVAS_HEIGHT * 0.75
    wrapped_lines = []
    final_font = None
    line_height = 0

    while content_size >= 25 * SCALE:
        try:
            test_font = ImageFont.truetype(font_path, content_size)
            test_lines = wrap_rich_text(segments, test_font, RIGHT_SECTION_WIDTH - PADDING*2, SCALE)
            test_lh = content_size * 1.5
            total_h = (len(test_lines) * test_lh) + (100 * SCALE)
            
            if total_h <= max_text_h:
                wrapped_lines = test_lines
                final_font = test_font
                line_height = test_lh
                break
        except: pass
        content_size -= 4

    if not final_font: # Fallback
        final_font = ImageFont.truetype(font_path, 25 * SCALE)
        line_height = (25 * SCALE) * 1.5
        wrapped_lines = wrap_rich_text(segments, final_font, RIGHT_SECTION_WIDTH - PADDING*2, SCALE)

    # Render Text
    total_text_h = (len(wrapped_lines) * line_height) + (100 * SCALE)
    current_y = (CANVAS_HEIGHT / 2) - (total_text_h / 2)
    
    for line in wrapped_lines:
        # Calculate line width for centering
        line_w = 0
        for seg in line:
            if seg.type == "text":
                line_w += final_font.getlength(seg.content)
            else:
                line_w += final_font.size + (4 * SCALE)
        
        current_x = RIGHT_SECTION_START + (RIGHT_SECTION_WIDTH / 2) - (line_w / 2)
        
        for seg in line:
            if seg.type == "text":
                draw.text((current_x, current_y), seg.content, font=final_font, fill=TEXT_COLOR)
                current_x += final_font.getlength(seg.content)
            else:
                # Emoji
                emoji_scaled = seg.content.resize((final_font.size, final_font.size), Image.Resampling.LANCZOS)
                emoji_y = current_y + (line_height / 2) - (final_font.size / 2) - (5 * SCALE)
                canvas.alpha_composite(emoji_scaled, (int(current_x), int(emoji_y)))
                current_x += final_font.size + (4 * SCALE)
        current_y += line_height

    # 6. Author Info
    current_y += 40 * SCALE
    nick_font = ImageFont.truetype(font_path, 28 * SCALE)
    user_font = ImageFont.truetype(font_path, 20 * SCALE)
    
    nick_text = f"- {nickname}"
    nick_w = nick_font.getlength(nick_text)
    nx = RIGHT_SECTION_START + (RIGHT_SECTION_WIDTH / 2) - (nick_w / 2)
    draw.text((nx, current_y), nick_text, font=nick_font, fill=(*role_color, 255))
    
    current_y += 45 * SCALE
    user_text = f"@{username}"
    user_w = user_font.getlength(user_text)
    ux = RIGHT_SECTION_START + (RIGHT_SECTION_WIDTH / 2) - (user_w / 2)
    draw.text((ux, current_y), user_text, font=user_font, fill=DIM_TEXT_COLOR)

    # 7. Watermark
    if Path(logo_path).exists():
        logo = Image.open(logo_path).convert("RGBA")
        logo = logo.resize((WATERMARK_SIZE, WATERMARK_SIZE), Image.Resampling.LANCZOS)
        # Apply opacity (40%)
        logo_data = logo.getdata()
        new_data = []
        for item in logo_data:
            new_data.append((item[0], item[1], item[2], int(item[3] * 0.4)))
        logo.putdata(new_data)
        
        wm_x = CANVAS_WIDTH - WATERMARK_SIZE - 40 * SCALE
        wm_y = CANVAS_HEIGHT - WATERMARK_SIZE - 40 * SCALE
        canvas.alpha_composite(logo, (wm_x, wm_y))
        
        footer_font = ImageFont.truetype(font_path, 16 * SCALE)
        footer_text = os.getenv("QUOTE_FOOTER", "discord.gg/aqwcruel | © Cruel Quote System")
        fw = footer_font.getlength(footer_text)
        draw.text((wm_x - fw - 15*SCALE, wm_y + WATERMARK_SIZE/2 - 10*SCALE), footer_text, font=footer_font, fill=FOOTER_COLOR)

    # 8. Final Downsample
    final_img = canvas.resize((TARGET_WIDTH, TARGET_HEIGHT), Image.Resampling.LANCZOS)
    
    buf = io.BytesIO()
    final_img.save(buf, format="PNG")
    return buf.getvalue()

# --- Example Usage ---
if __name__ == "__main__":
    async def main():
        # Test values
        img_bytes = await generate_quote_image(
            avatar_url="https://cdn.discordapp.com/embed/avatars/0.png",
            nickname="Test User",
            username="testuser",
            content="Hello World! <a:cool:123456789> \U0001F600",
            role_color=(255, 100, 100)
        )
        with open("quote_test_python.png", "wb") as f:
            f.write(img_bytes)
        print("Test image saved to quote_test_python.png")

    asyncio.run(main())
