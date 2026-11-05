// Its own file: "an unknown key makes [vanilla] reject" game_rules.dat, says this function's writer of it.
fn jade_rule_save(level: &Level, dir: &Path) {
    let name = crate::gamerules::hz_jade_rule::NAME;
    if let Some(value) = level.game_rules.get(name) {
        let data = Tag::compound([(name, Tag::Int(*value))]);
        let tag = Tag::compound([("data", data), ("DataVersion", Tag::Int(DATA_VERSION))]);
        let _ = write_dat(&dir.join("data/jade/game_rules.dat"), tag);
    }
}
