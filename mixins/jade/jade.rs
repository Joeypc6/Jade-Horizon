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
#[derive(Clone, Debug)]
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
    /// A chest minecart or chest boat and such (ContainerEntityProvider), by its index.
    Cart { index: usize },
    Boat { index: usize },
}

/// One of a line's elements (JadeUI.text, item, smallItem, progressArrow, a progress bar, HealthElement,
/// ArmorElement, spacer).
#[derive(Clone, Debug)]
pub enum El {
    /// Text at three quarters (Mod Name: Smaller).
    Small(String),
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
            El::Text(t) | El::Small(t) => t.replace(['\u{1}', '\u{1f}', '\n'], " "),
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

/// One of Jade's settings: the file it is kept in, its label, its default and its values with their labels.
struct Opt {
    key: &'static str,
    label: &'static str,
    default: &'static str,
    values: &'static [(&'static str, &'static str)],
}

const ONOFF: &[(&str, &str)] = &[("true", "On"), ("false", "Off")];
const fn on_by_default(key: &'static str, label: &'static str) -> Opt {
    Opt { key, label, default: "true", values: ONOFF }
}
const fn off_by_default(key: &'static str, label: &'static str) -> Opt {
    Opt { key, label, default: "false", values: ONOFF }
}

/// WailaConfig and the plugin options, in the order of Jade's settings screen.
const OPTIONS: &[Opt] = &[
    on_by_default("display_tooltip", "Enabled"),
    Opt { key: "display_mode", label: "Display Mode", default: "toggle", values: &[("toggle", "Toggle"), ("hold", "Hold"), ("lite", "Lite")] },
    on_by_default("display_blocks", "Display Blocks"),
    on_by_default("display_entities", "Display Entities"),
    on_by_default("display_bosses", "Display Bosses"),
    Opt { key: "display_fluids", label: "Display Fluids", default: "none", values: &[("none", "None"), ("any", "Any"), ("fallback", "Fallback")] },
    on_by_default("hide_from_tab_list", "Hide from Player List"),
    on_by_default("hide_from_guis", "Hide from GUIs"),
    Opt { key: "boss_bar_overlap", label: "Boss Bar Overlap", default: "push", values: &[("none", "No Operation"), ("hide_bar", "Hide Boss Bar"), ("hide_tip", "Hide Tooltip"), ("push", "Push Down Tooltip")] },
    Opt { key: "display_item", label: "Icon Position", default: "top", values: &[("hide", "Hide"), ("top", "Top"), ("centered", "Centered"), ("inline", "In-line")] },
    Opt { key: "tts_mode", label: "Text to Speech Mode", default: "press", values: &[("press", "Press"), ("toggle", "Toggle")] },
    Opt { key: "perspective_mode", label: "Perspective Mode", default: "camera", values: &[("camera", "From Camera"), ("eye", "From Eye")] },
    Opt { key: "reach_distance", label: "Extended Reach", default: "0", values: &[("0", "Off"), ("5", "5"), ("10", "10"), ("15", "15"), ("20", "20")] },
    on_by_default("item_mod_name", "Mod Name on Item Tooltip"),
    Opt { key: "overlay_x", label: "Overlay Position X", default: "center", values: &[("left", "Left"), ("center", "Center"), ("right", "Right")] },
    Opt { key: "position", label: "Overlay Position Y", default: "0", values: &[("0", "Top"), ("1", "Upper"), ("2", "Middle"), ("3", "Bottom")] },
    Opt { key: "scale", label: "Display Scale", default: "1", values: &[("0.5", "0.5"), ("0.75", "0.75"), ("1", "1"), ("1.25", "1.25"), ("1.5", "1.5")] },
    Opt { key: "text_background_opacity", label: "Text Background Opacity", default: "0", values: &[("0", "0%"), ("25", "25%"), ("50", "50%"), ("75", "75%"), ("100", "100%")] },
    off_by_default("flip_main_hand", "Flip Position with Main Hand"),
    Opt { key: "overlay_alpha", label: "Background Opacity", default: "70", values: &[("0", "0%"), ("25", "25%"), ("50", "50%"), ("70", "70%"), ("85", "85%"), ("100", "100%")] },
    Opt { key: "overlay_theme", label: "Theme", default: "dark", values: &[("dark", "Dark"), ("waila", "WAILA"), ("create", "Create"), ("top", "TOP")] },
    Opt { key: "theme_style", label: "Theme Style", default: "default", values: &[("default", "Default"), ("slim", "Slim")] },
    on_by_default("animation", "Overlay Animation"),
    off_by_default("narrate_keys", "Narrate Key Binds"),
    off_by_default("enable_profiles", "Profiles Enabled"),
    Opt { key: "profile", label: "Profile", default: "0", values: &[("0", "#0"), ("1", "#1"), ("2", "#2"), ("3", "#3")] },
    off_by_default("debug_mode", "Debug Mode"),
    // Core Features.
    on_by_default("object_name", "Target Name"),
    on_by_default("mod_name", "Mod Name"),
    off_by_default("mod_name_smaller", "Mod Name: Smaller"),
    on_by_default("mod_name_translated", "Translate Mod Name"),
    off_by_default("distance", "Distance"),
    off_by_default("coordinates", "Coordinates"),
    off_by_default("coordinates_rel", "Relative Coordinates"),
    off_by_default("block_face", "Block Face"),
    Opt { key: "registry_name", label: "Registry Name", default: "false", values: &[("false", "Off"), ("true", "On"), ("advanced", "Advanced Tooltips")] },
    off_by_default("registry_special", "Special IDs"),
    off_by_default("block_states", "Block States"),
    off_by_default("block_properties", "Block Properties"),
    off_by_default("loot_table", "Loot Table"),
    // Minecraft - Entity.
    on_by_default("entity_health", "Entity Health"),
    Opt { key: "health_max", label: "Max Health for Render", default: "40", values: &[("20", "20"), ("40", "40"), ("60", "60"), ("100", "100")] },
    Opt { key: "health_per_line", label: "Icons per Line", default: "10", values: &[("5", "5"), ("10", "10"), ("15", "15"), ("20", "20")] },
    off_by_default("health_fractions", "Show Fractions"),
    on_by_default("entity_armor", "Entity Armor"),
    Opt { key: "armor_max", label: "Max Armors for Render", default: "20", values: &[("10", "10"), ("20", "20"), ("40", "40")] },
    on_by_default("entity_stats", "Horse Stats"),
    on_by_default("potion_effects", "Entity Effects"),
    Opt { key: "effects_limit", label: "Effects Limit", default: "5", values: &[("1", "1"), ("3", "3"), ("5", "5"), ("10", "10"), ("99", "No limit")] },
    on_by_default("item_frame", "Item Frame"),
    on_by_default("container_entity", "Container Entity"),
    on_by_default("mob_growth", "Mob Growth"),
    on_by_default("mob_breeding", "Mob Breeding"),
    on_by_default("armor_stand", "Armor Stand"),
    on_by_default("painting", "Painting"),
    on_by_default("next_entity_drop", "Next Entity Drop"),
    on_by_default("animal_owner", "Animal Owner"),
    on_by_default("sulfur_cube", "Sulfur Cube"),
    on_by_default("villager_profession", "Villager Profession"),
    off_by_default("villager_restock", "Villager Restock"),
    on_by_default("zombie_villager", "Zombie Villager Conversion"),
    on_by_default("pet_armor", "Pet Armor"),
    Opt { key: "pet_armor_mode", label: "Pet Armor Shows", default: "damageable", values: &[("damageable", "Show Damageable"), ("all", "Show All")] },
    on_by_default("item_tooltip", "Item Description"),
    // Minecraft - Block.
    on_by_default("breaking_progress", "Breaking Progress"),
    on_by_default("harvest_tool", "Harvest Tool"),
    off_by_default("harvest_new_line", "Harvest Tool: Start in New Line"),
    on_by_default("harvest_effective", "Harvest Tool: Effective Tool"),
    off_by_default("harvest_unbreakable", "Harvest Tool: Show \"Unbreakable\""),
    off_by_default("harvest_creative", "Harvest Tool: Show in Creative Mode"),
    on_by_default("brewing_stand", "Brewing Stand"),
    on_by_default("tnt_stability", "TNT Stability"),
    on_by_default("beehive", "Beehive"),
    on_by_default("note_block", "Note Block"),
    on_by_default("command_block", "Command Block"),
    on_by_default("enchantment_power", "Enchantment Power"),
    on_by_default("total_enchantment_power", "Total Enchantment Power"),
    on_by_default("player_head", "Player Head"),
    on_by_default("item_storage", "Item Storage"),
    Opt { key: "storage_detailed", label: "Item Storage: Detailed Amount", default: "54", values: &[("27", "27"), ("54", "54"), ("108", "108")] },
    Opt { key: "storage_normal", label: "Item Storage: Normal Amount", default: "9", values: &[("0", "0"), ("9", "9"), ("18", "18"), ("27", "27"), ("54", "54")] },
    Opt { key: "storage_names", label: "Item Storage: Show Name Threshold", default: "4", values: &[("0", "0"), ("2", "2"), ("4", "4"), ("9", "9")] },
    Opt { key: "storage_per_line", label: "Item Storage: Items per Line", default: "9", values: &[("5", "5"), ("9", "9"), ("12", "12")] },
    off_by_default("storage_sort", "Item Storage: Sort by Count"),
    on_by_default("energy_storage", "Energy Storage"),
    off_by_default("energy_detailed", "Energy Storage: Detailed-Only"),
    on_by_default("fluid_storage", "Fluid Storage"),
    off_by_default("fluid_detailed", "Fluid Storage: Detailed-Only"),
    Opt { key: "fluid_style", label: "Fluid Storage: Display Style", default: "bar", values: &[("bar", "Progress Bar"), ("plain", "Plain Text"), ("icon", "Icon")] },
    on_by_default("lectern", "Lectern"),
    on_by_default("furnace", "Furnace Contents"),
    on_by_default("mob_spawner", "Mob Spawner"),
    on_by_default("crop_progress", "Crop Progress"),
    on_by_default("redstone", "Redstone Info"),
    on_by_default("jukebox", "Jukebox Record"),
    on_by_default("shelf", "Shelf"),
    off_by_default("waxed", "Waxed State"),
    // Accessibility.
    off_by_default("always_list_mode", "Always Use List Mode"),
    off_by_default("access_sign", "Sign Content"),
    off_by_default("access_block", "Block Details"),
    off_by_default("access_entity", "Entity Details"),
    off_by_default("access_variant", "Entity Variant"),
    off_by_default("access_held", "Entity Held Item"),
    off_by_default("access_npc", "NPC Description"),
];

/// Bumped when a setting changes, for every thread's copy.
static SETTINGS_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

thread_local! {
    static VALUES: RefCell<(u32, std::collections::HashMap<&'static str, String>)> = RefCell::new((u32::MAX, std::collections::HashMap::new()));
    static IGNORED: RefCell<(u32, std::collections::HashSet<String>, std::collections::HashSet<String>)> = RefCell::new((u32::MAX, Default::default(), Default::default()));
    /// Block Properties' fire numbers (the game's own block behaviours), read when first asked for.
    static BEHAVIOURS: RefCell<Option<serde_json::Value>> = const { RefCell::new(None) };
}

/// A setting's value now.
pub fn opt(game: &Game<'_>, key: &'static str) -> String {
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    VALUES.with_borrow_mut(|(at, values)| {
        if *at != epoch {
            *at = epoch;
            values.clear();
            for o in OPTIONS {
                values.insert(o.key, game.native_setting(ID, o.key).unwrap_or_else(|| o.default.to_owned()));
            }
            // config/jade/server-plugin-overrides.json (`/jadec generate_server_overrides`) wins.
            let overrides = Game::native_config_dir().ok().and_then(|d| std::fs::read_to_string(d.join("jade/server-plugin-overrides.json")).ok()).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
            if let Some(map) = overrides.as_ref().and_then(|v| v.as_object()) {
                for o in OPTIONS {
                    if let Some(v) = map.get(o.key).and_then(|v| v.as_str()) {
                        values.insert(o.key, v.to_owned());
                    }
                }
            }
        }
        values.get(key).cloned().unwrap_or_default()
    })
}

pub fn on(game: &Game<'_>, key: &'static str) -> bool {
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    // The common case without a copy: the overlay asks for many each frame.
    let quick = VALUES.with_borrow(|(at, values)| if *at == epoch { values.get(key).map(|v| v != "false") } else { None });
    quick.unwrap_or_else(|| opt(game, key) != "false")
}

fn num(game: &Game<'_>, key: &'static str) -> f64 {
    opt(game, key).parse().unwrap_or(0.0)
}

/// `config/jade/hide-blocks.json` and `hide-entities.json`: `{"values": ["minecraft:barrier", "#tag"]}`, as Jade's.
fn ignore_files() -> Option<[std::path::PathBuf; 2]> {
    let dir = Game::native_config_dir().ok()?.join("jade");
    let _ = std::fs::create_dir_all(&dir);
    let files = [dir.join("hide-blocks.json"), dir.join("hide-entities.json")];
    let defaults = ["{\n  \"values\": [\n    \"minecraft:barrier\",\n    \"minecraft:light\",\n    \"minecraft:structure_void\"\n  ]\n}\n", "{\n  \"values\": [\n    \"minecraft:area_effect_cloud\",\n    \"minecraft:firework_rocket\",\n    \"minecraft:interaction\",\n    \"minecraft:text_display\",\n    \"minecraft:lightning_bolt\"\n  ]\n}\n"];
    for (f, d) in files.iter().zip(defaults) {
        if !f.exists() {
            let _ = std::fs::write(f, d);
        }
    }
    Some(files)
}

fn ignored(game: &Game<'_>, block: Option<&str>, entity: Option<&str>) -> bool {
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    IGNORED.with_borrow_mut(|(at, blocks, entities)| {
        if *at != epoch {
            *at = epoch;
            blocks.clear();
            entities.clear();
            if let Some(files) = ignore_files() {
                let read = |f: &std::path::Path| -> Vec<String> {
                    std::fs::read_to_string(f).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| Some(v.get("values")?.as_array()?.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())).unwrap_or_default()
                };
                blocks.extend(read(&files[0]));
                entities.extend(read(&files[1]));
            }
        }
        if let Some(b) = block
            && (blocks.contains(b) || blocks.iter().any(|t| t.strip_prefix('#').is_some_and(|tag| game.res.items().block_in_tag(tag, b))))
        {
            return true;
        }
        entity.is_some_and(|e| entities.contains(e))
    })
}

#[derive(Default)]
struct Client {
    /// Fading in (Overlay Animation): how far along, and when it was last drawn.
    fade: f32,
    fade_at: Option<std::time::Instant>,
    /// The overlay was drawn this frame (Boss Bar Overlap: Hide Boss Bar).
    showing: bool,
    /// `/jadec pin`: this target stays on the box.
    pinned: Option<Target>,
    /// Text to Speech Mode Toggle: reads each new target out.
    tts_on: bool,
    tts_last: String,
    /// A line to say for a key (Narrate Key Binds).
    say: Option<String>,
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
    let fluids_mode = opt(game, "display_fluids");
    let liquids = fluids_mode != "none" || CLIENT.with_borrow(|c| c.liquids);
    if !on(game, "display_tooltip") {
        return None;
    }
    // Extended Reach, and Perspective Mode's camera: the block on the crosshair from there.
    let extra = num(game, "reach_distance");
    let block_reach = if extra > 0.0 { extra } else { 5.0 };
    let camera_ray = opt(game, "perspective_mode") == "camera" && !game.camera_type.first_person();
    let mut block = game.native_view_hit().filter(|(d, _)| f64::from(*d) <= block_reach);
    if camera_ray
        && let Some((eye, dir)) = game.aim()
    {
        let (origin, look) = match game.camera_type {
            crate::game::perspective::CameraType::ThirdPersonFront => ([eye[0] + dir[0] * 4.0, eye[1] + dir[1] * 4.0, eye[2] + dir[2] * 4.0], [-dir[0], -dir[1], -dir[2]]),
            _ => ([eye[0] - dir[0] * 4.0, eye[1] - dir[1] * 4.0, eye[2] - dir[2] * 4.0], dir),
        };
        if let (Some(hit), Some((d, mut base))) = (game.clip_ray(origin, look, block_reach + 4.0), game.native_view_hit()) {
            base.block_x = hit.pos[0];
            base.block_y = hit.pos[1];
            base.block_z = hit.pos[2];
            base.x = hit.point[0];
            base.y = hit.point[1];
            base.z = hit.point[2];
            let id = game.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
            if let Some(state) = game.pack.assets.registry.state(id) {
                base.block = state.name.to_string();
            }
            block = Some((d, base));
        }
    }
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
    if let Some((i, d)) = game.cart_hit() {
        offer(d, Target::Cart { index: i });
    }
    if let Some((i, d)) = game.boat_hit() {
        offer(d, Target::Boat { index: i });
    }
    // Extended Reach: mobs past the hand's reach.
    if extra > 0.0
        && let Some((eye, dir)) = game.aim()
    {
        for (i, m) in game.mobs.iter().enumerate() {
            let (hw, h) = (f64::from(m.kind.width) / 2.0, f64::from(m.kind.height));
            let lo = [m.position.x - hw, m.position.y, m.position.z - hw];
            let hi = [m.position.x + hw, m.position.y + h, m.position.z + hw];
            if let Some(t) = ray_box(eye, dir, lo, hi, extra) {
                offer(t, Target::Mob { index: i });
            }
        }
    }
    let is_fluid = |b: &str| matches!(b, "minecraft:water" | "minecraft:lava");
    let cfg = config(game);
    let block = block.filter(|(_, hit)| cfg.blocks && !ignored(game, Some(&hit.block), None));
    let entity = entity.filter(|_| cfg.entities);
    let entity = entity.filter(|(_, t)| {
        let id = match t {
            Target::Mob { index } => game.mobs.get(*index).map(|m| m.kind.id.to_string()),
            Target::Painting { .. } => Some("minecraft:painting".to_owned()),
            Target::Item { .. } => Some("minecraft:item".to_owned()),
            Target::Stand { .. } => Some("minecraft:armor_stand".to_owned()),
            Target::Cart { index } => game.carts.get(*index).map(|c| c.kind.id().to_owned()),
            _ => None,
        };
        !id.is_some_and(|id| ignored(game, None, Some(&id)))
    });
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

/// Where a ray enters a box, within `reach`.
fn ray_box(eye: [f64; 3], dir: [f64; 3], lo: [f64; 3], hi: [f64; 3], reach: f64) -> Option<f64> {
    let (mut t0, mut t1) = (0.0f64, reach);
    for a in 0..3 {
        if dir[a].abs() < 1e-9 {
            if eye[a] < lo[a] || eye[a] > hi[a] {
                return None;
            }
            continue;
        }
        let (mut ta, mut tb) = ((lo[a] - eye[a]) / dir[a], (hi[a] - eye[a]) / dir[a]);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
    }
    (t0 <= t1).then_some(t0)
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
    if game.creative && !on(game, "harvest_creative") {
        return None;
    }
    if facts.hardness < 0.0 {
        return on(game, "harvest_unbreakable").then(|| vec![El::Text("§cUnbreakable".to_owned())]);
    }
    let (_, icon) = tools.iter().find(|(n, _)| items.block_in_tag(&format!("minecraft:mineable/{n}"), block))?;
    // Effective Tool: a tool that only mines faster, not one the block needs.
    if !facts.requires_tool && !on(game, "harvest_effective") {
        return None;
    }
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
            if let Some(age) = prop("age").and_then(|a| a.parse::<u32>().ok()).filter(|_| on(game, "crop_progress")) {
                let most = reg.property_values(block, "age").iter().filter_map(|v| v.parse::<u32>().ok()).max().unwrap_or(7).max(1);
                let crop = block.ends_with("_crop") || matches!(block.as_str(), "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" | "minecraft:beetroots"
                    | "minecraft:melon_stem" | "minecraft:pumpkin_stem" | "minecraft:sweet_berry_bush" | "minecraft:nether_wart" | "minecraft:cocoa"
                    | "minecraft:torchflower_crop" | "minecraft:pitcher_crop");
                if crop {
                    lines.push(Line::text(if age >= most { "§aMature".to_owned() } else { format!("Growth: §f{}%", age * 100 / most) }));
                }
            }
            // RedstoneProvider: a lever's state, a repeater's delay, a comparator's mode, redstone power.
            let redstone = on(game, "redstone");
            if redstone && block == "minecraft:lever" {
                let on = prop("powered").as_deref() == Some("true");
                lines.push(Line::text(format!("State: {}", if on { "§aOn" } else { "§cOff" })));
            }
            if redstone && block == "minecraft:repeater" {
                lines.push(Line::text(format!("Delay: §f{}", prop("delay").unwrap_or_default())));
            }
            if redstone && block == "minecraft:comparator" {
                let m = if prop("mode").as_deref() == Some("subtract") { "Subtractor" } else { "Comparator" };
                lines.push(Line::text(format!("Mode: §f{m}")));
            }
            if let Some(power) = prop("power").filter(|_| redstone)
                && block != "minecraft:comparator"
            {
                lines.push(Line::text(format!("Power: §c{power}")));
            }
            // NoteBlockProvider: the instrument and the note.
            if block == "minecraft:note_block" && on(game, "note_block") {
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
            if block == "minecraft:tnt" && prop("unstable").as_deref() == Some("true") && on(game, "tnt_stability") {
                lines.push(Line::text("§cUnstable"));
            }
            // BeehiveProvider.Client: the honey level.
            if let Some(h) = prop("honey_level").filter(|_| on(game, "beehive")) {
                lines.push(Line::text(format!("Honey: §f{h} / 5")));
            }
            // EnchantmentPowerProvider: a bookshelf counts one for an enchanting table.
            if matches!(block.as_str(), "minecraft:bookshelf" | "minecraft:chiseled_bookshelf") && details && on(game, "enchantment_power") {
                lines.push(Line::text("Ench Power: §f1"));
            }
            // TotalEnchantmentPowerProvider: the bookshelves an enchanting table reads.
            // WaxedProvider: copper that won't weather.
            if block.starts_with("minecraft:waxed_") && on(game, "waxed") {
                lines.push(Line::text("§6Waxed"));
            }
            // SignContentProvider: what a sign says, front then back.
            if on(game, "access_sign")
                && let Some(horizon_client::containers::BlockEntity::Sign(sign)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))
            {
                for line in sign.front.lines.iter().chain(sign.back.lines.iter()).filter(|l| !l.is_empty()) {
                    lines.push(Line::text(format!("§f{line}")));
                }
            }
            // BlockDetailsProvider: the state's properties in words.
            if on(game, "access_block") {
                let mut props: Vec<String> = state.properties.iter().map(|(k, v)| format!("{k} {v}")).collect();
                props.sort();
                if !props.is_empty() {
                    lines.push(Line::text(format!("§7{}", props.join(", "))));
                }
            }
            if block == "minecraft:enchanting_table" && on(game, "total_enchantment_power") {
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
                let mut l = Line::new();
                if on(game, "entity_health") {
                    l = l.with(El::Hearts { health: m.health.max(0.0), max, absorb: 0.0 });
                }
                if m.kind.armor > 0.0 && on(game, "entity_armor") {
                    if !l.els.is_empty() {
                        l = l.with(El::Space(4.0));
                    }
                    l = l.with(El::Armor(m.kind.armor));
                }
                if !l.els.is_empty() {
                    lines.push(l);
                }
            }
            // The accessibility plugin: the mob's variant and what it holds, in words.
            if on(game, "access_variant") && m.extra.variant != 0 {
                lines.push(Line::text(format!("Variant: §f{}", m.extra.variant)));
            }
            if on(game, "access_held")
                && let Some(held) = m.extra.main_hand.as_deref()
            {
                lines.push(Line::text(format!("Holding: §f{}", item_name(game, held))));
            }
            if on(game, "access_entity") {
                lines.push(Line::text(format!("§7{:.1} tall, {:.1} wide", m.kind.height, m.kind.width)));
            }
            if on(game, "access_npc") && matches!(kind, "minecraft:villager" | "minecraft:wandering_trader") {
                lines.push(Line::text("§7Trades with players"));
            }
            // VillagerProfessionProvider: the profession, and its level for one with a job.
            if matches!(kind, "minecraft:villager" | "minecraft:zombie_villager") && on(game, "villager_profession") {
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
            if (kind == "minecraft:wolf" && on(game, "pet_armor") || kind == "minecraft:sulfur_cube" && on(game, "sulfur_cube"))
                && let Some(armor) = m.extra.head.as_deref()
                // "Show Damageable" leaves out what wears no armour bar.
                && (kind != "minecraft:wolf" || opt(game, "pet_armor_mode") != "damageable" || armor.ends_with("_armor"))
            {
                lines.push(Line::new().with(Line::small(armor)).with(El::Text(format!("§f{}", item_name(game, armor)))));
            }
            // StatusEffectsProvider: each effect and its time, by category (good green, bad red).
            let limit = num(game, "effects_limit") as usize;
            for (id, ticks, amp) in m.extra.effects.iter().filter(|_| config(game).effects).take(limit) {
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
            if let Some(owner) = m.extra.owner.filter(|_| on(game, "animal_owner")) {
                let uuid = ((owner[0] as u32 as u128) << 96) | ((owner[1] as u32 as u128) << 64) | ((owner[2] as u32 as u128) << 32) | (owner[3] as u32 as u128);
                let name = game.native_server_players().into_iter().find(|p| p.uuid == uuid).map(|p| p.name).unwrap_or_else(|| format!("{uuid:032x}"));
                lines.push(Line::text(format!("Owner: §f{name}")));
            }
        }
        // ItemFrameProvider: the framed item's name.
        Target::Frame { index } => {
            if let Some(stack) = game.item_frames.get(*index).and_then(|f| f.item.as_ref()).filter(|_| on(game, "item_frame")) {
                let name = game.stack_tooltip(stack).first().map_or_else(|| stack.item.to_string(), |(t, _)| t.plain());
                lines.push(Line::text(format!("§f{name}")));
            }
        }
        // PaintingProvider: the variant's title (yellow) and author (grey).
        Target::Painting { index } => {
            if let Some(p) = game.paintings.get(*index).filter(|_| on(game, "painting")) {
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
            if let Some(a) = game.armor_stands.get(*index).filter(|_| on(game, "armor_stand")) {
                for stack in a.equipment.iter().rev().flatten() {
                    let name = game.stack_tooltip(stack).first().map_or_else(|| stack.item.to_string(), |(t, _)| t.plain());
                    lines.push(Line::new().with(Line::small(&stack.item)).with(El::Text(format!("§f{name}"))));
                }
            }
        }
        Target::Falling { .. } => {}
        // ContainerEntityProvider: what a chest minecart or boat holds.
        Target::Cart { index } => {
            if let Some(c) = game.carts.get(*index).filter(|_| on(game, "container_entity")) {
                container_lines(c.items.iter().flatten(), lines);
            }
        }
        Target::Boat { index } => {
            if let Some(b) = game.boats.get(*index).filter(|b| b.chest && on(game, "container_entity")) {
                container_lines(b.items.iter().flatten(), lines);
            }
        }
        // ItemTooltipProvider: the dropped stack's tooltip under its name.
        Target::Item { index } => {
            if let Some(e) = game.dropped.get(*index).filter(|_| on(game, "item_tooltip")) {
                let mut tip = game.stack_tooltip(&e.stack);
                if on(game, "item_mod_name") {
                    tip.pop();
                }
                for (t, _) in tip.into_iter().skip(1) {
                    lines.push(Line::text(format!("§7{}", t.plain())));
                }
            }
        }
    }
}

/// A vehicle's stacks, the same items together, nine to a row.
fn container_lines<'a>(stacks: impl Iterator<Item = &'a ItemStack>, lines: &mut Vec<Line>) {
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
        lines.push(l);
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
        if on(game, "coordinates_rel") {
            let e = [eye[0].floor() as i32, eye[1].floor() as i32, eye[2].floor() as i32];
            lines.push(Line::text(format!("§7~{} ~{} ~{}", pos[0] - e[0], pos[1] - e[1], pos[2] - e[2])));
        }
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
            let facts = game.res.facts().get(id);
            lines.push(Line::text(format!("Destroy time: §f{}", trim2(f64::from(facts.hardness)))));
            lines.push(Line::text(format!("Explosion resistance: §f{}", trim2(f64::from(facts.explosion_resistance)))));
            lines.push(Line::text(format!("Friction: §f{}", trim2(f64::from(facts.friction)))));
            // Block.speedFactor and jumpFactor: the blocks that differ.
            let (speed, jump) = match name {
                "minecraft:soul_sand" => (0.4, 1.0),
                "minecraft:honey_block" => (0.4, 0.5),
                _ => (1.0, 1.0),
            };
            lines.push(Line::text(format!("Speed factor: §f{}", trim2(speed))));
            lines.push(Line::text(format!("Jump factor: §f{}", trim2(jump))));
            let fire = |odds: &str| -> Option<i64> {
                BEHAVIOURS.with_borrow_mut(|b| {
                    let v = b.get_or_insert_with(|| {
                        std::fs::read_to_string(game.options.resources.join("block-behaviours.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null)
                    });
                    v.get("fire")?.get(odds)?.get(name)?.as_i64()
                })
            };
            if let Some(n) = fire("igniteOdds") {
                lines.push(Line::text(format!("Ignite odds: §f{n}")));
            }
            if let Some(n) = fire("burnOdds") {
                lines.push(Line::text(format!("Burn odds: §f{n}")));
            }
        }
        if on(game, "loot_table") {
            let (ns, path) = name.split_once(':').unwrap_or(("minecraft", name));
            lines.push(Line::text(format!("§7Loot table: {ns}:blocks/{path}")));
        }
        // Advanced Tooltips: only with the game's F3+H on.
        let show = match opt(game, "registry_name").as_str() {
            "advanced" => game.settings.file.bool("advancedItemTooltips"),
            "false" => false,
            _ => true,
        };
        if show {
            lines.push(Line::text(format!("§8{name}")));
        }
        if on(game, "registry_special") {
            let facts = game.res.facts().get(id);
            if facts.block_entity {
                lines.push(Line::text(format!("§8BlockEntity: {name}")));
            }
            if name.ends_with(":water") || name.ends_with(":lava") {
                lines.push(Line::text(format!("§8Fluid: {name}")));
            }
        }
    } else if opt(game, "registry_name") != "false"
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
            if f.items.iter().any(Option::is_some) && on(game, "furnace") {
                let frac = if f.cooking_total == 0 { 0.0 } else { f.cooking_timer as f32 / f.cooking_total as f32 };
                let l = Line::new().with(item(&f.items[0])).with(item(&f.items[1])).with(El::Arrow(frac)).with(item(&f.items[2]));
                lines.push(encode(&l));
            }
        }
        // BrewingStandProvider: the blaze powder left and the time to brew.
        Be::Brewing(_) if !on(game, "brewing_stand") => {}
        Be::Brewing(b) => {
            let mut l = Line::new().with(Line::small("minecraft:blaze_powder")).with(El::Text(format!("§f{}", b.fuel)));
            if b.brew_time > 0 {
                l = l.with(El::Space(5.0)).with(Line::small("minecraft:clock")).with(El::Text(format!("§f{}", seconds(i64::from(b.brew_time)))));
            }
            lines.push(encode(&l));
            storage(game, details, lines, b.items.iter().flatten());
        }
        // ItemStorageProvider: what a container holds, in rows of nine.
        Be::Chest(items) => {
            if block == "minecraft:hopper" && game.native_block_property(pos, "enabled").as_deref() == Some("false") {
                lines.push("§cInventory is locked".to_owned());
            }
            storage(game, details, lines, items.iter().flatten());
        }
        Be::Crafter(c) => storage(game, details, lines, c.items.iter().flatten()),
        Be::Shelf(s) if on(game, "shelf") => storage(game, details, lines, s.items.iter().flatten()),
        Be::ChiseledBookshelf(s) if on(game, "shelf") => storage(game, details, lines, s.items.iter().flatten()),
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
        Be::Jukebox(_) if !on(game, "jukebox") => {}
        Be::Jukebox(j) => {
            if let Some(r) = j.record() {
                let name = item_name(game, &r.item);
                lines.push(encode(&Line::new().with(Line::small(&r.item)).with(El::Text(format!("§f{name}")))));
            } else {
                lines.push("§7Empty".to_owned());
            }
        }
        // LecternProvider: the book.
        Be::Lectern(l) if on(game, "lectern") => {
            if let Some(b) = &l.book {
                let name = item_name(game, &b.item);
                lines.push(encode(&Line::new().with(Line::small(&b.item)).with(El::Text(format!("§f{name}")))));
            }
        }
        // BeehiveProvider: the bees inside.
        Be::Beehive(h) if on(game, "beehive") => lines.push(format!("Bees: §f{} / 3", h.bees.len())),
        // CommandBlockProvider: its command.
        Be::CommandBlock(c) if on(game, "command_block") => {
            if !c.command.is_empty() {
                lines.push(format!("§f{}", c.command));
            }
        }
        // MobSpawnerProvider: what it spawns (the first potential's entity) and the delay.
        Be::Spawner(s) if on(game, "mob_spawner") => {
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
                    if !seen.contains(&t.tank) && (details || !on(game, "fluid_detailed")) {
                        seen.push(t.tank);
                        let name = t.fluid.as_deref().map(|f| fluid_name(game, f));
                        lines.push(fluid_line_styled(game, name.as_deref(), t.fluid.as_deref(), t.amount, t.capacity, details));
                    }
                }
            }
        }
        // MobSpawnerCooldownProvider: a trial spawner's cooldown.
        Be::TrialSpawner(t) if on(game, "mob_spawner") => {
            let now = game.world.game_time;
            if t.cooldown_ends_at > now {
                lines.push(format!("Cooldown: §f{}", seconds(t.cooldown_ends_at - now)));
            }
        }
        _ => {}
    }
}

