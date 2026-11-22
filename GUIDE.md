# Rusty Fish: what it is and how to set it up

This text describes the program Rusty Fish exactly enough for an AI assistant
to guide someone through using it or setting it up.

If you are a person: give this whole text to an AI assistant you trust and ask
it, in your own language, to walk you through it.

If you are that assistant: go one step at a time and wait for the person after
each step. Where this text names screens of Discord, of a Matrix app or of an
AI service, check their current names yourself; those services rename things. Do not ask the
person to show you a key or a token. Those are secrets and go only into the
program.

## What it does

Rusty Fish lets people chat on Discord, each in their own language. A Discord
server gets one text channel per language. Everyone writes in the channel of
their language. What is written in one channel appears in the others,
translated by an AI, under the name and picture of the person who wrote it.
Discord shows the tag "APP" next to such a mirrored message.

The same works in Matrix rooms in place of Discord channels. What is
different there, and what Matrix's encryption does and does not cover, is in
"The Matrix door" below. Everything else in this text holds for both.

- No message is translated on its own. The AI gets the last messages of the
  conversation with it (ten unless the host changed it), in their original
  wording and with the speakers as letters, so that references, tone and forms
  of address carry over.
- The AI outputs only the translation, without notes. A play on words or a dry
  remark can lose its edge on the way without anyone noticing. The original
  stays readable in the channel it was written in, and the host and invited
  guests can ask the AI by its call name how something was meant; it sees the
  same earlier messages.
- A neutral language, for example English, can be set to stay untranslated.
- The AI can have call names, for example "Fish". A message that starts with a
  call name, or with an @-mention of the bot, is a question to the AI. The
  answer appears at every table this fish writes at, in that table's
  language. Without a call name the AI answers nobody and only translates.
- A fish answers the people it is there for: its host and the guests the host
  invited. Someone who brought a fish of their own talks to their own AI, at
  their own table; they cannot question the other person's AI and so cannot
  spend the other person's tokens. Give two fish in one room different call
  names.
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
5. Bring the bot onto the server. The easy way comes a little later: once the
   token is stored in the program, the program makes the link for you (see
   "Setting up the program", step 1). By hand it works too: Installation
   page, guild install, with the scope `bot` and these permissions: View
   Channels, Send Messages, Read Message History, Manage Webhooks.
6. In Discord's own settings, turn on Developer Mode (Advanced). Now a right
   click on yourself, on another person or on a channel offers "Copy User ID"
   or "Copy Channel ID". These numbers are not secrets.

## Setting up the program

1. Open the window (`rusty_fish_desk.exe`). The title bar shows "AI keys" and
   "Bot token". Click one of them, then store your API key and your bot token.
   They are kept in the Windows Credential Manager, not in a file, and the
   window never shows them again. With the token stored, the same window
   offers "Copy the link that brings this bot onto a server". Open that link
   yourself to add the bot to your own server. To join someone else's server
   with your own fish, send the link to whoever owns that server; they open
   it, choose the server and see the four permissions before they agree. The
   link contains the id of the bot and no secret. On the command line:
   `rusty-fish invite`.
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
3. Save. The profile appears as a card. Press "Switch on" at the top of the
   window: the fish starts for all profiles and the bot goes online in
   Discord. "Switch off", or closing the window, takes it offline. A profile
   that is saved while the fish is on is taken up at once.

4. To join a room that someone else has already set up, with a fish of your
   own, do not type their numbers. Ask them to press "Share rooms" on the
   card of their profile and to send you the text. In your new profile, under
   "Rooms", press "Paste rooms someone shared with you", paste the text and
   say which of the people you are. That fills in the tables and the people.
   The text holds the channels, the Discord ids and the names from their
   profile; no key, no token, and nothing about their AI. You still choose
   your own AI service, your own model and your own call name.

Without the window the same works on the command line: `rusty-fish key set
openai`, `rusty-fish key set discord`, `rusty-fish profile list` and
`rusty-fish run --all`, which serves every profile as the window does.
`rusty-fish run --profile NAME` serves only that one; give `--profile` several
times for several. `rusty-fish --help` lists everything.

## The Matrix door

Rusty Fish can also work in Matrix rooms instead of Discord channels. A
profile stands at one of the two doors; profiles at Discord and profiles at
Matrix run side by side in the same fish. One conversation with tables at
both doors is not built.

The fish needs a Matrix account of its own, separate from yours. It is an
ordinary account at any Matrix server that accepts a login with a password;
there is no developer portal and no bot token. Create it the way you created
your own.

