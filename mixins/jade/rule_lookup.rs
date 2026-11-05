// A guest's screen sends every rule as minecraft:<name>, this one included.
fn jade_rule_lookup(name: &str) -> core::ops::ControlFlow<Option<&'static Rule>> {
    if name.strip_prefix("minecraft:").unwrap_or(name) == hz_jade_rule::NAME {
        core::ops::ControlFlow::Break(Some(&hz_jade_rule::RULE))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