/// ItemStorageProvider: the stacks, the same items together, as many rows as its amounts allow: a few types by name
/// (Show Name Threshold, or always with Always Use List Mode), the rest as icons.
fn storage<'a>(game: &Game<'_>, details: bool, lines: &mut Vec<String>, stacks: impl Iterator<Item = &'a ItemStack>) {
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
    if on(game, "storage_sort") {
        merged.sort_by(|a, b| b.1.cmp(&a.1));
    }
    let amount = num(game, if details { "storage_detailed" } else { "storage_normal" }) as usize;
    let extra = merged.len().saturating_sub(amount);
    merged.truncate(amount);
    let threshold = num(game, "storage_names") as usize;
    if on(game, "always_list_mode") || merged.len() <= threshold {
        for (id, n) in &merged {
            lines.push(encode(&Line::new().with(Line::small(id)).with(El::Text(format!("§f{} x{}", item_name(game, id), commas(i64::from(*n)))))));
        }
    } else {
        for row in merged.chunks((num(game, "storage_per_line") as usize).max(1)) {
            let mut l = Line::new();
            for (id, n) in row {
                l = l.with(Line::item(id, *n));
            }
            lines.push(encode(&l));
        }
    }
    if extra > 0 {
        lines.push(format!("§7and {extra} more"));
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
    if !details && on(game, "fluid_detailed") {
        return;
    }
    let name = fluid_name(game, fluid);
    lines.push(fluid_line_styled(game, Some(&name), Some(fluid), mb, 1000, details));
}

