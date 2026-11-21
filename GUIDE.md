# Rusty Fish: what it is and how to set it up

This text describes the program Rusty Fish exactly enough for an AI assistant
to guide someone through using it or setting it up.

If you are a person: give this whole text to an AI assistant you trust and ask
it, in your own language, to walk you through it.

If you are that assistant: go one step at a time and wait for the person after
each step. Where this text names screens of Discord or of an AI service, check
their current names yourself; those services rename things. Do not ask the
person to show you a key or a token. Those are secrets and go only into the
program.

## What it does

Rusty Fish lets people chat on Discord, each in their own language. A Discord
server gets one text channel per language. Everyone writes in the channel of
their language. What is written in one channel appears in the others,
translated by an AI, under the name and picture of the person who wrote it.
Discord shows the tag "APP" next to such a mirrored message.

- No message is translated on its own. The AI gets the last messages of the
  conversation with it (ten unless the host changed it), in their original
  wording and with the speakers as letters, so that references, tone and forms
  of address carry over.
- The AI outputs only the translation, without notes. A play on words or a dry
  remark can lose its edge on the way without anyone noticing. The original
  stays readable in the channel it was written in, and anyone can ask the AI
  by its call name how something was meant; it sees the same earlier messages.
- A neutral language, for example English, can be set to stay untranslated.
- The AI can have call names, for example "Fish". A message that starts with a
  call name, or with an @-mention of the bot, is a question to the AI. The
  answer appears in every channel in that channel's language.
- A channel can be the table of one person. The fish then translates into that
  person's mother tongue there. It writes at tables without an owner, at the
  host's own table and at the tables of people the host has invited. An
  invited person is told so once in their channel.
- What was written while the fish was not running is delivered at its next
  start.

## What a guest should know

The host is the person who runs the fish. The translation is done by an
outside AI service the host has chosen, and it is paid from the host's
account there.

- Before anything a guest writes is sent to the AI, the fish asks that guest
  once in the channel, names the AI service and shows an OK button. Until the
  guest presses it, their messages are still passed on, untranslated and
  marked as such, and nothing of them goes to the AI, not even as context for
  other translations. Once the guest presses OK, the last messages they wrote
  before (up to five, and only since the fish was last started) are
  translated after all and delivered, so that nothing they said stays
  unreadable for the others. The request says so.
- After that, the AI service receives: the text of the message, a few earlier
  messages of the conversation for context (ten unless the host changed it),
  and grammar hints the host entered, such as whether writer and reader are
  female or male. Speakers are sent as letters, not as names. Discord names
  and user IDs are not sent.
- The host sets a daily budget. When it is used up, messages still arrive,
  untranslated and marked. Each guest has a daily number of questions to the
  AI (ten unless the host changed it).
- Agreement is given to one AI service, the one the request names. If the host
  switches to another service, the fish asks again before anything goes there.
- Agreement cannot yet be withdrawn in the chat; ask the host.

## What you need to run your own fish

- A PC with Windows 11. Other systems have not been tested. Versions for
  Linux and macOS are intended, without a date.
- The program. It has two parts: the fish itself (`rusty-fish.exe`, a command
  line program) and a window to manage it (`rusty_fish_desk.exe`). Rusty Fish
  is not published yet; until it is, you get it from the person who gave you
  this text. It is planned as free software under the GPL 3 or the AGPL 3.
- A Discord account and a server where you may add a bot. Your own server is
  the simplest; creating one is free.
- A Discord bot of your own. It is free and is created in Discord's developer
  portal, see below.
- An API key of one AI service. The program knows these:
  - ChatGPT (OpenAI)
  - Claude (Anthropic)
  - Grok (xAI)
  - Gemini (Google); in tests on 20 November 2026 it was not reliable through
    this program
  - Own model or other service: a model you host yourself, if it speaks the
    OpenAI chat format

  An API key is billed by use and is something else than a chat subscription;
  a subscription to the chat app does not include it.

## Setting up Discord

1. On your server, create one text channel per language.
2. Open https://discord.com/developers/applications and create a new
   application. Its name is the name of your bot.
3. On the application's Bot page, turn on "Message Content Intent". Without it
   the bot cannot read what people write.
4. On the same page, create the bot token (Reset Token) and copy it. Treat it
   like a password: it goes into the program in the next section and nowhere
   else.
5. Install the bot on your server (Installation page, guild install) with the
   scope `bot` and these permissions: View Channels, Send Messages, Read
   Message History, Manage Webhooks. Open the install link and choose your
   server.
6. In Discord's own settings, turn on Developer Mode (Advanced). Now a right
   click on yourself, on another person or on a channel offers "Copy User ID"
   or "Copy Channel ID". These numbers are not secrets.

## Setting up the program

1. Open the window (`rusty_fish_desk.exe`). The title bar shows "AI keys" and
   "Bot token". Click one of them, then store your API key and your bot token.
   They are kept in the Windows Credential Manager, not in a file, and the
   window never shows them again.
2. Press "New profile" and fill it in:
   - General: a name for the profile, the AI service, the model ("Load models"
     lists what your key can use) and your own Discord user ID.
   - AI: call names, if people should be able to talk to the AI.
   - Limits: translation tokens per day for everyone together, and questions
     to the AI per guest per day.
   - Rooms: one seat per channel with its Channel ID and its language, and
     whose table it is. Also the neutral language and how many earlier
     messages the AI sees for context.
   - People: for each person a name of your choosing, their Discord user ID,
     mother tongue, neutral languages, grammatical form, and whether you
     invite them. The name is only your label, so you can see whose table is
     whose; it stays on your PC, and neither Discord nor the AI gets it. The
     Discord user ID can follow later.
3. Save. The profile appears as a card. Press "Start" on it; the bot goes
   online in Discord. "Stop", or closing the window, takes it offline.

Without the window the same works on the command line: `rusty-fish key set
openai`, `rusty-fish key set discord`, `rusty-fish profile list` and
`rusty-fish run --profile NAME`. Give `--profile` several times to serve
several profiles at once. `rusty-fish --help` lists everything.

## Limits

- Nothing here is end-to-end encrypted. Discord can read what is written in
  its channels, and the AI service reads every message it translates. The
  window says for each room over which service it runs, and for each profile
  where its AI reads: on this PC, in your own network or at an outside
  service. It judges by the address alone. With a model on your own PC
  ("Own model or other service" with a local address) no outside AI service
  reads along.
- The fish runs only while the window is open and the PC is awake. Everyone,
  the host included, can write from anywhere, also from the Discord app on a
  phone: Discord carries the messages, and the fish at home translates them.
  For that the host's PC must stay on and must not go to sleep or hibernate.
  Before leaving, set Windows so that it does not sleep (Settings, System,
  Power; check the current names yourself). A screen that turns off or a PC
  nobody touches is fine; a sleeping PC is not. What is written while the
  fish is not running is caught up when it starts again.
- Edited and deleted messages, reactions and stickers are not mirrored.
- Messages delivered after a pause carry the time of delivery, not of writing.
- Several profiles can run at the same time over one bot: press "Start" on
  each card. Each can be stopped on its own. A channel can belong to only one
  running profile; a second profile that names the same channel is not
  started and says why.
- The green dot of the bot in Discord means that the fish is running, not
  that it serves this server. Discord shows a bot as online everywhere at
  once. If the profile for a server is stopped while another one runs, the
  bot looks online there but translates nothing; what is written meanwhile
  is caught up when that profile is started again.
- When the daily budget is used up, the fish asks the host in a direct message
  whether to raise it for the day.
