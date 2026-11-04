// Jade (snownee.jade, Jade-mc26.3-Fabric-26.3.5): what the player is looking at, in a box at the top of the screen
// (vanilla's tooltip frame, Jade's default "dark" theme): the block or mob's icon and name, what Jade and its
// plugins say about it, and the mod it comes from. Plugins add lines on the client (`register_client`) and from
// the server's state (`register_server`), as Jade's IBlockComponentProvider and IServerDataProvider do; the
// server's lines reach the client over `jade:data`, asked for while the player looks.
//
// A line is a row of elements (JadeUI): text, items, the progress arrow, a bar, hearts and armour. A server line
// carries them in its text (`encode`), so a plugin's plain text lines stay plain text.
use crate::game::Game;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use std::cell::RefCell;
use std::sync::Mutex;

pub const ID: &str = "jade";
const NET_REQUEST: &str = "jade:request";
const NET_DATA: &str = "jade:data";

/// What the player looks at.
pub enum Target {
    Block { pos: [i32; 3], block: String },
    Mob { index: usize },
    /// An item frame, painting, armour stand or dropped item, by its index in the game's list.
    Frame { index: usize },
    Painting { index: usize },
    Stand { index: usize },
    Item { index: usize },
    /// A falling block (FallingBlockProvider), by its index in the game's list.
    Falling { index: usize },
}

/// One of a line's elements (JadeUI.text, item, smallItem, progressArrow, a progress bar, HealthElement,
/// ArmorElement, spacer).
#[derive(Clone, Debug)]
pub enum El {
    Text(String),
    Item { id: String, count: u32, small: bool },
    Arrow(f32),
    Bar { frac: f32, color: u32, text: String },
    Hearts { health: f32, max: f32, absorb: f32 },
    Armor(f32),
    Space(f32),
}

/// One line of the box.
#[derive(Clone, Debug, Default)]
pub struct Line {
    pub els: Vec<El>,
}