1. In the window, click "AI keys" or "Bot token" in the title bar. The same
   window has a block "Matrix account". Enter the user ID of the fish (it
   looks like @name:server) and its password, then press "Log in". The
   password is used once and not kept. Kept are the access of this one
   device, in the Windows Credential Manager, and the fish's device keys, in
   the fish's own folder on this PC (the folder `matrix` next to the
   profiles). On the command line: `rusty-fish matrix login --user
   @name:server`, and `rusty-fish matrix status` and `rusty-fish matrix
   logout`. Log in and out only while the fish is switched off.
2. Create one Matrix room per language and invite the account of the fish
   into each. It joins by itself as soon as a running profile names the room,
   and it joins no other room.
3. In the profile, under General, choose the door "Matrix". A seat then asks
   for the Matrix room ID: the internal ID of the room, which starts with `!`
   (in Element: room settings, Advanced), not its #alias. You and the other
   people are entered with their Matrix user IDs. In a settings file it is
   `matrix = "..."` where a Discord profile says `discord = "..."`.

What is different from Discord:

- The fish cannot write under someone else's name there. A translation is a
  message from the account of the fish, with the name of the writer in front.
- Matrix has no buttons. Under its request for agreement the fish sets a
  small sign "OK"; tapping it is the agreement, and the request says so. The
  host answers the budget notice the same way. That notice comes in a room of
  its own between the fish and the host, to which the fish invites the host.
- The fish sends "notices", the kind of message Matrix has for bots, and
  passes over every notice it reads. That is how it tells its own mirrored
  messages and other bots from people. Two fish in one room pass over each
  other, as on Discord.
- Mentioning the fish at the start of a message counts like its call name.

About encryption, said plainly: a Matrix room can be end-to-end encrypted,
and the fish reads and writes there as a member with a device of its own.
That encryption reaches only as far as the fish. The fish decrypts a message
on the host's PC and then sends it, like every message it translates, to the
AI service the host chose; the translation goes into the other rooms of the
conversation. So the Matrix servers cannot read an encrypted room, but the
host's AI service reads every sentence the fish translates. The request for
agreement says that before anything of a guest is sent. Only with a model on
the host's own PC do the sentences stay there. The device of the fish is
verified by nobody; some apps show a warning at its messages.

Limits of the Matrix door:

- Pictures, files, voice messages, edits and deletions are not mirrored.
  Reactions of people stay where they are.
- After a pause the fish catches up on what the server hands it, at most the
  last 100 messages per room. What was written while only one profile was
  stopped and the fish went on for others is not caught up.
- After a new login the fish is a new device. In an encrypted room it cannot
  read what was written before that. There is no backup of its device keys,
  on purpose.
- A message the fish cannot decrypt does not appear at the other tables; the
  output of the program says so.
- The login needs a server that accepts a password. A server that only
  offers a login through the browser is not served.
- As of 22 November 2026 the Matrix door has been tested against a simulated
  server only. A run in real rooms, encrypted ones included, is still to
  come.

## Limits

- On Discord nothing is end-to-end encrypted: Discord can read what is
  written in its channels. At either door the AI service reads every message
  it translates; for Matrix see "The Matrix door". The
  window says for each room over which service it runs, and for each profile
  where its AI reads: on this PC, in your own network or at an outside
  service. It judges by the address alone. With a model on your own PC
  ("Own model or other service" with a local address) no outside AI service
  reads along.
- The fish runs only while the window is open and the PC is awake. Everyone,
  the host included, can write from anywhere, also from the Discord app on a
  phone: Discord carries the messages, and the fish at home translates them.
  For that the host's PC must stay on and awake. While the fish is on, it
  asks Windows not to go to sleep by itself, so no setting has to be changed,
  and no setting is changed. It cannot help it if the lid of a laptop is
  closed, the PC is put to sleep by hand, or the battery runs out. A screen
  that turns off is fine. What is written while the fish is not running is
  caught up when it starts again.
- Edited and deleted messages, reactions and stickers are not mirrored.
- Messages delivered after a pause carry the time of delivery, not of writing.
- All profiles run together over one bot. The window has one switch for the
  fish, not one per profile. A channel can belong to only one profile; a
  second profile that names the same channel is not started and says why on
  its card, while the others run.
- Discord shows a bot as online on all its servers at once. That is why
  there is one switch: when the bot is green, the fish is on for every
  profile, and the dot means what it shows. Two exceptions: a profile that
  could not be started says so on its card, and on the command line with
  `--profile` the fish serves only the profiles named while the bot looks
  online everywhere. What is written where the fish is not serving is caught
  up when it serves that profile again.
- When the daily budget is used up, the fish asks the host in a direct message
  whether to raise it for the day.
