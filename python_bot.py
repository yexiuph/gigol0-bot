import discord
from discord import app_commands
from discord.ext import commands
import os
import io
from dotenv import load_dotenv
from quote_generator import generate_quote_image
import re

# Load .env file
load_dotenv()
TOKEN = os.getenv("DISCORD_TOKEN")

class QuoteBot(commands.Bot):
    def __init__(self):
        intents = discord.Intents.default()
        intents.message_content = True
        super().__init__(command_prefix="!", intents=intents)

    async def setup_hook(self):
        # Sync the app commands (context menus)
        await self.tree.sync()
        print(f"Synced slash commands for {self.user}")

bot = QuoteBot()

# --- Media Filter ---
def is_unsupported_media(message: discord.Message) -> bool:
    url_regex = re.compile(r"https?://[^\s]+")
    if message.attachments or message.stickers or message.embeds:
        return True
    if url_regex.search(message.content):
        return True
    return False

# --- Context Menu Command ---
@bot.tree.context_menu(name="Quote Message")
async def quote_message(interaction: discord.Interaction, message: discord.Message):
    await interaction.response.defer()

    # 1. Filter Check
    if is_unsupported_media(message):
        await interaction.followup.send("Mofo, I don't support stickers, images, gifs or embeds")
        return

    # 2. Get Member Data & Role Color
    author = message.author
    display_name = author.display_name
    username = author.name
    
    # Try to get high-res avatar
    avatar_url = str(author.display_avatar.with_size(4096).url)
    
    # Get role color if in guild
    role_color = (255, 255, 255)
    if isinstance(author, discord.Member):
        if author.color != discord.Color.default():
            role_color = author.color.to_rgb()

    # 3. Mention Resolution
    # Replicating the Rust logic of resolving mentions to names
    content = message.content
    
    # Users
    for user in message.mentions:
        content = content.replace(f"<@{user.id}>", f"@{user.display_name}")
        content = content.replace(f"<@!{user.id}>", f"@{user.display_name}")
    
    # Roles
    for role in message.role_mentions:
        content = content.replace(f"<&{role.id}>", f"@{role.name}")
    
    # Channels
    for channel in message.channel_mentions:
        content = content.replace(f"<#{channel.id}>", f"#{channel.name}")

    try:
        # 4. Generate Image
        image_bytes = await generate_quote_image(
            avatar_url=avatar_url,
            nickname=display_name,
            username=username,
            content=content,
            role_color=role_color
        )

        # 5. Send Result with "Jump to Message" Button
        view = discord.ui.View()
        view.add_item(discord.ui.Button(label="Jump to Message", url=message.jump_url))
        
        file = discord.File(io.BytesIO(image_bytes), filename="quote.png")
        await interaction.followup.send(file=file, view=view)
        
    except Exception as e:
        print(f"Error generating quote: {e}")
        await interaction.followup.send("Mofo, something went wrong while generating the quote.")

@bot.event
async def on_ready():
    print(f"Bot logged in as {bot.user}")

if __name__ == "__main__":
    bot.run(TOKEN)