/// A fluid's name (its block's).
fn fluid_name(game: &Game<'_>, fluid: &str) -> String {
    let (ns, path) = fluid.split_once(':').unwrap_or(("minecraft", fluid));
    game.translate(&format!("block.{ns}.{path}"), path)
}

/// Fluid Storage's Display Style: the bar, or its text with or without a bucket.
fn fluid_line_styled(game: &Game<'_>, name: Option<&str>, fluid: Option<&str>, mb: i32, capacity: i32, details: bool) -> String {
    let style = opt(game, "fluid_style");
    if style == "bar" {
        return fluid_line(name, fluid, mb, capacity, details);
    }
    let text = match name {
        Some(n) if mb > 0 => format!("{n} §f{} mB", commas(mb.into())),
        _ => format!("Empty {} mB", commas(capacity.into())),
    };
    let mut line = Line::new();
    if style == "icon" {
        line = line.with(Line::small("minecraft:bucket"));
    }
    encode(&line.with(El::Text(text)))
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
        El::Small(t) => (game.native_text_width(&plain(t)) * 0.75, 7.0),
        El::Hearts { max, absorb, .. } => {
            let icons = ((max.ceil() + absorb.ceil()) / 2.0).ceil().max(1.0);
            let line = num(game, "health_per_line").max(1.0) as f32;
            if icons * 2.0 > num(game, "health_max") as f32 || on(game, "health_fractions") {
                (game.native_text_width(&format!("{:.0}/{:.0}", 0, max)) + 10.0, 9.0)
            } else {
                let per = icons.min(line);
                let rows = (icons / line).ceil();
                (8.0 * per + 1.0, 5.0 + 4.0 * rows)
            }
        }
        El::Armor(a) => {
            if *a > num(game, "armor_max") as f32 {
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
            let line = num(game, "health_per_line").max(1.0) as i32;
            if icons * 2 > num(game, "health_max") as i32 || on(game, "health_fractions") {
                if let Some(r) = sprite(game, "hud/heart/full") {
                    batch.rect_uv(x, y, 9.0, 9.0, r, white);
                }
                let t = format!("§f{:.0}/{:.0}", (health + absorb).ceil(), max.ceil());
                batch.text_styled(atlas, &horizon_client::text::Text::legacy(&t), x + 10.0, y + 1.0, white, true);
                return;
            }
            let hearts = health.round() * 0.5;
            let per = icons.min(line);
            let rows = (icons + line - 1) / line;
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
        El::Small(t) => {
            let start = batch.vertices.len();
            batch.text_styled(atlas, &rich(t), x, y, white, true);
            batch.scale_since(start, x, y, 0.75, 0.75);
        }
        El::Armor(a) => {
            if *a > num(game, "armor_max") as f32 {
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
    let tts_toggle = opt(game, "tts_mode") == "toggle";
    let hold = key("show_overlay").is_some_and(|k| game.physical_keys.contains(&k));
    CLIENT.with_borrow_mut(|c| {
        for (name, k) in &pressed {
            if !c.keys_down.contains(k) {
                match *name {
                    "show_overlay" if toggle => c.hidden = !c.hidden,
                    "toggle_liquid" => {
                        c.liquids = !c.liquids;
                        c.say = Some(format!("Liquids {}", if c.liquids { "shown" } else { "hidden" }));
                    }
                    "narrate" if tts_toggle => {
                        c.tts_on = !c.tts_on;
                        c.tts_last.clear();
                    }
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
    thread_local! {
        static READ_AT: std::cell::Cell<u32> = const { std::cell::Cell::new(u32::MAX) };
    }
    let epoch = SETTINGS_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    if READ_AT.replace(epoch) != epoch {
        CONFIG.with_borrow_mut(|c| *c = None);
    }
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
                registry_name: game.native_setting(ID, "registry_name").is_some_and(|v| v != "false"),
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
    for o in OPTIONS {
        let value = opt(game, o.key);
        let shown = o.values.iter().find(|(v, _)| *v == value).map_or(value.as_str(), |(_, l)| l);
        buttons.push(format!("{}: {shown}", o.label));
    }
    buttons.push("Edit ignore list in Files...".to_owned());
    buttons.push("Reload Plugins".to_owned());
    buttons.push("Reset Settings".to_owned());
}

/// Profiles: leaving one keeps its settings, entering one brings its own back.
fn switch_profile(game: &mut Game<'_>, from: &str, to: &str) {
    let saved: Vec<String> = OPTIONS.iter().filter(|p| p.key != "profile" && p.key != "enable_profiles").map(|p| format!("{}={}", p.key, opt(game, p.key))).collect();
    game.native_set_setting(ID, &format!("saved_profile_{from}"), &saved.join("\n"));
    if let Some(text) = game.native_setting(ID, &format!("saved_profile_{to}")) {
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                game.native_set_setting(ID, k, v);
            }
        }
    }
}

fn settings_changed() {
    SETTINGS_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    CONFIG.with_borrow_mut(|c| *c = None);
    SERVER_CONFIG.with_borrow_mut(|c| *c = None);
}

/// `/jadec ...` (JadeClientCommand) on the player's own client, `/jade ...` to the server.
pub fn client_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    // A guest's command that reached the server's dispatcher is theirs, not this client's.
    if let Some(player) = game.native_acting_player() {
        perform_command(game, &player, command, ran);
        return;
    }
    let mut words = command.split_whitespace();
    match words.next() {
        Some("jadec") => {
            *ran = true;
            let args: Vec<&str> = words.collect();
            match args.as_slice() {
                ["config"] => CLIENT.with_borrow_mut(|c| c.open_config = true),
                ["use_profile", n @ ("0" | "1" | "2" | "3")] => {
                    let from = opt(game, "profile");
                    if on(game, "enable_profiles") {
                        switch_profile(game, &from, n);
                    }
                    game.native_set_setting(ID, "profile", n);
                    settings_changed();
                    game.native_chat(&format!("Jade profile #{n}"));
                }
                ["pin"] => {
                    let was = CLIENT.with_borrow_mut(|c| c.pinned.take());
                    if was.is_none() {
                        let now = target(game);
                        CLIENT.with_borrow_mut(|c| c.pinned = now);
                    }
                }
                ["handlers"] => {
                    let text: Vec<String> = OPTIONS.iter().map(|o| format!("- `{}`: {} (default {})", o.key, o.label, o.default)).collect();
                    let done = Game::native_config_dir().ok().and_then(|d| std::fs::write(d.join("jade_handlers.md"), text.join("\n")).ok());
                    game.native_chat(if done.is_some() { "Dumped successfully" } else { "Couldn't write jade_handlers.md" });
                }
                ["generate_server_overrides", pattern @ ..] => {
                    let pattern = pattern.join(" ");
                    let wanted: serde_json::Map<String, serde_json::Value> = OPTIONS
                        .iter()
                        .filter(|o| pattern.is_empty() || o.key.contains(pattern.as_str()))
                        .map(|o| (o.key.to_owned(), serde_json::Value::String(opt(game, o.key))))
                        .collect();
                    if wanted.is_empty() {
                        game.native_chat("No config options match the pattern");
                    } else if let Some(file) = ignore_files().and_then(|f| f[0].parent().map(|d| d.join("server-plugin-overrides.json"))) {
                        let _ = std::fs::write(&file, serde_json::Value::Object(wanted).to_string());
                        settings_changed();
                        game.native_chat(&format!("Generated override file: {}", file.display()));
                    }
                }
                _ => game.native_chat("/jadec config | use_profile <0-3> | pin | handlers | generate_server_overrides [pattern]"),
            }
        }
        Some("jade") => {
            *ran = true;
            let rest: Vec<&str> = words.collect();
            let msg = format!("c {}", rest.join(" "));
            if game.native_net_send_to_server(NET_REQUEST, msg.as_bytes()).is_err() {
                let me = game.native_server_players().into_iter().next();
                if let Some(me) = me {
                    server_command(game, me.uuid, me.permission, &rest);
                }
            }
        }
        _ => {}
    }
}

/// `/jade show|hide <targets>` (JadeServerCommand, level 2): each player's overlay on or off.
fn server_command(game: &mut Game<'_>, from: u128, level: u8, args: &[&str]) {
    let tell = |game: &mut Game<'_>, t: &str| {
        game.native_tell(from, horizon_client::text::Text::legacy(t));
    };
    if level < 2 {
        tell(game, "You do not have permission to use this command");
        return;
    }
    let (show, wanted) = match args {
        ["show", t] => (true, *t),
        ["hide", t] => (false, *t),
        _ => {
            tell(game, "/jade show|hide <targets>");
            return;
        }
    };
    let players = game.native_server_players();
    let mut told = 0;
    for p in players.iter().filter(|p| wanted == "@a" || (matches!(wanted, "@s" | "@p") && p.uuid == from) || p.name == wanted) {
        let line = format!("o {}", u8::from(show));
        if game.native_net_send_to_player(p.uuid, NET_DATA, line.as_bytes()).is_ok() {
            told += 1;
        }
    }
    tell(game, &format!("Overlay {} for {told} players", if show { "shown" } else { "hidden" }));
}

/// `native_perform_command`: a LAN guest's `/jade`.
pub fn perform_command(game: &mut Game<'_>, player: &crate::game::native_server::NativeServerPlayer, command: &str, ran: &mut bool) {
    let mut words = command.split_whitespace();
    if words.next() == Some("jade") {
        *ran = true;
        let args: Vec<&str> = words.collect();
        server_command(game, player.uuid, player.permission, &args);
    }
}

pub fn register_commands(commands: &mut Vec<crate::game::native_server::NativeCommand>) {
    for syntax in ["jade show @a", "jade hide @a", "jade show @s", "jade hide @s"] {
        commands.push(crate::game::native_server::NativeCommand::op(syntax, 2));
    }
}

/// `native_settings_click`: a click steps that option on.
pub fn settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ID {
        return;
    }
    if let Some(o) = OPTIONS.get(button) {
        let value = opt(game, o.key);
        let at = o.values.iter().position(|(v, _)| *v == value).unwrap_or(0);
        let next = o.values[(at + 1) % o.values.len()].0;
        if o.key == "profile" && on(game, "enable_profiles") {
            switch_profile(game, &value, next);
        }
        game.native_set_setting(ID, o.key, next);
    } else {
        match button - OPTIONS.len() {
            0 => {
                if let Some(files) = ignore_files()
                    && let Some(dir) = files[0].parent()
                {
                    crate::game::packs::open_folder(dir);
                }
            }
            1 => {}
            _ => {
                for o in OPTIONS {
                    game.native_set_setting(ID, o.key, o.default);
                }
            }
        }
    }
    settings_changed();
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
    CLIENT.with_borrow_mut(|c| c.showing = false);
    if on(game, "narrate_keys")
        && let Some(line) = CLIENT.with_borrow_mut(|c| c.say.take())
    {
        game.say_system_now(&horizon_client::text::Text::literal(&line));
    }
    // Jade hides while a screen is open (Hide from GUIs), over the player list (Hide from Player List), and with Show
    // Overlay toggled off.
    let screen_hides = !matches!(game.screen, crate::game::Screen::Playing) && on(game, "hide_from_guis");
    let list_hides = on(game, "hide_from_tab_list") && game.physical_keys.contains(&winit::keyboard::KeyCode::Tab);
    if !game.native_on(ID) || game.player.is_none() || screen_hides || list_hides || CLIENT.with_borrow(|c| c.hidden) {
        return;
    }
    // Boss Bar Overlap's Hide Tooltip, and how far Push Down moves it.
    let overlap = opt(game, "boss_bar_overlap");
    let bars = game.boss_overlay.bars(horizon_client::boss_overlay::now_millis()).len();
    if bars > 0 && overlap == "hide_tip" && opt(game, "position") == "0" {
        return;
    }
    let pinned = CLIENT.with_borrow(|c| c.pinned.clone());
    let Some(t) = pinned.or_else(|| target(game)) else {
        CLIENT.with_borrow_mut(|c| {
            c.progress_alpha = 0.0;
            c.lookup = None;
            c.fade = 0.0;
            c.tts_last.clear();
        });
        return;
    };
    CLIENT.with_borrow_mut(|c| c.showing = true);
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
            let bit = |k: &'static str| u8::from(on(game, k));
            let msg = format!("m {id} {} {} {} {} {} {} {}", u8::from(details), u8::from(cfg.restock), u8::from(cfg.entity_stats), bit("mob_growth"), bit("mob_breeding"), bit("next_entity_drop"), bit("zombie_villager"));
            if game.native_net_send_to_server(NET_REQUEST, msg.as_bytes()).is_ok() {
                CLIENT.with_borrow_mut(|c| {
                    c.mob_asked = Some((id, std::time::Instant::now()));
                    c.asked_details = details;
                });
            }
        }
    }
    let (icon, title, ns) = match &t {
        Target::Cart { index } => {
            let Some(c) = game.carts.get(*index) else { return };
            let id = c.kind.id();
            let short = id.trim_start_matches("minecraft:");
            (Some(id.to_owned()), game.translate(&format!("entity.minecraft.{short}"), short), "minecraft".to_owned())
        }
        Target::Boat { index } => {
            let Some(b) = game.boats.get(*index) else { return };
            let id = if b.chest { format!("minecraft:{}_chest_boat", b.wood) } else { format!("minecraft:{}_boat", b.wood) };
            let short = id.trim_start_matches("minecraft:").to_owned();
            (Some(id), game.translate(&format!("entity.minecraft.{short}"), &short), "minecraft".to_owned())
        }
        // FallingBlockProvider: the block's item as its icon, its name.
        Target::Falling { index } => {
            let Some(f) = game.falling.get(*index) else { return };
            let block = game.res.facts().get(f.state).name.to_string();
            let ns = block.split_once(':').map_or("minecraft", |(n, _)| n).to_owned();
            (game.res.items().get(&block).is_some().then(|| block.clone()), item_name(game, &block), ns)
        }
        // PlayerHeadProvider: a head with an owner is named after them.
        Target::Block { pos, block } if (block == "minecraft:player_head" || block == "minecraft:player_wall_head") && on(game, "player_head") => {
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
            let _ = game.key_pressed(winit::keyboard::KeyCode::KeyE, winit::keyboard::KeyCode::KeyE);
        }
    }
    // The title line: the name, the harvest tool at its end (HarvestToolProvider, on the title by default).
    let mut title_line = Line::new();
    if on(game, "object_name") {
        title_line = title_line.with(El::Text(format!("§f{title}")));
    }
    let mut tool_line = None;
    if let Target::Block { pos, block } = &t
        && config(game).harvest_tool
        && let Some(tool) = harvest_tool(game, *pos, block)
    {
        if on(game, "harvest_new_line") {
            let mut l = Line::new();
            l.els.extend(tool);
            tool_line = Some(l);
        } else {
            title_line = title_line.with(El::Space(6.0));
            title_line.els.extend(tool);
        }
    }
    let mut lines = vec![title_line];
    lines.extend(tool_line);
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
    // Display Mode Lite: the name and the mod only, the rest while Show Details is held.
    if opt(game, "display_mode") == "lite" && !details {
        lines.truncate(1);
    }
    info_lines(game, &t, &cfg, &mut lines);
    if cfg.mod_name {
        let text = format!("§9§o{}", mod_name(&ns));
        lines.push(if on(game, "mod_name_smaller") { Line::new().with(El::Small(text)) } else { Line::text(text) });
    }
    if on(game, "debug_mode") {
        lines.push(Line::text(format!("§8{t:?}")));
    }
    // Text to Speech Mode Toggle: each new target read out.
    if CLIENT.with_borrow(|c| c.tts_on) {
        let spoken: String = lines.iter().flat_map(|l| l.els.iter()).filter_map(|e| if let El::Text(t) = e { Some(plain(t)) } else { None }).collect::<Vec<_>>().join(". ");
        if CLIENT.with_borrow(|c| c.tts_last != spoken) {
            game.say_system_now(&horizon_client::text::Text::literal(&spoken));
            CLIENT.with_borrow_mut(|c| c.tts_last = spoken);
        }
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
    // Icon Position: beside the box's top, its middle, or in the title's line.
    let icon_pos = opt(game, "display_item");
    let icon = icon.filter(|_| icon_pos != "hide");
    let inline = icon_pos == "inline";
    let icon_w = if icon.is_none() { 0.0 } else if inline { 0.0 } else { 20.0 };
    let title_extra = if inline && icon.is_some() { 11.0 } else { 0.0 };
    let w = sizes.iter().enumerate().map(|(i, s)| s.0 + if i == 0 { title_extra } else { 0.0 }).fold(0.0, f32::max) + icon_w;
    let h = sizes.iter().map(|s| s.1 + 1.0).sum::<f32>() - 1.0;
    // The overlay's anchor, Flip Position with Main Hand, and its scale about it.
    let side = opt(game, "overlay_x");
    let mut x = match side.as_str() {
        "left" => 12.0,
        "right" => (batch.width - w - 12.0).floor(),
        _ => ((batch.width - w) / 2.0).floor(),
    };
    if on(game, "flip_main_hand") && game.settings.main_hand_left {
        x = (batch.width - w - x).floor();
    }
    let mut y = match cfg.position {
        1 => (batch.height / 3.0).floor(),
        2 => ((batch.height - h) / 2.0).floor(),
        3 => (batch.height - h - 60.0).floor(),
        _ => 8.0,
    };
    if cfg.position == 0 && overlap == "push" {
        y += 19.0 * bars as f32;
    }
    let start = batch.vertices.len();
    draw_theme(game, batch, [x - 4.0, y - 4.0, w + 8.0, h + 8.0]);
    if let Some(icon) = &icon {
        if inline {
            game.native_item(batch, icon, [x, y], 9.0);
        } else {
            let iy = if icon_pos == "centered" { y + ((h - 16.0) / 2.0).max(0.0).floor() } else { y + ((h - 16.0) / 2.0).min(2.0).max(0.0) };
            game.native_item(batch, icon, [x, iy], 16.0);
        }
    }
    let text_alpha = num(game, "text_background_opacity");
    let mut ly = y;
    for (row, (l, (lw, lh))) in lines.iter().zip(&sizes).enumerate() {
        if text_alpha > 0.0 {
            let a = (text_alpha / 100.0 * 255.0) as u32;
            game.native_fill(batch, [x + icon_w - 1.0, ly - 1.0, lw + 2.0, lh + 1.0], a << 24);
        }
        let mut lx = x + icon_w + if row == 0 { title_extra } else { 0.0 };
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
    // Overlay Animation: fades in over a tenth of a second.
    if on(game, "animation") {
        let fade = CLIENT.with_borrow_mut(|c| {
            let now = std::time::Instant::now();
            let dt = c.fade_at.map_or(0.0, |t| now.duration_since(t).as_secs_f32());
            c.fade_at = Some(now);
            c.fade = (c.fade + dt / 0.1).min(1.0);
            c.fade
        });
        if fade < 1.0 {
            for v in &mut batch.vertices[start..] {
                let a = ((v.color >> 24) as f32 * fade) as u32;
                v.color = (v.color & 0x00FF_FFFF) | (a << 24);
            }
        }
    }
}

/// Jade's themes: `jade_themes/*.json` and their sprites' colours, the box a fill with its border, its corners cut in
/// the default style and square in Slim; Background Opacity on the fill.
fn draw_theme(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: [f32; 4]) {
    let theme = opt(game, "overlay_theme");
    let (fill, top, bottom, border) = match theme.as_str() {
        "waila" => (0x10_0010u32, 0x50_00FFu32, 0x28_007Fu32, 1.0),
        "create" => (0x00_0000, 0x40_3C37, 0x2A_2723, 1.0),
        "top" => (0x00_6699, 0x99_99AD, 0x99_99AD, 2.0),
        _ => (0x13_1313, 0x38_3838, 0x28_2828, 1.0),
    };
    let alpha = (num(game, "overlay_alpha") / 100.0 * 255.0) as u32;
    let alpha = if theme == "top" { alpha.min(0x80) } else { alpha };
    let bg = (alpha << 24) | fill;
    let (a, b) = (0xFF00_0000 | top, 0xFF00_0000 | bottom);
    let slim = opt(game, "theme_style") == "slim" || theme == "top";
    let cut = if slim { 0.0 } else { 1.0 };
    game.native_fill(batch, [x + cut, y, w - 2.0 * cut, h], bg);
    if !slim {
        game.native_fill(batch, [x, y + cut, cut, h - 2.0 * cut], bg);
        game.native_fill(batch, [x + w - cut, y + cut, cut, h - 2.0 * cut], bg);
    }
    let i = if slim { 0.0 } else { 1.0 };
    game.native_fill(batch, [x + i, y + i, w - 2.0 * i, border], a);
    game.native_fill(batch, [x + i, y + h - i - border, w - 2.0 * i, border], b);
    game.native_fill(batch, [x + i, y + i, border, h - 2.0 * i], a);
    game.native_fill(batch, [x + w - i - border, y + i, border, h - 2.0 * i], b);
}

/// Boss Bar Overlap's Hide Boss Bar: no bars while the box shows.
pub fn hide_boss_bars(game: &Game<'_>) -> bool {
    CLIENT.with_borrow(|c| c.showing) && opt(game, "boss_bar_overlap") == "hide_bar"
}

/// Mod Name on Item Tooltip: the stack's mod at the end of its tooltip.
pub fn item_tooltip(game: &Game<'_>, stack: &ItemStack, mut lines: Vec<(horizon_client::text::Text, u32)>) -> Vec<(horizon_client::text::Text, u32)> {
    if game.native_on(ID) && on(game, "item_mod_name") {
        let ns = stack.item.split_once(':').map_or("minecraft", |(n, _)| n);
        lines.push((horizon_client::text::Text::legacy(&format!("§9§o{}", mod_name(ns))), u32::MAX));
    }
    lines
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
    if let Some(rest) = text.strip_prefix("c ") {
        let level = game.native_server_players().into_iter().find(|p| p.uuid == player).map_or(0, |p| p.permission);
        let args: Vec<&str> = rest.split_whitespace().collect();
        server_command(game, player, level, &args);
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
    let [id, details, restock, stats, growth, breeding, drop, cure] = v.as_slice() else { return };
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
    for l in mob_lines(m, game.world.game_time, *details != 0, *restock != 0, *stats != 0, [*growth != 0, *breeding != 0, *drop != 0, *cure != 0]) {
        out.push('\n');
        out.push_str(&encode(&l).replace('\n', " "));
    }
    let _ = game.native_net_send_to_player(player, NET_DATA, out.as_bytes());
}

/// The lines of a mob that only the server knows.
fn mob_lines(m: &horizon_client::mobs::Mob, game_time: i64, details: bool, restock: bool, entity_stats: bool, [growth, breeding, drops, cure]: [bool; 4]) -> Vec<Line> {
    let mut lines = Vec::new();
    let kind = m.kind.id.as_ref();
    // MobGrowthProvider: a baby's growing time (AgeableMob.age counts up to 0).
    if growth && m.extra.age < 0 {
        lines.push(Line::text(if m.extra.age_locked { "Growing time: §fPaused".to_owned() } else { format!("Growing time: §f{}", seconds(-i64::from(m.extra.age))) }));
    }
    // MobBreedingProvider: in love, or the breeding cooldown (age above 0).
    if breeding && m.extra.in_love > 0 {
        lines.push(Line::text("Breeding CD: §fIn Love"));
    } else if breeding && m.extra.age > 0 {
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
        && cure
        && let Some(c) = horizon_client::mobs::zombie_villager::cure(m)
        && c.time > 0
    {
        lines.push(Line::text(format!("Conversion time: §f{}", seconds(i64::from(c.time)))));
    }
    // NextEntityDropProvider: a grown chicken's next egg, an armadillo's next scute.
    if drops && matches!(kind, "minecraft:chicken" | "minecraft:armadillo") && !m.is_baby() && m.extra.egg_time < 48000 {
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
    // ShowOverlayPacket.
    if let Some(rest) = text.strip_prefix("o ") {
        CLIENT.with_borrow_mut(|c| c.hidden = rest.trim() != "1");
        return;
    }
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
