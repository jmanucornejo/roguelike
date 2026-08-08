# Running

```powershell
cargo run --bin server
cargo run --bin client
```

# Optional client features

```powershell
# Local movement prediction
cargo run --bin client --features client_prediction
```

# Optional batched position snapshots

The default build keeps the legacy one-message-per-entity position replication.
Enable visibility-filtered snapshot batches by starting both binaries with the
same Cargo feature:

```powershell
cargo run --bin server --features batched_position_snapshots
cargo run --bin client --features batched_position_snapshots
```

Each client receives only positions from its own line-of-sight set. Batches are
split at 48 entities so each unreliable message stays below Renet's packet slice
size.

Left-click the map to walk to a destination. Keep holding left-click and move
the cursor to continuously update the walking destination as it enters new map
cells. Holds that begin over UI, enemies, loot, or spell targets remain one-shot
interactions.

Click a monster once for one basic attack. Hold left-click on a monster for
continuous attacks until the button is released, or use `Ctrl + click` to keep
auto-attacking after release. Moving or selecting another action cancels the
locked attack.

Press `Insert` to sit, and press it again to stand. Sitting immediately cancels
walking, attacks, pending pickups, and queued post-hit movement. A sitting
character cannot move, attack, pick up ground items, or cast spells until they
stand. The current pose is a temporary static, shortened version of the existing
directional sprite until dedicated sitting artwork is added.

Living players passively recover 1% of maximum HP and SP (at least 1 point)
every five seconds. Sitting doubles the recovery clock, producing the same
recovery tick every 2.5 seconds. Recovery is calculated by the server, cannot
raise a dead character, and is capped at the character's maximum HP and SP.

Selecting an area-damage spell from the action bar displays its affected area
on the floor while choosing the cast position. The translucent disc and rings
follow the map surface and use the spell's server-shared radius. They are cyan
at a valid position and red outside the caster's maximum range. Left-click casts
at the chosen position; right-click cancels targeting.

# MySQL accounts and character persistence

Account login and character selection require `DATABASE_URL`. When it is not
set, the server can still start for diagnostics, but clients cannot log in or
enter the world. For local development, start the included MySQL container:

```powershell
docker compose -f compose.dev.yaml up -d mysql phpmyadmin
docker compose -f compose.dev.yaml ps
```

The database files are stored in `docker/mysql/` inside the project and are
ignored by Git. Running `docker compose -f compose.dev.yaml down` stops and removes the container but
keeps that database data.

Copy `.env.example` to `.env`, or set the connection URL in the shell:

```powershell
$env:DATABASE_URL="mysql://roguelike:change-me@127.0.0.1:3306/roguelike"
cargo run --bin server
```

The server applies the versioned migrations in `migrations/` when it connects.
The Compose service provides the dedicated `roguelike` user with the development
password `change-me`; do not reuse these credentials in production.

phpMyAdmin is available at `http://localhost:8080` by default. Sign in with:

- Server: `mysql`
- Username: `roguelike`
- Password: the value of `MYSQL_PASSWORD` in `.env`

To use another host port, change `PHPMYADMIN_PORT` in `.env`. phpMyAdmin waits
for the MySQL health check before starting.

## Public Ubuntu deployment

The production server runs headlessly and uses configurable socket addresses.
Public connections use short-lived Renet connect tokens delivered through an
HTTPS endpoint; the network private key remains on the server. The production
Compose stack runs the game server, a private MySQL service, and Caddy for TLS.

Follow [`deploy/README.md`](deploy/README.md) for the Ubuntu, firewall, DNS,
Docker, and Windows client packaging instructions. Local development remains
available on loopback without a token service. The client refuses unencrypted
connections to non-loopback addresses so account passwords cannot accidentally
be sent to a public host in plaintext.

## Accounts and characters

Press **Account Login** in the client. You can create an account with a unique
username and password, or log in to an existing account. After authentication,
the server returns only that account's characters. Select one to enter the world,
or create a character in the first available one of nine slots. Passwords are
stored as salted Argon2 hashes; the client no longer reads `PLAYER_ACCOUNT_ID`.

To retain a character made by the old development-ID flow, use **Create
Account** once with its legacy username (for example, `dev_1`) and choose a new
password. The server activates that account and keeps all characters already
owned by it.

Character position, facing, HP, SP, levels, experience, Gold, inventory,
equipment, skills, and action-bar assignments are restored for the selected
character. Action-bar bindings can be dragged between F1-F10 to swap their
positions; both positions are saved together.

## Base and Job progression

Monster kills award separate Base and Job experience to the credited player.
Both tracks are server-authoritative, replicated to clients, displayed in the
bottom-left HUD, and immediately queued for a character save.

The placeholder class roster uses all 37 Inca- and Spanish-inspired role names
listed in `tareas.md`, from Chasqui through Capataz. Press `J` in game to cycle
through them while job-change NPCs and requirements are still under
development. Changing class resets only Job Level and Job EXP, and clears the
temporary class skill allocation; Base progression is preserved. The first
seven stable class IDs retain the existing placeholder skill-tree data.