impl Line {
    pub fn text(t: impl Into<String>) -> Self {
        Self { els: vec![El::Text(t.into())] }
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with(mut self, e: El) -> Self {
        self.els.push(e);
        self
    }
    pub fn item(id: &str, count: u32) -> El {
        El::Item { id: id.to_owned(), count, small: false }
    }
    pub fn small(id: &str) -> El {
        El::Item { id: id.to_owned(), count: 1, small: true }
    }
}

/// A line as a server line's text: elements apart by `\u{1f}`, each but text starting `\u{1}`.
pub fn encode(line: &Line) -> String {
    let parts: Vec<String> = line
        .els
        .iter()
        .map(|e| match e {
            El::Text(t) => t.replace(['\u{1}', '\u{1f}', '\n'], " "),
            El::Item { id, count, small } => format!("\u{1}i {id} {count} {}", u8::from(*small)),
            El::Arrow(f) => format!("\u{1}a {f}"),
            El::Bar { frac, color, text } => format!("\u{1}b {frac} {color} {text}"),
            El::Hearts { health, max, absorb } => format!("\u{1}h {health} {max} {absorb}"),
            El::Armor(a) => format!("\u{1}r {a}"),
            El::Space(w) => format!("\u{1}s {w}"),
        })
        .collect();
    parts.join("\u{1f}")
}

fn decode(s: &str) -> Line {
    let mut line = Line::new();
    for part in s.split('\u{1f}') {
        let Some(cmd) = part.strip_prefix('\u{1}') else {
            if !part.is_empty() {
                line.els.push(El::Text(part.to_owned()));
            }
            continue;
        };
        let w: Vec<&str> = cmd.splitn(4, ' ').collect();
        let f = |i: usize| w.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        match w.first().copied() {
            Some("i") => line.els.push(El::Item {
                id: w.get(1).copied().unwrap_or("minecraft:air").to_owned(),
                count: f(2) as u32,
                small: w.get(3) == Some(&"1"),
            }),
            Some("a") => line.els.push(El::Arrow(f(1))),
            Some("b") => line.els.push(El::Bar { frac: f(1), color: f(2) as u32, text: w.get(3).copied().unwrap_or("").to_owned() }),
            Some("h") => line.els.push(El::Hearts { health: f(1), max: f(2), absorb: f(3) }),
            Some("r") => line.els.push(El::Armor(f(1))),
            Some("s") => line.els.push(El::Space(f(1))),
            _ => {}
        }
    }
    line
}

/// A plugin's lines on the client, from what the client knows.
pub type ClientProvider = fn(&Game<'_>, &Target, &mut Vec<Line>);
/// A plugin's lines from the server's state, for the block at a position (plain text, or `encode`d lines).
pub type ServerProvider = fn(&mut Game<'_>, [i32; 3], &str, &mut Vec<String>);

/// A plugin's name for the block at a position, where the crosshair meets it: its icon's item, title and mod.
pub type NameProvider = fn(&Game<'_>, [i32; 3], &str, [f64; 3]) -> Option<(String, String, String)>;

static NAME_PROVIDERS: Mutex<Vec<NameProvider>> = Mutex::new(Vec::new());
static CLIENT_PROVIDERS: Mutex<Vec<ClientProvider>> = Mutex::new(Vec::new());

/// Adds a name provider (once).
pub fn register_name(f: NameProvider) {
    if let Ok(mut v) = NAME_PROVIDERS.lock()
        && !v.iter().any(|g| *g as usize == f as usize)
    {
        v.push(f);
    }
}
static SERVER_PROVIDERS: Mutex<Vec<ServerProvider>> = Mutex::new(Vec::new());

/// Adds a client provider (once; calling again does nothing).
pub fn register_client(f: ClientProvider) {
    if let Ok(mut v) = CLIENT_PROVIDERS.lock()
        && !v.iter().any(|g| *g as usize == f as usize)
    {
        v.push(f);
    }
}

/// Adds a server provider (once).
pub fn register_server(f: ServerProvider) {
    if let Ok(mut v) = SERVER_PROVIDERS.lock()
        && !v.iter().any(|g| *g as usize == f as usize)
    {
        v.push(f);
    }
}

#[derive(Default)]
struct Client {
    /// Where the crosshair meets the block it is on.
    hit_point: [f64; 3],
    /// The server's lines for a block, and when they came.
    data: Option<([i32; 3], Vec<String>, std::time::Instant)>,
    /// The mob last asked about, and when.
    mob_asked: Option<(u32, std::time::Instant)>,
    /// A mob's lines from the server.
    mob_data: Option<(u32, Vec<String>, std::time::Instant)>,
    asked: Option<([i32; 3], std::time::Instant)>,
    asked_details: bool,
    /// Jade's key toggles: the overlay shown (Show Overlay), fluids targeted (Toggle Fluid).
    hidden: bool,
    liquids: bool,
    /// The keys down last frame (each toggle acts on its press).
    keys_down: Vec<winit::keyboard::KeyCode>,
    /// Jade's breaking progress fade (BreakingProgressProvider.progressAlpha).
    progress_alpha: f32,
    /// The narrate key was pressed: the next box is read out (JadeClient.narrate).
    narrate: bool,
    /// The recipes (false) or uses (true) key was pressed, and the item it asks about once the inventory is open
    /// (JEICompat.lookup: JEI's recipe view of the looked-at block's item).
    lookup: Option<bool>,
    lookup_item: Option<(String, bool)>,
    /// The config key: Options > Mods, where Jade's page is.
    open_config: bool,
}

thread_local! {
    static CLIENT: RefCell<Client> = RefCell::new(Client::default());
}

/// A mod id's name as Jade shows it under the box (ModIdentification: the mod's display name).
fn mod_name(ns: &str) -> String {
    match ns {
        "minecraft" => "Minecraft".to_owned(),
        "ae2" => "Applied Energistics 2".to_owned(),
        "jade" => "Jade".to_owned(),
        other => other
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Legacy-coded text that may also carry `§#RRGGBB` colours (what follows takes the colour until the next code).
fn rich(s: &str) -> horizon_client::text::Text {
    let mut parts = s.split("§#");
    let mut out = horizon_client::text::Text::legacy(parts.next().unwrap_or(""));
    for part in parts {
        let (hex, rest) = part.split_at(part.len().min(6));
        let color = u32::from_str_radix(hex, 16).unwrap_or(0xFFFFFF);
        let mut t = horizon_client::text::Text::legacy(rest);
        // Spans before any colour code of their own take the hex colour (`§l` etc. keep it).
        for span in &mut t.spans {
            if span.style.color.is_some() {
                break;
            }
            span.style.color = Some(color);
        }
        out.spans.extend(t.spans);
    }
    out
}

/// Text without its `§` codes, for measuring.
fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut skip = 0u8;
    for c in s.chars() {
        if skip > 0 {
            // `§#RRGGBB` skips the six digits too.
            skip = if skip == 1 && c == '#' { 6 } else { skip - 1 };
        } else if c == '§' {
            skip = 1;
        } else {
            out.push(c);
        }
    }
    out
}

/// An item's name as its tooltip's first line.
pub fn item_name(game: &Game<'_>, id: &str) -> String {
    game.stack_tooltip(&ItemStack::new(id, 1)).first().map_or_else(|| id.to_owned(), |(t, _)| t.plain())
}

/// IThemeHelper.seconds: a duration in ticks as Jade writes it ("12.5s", "2:05").
pub fn seconds(ticks: i64) -> String {
    let s = ticks as f64 / 20.0;
    if s >= 60.0 {
        format!("{}:{:02}", (s / 60.0) as i64, (s % 60.0) as i64)
    } else if s.fract() == 0.0 {
        format!("{s:.0}s")
    } else {
        format!("{s:.1}s")
    }
}

/// What the player looks at: the nearer of the block and the mob under the crosshair (fluids only with Toggle
/// Fluid on).
fn target(game: &Game<'_>) -> Option<Target> {
    let liquids = CLIENT.with_borrow(|c| c.liquids);
    let block = game.native_view_hit().filter(|(d, _)| *d <= 5.0);
    if let Some((_, hit)) = &block {
        CLIENT.with_borrow_mut(|c| c.hit_point = [hit.x, hit.y, hit.z]);
    }
    // The nearest entity Jade can show: mobs, frames, paintings, armour stands, dropped items.
    let mut entity: Option<(f64, Target)> = None;
    let mut offer = |d: f64, t: Target| {
        if entity.as_ref().is_none_or(|(b, _)| d < *b) {
            entity = Some((d, t));
        }
    };
    if let Some((i, d)) = game.mob_hit() {
        offer(d, Target::Mob { index: i });
    }
    if let Some((i, d)) = game.frame_hit() {
        offer(d, Target::Frame { index: i });
    }
    if let Some((i, d)) = game.painting_hit() {
        offer(d, Target::Painting { index: i });
    }
    if let Some((i, d, _)) = game.armor_stand_hit() {
        offer(d, Target::Stand { index: i });
    }
    if let Some((i, d)) = item_hit(game) {
        offer(d, Target::Item { index: i });
    }
    if let Some((i, d)) = falling_hit(game) {
        offer(d, Target::Falling { index: i });
    }
    let is_fluid = |b: &str| matches!(b, "minecraft:water" | "minecraft:lava");
    let cfg = config(game);
    let block = block.filter(|_| cfg.blocks);
    let entity = entity.filter(|_| cfg.entities);
    let entity = entity.filter(|(_, t)| {
        cfg.bosses || !matches!(t, Target::Mob { index } if game.mobs.get(*index).is_some_and(|m| matches!(m.kind.id.as_ref(), "minecraft:ender_dragon" | "minecraft:wither")))
    });
    match (block, entity) {
        (Some((bd, _)), Some((ed, t))) if ed < f64::from(bd) => Some(t),
        (None, Some((_, t))) => Some(t),
        (Some((_, hit)), _) if !hit.block.ends_with(":air") && (liquids || !is_fluid(&hit.block)) => {
            Some(Target::Block { pos: [hit.block_x, hit.block_y, hit.block_z], block: hit.block })
        }
        _ => None,
    }
}

/// The falling block under the crosshair (its whole-block box) within reach, and its distance.
fn falling_hit(game: &Game<'_>) -> Option<(usize, f64)> {
    let (eye, dir) = game.aim()?;
    let reach: f64 = if game.creative { 5.0 } else { 3.0 };
    let mut best: Option<(usize, f64)> = None;
    for (i, f) in game.falling.iter().enumerate() {
        let p = [f.position.x, f.position.y, f.position.z];
        let lo = [p[0] - 0.49, p[1], p[2] - 0.49];
        let hi = [p[0] + 0.49, p[1] + 0.98, p[2] + 0.49];
        let (mut t0, mut t1) = (0.0f64, reach);
        let mut hit = true;
        for a in 0..3 {
            if dir[a].abs() < 1e-9 {
                hit &= eye[a] >= lo[a] && eye[a] <= hi[a];
                continue;
            }
            let (mut ta, mut tb) = ((lo[a] - eye[a]) / dir[a], (hi[a] - eye[a]) / dir[a]);
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
        }
        if hit && t0 <= t1 && best.is_none_or(|b| t0 < b.1) {
            best = Some((i, t0));
        }
    }
    best
}

/// The dropped item under the crosshair (ItemEntity's 0.25 box) within reach, and its distance.
fn item_hit(game: &Game<'_>) -> Option<(usize, f64)> {
    let (eye, dir) = game.aim()?;
    let reach: f64 = if game.creative { 5.0 } else { 3.0 };
    let mut best: Option<(usize, f64)> = None;
    for (i, e) in game.dropped.iter().enumerate() {
        let p = [e.position.x, e.position.y, e.position.z];
        let lo = [p[0] - 0.125, p[1], p[2] - 0.125];
        let hi = [p[0] + 0.125, p[1] + 0.25, p[2] + 0.125];
        let (mut t0, mut t1) = (0.0f64, reach);
        let mut hit = true;
        for a in 0..3 {
            if dir[a].abs() < 1e-9 {
                hit &= eye[a] >= lo[a] && eye[a] <= hi[a];
                continue;
            }
            let (mut ta, mut tb) = ((lo[a] - eye[a]) / dir[a], (hi[a] - eye[a]) / dir[a]);
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
        }
        if hit && t0 <= t1 && best.is_none_or(|b| t0 < b.1) {
            best = Some((i, t0));
        }
    }
    best
}

/// HarvestToolProvider: the tool the block wants, ticked when the hand would get its drops.
fn harvest_tool(game: &Game<'_>, pos: [i32; 3], block: &str) -> Option<Vec<El>> {
    let items = game.res.items();
    let tools = [("pickaxe", "minecraft:wooden_pickaxe"), ("axe", "minecraft:wooden_axe"), ("shovel", "minecraft:wooden_shovel"), ("hoe", "minecraft:wooden_hoe"), ("sword", "minecraft:wooden_sword")];
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let facts = game.res.facts().get(state);
    let (_, icon) = tools.iter().find(|(n, _)| items.block_in_tag(&format!("minecraft:mineable/{n}"), block))?;
    let ok = !facts.requires_tool || items.correct_for_drops(game.held(), block);
    Some(vec![El::Item { id: (*icon).to_owned(), count: 1, small: true }, El::Text(if ok { "§a✔".to_owned() } else { "§c✕".to_owned() })])
}

/// Jade's own client lines (VanillaPlugin's client components): growth, redstone, note blocks, TNT, beehives'
/// honey, enchanting power, heads (blocks); health, armour, growth, breeding, owner, horse stats, villagers,
/// item frames, armour stands, paintings, items (mobs and entities).
fn vanilla(game: &Game<'_>, t: &Target, details: bool, lines: &mut Vec<Line>) {
    match t {
        Target::Block { pos, block } => {
            let id = game.world.block(pos[0], pos[1], pos[2]);
            let Some(state) = game.pack.assets.registry.state(id) else { return };
            let prop = |k: &str| state.properties.get(k).cloned();
            let reg = &game.pack.assets.registry;
            // CropProgressProvider: crops, stems, sweet berries, nether wart, cocoa (their `age`).
            if let Some(age) = prop("age").and_then(|a| a.parse::<u32>().ok()) {
                let most = reg.property_values(block, "age").iter().filter_map(|v| v.parse::<u32>().ok()).max().unwrap_or(7).max(1);
                let crop = block.ends_with("_crop") || matches!(block.as_str(), "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" | "minecraft:beetroots"
                    | "minecraft:melon_stem" | "minecraft:pumpkin_stem" | "minecraft:sweet_berry_bush" | "minecraft:nether_wart" | "minecraft:cocoa"
                    | "minecraft:torchflower_crop" | "minecraft:pitcher_crop");
                if crop {
                    lines.push(Line::text(if age >= most { "§aMature".to_owned() } else { format!("Growth: §f{}%", age * 100 / most) }));
                }
            }
            // RedstoneProvider: a lever's state, a repeater's delay, a comparator's mode, redstone power.
            if block == "minecraft:lever" {
                let on = prop("powered").as_deref() == Some("true");
                lines.push(Line::text(format!("State: {}", if on { "§aOn" } else { "§cOff" })));
            }
            if block == "minecraft:repeater" {
                lines.push(Line::text(format!("Delay: §f{}", prop("delay").unwrap_or_default())));
            }
            if block == "minecraft:comparator" {
                let m = if prop("mode").as_deref() == Some("subtract") { "Subtractor" } else { "Comparator" };
                lines.push(Line::text(format!("Mode: §f{m}")));
            }
            if let Some(power) = prop("power")
                && block != "minecraft:comparator"
            {
                lines.push(Line::text(format!("Power: §c{power}")));
            }
            // NoteBlockProvider: the instrument and the note.
            if block == "minecraft:note_block" {
                let inst = prop("instrument").unwrap_or_default();
                let name = match inst.as_str() {
                    "basedrum" => "Bass Drum", "snare" => "Snare Drum", "hat" => "Clicks and Sticks", "bass" => "Bass", "flute" => "Flute",
                    "bell" => "Bells", "guitar" => "Guitar", "chime" => "Chimes", "xylophone" => "Xylophone", "iron_xylophone" => "Vibraphone",
                    "cow_bell" => "Cow Bell", "didgeridoo" => "Didgeridoo", "bit" => "Square Wave", "banjo" => "Banjo", "pling" => "Electric Piano",
                    "zombie" => "Zombie", "skeleton" => "Skeleton", "creeper" => "Creeper", "dragon" => "Dragon", "wither_skeleton" => "Wither Skeleton",
                    "piglin" => "Piglin", "custom_head" => "Custom Head", _ => "Harp",
                };
                let note: usize = prop("note").and_then(|n| n.parse().ok()).unwrap_or(0);
                const NOTES: [&str; 12] = ["F♯", "G", "G♯", "A", "A♯", "B", "C", "C♯", "D", "D♯", "E", "F"];
                let octave = if note < 6 { 1 } else if note < 18 { 2 } else { 3 };
                lines.push(Line::text(format!("§f{name} - {}{octave}", NOTES[note % 12])));
            }
            // TNTStabilityProvider.
            if block == "minecraft:tnt" && prop("unstable").as_deref() == Some("true") {
                lines.push(Line::text("§cUnstable"));
            }
            // BeehiveProvider.Client: the honey level.
            if let Some(h) = prop("honey_level") {
                lines.push(Line::text(format!("Honey: §f{h} / 5")));
            }
            // EnchantmentPowerProvider: a bookshelf counts one for an enchanting table.
            if matches!(block.as_str(), "minecraft:bookshelf" | "minecraft:chiseled_bookshelf") && details {
                lines.push(Line::text("Ench Power: §f1"));
            }
            // TotalEnchantmentPowerProvider: the bookshelves an enchanting table reads.
            if block == "minecraft:enchanting_table" {
                let mut power = 0;
                for dx in -2..=2i32 {
                    for dz in -2..=2i32 {
                        if dx.abs() != 2 && dz.abs() != 2 {
                            continue;
                        }
                        for dy in 0..=1 {
                            let b = game.world.block(pos[0] + dx, pos[1] + dy, pos[2] + dz);
                            if game.pack.assets.registry.state(b).is_some_and(|s| s.name.as_str() == "minecraft:bookshelf") {
                                power += 1;
                            }
                        }
                    }
                }
                lines.push(Line::text(format!("Ench Power: §f{}", power.min(15))));
            }
        }
        Target::Mob { index } => {
            let Some(m) = game.mobs.get(*index) else { return };
            let kind = m.kind.id.as_ref();
            // EntityHealthAndArmorProvider (not armour stands, not the creaking).
            if !matches!(kind, "minecraft:armor_stand" | "minecraft:creaking") {
                // The mob's own max_health attribute (a horse rolls its own).
                let max = m.attrs.max_health.max(1.0);
                let mut l = Line::new().with(El::Hearts { health: m.health.max(0.0), max, absorb: 0.0 });
                if m.kind.armor > 0.0 {
                    l = l.with(El::Space(4.0)).with(El::Armor(m.kind.armor));
                }
                lines.push(l);
            }
            // VillagerProfessionProvider: the profession, and its level for one with a job.
            if matches!(kind, "minecraft:villager" | "minecraft:zombie_villager") {
                const PROFESSIONS: [&str; 15] = ["none", "armorer", "butcher", "cartographer", "cleric", "farmer", "fisherman", "fletcher",
                    "leatherworker", "librarian", "mason", "nitwit", "shepherd", "toolsmith", "weaponsmith"];
                let prof = PROFESSIONS[m.extra.variant2 as usize % PROFESSIONS.len()];
                let mut text = game.translate(&format!("entity.minecraft.villager.{prof}"), prof);
                let level = m.extra.variant3;
                if !matches!(prof, "none" | "nitwit") && (1..=5).contains(&level) {
                    const LEVELS: [&str; 5] = ["Novice", "Apprentice", "Journeyman", "Expert", "Master"];
                    text = format!("{text} - {}", game.translate(&format!("merchant.level.{level}"), LEVELS[level as usize - 1]));
                }
                lines.push(Line::text(format!("§f{text}")));
            }
            // PetArmorProvider: a wolf's body armour. SulfurCubeProvider: what a sulfur cube swallowed.
            if matches!(kind, "minecraft:wolf" | "minecraft:sulfur_cube")
                && let Some(armor) = m.extra.head.as_deref()
            {
                lines.push(Line::new().with(Line::small(armor)).with(El::Text(format!("§f{}", item_name(game, armor)))));
            }
            // StatusEffectsProvider: each effect and its time, by category (good green, bad red).
            for (id, ticks, amp) in m.extra.effects.iter().filter(|_| config(game).effects) {
                let short = id.trim_start_matches("minecraft:");
                let mut name = game.translate(&format!("effect.minecraft.{short}"), short);
                if (1..=9).contains(amp) {
                    name = format!("{name} {}", game.translate(&format!("potion.potency.{amp}"), &roman(*amp + 1)));
                }
                let time = if *ticks < 0 { "∞".to_owned() } else { tick_duration(i64::from(*ticks)) };
                let color = match horizon_client::brewing::effect(short).map(|e| e.category) {
                    Some(horizon_client::brewing::Category::Harmful) => "§c",
                    Some(horizon_client::brewing::Category::Beneficial) => "§a",
                    _ => "§f",
                };
                lines.push(Line::text(format!("{color}{name} ({time})")));
            }
            // AnimalOwnerProvider: a tamed animal's owner (a player in the world, or the uuid).
            if let Some(owner) = m.extra.owner {
                let uuid = ((owner[0] as u32 as u128) << 96) | ((owner[1] as u32 as u128) << 64) | ((owner[2] as u32 as u128) << 32) | (owner[3] as u32 as u128);
                let name = game.native_server_players().into_iter().find(|p| p.uuid == uuid).map(|p| p.name).unwrap_or_else(|| format!("{uuid:032x}"));
                lines.push(Line::text(format!("Owner: §f{name}")));
            }
        }
        // ItemFrameProvider: the framed item's name.
        Target::Frame { index } => {
            if let Some(stack) = game.item_frames.get(*index).and_then(|f| f.item.as_ref()) {
                let name = game.stack_tooltip(stack).first().map_or_else(|| stack.item.to_string(), |(t, _)| t.plain());
                lines.push(Line::text(format!("§f{name}")));
            }
        }
        // PaintingProvider: the variant's title (yellow) and author (grey).
        Target::Painting { index } => {
            if let Some(p) = game.paintings.get(*index) {
                let (ns, v) = p.variant.split_once(':').unwrap_or(("minecraft", &p.variant));
                let title = game.translate(&format!("painting.{ns}.{v}.title"), "");
                let author = game.translate(&format!("painting.{ns}.{v}.author"), "");
                if !title.is_empty() {
                    lines.push(Line::text(format!("§e{title}")));
                }
                if !author.is_empty() {
                    lines.push(Line::text(format!("§7{author}")));
                }
            }
        }
        // ArmorStandProvider: each worn or held item, small, with its name.
        Target::Stand { index } => {
            if let Some(a) = game.armor_stands.get(*index) {
                for stack in a.equipment.iter().rev().flatten() {
                    let name = game.stack_tooltip(stack).first().map_or_else(|| stack.item.to_string(), |(t, _)| t.plain());
                    lines.push(Line::new().with(Line::small(&stack.item)).with(El::Text(format!("§f{name}"))));
                }
            }
        }
        Target::Falling { .. } => {}
        // ItemTooltipProvider: the dropped stack's tooltip under its name.
        Target::Item { index } => {
            if let Some(e) = game.dropped.get(*index) {
                for (t, _) in game.stack_tooltip(&e.stack).into_iter().skip(1) {
                    lines.push(Line::text(format!("§7{}", t.plain())));
                }
            }
        }
    }
}

/// The owner's name on a player head (`SkullBlockEntity`'s custom name, or its profile: a name, or a compound's).
fn head_owner(game: &Game<'_>, pos: [i32; 3]) -> Option<String> {
    use horizon_world::nbt::{JavaString, Tag};
    let Some(horizon_client::containers::BlockEntity::Skull(s)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2])) else { return None };
    match s.profile.as_ref()? {
        Tag::String(j) => j.to_string().ok(),
        Tag::Compound(c) => match c.get(&JavaString::from("name")) {
            Some(Tag::String(j)) => j.to_string().ok(),
            _ => None,
        },
        _ => None,
    }
    .filter(|n| !n.is_empty())
}

/// The information options, all off by default: BlockFaceProvider, CoordinatesProvider, DistanceProvider,
/// BlockStatesProvider, BlockPropertiesProvider, RegistryNameProvider.
fn info_lines(game: &Game<'_>, t: &Target, cfg: &Config, lines: &mut Vec<Line>) {
    let (eye, dir) = game.aim().unwrap_or(([0.0; 3], [0.0; 3]));
    let block = if let Target::Block { pos, block } = t { Some((*pos, block.as_str())) } else { None };
    if cfg.block_face
        && block.is_some()
        && let Some(hit) = game.clip_ray(eye, dir, 6.0)
        && let Some(first) = lines.first_mut()
    {
        const FACES: [([i32; 3], &str); 6] = [([0, -1, 0], "down"), ([0, 1, 0], "up"), ([0, 0, -1], "north"), ([0, 0, 1], "south"), ([-1, 0, 0], "west"), ([1, 0, 0], "east")];
        if let Some((_, name)) = FACES.iter().find(|(d, _)| *d == hit.face) {
            first.els.push(El::Text(format!("§f ({name})")));
        }
    }
    if cfg.coordinates
        && let Some((pos, _)) = block
    {
        lines.push(Line::text(format!("§7{} {} {}", pos[0], pos[1], pos[2])));
    }
    if cfg.distance {
        let at = match t {
            Target::Block { pos, .. } => Some([f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5]),
            Target::Mob { index } => game.mobs.get(*index).map(|m| [m.position.x, m.position.y, m.position.z]),
            _ => None,
        };
        if let Some(at) = at {
            let d = (0..3).map(|i| (at[i] - eye[i]).powi(2)).sum::<f64>().sqrt();
            lines.push(Line::text(format!("Distance: §f{}", trim2(d))));
        }
    }
    if let Some((pos, name)) = block {
        let id = game.world.block(pos[0], pos[1], pos[2]);
        if cfg.block_states
            && let Some(state) = game.pack.assets.registry.state(id)
        {
            let mut props: Vec<(String, String)> = state.properties.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            props.sort();
            for (k, v) in props {
                lines.push(Line::text(format!("§7{k}: §f{v}")));
            }
        }
        if cfg.block_properties {
            lines.push(Line::text(format!("Destroy time: §f{}", trim2(f64::from(game.res.facts().get(id).hardness)))));
        }
        if cfg.registry_name {
            lines.push(Line::text(format!("§8{name}")));
        }
    } else if cfg.registry_name
        && let Target::Mob { index } = t
        && let Some(m) = game.mobs.get(*index)
    {
        lines.push(Line::text(format!("§8{}", m.kind.id)));
    }
}

/// DisplayHelper.dfCommas: at most two decimals.
fn trim2(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// StringUtil.formatTickDuration: m:ss (h:mm:ss past an hour).
fn tick_duration(ticks: i64) -> String {
    let s = ticks / 20;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

fn roman(n: i32) -> String {
    ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"].get((n - 1).max(0) as usize).map_or_else(|| n.to_string(), |r| (*r).to_owned())
}

/// The server's lines Jade itself gives (VanillaPlugin's server data, the universal storage providers):
/// furnaces, brewing stands, containers, campfires, jukeboxes, lecterns, comparators, hoppers, beehives,
/// command blocks, shelves, spawners and cauldrons.
fn vanilla_server(game: &mut Game<'_>, pos: [i32; 3], block: &str, details: bool, lines: &mut Vec<String>) {
    let cfg = server_config(game);
    SERVER_ITEMS.set(cfg.items);
    use horizon_client::containers::BlockEntity as Be;
    let key = (pos[0], pos[1], pos[2]);
    let Some(be) = game.world.block_entities.get(&key) else {
        if cfg.fluids {
            cauldron(block, game, pos, details, lines);
        }
        return;
    };
    let item = |s: &Option<ItemStack>| s.as_ref().map_or_else(|| Line::item("minecraft:air", 0), |s| Line::item(&s.item, s.count));
    match be {
        // FurnaceProvider: input, fuel, the progress arrow, output.
        Be::Furnace(f) => {
            if f.items.iter().any(Option::is_some) {
                let frac = if f.cooking_total == 0 { 0.0 } else { f.cooking_timer as f32 / f.cooking_total as f32 };
                let l = Line::new().with(item(&f.items[0])).with(item(&f.items[1])).with(El::Arrow(frac)).with(item(&f.items[2]));
                lines.push(encode(&l));
            }
        }
        // BrewingStandProvider: the blaze powder left and the time to brew.
        Be::Brewing(b) => {
            let mut l = Line::new().with(Line::small("minecraft:blaze_powder")).with(El::Text(format!("§f{}", b.fuel)));
            if b.brew_time > 0 {
                l = l.with(El::Space(5.0)).with(Line::small("minecraft:clock")).with(El::Text(format!("§f{}", seconds(i64::from(b.brew_time)))));
            }
            lines.push(encode(&l));
            storage(lines, b.items.iter().flatten());
        }
        // ItemStorageProvider: what a container holds, in rows of nine.
        Be::Chest(items) => {
            if block == "minecraft:hopper" && game.native_block_property(pos, "enabled").as_deref() == Some("false") {
                lines.push("§cInventory is locked".to_owned());
            }
            storage(lines, items.iter().flatten());
        }
        Be::Crafter(c) => storage(lines, c.items.iter().flatten()),
        Be::Shelf(s) => storage(lines, s.items.iter().flatten()),
        Be::ChiseledBookshelf(s) => storage(lines, s.items.iter().flatten()),
        // CampfireProvider: what cooks, and how far along.
        Be::Campfire(c) => {
            let mut l = Line::new();
            for (i, s) in c.items.iter().enumerate() {
                if let Some(s) = s {
                    l = l.with(Line::item(&s.item, s.count));
                    if c.total[i] > 0 {
                        l = l.with(El::Text(format!("§f{}", seconds(i64::from((c.total[i] - c.progress[i]).max(0))))));
                    }
                }
            }
            if !l.els.is_empty() {
                lines.push(encode(&l));
            }
        }
        // JukeboxProvider: the record.
        Be::Jukebox(j) => {
            if let Some(r) = j.record() {
                let name = item_name(game, &r.item);
                lines.push(encode(&Line::new().with(Line::small(&r.item)).with(El::Text(format!("§f{name}")))));
            } else {
                lines.push("§7Empty".to_owned());
            }
        }
        // LecternProvider: the book.
        Be::Lectern(l) => {
            if let Some(b) = &l.book {
                let name = item_name(game, &b.item);
                lines.push(encode(&Line::new().with(Line::small(&b.item)).with(El::Text(format!("§f{name}")))));
            }
        }
        // BeehiveProvider: the bees inside.
        Be::Beehive(h) => lines.push(format!("Bees: §f{} / 3", h.bees.len())),
        // CommandBlockProvider: its command.
        Be::CommandBlock(c) => {
            if !c.command.is_empty() {
                lines.push(format!("§f{}", c.command));
            }
        }
        // MobSpawnerProvider: what it spawns (the first potential's entity) and the delay.
        Be::Spawner(s) => {
            let id = s.potentials.first().and_then(|(d, _)| match d.entity.get(&horizon_world::nbt::JavaString::from("id")) {
                Some(horizon_world::nbt::Tag::String(t)) => t.to_string().ok(),
                _ => None,
            });
            if let Some(id) = id {
                let short = id.trim_start_matches("minecraft:");
                let name = game.translate(&format!("entity.minecraft.{short}"), short);
                lines.push(format!("§f{name}"));
            }
            lines.push(format!("Delay: §f{}", seconds(i64::from(s.spawn_delay.max(0)))));
        }
        // FluidStorageProvider: a mod block's tanks, each a bar.
        Be::Mod(_) if cfg.fluids => {
            let dim = game.dim.id().to_owned();
            let mut seen = Vec::new();
            for face in horizon_core::geometry::Direction::ALL {
                for t in game.native_fluid_tanks(&dim, pos, face).unwrap_or_default() {
                    if !seen.contains(&t.tank) {
                        seen.push(t.tank);
                        let name = t.fluid.as_deref().map(|f| fluid_name(game, f));
                        lines.push(fluid_line(name.as_deref(), t.fluid.as_deref(), t.amount, t.capacity, details));
                    }
                }
            }
        }
        // MobSpawnerCooldownProvider: a trial spawner's cooldown.
        Be::TrialSpawner(t) => {
            let now = game.world.game_time;
            if t.cooldown_ends_at > now {
                lines.push(format!("Cooldown: §f{}", seconds(t.cooldown_ends_at - now)));
            }
        }
        _ => {}
    }
}

/// ItemStorageProvider: the stacks, the same items together, nine to a row (up to 54).
fn storage<'a>(lines: &mut Vec<String>, stacks: impl Iterator<Item = &'a ItemStack>) {
    if !SERVER_ITEMS.get() {
        return;
    }
    let mut merged: Vec<(String, u32)> = Vec::new();
    for s in stacks {
        match merged.iter_mut().find(|(id, _)| *id == *s.item) {
            Some(e) => e.1 += s.count,
            None => merged.push((s.item.to_string(), s.count)),
        }
    }
    for row in merged.chunks(9).take(6) {
        let mut l = Line::new();
        for (id, n) in row {
            l = l.with(Line::item(id, *n));
        }
        lines.push(encode(&l));
    }
}

/// FluidStorageProvider for cauldrons: water, lava or powder snow by level.
fn cauldron(block: &str, game: &mut Game<'_>, pos: [i32; 3], details: bool, lines: &mut Vec<String>) {
    let level = |game: &mut Game<'_>| -> i32 { game.native_block_property(pos, "level").and_then(|l| l.parse().ok()).unwrap_or(0) };
    let (fluid, mb) = match block {
        "minecraft:water_cauldron" => ("minecraft:water", level(game) * 1000 / 3),
        "minecraft:lava_cauldron" => ("minecraft:lava", 1000),
        "minecraft:powder_snow_cauldron" => ("minecraft:powder_snow", level(game) * 1000 / 3),
        _ => return,
    };
    let name = fluid_name(game, fluid);
    lines.push(fluid_line(Some(&name), Some(fluid), mb, 1000, details));
}

/// A fluid's name (its block's).
fn fluid_name(game: &Game<'_>, fluid: &str) -> String {
    let (ns, path) = fluid.split_once(':').unwrap_or(("minecraft", fluid));
    game.translate(&format!("block.{ns}.{path}"), path)
}

/// FluidStorageProvider's progress bar: "Water 4,000 mB" (with Show Details, "/ 16,000 mB"; "Empty" when empty).
fn fluid_line(name: Option<&str>, fluid: Option<&str>, mb: i32, capacity: i32, details: bool) -> String {
    let color = match fluid {
        Some("minecraft:water") => 0x3F76E4,
        Some("minecraft:lava") => 0xD96415,
        Some("minecraft:powder_snow") => 0xF0F0F0,
        _ => 0x8080FF,
    };
    let text = match name {
        Some(n) if mb > 0 => {
            if details { format!("{n} §f{} mB§r / {} mB", commas(mb.into()), commas(capacity.into())) } else { format!("{n} §f{} mB", commas(mb.into())) }
        }
        _ => format!("Empty {} mB", commas(capacity.into())),
    };
    encode(&Line::new().with(El::Bar { frac: mb as f32 / capacity.max(1) as f32, color, text }))
}

/// A whole number with thousands commas.
pub fn commas(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// A vanilla GUI sprite's region ("hud/heart/full"), as the atlas names it.
fn sprite(game: &Game<'_>, name: &str) -> Option<horizon_client::hud::Rect> {
    let atlas = &game.gui.atlas;
    atlas.sprites.get(&format!("gui/sprites/{name}")).or_else(|| atlas.sprites.get(name)).copied()
}

/// Jade's progress_base.png and progress.png as (row, from, to, grey) pixel runs.
const ARROW_BASE: &[(u8, u8, u8, u32)] = &[
    (1, 14, 15, 0x8B), (2, 14, 16, 0x8B), (3, 14, 17, 0x8B), (4, 14, 18, 0x8B), (5, 14, 19, 0x8B), (6, 14, 20, 0x8B),
    (7, 0, 21, 0x8B), (8, 0, 22, 0x8B), (9, 0, 21, 0x8B), (10, 14, 20, 0x8B), (11, 14, 19, 0x8B), (12, 14, 18, 0x8B),
    (13, 14, 17, 0x8B), (14, 14, 16, 0x8B), (15, 14, 15, 0x8B),
];
const ARROW_DONE: &[(u8, u8, u8, u32)] = &[
    (0, 14, 15, 0xFF), (1, 14, 16, 0xFF), (2, 14, 17, 0xFF), (3, 14, 18, 0xFF), (4, 14, 19, 0xFF), (5, 14, 20, 0xFF),
    (6, 0, 21, 0xFF), (7, 0, 22, 0xFF), (8, 0, 21, 0xFF), (8, 21, 22, 0x68), (9, 0, 14, 0x68), (9, 14, 20, 0xFF),
    (9, 20, 21, 0x68), (10, 14, 19, 0xFF), (10, 19, 20, 0x68), (11, 14, 18, 0xFF), (11, 18, 19, 0x68), (12, 14, 17, 0xFF),
    (12, 17, 18, 0x68), (13, 14, 16, 0xFF), (13, 16, 17, 0x68), (14, 14, 15, 0xFF), (14, 15, 16, 0x68), (15, 14, 15, 0x68),
];

/// An element's size.
fn size(game: &Game<'_>, e: &El) -> (f32, f32) {
    match e {
        El::Text(t) => (game.native_text_width(&plain(t)), 9.0),
        El::Item { small: true, .. } => (10.0, 9.0),
        El::Item { .. } => (18.0, 18.0),
        El::Arrow(_) => (22.0, 16.0),
        El::Bar { text, .. } => ((game.native_text_width(&plain(text)) + 6.0).max(100.0), 12.0),
        El::Hearts { max, absorb, .. } => {
            let icons = ((max.ceil() + absorb.ceil()) / 2.0).ceil().max(1.0);
            if icons * 2.0 > 40.0 {
                (game.native_text_width(&format!("{:.0}/{:.0}", 0, max)) + 10.0, 9.0)
            } else {
                let per = icons.min(10.0);
                let rows = (icons / 10.0).ceil();
                (8.0 * per + 1.0, 5.0 + 4.0 * rows)
            }
        }
        El::Armor(a) => {
            if *a > 20.0 {
                (game.native_text_width(&format!("{a:.0}")) + 10.0, 9.0)
            } else {
                let icons = (a / 2.0).ceil();
                (8.0 * icons.min(10.0) + 1.0, 5.0 + 4.0 * (icons / 10.0).ceil())
            }
        }
        El::Space(w) => (*w, 0.0),
    }
}

fn draw_el(game: &Game<'_>, batch: &mut HudBatch, e: &El, x: f32, y: f32, h: f32) {
    let atlas = &game.gui.atlas;
    let white = horizon_client::hud::rgba(255, 255, 255, 255);
    let (_, eh) = size(game, e);
    let y = y + ((h - eh) / 2.0).floor();
    match e {
        El::Text(t) => {
            batch.text_styled(atlas, &rich(t), x, y, white, true);
        }
        El::Item { id, count, small } => {
            if id == "minecraft:air" {
                return;
            }
            if *small {
                game.native_item(batch, id, [x, y], 8.0);
            } else {
                game.native_item(batch, id, [x + 1.0, y + 1.0], 16.0);
                if *count > 1 {
                    let s = if *count >= 10_000 { format!("{}k", count / 1000) } else { count.to_string() };
                    let w = game.native_text_width(&s);
                    batch.text_styled(atlas, &horizon_client::text::Text::legacy(&s), x + 18.0 - w, y + 10.0, white, true);
                }
            }
        }
        // JadeUI.progressArrow: jade:progress_base (22x16), then jade:progress clipped to the done part.
        El::Arrow(f) => {
            for &(row, x0, x1, c) in ARROW_BASE {
                game.native_fill(batch, [x + x0 as f32, y + row as f32, (x1 - x0) as f32, 1.0], 0xFF00_0000 | c * 0x010101);
            }
            let done = (22.0 * f.clamp(0.0, 1.0)).floor() as u8;
            for &(row, x0, x1, c) in ARROW_DONE {
                let x1 = x1.min(done);
                if x1 > x0 {
                    game.native_fill(batch, [x + x0 as f32, y + row as f32, (x1 - x0) as f32, 1.0], 0xFF00_0000 | c * 0x010101);
                }
            }
        }
        // A fluid or energy bar with its text.
        El::Bar { frac, color, text } => {
            let (w, _) = size(game, e);
            game.native_fill(batch, [x, y, w, 12.0], 0xFF2A2A2A);
            let c = 0xFF00_0000 | color;
            game.native_fill(batch, [x + 1.0, y + 1.0, ((w - 2.0) * frac.clamp(0.0, 1.0)).floor(), 10.0], c);
            batch.text_styled(atlas, &rich(text), x + 3.0, y + 2.0, white, true);
        }
        // HealthElement: hearts in rows of ten (or "health/max" past 40).
        El::Hearts { health, max, absorb } => {
            let icons = ((max.ceil() + absorb.ceil()) / 2.0).ceil().max(1.0) as i32;
            if icons * 2 > 40 {
                if let Some(r) = sprite(game, "hud/heart/full") {
                    batch.rect_uv(x, y, 9.0, 9.0, r, white);
                }
                let t = format!("§f{:.0}/{:.0}", (health + absorb).ceil(), max.ceil());
                batch.text_styled(atlas, &horizon_client::text::Text::legacy(&t), x + 10.0, y + 1.0, white, true);
                return;
            }
            let hearts = health.round() * 0.5;
            let per = icons.min(10);
            let rows = (icons + 9) / 10;
            for i in (1..=icons).rev() {
                let col = (i - 1) % per;
                let row = (i - 1) / per;
                let px = x + 8.0 * col as f32;
                let py = y + 4.0 * (rows - 1 - row) as f32;
                if let Some(r) = sprite(game, "hud/heart/container") {
                    batch.rect_uv(px, py, 9.0, 9.0, r, white);
                }
                let absorbing = i > (max * 0.5).ceil() as i32;
                let (full, half) = if absorbing { ("hud/heart/absorbing_full", "hud/heart/absorbing_half") } else { ("hud/heart/full", "hud/heart/half") };
                let cur = if absorbing { (max.ceil() + absorb) * 0.5 } else { hearts };
                if (i as f32) <= cur.floor() {
                    if let Some(r) = sprite(game, full) {
                        batch.rect_uv(px, py, 9.0, 9.0, r, white);
                    }
                } else if (i as f32) < cur + 1.0 {
                    if let Some(r) = sprite(game, half) {
                        batch.rect_uv(px, py, 9.0, 9.0, r, white);
                    }
                }
            }
        }
        // ArmorElement: armour icons (or the number past 20).
        El::Armor(a) => {
            if *a > 20.0 {
                if let Some(r) = sprite(game, "hud/armor_full") {
                    batch.rect_uv(x, y, 9.0, 9.0, r, white);
                }
                batch.text_styled(atlas, &horizon_client::text::Text::legacy(&format!("§f{a:.0}")), x + 10.0, y + 1.0, white, true);
                return;
            }
            let half = a * 0.5;
            let icons = half.ceil() as i32;
            for i in 1..=icons {
                let px = x + 8.0 * ((i - 1) % 10) as f32;
                let py = y + 4.0 * ((i - 1) / 10) as f32;
                let name = if (i as f32) <= half.floor() { "hud/armor_full" } else { "hud/armor_half" };
                if let Some(r) = sprite(game, name) {
                    batch.rect_uv(px, py, 9.0, 9.0, r, white);
                }
            }
        }
        El::Space(_) => {}
    }
}

/// Jade's keys (JadeClient: Show Overlay numpad 1, Toggle Fluid numpad 2, Show Details left shift): the toggles
/// act on a press.
fn keys(game: &Game<'_>) -> bool {
    let key = |name: &str| {
        let default = KEYS.iter().find(|k| k[0] == name).map_or("", |k| k[2]);
        horizon_client::keybinds::key_code(&game.native_mod_key(ID, name, default))
    };
    let pressed: Vec<(&str, winit::keyboard::KeyCode)> = ["config", "show_overlay", "toggle_liquid", "show_recipes", "show_uses", "narrate"]
        .into_iter()
        .filter_map(|n| Some((n, key(n)?)))
        .filter(|(_, k)| game.physical_keys.contains(k))
        .collect();
    let toggle = config(game).toggle_mode;
    let hold = key("show_overlay").is_some_and(|k| game.physical_keys.contains(&k));
    CLIENT.with_borrow_mut(|c| {
        for (name, k) in &pressed {
            if !c.keys_down.contains(k) {
                match *name {
                    "show_overlay" if toggle => c.hidden = !c.hidden,
                    "toggle_liquid" => c.liquids = !c.liquids,
                    "narrate" => c.narrate = true,
                    "config" => c.open_config = true,
                    "show_recipes" => c.lookup = Some(false),
                    "show_uses" => c.lookup = Some(true),
                    _ => {}
                }
            }
        }
        // DisplayMode.HOLD_KEY: shown only while the key is held.
        if !toggle {
            c.hidden = !hold;
        }
        c.keys_down = pressed.iter().map(|(_, k)| *k).collect();
    });
    key("show_details").is_some_and(|k| game.physical_keys.contains(&k))
}

/// JadeKeys: name, label and default key (Options > Mods > Jade lists them; the player rebinds them there).
pub const KEYS: [[&str; 3]; 7] = [
    ["config", "Open config screen", "key.keyboard.keypad.0"],
    ["show_overlay", "Show overlay", "key.keyboard.keypad.1"],
    ["toggle_liquid", "Toggle liquid", "key.keyboard.keypad.2"],
    ["show_recipes", "Show recipes", "key.keyboard.keypad.3"],
    ["show_uses", "Show uses", "key.keyboard.keypad.4"],
    ["narrate", "Narrate", "key.keyboard.keypad.5"],
    ["show_details", "Show details", "key.keyboard.left.shift"],
];

/// Jade's settings (WailaConfig's general, overlay and plugin options this port has), kept in its settings folder.
#[derive(Clone)]
struct Config {
    blocks: bool,
    entities: bool,
    /// DisplayMode: TOGGLE (the key shows and hides) or HOLD_KEY.
    toggle_mode: bool,
    mod_name: bool,
    harvest_tool: bool,
    breaking: bool,
    effects: bool,
    entity_stats: bool,
    items: bool,
    fluids: bool,
    scale: f32,
    /// Overlay anchor Y: 0 top, 1 a third down, 2 middle.
    position: u8,
    bosses: bool,
    restock: bool,
    coordinates: bool,
    distance: bool,
    block_face: bool,
    block_states: bool,
    block_properties: bool,
    registry_name: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { blocks: true, entities: true, toggle_mode: true, mod_name: true, harvest_tool: true, breaking: true, effects: true,
            entity_stats: true, items: true, fluids: true, scale: 1.0, position: 0, bosses: true, restock: false, coordinates: false, distance: false, block_face: false, block_states: false, block_properties: false, registry_name: false }
    }
}

const SCALES: [f32; 5] = [0.5, 0.75, 1.0, 1.25, 1.5];

thread_local! {
    static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
}

fn config(game: &Game<'_>) -> Config {
    CONFIG.with_borrow_mut(|c| {
        c.get_or_insert_with(|| {
            let flag = |k: &str, d: bool| game.native_setting(ID, k).map_or(d, |v| v == "true");
            let d = Config::default();
            Config {
                blocks: flag("display_blocks", d.blocks),
                entities: flag("display_entities", d.entities),
                toggle_mode: game.native_setting(ID, "display_mode").is_none_or(|v| v != "hold"),
                mod_name: flag("mod_name", d.mod_name),
                harvest_tool: flag("harvest_tool", d.harvest_tool),
                breaking: flag("breaking_progress", d.breaking),
                effects: flag("potion_effects", d.effects),
                entity_stats: flag("entity_stats", d.entity_stats),
                items: flag("item_storage", d.items),
                fluids: flag("fluid_storage", d.fluids),
                scale: game.native_setting(ID, "scale").and_then(|v| v.parse().ok()).unwrap_or(1.0),
                position: game.native_setting(ID, "position").and_then(|v| v.parse().ok()).unwrap_or(0),
                bosses: flag("display_bosses", d.bosses),
                restock: flag("villager_restock", d.restock),
                coordinates: flag("coordinates", d.coordinates),
                distance: flag("distance", d.distance),
                block_face: flag("block_face", d.block_face),
                block_states: flag("block_states", d.block_states),
                block_properties: flag("block_properties", d.block_properties),
                registry_name: flag("registry_name", d.registry_name),
            }
        })
        .clone()
    })
}

/// `native_mods`: Jade in Options > Mods, with its keys.
pub fn listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: ID.into(),
        title: "Jade 26.3.5".into(),
        description: "Shows what you are looking at.".into(),
        keys: KEYS.iter().map(|k| k.map(String::from)).collect(),
    });
}

/// `native_settings`: Jade's options page (WailaConfigScreen's general, overlay and plugin pages).
pub fn settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != ID {
        return;
    }
    let c = config(game);
    let on = |b: bool| if b { "On" } else { "Off" };
    buttons.push(format!("Display Mode: {}", if c.toggle_mode { "Toggle" } else { "Hold Key" }));
    buttons.push(format!("Display Blocks: {}", on(c.blocks)));
    buttons.push(format!("Display Entities: {}", on(c.entities)));
    buttons.push(format!("Show Mod Name: {}", on(c.mod_name)));
    buttons.push(format!("Harvest Tool: {}", on(c.harvest_tool)));
    buttons.push(format!("Breaking Progress: {}", on(c.breaking)));
    buttons.push(format!("Entity Effects: {}", on(c.effects)));
    buttons.push(format!("Entity Stats: {}", on(c.entity_stats)));
    buttons.push(format!("Item Storage: {}", on(c.items)));
    buttons.push(format!("Fluid Storage: {}", on(c.fluids)));
    buttons.push(format!("Overlay Scale: {}", c.scale));
    buttons.push(format!("Overlay Position: {}", ["Top", "Upper", "Middle"][c.position.min(2) as usize]));
    buttons.push(format!("Display Bosses: {}", on(c.bosses)));
    buttons.push(format!("Villager Restock: {}", on(c.restock)));
    buttons.push(format!("Coordinates: {}", on(c.coordinates)));
    buttons.push(format!("Distance: {}", on(c.distance)));
    buttons.push(format!("Block Face: {}", on(c.block_face)));
    buttons.push(format!("Block States: {}", on(c.block_states)));
    buttons.push(format!("Block Properties: {}", on(c.block_properties)));
    buttons.push(format!("Registry Name: {}", on(c.registry_name)));
}