Changing class also replaces the player's headless job body. The 29 jobs in
`assets/spritesheets/jobs/manifest.json` use their dedicated walk, idle, sit,
death, pickup, cast, hit, attack1, and attack2 sheets. The eight Inca jobs not
included in that export use their existing directional headless atlases, so all
37 placeholder classes remain visually distinct. Job sheets are loaded and
cached on demand, and the old player-atlas animator remains available for
legacy entities. Character gender is not stored yet, so the male body variant
is the current default.

The temporary balancing values live in `src/shared/gameplay/progression.rs`.
Base levels require `100 * current_level` EXP. Novice Job levels require
`40 * current_job_level` Job EXP and cap at Job 10; the placeholder first jobs
require `75 * current_job_level` and cap at Job 50. Pigs award 50 Base/30 Job
EXP, while Orcs award 120 Base/80 Job EXP. These centralized curves can later be
replaced with Ragnarok-style tables without changing combat or persistence.

## Placeholder skill trees

Press `K` in game to open the active class's placeholder skill tree. Every Job
Level gained after Job 1 awards one skill point. Each placeholder class currently
has three described skills, including chained prerequisites such as Mage's Fire
Ball requiring Bolt Studies Lv. 2 and Mana Focus requiring Fire Ball Lv. 3.

Skill ranks, remaining points, maximum ranks, class restrictions, and
prerequisites are validated by the server. Allocations are stored in the existing
`character_skills` table and restored when the character reconnects. The current
skills describe intended future effects; allocating them does not yet modify
combat statistics.

Once a skill has at least one rank, drag its `DRAG` handle onto any F1-F10 slot.
Skill bindings are saved per character and can be reordered like spells and
items. The slot displays the learned rank; activating these placeholder skills
has no combat effect yet. Changing placeholder class clears its old skill
bindings from the action bar.

## Economy foundation

Gold is the character currency and its balance is persisted with the character.
The current placeholder item prices are 10 Gold for Pig Meat, 5 Gold for a Red
Herb, and 10 Gold for a Lucky Clover. Buying, selling, and shops are not
implemented yet.

## Death, save points, stats, and equipment

When a player reaches zero HP, the server marks the character dead, stops combat
and movement, and removes 1% of the Base EXP currently accumulated toward the
next level. Any non-zero EXP balance loses at least one point. Dead characters
cannot move, attack, cast, pick up loot, or use items.

The death screen remains open until the player chooses `Return to Save Point` or
`Quit Game`. Returning restores HP and SP and teleports to the character's saved
map position. Characters without a saved point use the normal first-game spawn
at `[-10, 1, 0]`. Quit Game closes the client. The explicit dead state is also
the integration point for a future resurrection spell.

Characters now have persisted Might, Finesse, Agility, Vitality, Intellect,
Spirit, and available attribute points. Finesse supplies physical HIT and
Agility supplies FLEE. Basic attacks begin at an 85% chance and add the
attacker's HIT minus the defender's FLEE, clamped between 20% and 95%. Missed
attacks display `MISS`; skills and spells do not use this calculation yet.

Press `C` to open the server-authoritative character-status panel. New
characters start with 48 attribute points. Following classic Ragnarok, advancing
from Base Level `X` to `X + 1` awards `floor(X / 5) + 3` points, while raising an
attribute from `X` to `X + 1` costs `floor((X - 1) / 10) + 2` points. The panel
shows the next cost on every `+` button. Attributes are capped at 99 and saved
with the character. The panel displays live HIT, FLEE, HP, SP, physical attack,
and magic power values. Might raises basic-attack damage, Finesse raises HIT,
Agility raises FLEE, Vitality raises maximum HP, and Intellect and Spirit
contribute to maximum SP and magic power.

Taking positive damage interrupts a walking player for 200 ms, then resumes the
previous or newly queued destination. A 400 ms flinch-immunity window prevents
overlapping attacks from permanently hit-locking movement.

Equipment has ten typed slots: upper, middle, and lower head; armor; main hand;
off hand; garment; shoes; and two accessories. Existing database
equipment rows are loaded into those slots and synchronized to the owning
client. Press `E` to open or close the equipment panel. Lucky Clover is the
first placeholder equippable item and can occupy either accessory slot.
Double-click it in the inventory to equip it; Accessory 1 is filled before
Accessory 2. Double-click an occupied equipment row to unequip it and return
the item to inventory. Both operations are server-validated and persisted.

Equipment bonuses are server-authoritative and update immediately when an item
is equipped or removed. Lucky Clover grants `+1 Spirit, +2 FLEE`; Basic Sword
grants `+5 ATK`; Cloth Armor grants `+4 DEF, +10 HP`; Simple Boots grant
`+2 FLEE`; and Apprentice Staff grants `+5 Magic Power, +5 SP`. The four new
test items each have a temporary 15% Pig drop chance. Inventory hover details
show an item's bonuses beside the compatible equipped slot, and the equipment
panel lists bonuses inline.

Vitality now supplies base DEF and Spirit supplies base MDEF. DEF reduces basic
attack damage, MDEF reduces direct and area spell damage, and every positive hit
still deals at least one damage. Equipment attributes feed the same derived
formulas as invested attributes. Maximum HP/SP changes do not heal or refill the
character; unequipping safely clamps current values to the reduced maximum.