/// `native_settings_click`: a click steps that option on.
pub fn settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ID {
        return;
    }
    let c = config(game);
    let flip = |game: &mut Game<'_>, k: &str, v: bool| game.native_set_setting(ID, k, if v { "false" } else { "true" });
    match button {
        0 => game.native_set_setting(ID, "display_mode", if c.toggle_mode { "hold" } else { "toggle" }),
        1 => flip(game, "display_blocks", c.blocks),
        2 => flip(game, "display_entities", c.entities),
        3 => flip(game, "mod_name", c.mod_name),
        4 => flip(game, "harvest_tool", c.harvest_tool),
        5 => flip(game, "breaking_progress", c.breaking),
        6 => flip(game, "potion_effects", c.effects),
        7 => flip(game, "entity_stats", c.entity_stats),
        8 => flip(game, "item_storage", c.items),
        9 => flip(game, "fluid_storage", c.fluids),
        10 => {
            let i = SCALES.iter().position(|v| (*v - c.scale).abs() < 0.01).unwrap_or(2);
            game.native_set_setting(ID, "scale", &SCALES[(i + 1) % SCALES.len()].to_string());
        }
        11 => game.native_set_setting(ID, "position", &((c.position + 1) % 3).to_string()),
        12 => flip(game, "display_bosses", c.bosses),
        13 => flip(game, "villager_restock", c.restock),
        14 => flip(game, "coordinates", c.coordinates),
        15 => flip(game, "distance", c.distance),
        16 => flip(game, "block_face", c.block_face),
        17 => flip(game, "block_states", c.block_states),
        18 => flip(game, "block_properties", c.block_properties),
        19 => flip(game, "registry_name", c.registry_name),
        _ => {}
    }
    CONFIG.with_borrow_mut(|c| *c = None);
    // The server reads the storage options from the same files.
    SERVER_CONFIG.with_borrow_mut(|c| *c = None);
}

thread_local! {
    static SERVER_CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
    /// Item Storage, for the request being answered.
    static SERVER_ITEMS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// `native_hud`: the box, at the top centre as Jade's default places it.
pub fn hud(game: &mut Game<'_>, batch: &mut HudBatch) {
    let details = keys(game);
    if CLIENT.with_borrow_mut(|c| std::mem::take(&mut c.open_config)) && matches!(game.screen, crate::game::Screen::Playing) {
        if game.title.is_none() {
            game.capture(false);
        }
        game.open_options(game.title.is_some());
        game.ui.options.show(horizon_client::options_ui::Page::Mods);
        return;
    }
    // JEI's recipe view opens over the inventory: once it is open, JEI hears its own key for the item.
    if !matches!(game.screen, crate::game::Screen::Playing)
        && let Some((item, uses)) = CLIENT.with_borrow_mut(|c| c.lookup_item.take())
    {
        let name = if uses { "uses" } else { "recipe" };
        let key = game.native_mod_key("jei", name, if uses { "key.keyboard.u" } else { "key.keyboard.r" });
        let stack = crate::game::native_hooks::NativeStack { item, count: 1, components: String::new() };
        game.native_panel_event("jei", &crate::game::native_hooks::NativePanelEvent::Key { key, stack });
    }
    // Jade hides while a screen is open, and with Show Overlay toggled off.
    if !game.native_on(ID) || game.player.is_none() || !matches!(game.screen, crate::game::Screen::Playing) || CLIENT.with_borrow(|c| c.hidden) {
        return;
    }
    let Some(t) = target(game) else {
        CLIENT.with_borrow_mut(|c| {
            c.progress_alpha = 0.0;
            c.lookup = None;
        });
        return;
    };
    // The server's lines for a block, asked for a few times a second while the player looks.
    if let Target::Block { pos, .. } = &t {
        let ask = CLIENT.with_borrow(|c| c.asked.is_none_or(|(p, at)| p != *pos || at.elapsed().as_millis() > 250) || c.asked_details != details);
        if ask {
            let msg = format!("{} {} {} {}", pos[0], pos[1], pos[2], u8::from(details));
            if game.native_net_send_to_server(NET_REQUEST, msg.as_bytes()).is_ok() {
                CLIENT.with_borrow_mut(|c| {
                    c.asked = Some((*pos, std::time::Instant::now()));
                    c.asked_details = details;
                });
            }
        }
    }
    // The server's lines for a mob: only the server's copy has its timers and trades.
    if let Target::Mob { index } = &t
        && let Some(id) = game.mobs.get(*index).map(|m| m.id)
    {
        let ask = CLIENT.with_borrow(|c| c.mob_asked.is_none_or(|(i, at)| i != id || at.elapsed().as_millis() > 250) || c.asked_details != details);
        if ask {
            let cfg = config(game);
            let msg = format!("m {id} {} {} {}", u8::from(details), u8::from(cfg.restock), u8::from(cfg.entity_stats));
            if game.native_net_send_to_server(NET_REQUEST, msg.as_bytes()).is_ok() {
                CLIENT.with_borrow_mut(|c| {
                    c.mob_asked = Some((id, std::time::Instant::now()));
                    c.asked_details = details;
                });
            }
        }
    }
    let (icon, title, ns) = match &t {
        // FallingBlockProvider: the block's item as its icon, its name.
        Target::Falling { index } => {
            let Some(f) = game.falling.get(*index) else { return };
            let block = game.res.facts().get(f.state).name.to_string();
            let ns = block.split_once(':').map_or("minecraft", |(n, _)| n).to_owned();
            (game.res.items().get(&block).is_some().then(|| block.clone()), item_name(game, &block), ns)
        }
        // PlayerHeadProvider: a head with an owner is named after them.
        Target::Block { pos, block } if block == "minecraft:player_head" || block == "minecraft:player_wall_head" => {
            let name = head_owner(game, *pos);
            let title = match name {
                Some(owner) => game.translate("block.minecraft.player_head.named", "%s's Head").replace("%s", &owner),
                None => item_name(game, "minecraft:player_head"),
            };
            (Some("minecraft:player_head".to_owned()), title, "minecraft".to_owned())
        }
        Target::Block { pos, block } => {
            let hit = CLIENT.with_borrow(|c| c.hit_point);
            let named = NAME_PROVIDERS.lock().map(|v| v.clone()).unwrap_or_default().into_iter().find_map(|f| f(game, *pos, block, hit));
            match named {
                Some((icon, title, ns)) => (game.res.items().get(&icon).is_some().then_some(icon), title, ns),
                None => {
                    let has_item = game.res.items().get(block).is_some();
                    (has_item.then(|| block.clone()), item_name(game, block), block.split_once(':').map_or("minecraft", |(n, _)| n).to_owned())
                }
            }
        }
        Target::Mob { index } => {
            let Some(m) = game.mobs.get(*index) else { return };
            let id = m.kind.id.trim_start_matches("minecraft:");
            let name = m.custom_name.clone().unwrap_or_else(|| game.translate(&format!("entity.minecraft.{id}"), &horizon_client::text::Text::legacy(id).plain()));
            let egg = format!("minecraft:{id}_spawn_egg");
            (game.res.items().get(&egg).is_some().then_some(egg), name, m.kind.id.split_once(':').map_or("minecraft", |(n, _)| n).to_owned())
        }
        // Entity.getPickResult as the icon: the framed item, the painting, the stand, the dropped item.
        Target::Frame { index } => {
            let Some(f) = game.item_frames.get(*index) else { return };
            let kind = f.kind();
            let name = game.translate(&format!("entity.minecraft.{}", &kind[10..]), "Item Frame");
            (Some(f.item.as_ref().map_or_else(|| kind.to_owned(), |s| s.item.to_string())), name, "minecraft".to_owned())
        }
        Target::Painting { .. } => (Some("minecraft:painting".to_owned()), game.translate("entity.minecraft.painting", "Painting"), "minecraft".to_owned()),
        Target::Stand { .. } => (Some("minecraft:armor_stand".to_owned()), game.translate("entity.minecraft.armor_stand", "Armor Stand"), "minecraft".to_owned()),
        Target::Item { index } => {
            let Some(e) = game.dropped.get(*index) else { return };
            let name = game.stack_tooltip(&e.stack).first().map_or_else(|| e.stack.item.to_string(), |(t, _)| t.plain());
            let ns = e.stack.item.split_once(':').map_or("minecraft", |(n, _)| n).to_owned();
            (Some(e.stack.item.to_string()), name, ns)
        }
    };
    if let Some(uses) = CLIENT.with_borrow_mut(|c| c.lookup.take())
        && let Some(item) = icon.clone()
    {
        CLIENT.with_borrow_mut(|c| c.lookup_item = Some((item, uses)));
        // The inventory screen, where JEI's recipes show (creative's own E opens the creative screen, which has none).
        if game.creative {
            game.screen = crate::game::Screen::Inventory;
            game.capture(false);
        } else {
            let _ = game.key_pressed(winit::keyboard::KeyCode::KeyE);
        }
    }
    // The title line: the name, the harvest tool at its end (HarvestToolProvider, on the title by default).
    let mut title_line = Line::new().with(El::Text(format!("§f{title}")));
    if let Target::Block { pos, block } = &t
        && config(game).harvest_tool
        && let Some(tool) = harvest_tool(game, *pos, block)
    {
        title_line = title_line.with(El::Space(6.0));
        title_line.els.extend(tool);
    }
    let mut lines = vec![title_line];
    vanilla(game, &t, details, &mut lines);
    if let Ok(providers) = CLIENT_PROVIDERS.lock().map(|v| v.clone()) {
        for p in providers {
            p(game, &t, &mut lines);
        }
    }
    if let Target::Block { pos, .. } = &t {
        CLIENT.with_borrow(|c| {
            if let Some((p, server, at)) = &c.data
                && p == pos
                && at.elapsed().as_secs() < 2
            {
                lines.extend(server.iter().map(|s| decode(s)));
            }
        });
    }
    if let Target::Mob { index } = &t
        && let Some(id) = game.mobs.get(*index).map(|m| m.id)
    {
        CLIENT.with_borrow(|c| {
            if let Some((i, server, at)) = &c.mob_data
                && *i == id
                && at.elapsed().as_secs() < 2
            {
                lines.extend(server.iter().map(|s| decode(s)));
            }
        });
    }
    let cfg = config(game);
    info_lines(game, &t, &cfg, &mut lines);
    if cfg.mod_name {
        lines.push(Line::text(format!("§9§o{}", mod_name(&ns))));
    }
    // The narrate key: every text of the box, line by line, through the game's narrator.
    if CLIENT.with_borrow_mut(|c| std::mem::take(&mut c.narrate)) {
        let spoken: Vec<String> = lines
            .iter()
            .map(|l| l.els.iter().filter_map(|e| if let El::Text(t) = e { Some(plain(t)) } else { None }).collect::<Vec<_>>().join(" "))
            .filter(|t| !t.trim().is_empty())
            .collect();
        game.say_system_now(&horizon_client::text::Text::literal(&spoken.join(". ")));
    }

    // The box: vanilla's tooltip frame round the icon column and the lines (Jade's "dark" theme).
    let line_size = |l: &Line| -> (f32, f32) {
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        for (i, e) in l.els.iter().enumerate() {
            let (ew, eh) = size(game, e);
            w += ew + if i > 0 { 1.0 } else { 0.0 };
            h = h.max(eh);
        }
        (w, h.max(9.0))
    };
    let sizes: Vec<(f32, f32)> = lines.iter().map(line_size).collect();
    let icon_w = if icon.is_some() { 20.0 } else { 0.0 };
    let w = sizes.iter().map(|s| s.0).fold(0.0, f32::max) + icon_w;
    let h = sizes.iter().map(|s| s.1 + 1.0).sum::<f32>() - 1.0;
    let x = ((batch.width - w) / 2.0).floor();
    // The overlay's anchor (top centre, a third down, or the middle), and its scale about it.
    let y = match cfg.position {
        1 => (batch.height / 3.0).floor(),
        2 => ((batch.height - h) / 2.0).floor(),
        _ => 8.0,
    };
    let start = batch.vertices.len();
    let atlas = &game.gui.atlas;
    horizon_client::ui::nine_slice(batch, atlas, "gui/sprites/tooltip/background", x - 12.0, y - 12.0, w + 24.0, h + 24.0, u32::MAX);
    horizon_client::ui::nine_slice(batch, atlas, "gui/sprites/tooltip/frame", x - 12.0, y - 12.0, w + 24.0, h + 24.0, u32::MAX);
    if let Some(icon) = &icon {
        game.native_item(batch, icon, [x, y + ((h - 16.0) / 2.0).min(2.0).max(0.0)], 16.0);
    }
    let mut ly = y;
    for (l, (_, lh)) in lines.iter().zip(&sizes) {
        let mut lx = x + icon_w;
        for e in &l.els {
            draw_el(game, batch, e, lx, ly, *lh);
            lx += size(game, e).0 + 1.0;
        }
        ly += lh + 1.0;
    }
    // BreakingProgressProvider: a line along the box's foot while the block is mined (red when it won't drop).
    if let (Target::Block { pos, block }, Some(m)) = (&t, game.mining.as_ref())
        && m.pos == *pos
        && cfg.breaking
    {
        let alpha = CLIENT.with_borrow_mut(|c| {
            c.progress_alpha = (c.progress_alpha + 0.05).min(0.6);
            c.progress_alpha
        });
        let state = game.world.block(pos[0], pos[1], pos[2]);
        let ok = !game.res.facts().get(state).requires_tool || game.res.items().correct_for_drops(game.held(), block);
        let a = (alpha * 255.0) as u32;
        let c = (a << 24) | if ok { 0xFFFFFF } else { 0xFF5555 };
        game.native_fill(batch, [x - 3.0, y + h + 2.0, ((w + 6.0) * m.progress.clamp(0.0, 1.0)).floor(), 1.0], c);
    } else {
        CLIENT.with_borrow_mut(|c| c.progress_alpha = 0.0);
    }
    if (cfg.scale - 1.0).abs() > 0.01 {
        batch.scale_since(start, batch.width / 2.0, y - 12.0, cfg.scale, cfg.scale);
    }
}

/// The settings as the server reads them (its own thread's copy).
fn server_config(game: &Game<'_>) -> Config {
    SERVER_CONFIG.with_borrow_mut(|c| {
        c.get_or_insert_with(|| {
            let flag = |k: &str| game.native_setting(ID, k).is_none_or(|v| v == "true");
            Config { items: flag("item_storage"), fluids: flag("fluid_storage"), ..Config::default() }
        })
        .clone()
    })
}

/// `native_net_from_client` (server): a player asks for a block's lines.
pub fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != NET_REQUEST {
        return;
    }
    let text = String::from_utf8_lossy(data);
    if let Some(rest) = text.strip_prefix("m ") {
        mob_request(game, player, rest);
        return;
    }
    let v: Vec<i32> = text.split(' ').filter_map(|s| s.parse().ok()).collect();
    let [x, y, z, rest @ ..] = v.as_slice() else { return };
    let pos = [*x, *y, *z];
    let details = rest.first().is_some_and(|d| *d != 0);
    // Within reach of the player only (a client can ask anything).
    let near = game
        .native_server_players()
        .into_iter()
        .find(|p| p.uuid == player)
        .is_some_and(|p| (0..3).map(|i| (p.position[i] - f64::from(pos[i])).powi(2)).sum::<f64>() < 100.0);
    if !near {
        return;
    }
    let block = game.native_block(pos);
    let mut lines = Vec::new();
    vanilla_server(game, pos, &block, details, &mut lines);
    if let Ok(providers) = SERVER_PROVIDERS.lock().map(|v| v.clone()) {
        for p in providers {
            p(game, pos, &block, &mut lines);
        }
    }
    let mut out = format!("{x} {y} {z}");
    for l in lines {
        out.push('\n');
        out.push_str(&l.replace('\n', " "));
    }
    let _ = game.native_net_send_to_player(player, NET_DATA, out.as_bytes());
}

/// A player asks for a mob's lines: its id, then Show Details, Villager Restock and entity stats as that player has them.
fn mob_request(game: &mut Game<'_>, player: u128, text: &str) {
    let v: Vec<u32> = text.split(' ').filter_map(|s| s.parse().ok()).collect();
    let [id, details, restock, stats] = v.as_slice() else { return };
    let Some(m) = game.mobs.iter().find(|m| m.id == *id) else { return };
    // Within reach of the player only (a client can ask anything).
    let near = game
        .native_server_players()
        .into_iter()
        .find(|p| p.uuid == player)
        .is_some_and(|p| (p.position[0] - m.position.x).powi(2) + (p.position[1] - m.position.y).powi(2) + (p.position[2] - m.position.z).powi(2) < 100.0);
    if !near {
        return;
    }
    let mut out = format!("m {id}");
    for l in mob_lines(m, game.world.game_time, *details != 0, *restock != 0, *stats != 0) {
        out.push('\n');
        out.push_str(&encode(&l).replace('\n', " "));
    }
    let _ = game.native_net_send_to_player(player, NET_DATA, out.as_bytes());
}

/// The lines of a mob that only the server knows.
fn mob_lines(m: &horizon_client::mobs::Mob, game_time: i64, details: bool, restock: bool, entity_stats: bool) -> Vec<Line> {
    let mut lines = Vec::new();
    let kind = m.kind.id.as_ref();
    // MobGrowthProvider: a baby's growing time (AgeableMob.age counts up to 0).
    if m.extra.age < 0 {
        lines.push(Line::text(if m.extra.age_locked { "Growing time: §fPaused".to_owned() } else { format!("Growing time: §f{}", seconds(-i64::from(m.extra.age))) }));
    }
    // MobBreedingProvider: in love, or the breeding cooldown (age above 0).
    if m.extra.in_love > 0 {
        lines.push(Line::text("Breeding CD: §fIn Love"));
    } else if m.extra.age > 0 {
        lines.push(Line::text(format!("Breeding CD: §f{}", seconds(i64::from(m.extra.age)))));
    }
    // HorseStatsProvider: a llama's strength; a horse's jump height and speed (out of the most with Show Details).
    let stat = |name: &str, v: f64, most: f64| {
        let v = trim2(v);
        if details { format!("{name}: §f{v} / {}", trim2(most)) } else { format!("{name}: §f{v}") }
    };
    match kind {
        _ if !entity_stats => {}
        "minecraft:llama" | "minecraft:trader_llama" => lines.push(Line::text(stat("Strength", f64::from(m.extra.variant2.max(1)), 5.0))),
        "minecraft:horse" | "minecraft:donkey" | "minecraft:mule" | "minecraft:skeleton_horse" | "minecraft:zombie_horse" => {
            let jump = |j: f64| 4.53680079 * j * j + 1.6143173 * j - 0.22656224;
            let speed = |v: f64| v * 43.17181546666666 - 3.39999999E-7;
            lines.push(Line::text(stat("Jump height", jump(m.attrs.jump_strength), jump(1.0))));
            lines.push(Line::text(stat("Speed", speed(m.attrs.speed), speed(0.3375))));
        }
        _ => {}
    }
    // ZombieVillagerProvider: the time left to cure.
    if kind == "minecraft:zombie_villager"
        && let Some(c) = horizon_client::mobs::zombie_villager::cure(m)
        && c.time > 0
    {
        lines.push(Line::text(format!("Conversion time: §f{}", seconds(i64::from(c.time)))));
    }
    // NextEntityDropProvider: a grown chicken's next egg, an armadillo's next scute.
    if matches!(kind, "minecraft:chicken" | "minecraft:armadillo") && !m.is_baby() && m.extra.egg_time < 48000 {
        let what = if kind == "minecraft:chicken" { "Next egg" } else { "Next scute" };
        lines.push(Line::text(format!("{what}: §f{}", seconds(i64::from(m.extra.egg_time)))));
    }
    // VillagerRestockProvider (off by default): ready, or when the next restock can be (two a day, 2400 ticks
    // apart, then the next day).
    if restock
        && kind == "minecraft:villager"
        && let Some(trade) = m.extra.trade.as_ref().filter(|t| t.offers.iter().any(|o| o.uses > 0))
    {
        let limit = trade.restocks_today >= 2;
        let interval: i64 = if limit { 12000 } else { 2400 };
        let left = trade.last_restock + interval - game_time;
        lines.push(Line::text(if left <= 0 {
            "§aReady to restock".to_owned()
        } else if limit {
            format!("Restock limit reached, next in §f{}", seconds(left))
        } else {
            format!("Restock in §f{}", seconds(left))
        }));
    }
    lines
}

/// `native_net_from_server` (client): a block's or a mob's lines.
pub fn from_server(channel: &str, data: &[u8]) {
    if channel != NET_DATA {
        return;
    }
    let text = String::from_utf8_lossy(data);
    if let Some(rest) = text.strip_prefix("m ") {
        let mut it = rest.split('\n');
        let Some(id) = it.next().and_then(|i| i.parse().ok()) else { return };
        let lines: Vec<String> = it.map(str::to_owned).collect();
        CLIENT.with_borrow_mut(|c| c.mob_data = Some((id, lines, std::time::Instant::now())));
        return;
    }
    let mut it = text.split('\n');
    let v: Vec<i32> = it.next().unwrap_or("").split(' ').filter_map(|s| s.parse().ok()).collect();
    let [x, y, z] = v.as_slice() else { return };
    let lines: Vec<String> = it.map(str::to_owned).collect();
    CLIENT.with_borrow_mut(|c| c.data = Some(([*x, *y, *z], lines, std::time::Instant::now())));
}
